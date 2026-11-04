//! Server-side SQL statement policy built on a real parser (never regular expressions).
//!
//! The policy runs before any database is contacted. It accepts exactly one
//! statement and classifies it as a read or a data write; everything else
//! (DDL, administration, privileges, COPY, PRAGMA, file/network/extension
//! functions, locking reads, SELECT INTO, data-modifying CTEs, multi-statements)
//! is refused. Database privileges remain an independent second boundary.

use std::ops::ControlFlow;

use sqlparser::{
    ast::{
        Expr, Query, SetExpr, Statement, Value, visit_expressions, visit_expressions_mut,
        visit_statements,
    },
    dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect},
    parser::Parser,
};

pub const MAX_SQL_BYTES: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialectKind {
    PostgreSql,
    Sqlite,
    MySql,
}

impl DialectKind {
    fn dialect(self) -> Box<dyn Dialect> {
        match self {
            Self::PostgreSql => Box::new(PostgreSqlDialect {}),
            Self::Sqlite => Box::new(SQLiteDialect {}),
            Self::MySql => Box::new(MySqlDialect {}),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub class: Class,
    /// `select`, `insert`, `update`, `delete`, `explain`, `show`.
    pub kind: &'static str,
    /// Statement text with every literal replaced by `?`, bounded for audit logs.
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    Empty,
    TooLong,
    Syntax,
    MultipleStatements,
    /// A statement class that is never allowed here.
    StatementNotAllowed(&'static str),
    /// A forbidden function (file, network, extension, sleep, lock, session control).
    FunctionNotAllowed,
    /// Data writes are not enabled for this request.
    WritesNotAllowed,
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Empty => "enter one SQL statement",
            Self::TooLong => "the statement is too long",
            Self::Syntax => "the statement could not be parsed",
            Self::MultipleStatements => "only one statement at a time is allowed",
            Self::StatementNotAllowed(why) => why,
            Self::FunctionNotAllowed => "the statement uses a function that is not allowed",
            Self::WritesNotAllowed => "data-changing statements are not allowed here",
        })
    }
}

/// Lower-case function names that read files, reach the network or other
/// sessions, load code, sleep, take locks, advance sequences or change settings.
const DENIED_FUNCTIONS: &[&str] = &[
    // PostgreSQL files, large objects, admin, sessions, locks, sequences
    "pg_read_file",
    "pg_read_binary_file",
    "pg_ls_dir",
    "pg_ls_logdir",
    "pg_ls_waldir",
    "pg_stat_file",
    "pg_file_write",
    "pg_file_unlink",
    "lo_import",
    "lo_export",
    "lo_create",
    "lo_unlink",
    "lo_put",
    "pg_terminate_backend",
    "pg_cancel_backend",
    "pg_reload_conf",
    "pg_rotate_logfile",
    "pg_sleep",
    "pg_sleep_for",
    "pg_sleep_until",
    "set_config",
    "pg_advisory_lock",
    "pg_advisory_xact_lock",
    "pg_try_advisory_lock",
    "pg_try_advisory_xact_lock",
    "pg_advisory_lock_shared",
    "nextval",
    "setval",
    "pg_switch_wal",
    "pg_create_restore_point",
    "pg_replication_slot_advance",
    "dblink",
    "dblink_exec",
    "dblink_connect",
    "query_to_xml",
    "query_to_xml_and_xmlschema",
    "table_to_xml",
    "cursor_to_xml",
    "database_to_xml",
    "schema_to_xml",
    "pg_notify",
    "txid_current",
    "pg_current_wal_lsn",
    // SQLite extension and file access
    "load_extension",
    "readfile",
    "writefile",
    "edit",
    "fts3_tokenizer",
    "sqlite_compileoption_used",
    // MySQL file, network, sleep, locks
    "load_file",
    "sleep",
    "benchmark",
    "get_lock",
    "release_lock",
    "master_pos_wait",
    "sys_exec",
    "sys_eval",
];

fn denied_function(name: &str) -> bool {
    // Compare the unqualified, lower-cased name (pg_catalog.pg_sleep and "pg_sleep" included).
    let last = name
        .rsplit('.')
        .next()
        .unwrap_or(name)
        .trim_matches('"')
        .to_lowercase();
    DENIED_FUNCTIONS.contains(&last.as_str())
}

fn check_query(query: &Query) -> Result<(), PolicyError> {
    if !query.locks.is_empty() {
        return Err(PolicyError::StatementNotAllowed(
            "locking reads (FOR UPDATE/SHARE) are not allowed",
        ));
    }
    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            check_query(&cte.query)?;
        }
    }
    check_set_expr(&query.body)
}

fn check_set_expr(body: &SetExpr) -> Result<(), PolicyError> {
    match body {
        SetExpr::Select(select) => {
            if select.into.is_some() {
                return Err(PolicyError::StatementNotAllowed(
                    "SELECT INTO is not allowed",
                ));
            }
            Ok(())
        }
        SetExpr::Query(query) => check_query(query),
        SetExpr::SetOperation { left, right, .. } => {
            check_set_expr(left)?;
            check_set_expr(right)
        }
        SetExpr::Values(_) | SetExpr::Table(_) => Ok(()),
        _ => Err(PolicyError::StatementNotAllowed(
            "data-modifying subqueries are not allowed",
        )),
    }
}

fn functions_are_allowed(statement: &Statement) -> bool {
    let flow = visit_expressions(statement, |expr| {
        if let Expr::Function(function) = expr
            && denied_function(&function.name.to_string())
        {
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    });
    flow.is_continue()
}

/// Table functions (`FROM pg_read_file(...)`) appear as relations with arguments.
fn relations_are_allowed(statement: &Statement) -> bool {
    use sqlparser::ast::visit_relations;
    visit_relations(statement, |name| {
        if denied_function(&name.to_string()) {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })
    .is_continue()
}

fn summarize(mut statement: Statement) -> String {
    let _ = visit_expressions_mut(&mut statement, |expr| {
        if matches!(expr, Expr::Value(_)) {
            *expr = Expr::Value(Value::Placeholder("?".into()).with_empty_span());
        }
        ControlFlow::<()>::Continue(())
    });
    statement
        .to_string()
        .chars()
        .filter(|c| !c.is_control())
        .take(300)
        .collect()
}

/// Parse and classify. `allow_writes` is the caller's decision (feature flag,
/// profile access and grants); a write is refused here when it is `false`.
pub fn classify(
    dialect: DialectKind,
    sql: &str,
    allow_writes: bool,
) -> Result<Classified, PolicyError> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err(PolicyError::Empty);
    }
    if trimmed.len() > MAX_SQL_BYTES || trimmed.chars().any(|c| c == '\0') {
        return Err(PolicyError::TooLong);
    }
    let statements =
        Parser::parse_sql(&*dialect.dialect(), trimmed).map_err(|_| PolicyError::Syntax)?;
    let [statement] = <[Statement; 1]>::try_from(statements).map_err(|rest| {
        if rest.is_empty() {
            PolicyError::Empty
        } else {
            PolicyError::MultipleStatements
        }
    })?;

    let (class, kind) = match &statement {
        Statement::Query(query) => {
            check_query(query)?;
            (Class::Read, "select")
        }
        Statement::Explain {
            analyze,
            statement: inner,
            ..
        } => {
            // EXPLAIN ANALYZE would execute the statement.
            if *analyze {
                return Err(PolicyError::StatementNotAllowed(
                    "EXPLAIN ANALYZE is not allowed",
                ));
            }
            match inner.as_ref() {
                Statement::Query(query) => check_query(query)?,
                _ => {
                    return Err(PolicyError::StatementNotAllowed(
                        "only EXPLAIN of a SELECT is allowed",
                    ));
                }
            }
            (Class::Read, "explain")
        }
        Statement::Insert(_) => (Class::Write, "insert"),
        Statement::Update { .. } => (Class::Write, "update"),
        Statement::Delete(_) => (Class::Write, "delete"),
        Statement::Pragma { .. } => {
            return Err(PolicyError::StatementNotAllowed("PRAGMA is not allowed"));
        }
        Statement::Copy { .. } | Statement::CopyIntoSnowflake { .. } => {
            return Err(PolicyError::StatementNotAllowed("COPY is not allowed"));
        }
        Statement::Merge { .. } => {
            return Err(PolicyError::StatementNotAllowed("MERGE is not allowed"));
        }
        Statement::Truncate { .. } => {
            return Err(PolicyError::StatementNotAllowed("TRUNCATE is not allowed"));
        }
        Statement::Grant { .. } | Statement::Revoke { .. } => {
            return Err(PolicyError::StatementNotAllowed(
                "privilege changes are not allowed",
            ));
        }
        Statement::AttachDatabase { .. } | Statement::AttachDuckDBDatabase { .. } => {
            return Err(PolicyError::StatementNotAllowed("ATTACH is not allowed"));
        }
        _ => {
            return Err(PolicyError::StatementNotAllowed(
                "only SELECT, EXPLAIN, INSERT, UPDATE and DELETE statements are allowed",
            ));
        }
    };

    // A SELECT must not smuggle in a write through a nested statement.
    let mut nested_write = false;
    let _ = visit_statements(&statement, |nested| {
        if matches!(
            nested,
            Statement::Insert(_)
                | Statement::Update { .. }
                | Statement::Delete(_)
                | Statement::Merge { .. }
        ) && class == Class::Read
        {
            nested_write = true;
        }
        ControlFlow::<()>::Continue(())
    });
    if nested_write {
        return Err(PolicyError::StatementNotAllowed(
            "data-modifying subqueries are not allowed",
        ));
    }
    if !functions_are_allowed(&statement) || !relations_are_allowed(&statement) {
        return Err(PolicyError::FunctionNotAllowed);
    }
    if class == Class::Write && !allow_writes {
        return Err(PolicyError::WritesNotAllowed);
    }
    Ok(Classified {
        class,
        kind,
        summary: summarize(statement),
    })
}

/// Keep `GenericDialect` linked for callers that need a neutral parse in tests.
pub fn parses_generic(sql: &str) -> bool {
    Parser::parse_sql(&GenericDialect {}, sql).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [DialectKind; 3] = [
        DialectKind::PostgreSql,
        DialectKind::Sqlite,
        DialectKind::MySql,
    ];

    fn read(sql: &str) -> Classified {
        classify(DialectKind::PostgreSql, sql, false).unwrap_or_else(|e| panic!("{sql}: {e:?}"))
    }

    #[test]
    fn plain_reads_are_classified_and_literals_are_stripped_from_the_summary() {
        let c = read(
            "SELECT id, name FROM accounts WHERE name = 'secret-value' AND id > 42 ORDER BY id LIMIT 10",
        );
        assert_eq!((c.class, c.kind), (Class::Read, "select"));
        assert!(
            !c.summary.contains("secret-value") && !c.summary.contains("42"),
            "{}",
            c.summary
        );
        assert!(c.summary.contains("accounts"));
        for sql in [
            "WITH x AS (SELECT 1) SELECT * FROM x",
            "SELECT a FROM t UNION SELECT b FROM u",
            "VALUES (1), (2)",
            "EXPLAIN SELECT * FROM t",
            "select count(*) from t group by a having count(*) > 1",
        ] {
            assert_eq!(read(sql).class, Class::Read, "{sql}");
        }
    }

    #[test]
    fn exactly_one_statement_is_required_in_every_dialect() {
        for dialect in ALL {
            for sql in [
                "SELECT 1; SELECT 2",
                "SELECT 1; DROP TABLE t",
                "SELECT 1;;DELETE FROM t",
                ";",
            ] {
                assert!(
                    matches!(
                        classify(dialect, sql, true),
                        Err(PolicyError::MultipleStatements
                            | PolicyError::Empty
                            | PolicyError::Syntax)
                    ),
                    "{dialect:?} {sql}"
                );
            }
            assert_eq!(classify(dialect, "   ", true), Err(PolicyError::Empty));
            // A trailing semicolon on a single statement is fine.
            assert!(classify(dialect, "SELECT 1;", false).is_ok());
            assert_eq!(
                classify(dialect, "SELEC 1", false),
                Err(PolicyError::Syntax)
            );
        }
    }

    #[test]
    fn schema_changes_administration_and_privileges_are_refused() {
        for sql in [
            "DROP TABLE t",
            "CREATE TABLE t (a int)",
            "ALTER TABLE t ADD COLUMN b int",
            "TRUNCATE t",
            "GRANT ALL ON t TO bob",
            "REVOKE ALL ON t FROM bob",
            "CREATE ROLE evil SUPERUSER",
            "COPY t TO '/tmp/x'",
            "COPY t FROM PROGRAM 'id'",
            "SET ROLE postgres",
            "SET search_path = evil",
            "RESET ALL",
            "VACUUM",
            "CREATE FUNCTION f() RETURNS int AS 'select 1' LANGUAGE sql",
            "DO $$ BEGIN PERFORM 1; END $$",
            "CALL p()",
            "LOCK TABLE t",
            "BEGIN",
            "COMMIT",
            "MERGE INTO t USING u ON t.id = u.id WHEN MATCHED THEN DELETE",
            "SELECT * INTO newt FROM t",
            "SELECT * FROM t FOR UPDATE",
        ] {
            let dialect = DialectKind::PostgreSql;
            assert!(
                matches!(
                    classify(dialect, sql, true),
                    Err(PolicyError::StatementNotAllowed(_) | PolicyError::Syntax)
                ),
                "{sql} must be refused, got {:?}",
                classify(dialect, sql, true)
            );
        }
        for sql in [
            "PRAGMA writable_schema = 1",
            "PRAGMA journal_mode = off",
            "ATTACH DATABASE '/etc/passwd' AS x",
            "VACUUM INTO '/tmp/copy.db'",
            "DROP TABLE t",
        ] {
            assert!(classify(DialectKind::Sqlite, sql, true).is_err(), "{sql}");
        }
        for sql in [
            "SHOW DATABASES",
            "USE mysql",
            "LOAD DATA INFILE '/etc/passwd' INTO TABLE t",
            "SET GLOBAL x = 1",
        ] {
            assert!(classify(DialectKind::MySql, sql, true).is_err(), "{sql}");
        }
    }

    #[test]
    fn dangerous_functions_are_refused_wherever_they_appear() {
        for sql in [
            "SELECT pg_read_file('/etc/passwd')",
            "SELECT pg_catalog.pg_read_file('/etc/passwd')",
            "SELECT PG_SLEEP(100)",
            "SELECT \"pg_sleep\"(1)",
            "SELECT * FROM pg_ls_dir('/')",
            "SELECT lo_import('/etc/passwd')",
            "SELECT nextval('s')",
            "SELECT set_config('x','y',false)",
            "SELECT dblink('host=evil','select 1')",
            "SELECT a FROM t WHERE a = (SELECT length(pg_read_file('/x')))",
            "SELECT CASE WHEN true THEN pg_sleep(1) END",
            "SELECT * FROM t ORDER BY pg_sleep(1)",
            "WITH x AS (SELECT pg_read_file('/x')) SELECT * FROM x",
            "UPDATE t SET a = nextval('s')",
            "SELECT pg_terminate_backend(1)",
        ] {
            assert_eq!(
                classify(DialectKind::PostgreSql, sql, true),
                Err(PolicyError::FunctionNotAllowed),
                "{sql}"
            );
        }
        for sql in [
            "SELECT load_extension('/tmp/evil.so')",
            "SELECT readfile('/etc/passwd')",
            "SELECT writefile('/tmp/x', 'y')",
        ] {
            assert_eq!(
                classify(DialectKind::Sqlite, sql, true),
                Err(PolicyError::FunctionNotAllowed),
                "{sql}"
            );
        }
        for sql in [
            "SELECT LOAD_FILE('/etc/passwd')",
            "SELECT SLEEP(10)",
            "SELECT BENCHMARK(1000000000, MD5('a'))",
        ] {
            assert_eq!(
                classify(DialectKind::MySql, sql, true),
                Err(PolicyError::FunctionNotAllowed),
                "{sql}"
            );
        }
        // Harmless functions and lookalike identifiers stay allowed.
        for sql in [
            "SELECT now(), lower('A'), count(*) FROM t",
            "SELECT sleeper FROM t",
            "SELECT pg_sleep_note FROM t",
        ] {
            assert!(
                classify(DialectKind::PostgreSql, sql, false).is_ok(),
                "{sql}"
            );
        }
    }

    #[test]
    fn data_changes_need_the_caller_to_allow_writes_and_are_classified() {
        for (sql, kind) in [
            ("INSERT INTO t (a) VALUES (1)", "insert"),
            ("UPDATE t SET a = 2 WHERE id = 1", "update"),
            ("DELETE FROM t WHERE id = 1", "delete"),
            ("INSERT INTO t SELECT * FROM u", "insert"),
        ] {
            assert_eq!(
                classify(DialectKind::PostgreSql, sql, false),
                Err(PolicyError::WritesNotAllowed),
                "{sql}"
            );
            let c = classify(DialectKind::PostgreSql, sql, true).unwrap();
            assert_eq!((c.class, c.kind), (Class::Write, kind));
            assert!(
                !c.summary.contains('1') || kind == "insert" && sql.contains("SELECT"),
                "{}",
                c.summary
            );
        }
    }

    #[test]
    fn data_modifying_ctes_and_subqueries_cannot_hide_inside_a_select() {
        for sql in [
            "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
            "WITH i AS (INSERT INTO t VALUES (1) RETURNING *) SELECT * FROM i",
            "WITH u AS (UPDATE t SET a = 1 RETURNING *) SELECT * FROM u",
        ] {
            assert!(
                matches!(
                    classify(DialectKind::PostgreSql, sql, true),
                    Err(PolicyError::StatementNotAllowed(_))
                ),
                "{sql} -> {:?}",
                classify(DialectKind::PostgreSql, sql, true)
            );
        }
        assert!(
            classify(
                DialectKind::PostgreSql,
                "EXPLAIN ANALYZE DELETE FROM t",
                true
            )
            .is_err()
        );
        assert!(classify(DialectKind::PostgreSql, "EXPLAIN ANALYZE SELECT 1", true).is_err());
        assert!(classify(DialectKind::PostgreSql, "EXPLAIN DELETE FROM t", true).is_err());
    }

    #[test]
    fn oversized_and_odd_input_is_rejected_cheaply() {
        let huge = format!("SELECT '{}'", "a".repeat(MAX_SQL_BYTES));
        assert_eq!(
            classify(DialectKind::PostgreSql, &huge, false),
            Err(PolicyError::TooLong)
        );
        assert_eq!(
            classify(DialectKind::PostgreSql, "SELECT 1\0", false),
            Err(PolicyError::TooLong)
        );
        // Deeply nested expressions fail to parse instead of overflowing the stack.
        let deep = format!("SELECT {}1{}", "(".repeat(5_000), ")".repeat(5_000));
        assert!(classify(DialectKind::PostgreSql, &deep, false).is_err());
        // Comments cannot be used to split or disguise statements.
        assert_eq!(
            classify(
                DialectKind::PostgreSql,
                "SELECT 1 /* ; DROP TABLE t */",
                false
            )
            .map(|c| c.class),
            Ok(Class::Read)
        );
        assert!(
            classify(
                DialectKind::PostgreSql,
                "SELECT 1 -- x\n; DROP TABLE t",
                false
            )
            .is_err()
        );
        assert!(parses_generic("SELECT 1"));
    }
}
