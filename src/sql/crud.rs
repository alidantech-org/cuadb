//! Typed row edits: INSERT / UPDATE / DELETE built from catalog metadata only.
//!
//! Identifiers must exist in the relation's catalog columns and are quoted per
//! dialect; every value is a bound text parameter cast to the column's catalog
//! type. Updates and deletes address exactly one row by its full primary key and
//! updates carry the previous values of the changed columns so a concurrent change
//! is detected instead of silently overwritten.

use super::policy::DialectKind;
use crate::db::types::ColumnDetail;

pub const MAX_VALUE_BYTES: usize = 65_536;
pub const MAX_COLUMNS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Insert {
        values: Vec<(String, Value)>,
    },
    Update {
        key: Vec<(String, String)>,
        set: Vec<(String, Value)>,
        expected: Vec<(String, Value)>,
    },
    Delete {
        key: Vec<(String, String)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrudError {
    UnknownColumn,
    /// Generated or `GENERATED ALWAYS` identity column.
    ReadOnlyColumn,
    /// A NOT NULL column was set to NULL.
    NullNotAllowed,
    /// Update/delete need every primary-key column (and the table needs a key).
    KeyRequired,
    /// Nothing to change, duplicate columns, too many columns, or an oversized value.
    BadInput,
    /// Binary/unsupported column type.
    UnsupportedType,
}

impl std::fmt::Display for CrudError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnknownColumn => "unknown column",
            Self::ReadOnlyColumn => "generated and always-identity columns cannot be written",
            Self::NullNotAllowed => "a NOT NULL column cannot be set to NULL",
            Self::KeyRequired => "the full primary key is required (and the table must have one)",
            Self::BadInput => "invalid edit request",
            Self::UnsupportedType => "this column type cannot be edited here",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub sql: String,
    pub binds: Vec<Option<String>>,
    /// Column names and operation only; never values.
    pub summary: String,
    pub kind: &'static str,
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// A catalog type name is only interpolated into SQL after this check.
fn safe_type(native: &str) -> bool {
    !native.is_empty()
        && native.len() <= 80
        && native.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, ' ' | '_' | '(' | ')' | ',' | '.' | '[' | ']' | '"')
        })
}

fn is_binary(native: &str) -> bool {
    let n = native.to_ascii_lowercase();
    n.contains("bytea") || n.contains("blob") || n.contains("binary")
}

struct Ctx<'a> {
    dialect: DialectKind,
    columns: &'a [ColumnDetail],
    binds: Vec<Option<String>>,
}

impl Ctx<'_> {
    fn column(&self, name: &str) -> Result<&ColumnDetail, CrudError> {
        self.columns
            .iter()
            .find(|c| c.column.name == name)
            .ok_or(CrudError::UnknownColumn)
    }

    /// Push a bound value; returns its placeholder, cast to the column type on PostgreSQL.
    fn bind(&mut self, column: &ColumnDetail, value: Option<String>) -> Result<String, CrudError> {
        if is_binary(&column.column.native_type) {
            return Err(CrudError::UnsupportedType);
        }
        if value
            .as_ref()
            .is_some_and(|v| v.len() > MAX_VALUE_BYTES || v.contains('\0'))
        {
            return Err(CrudError::BadInput);
        }
        self.binds.push(value);
        Ok(match self.dialect {
            DialectKind::PostgreSql => {
                if !safe_type(&column.column.native_type) {
                    return Err(CrudError::UnsupportedType);
                }
                format!(
                    "CAST(${} AS {})",
                    self.binds.len(),
                    column.column.native_type
                )
            }
            _ => "?".to_string(),
        })
    }

    fn writable(&self, name: &str) -> Result<&ColumnDetail, CrudError> {
        let column = self.column(name)?;
        if column.generated.is_some() || column.identity.as_deref() == Some("ALWAYS") {
            return Err(CrudError::ReadOnlyColumn);
        }
        Ok(column)
    }

    fn key_clause(&mut self, key: &[(String, String)]) -> Result<String, CrudError> {
        let pk: Vec<&str> = self
            .columns
            .iter()
            .filter(|c| c.column.primary_key)
            .map(|c| c.column.name.as_str())
            .collect();
        if pk.is_empty()
            || key.len() != pk.len()
            || !pk.iter().all(|name| key.iter().any(|(k, _)| k == name))
        {
            return Err(CrudError::KeyRequired);
        }
        let mut parts = vec![];
        for (name, value) in key {
            let column = self.column(name)?.clone();
            let placeholder = self.bind(&column, Some(value.clone()))?;
            parts.push(format!("{} = {placeholder}", quote(name)));
        }
        Ok(parts.join(" AND "))
    }
}

fn distinct(names: &[&str]) -> bool {
    let mut seen = std::collections::HashSet::new();
    names.iter().all(|n| seen.insert(*n))
}

pub fn build(
    dialect: DialectKind,
    schema: &str,
    table: &str,
    columns: &[ColumnDetail],
    op: &Op,
) -> Result<Built, CrudError> {
    let relation = match dialect {
        DialectKind::Sqlite => quote(table),
        _ => format!("{}.{}", quote(schema), quote(table)),
    };
    let target = format!("{schema}.{table}");
    let mut ctx = Ctx {
        dialect,
        columns,
        binds: vec![],
    };
    match op {
        Op::Insert { values } => {
            if values.len() > MAX_COLUMNS
                || !distinct(&values.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>())
            {
                return Err(CrudError::BadInput);
            }
            if values.is_empty() {
                return Ok(Built {
                    sql: format!("INSERT INTO {relation} DEFAULT VALUES"),
                    binds: vec![],
                    summary: format!("insert into {target} (defaults)"),
                    kind: "insert",
                });
            }
            let mut names = vec![];
            let mut placeholders = vec![];
            for (name, value) in values {
                let column = ctx.writable(name)?.clone();
                if matches!(value, Value::Null)
                    && !column.column.nullable
                    && column.column.default_expression.is_none()
                {
                    return Err(CrudError::NullNotAllowed);
                }
                let bound = match value {
                    Value::Null => None,
                    Value::Text(t) => Some(t.clone()),
                };
                placeholders.push(ctx.bind(&column, bound)?);
                names.push(quote(name));
            }
            Ok(Built {
                sql: format!(
                    "INSERT INTO {relation} ({}) VALUES ({})",
                    names.join(", "),
                    placeholders.join(", ")
                ),
                binds: ctx.binds,
                summary: format!(
                    "insert into {target} ({})",
                    values
                        .iter()
                        .map(|(n, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                kind: "insert",
            })
        }
        Op::Update { key, set, expected } => {
            if set.is_empty()
                || set.len() > MAX_COLUMNS
                || !distinct(&set.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>())
            {
                return Err(CrudError::BadInput);
            }
            // Every changed column needs its previous value (stale-row check).
            if expected.len() != set.len()
                || !set
                    .iter()
                    .all(|(n, _)| expected.iter().any(|(e, _)| e == n))
            {
                return Err(CrudError::BadInput);
            }
            let mut assignments = vec![];
            for (name, value) in set {
                let column = ctx.writable(name)?.clone();
                if matches!(value, Value::Null) && !column.column.nullable {
                    return Err(CrudError::NullNotAllowed);
                }
                let bound = match value {
                    Value::Null => None,
                    Value::Text(t) => Some(t.clone()),
                };
                assignments.push(format!("{} = {}", quote(name), ctx.bind(&column, bound)?));
            }
            let mut conditions = vec![ctx.key_clause(key)?];
            for (name, value) in expected {
                let column = ctx.column(name)?.clone();
                let bound = match value {
                    Value::Null => None,
                    Value::Text(t) => Some(t.clone()),
                };
                let placeholder = ctx.bind(&column, bound)?;
                conditions.push(match dialect {
                    DialectKind::PostgreSql => {
                        format!("{} IS NOT DISTINCT FROM {placeholder}", quote(name))
                    }
                    _ => format!("{} IS {placeholder}", quote(name)),
                });
            }
            Ok(Built {
                sql: format!(
                    "UPDATE {relation} SET {} WHERE {}",
                    assignments.join(", "),
                    conditions.join(" AND ")
                ),
                binds: ctx.binds,
                summary: format!(
                    "update {target} set {}",
                    set.iter()
                        .map(|(n, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                kind: "update",
            })
        }
        Op::Delete { key } => {
            let clause = ctx.key_clause(key)?;
            Ok(Built {
                sql: format!("DELETE FROM {relation} WHERE {clause}"),
                binds: ctx.binds,
                summary: format!("delete from {target} by primary key"),
                kind: "delete",
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::{Availability, Column};

    fn col(name: &str, ty: &str, pk: bool, nullable: bool) -> ColumnDetail {
        ColumnDetail {
            column: Column {
                name: name.into(),
                native_type: ty.into(),
                nullable,
                primary_key: pk,
                default_expression: None,
            },
            position: 1,
            unique_key: pk,
            identity: None,
            generated: None,
            domain: None,
            enum_values: Availability::Unavailable,
            comment: None,
        }
    }

    fn table() -> Vec<ColumnDetail> {
        let mut generated = col("total", "integer", false, true);
        generated.generated = Some("STORED".into());
        let mut identity = col("seq", "integer", false, false);
        identity.identity = Some("ALWAYS".into());
        vec![
            col("id", "integer", true, false),
            col("name", "character varying(20)", false, false),
            col("note", "text", false, true),
            col("bin", "bytea", false, true),
            generated,
            identity,
        ]
    }

    fn text(v: &str) -> Value {
        Value::Text(v.into())
    }

    #[test]
    fn insert_quotes_identifiers_binds_values_and_casts_on_postgres() {
        let b = build(
            DialectKind::PostgreSql,
            "pub\"lic",
            "t\"x",
            &table(),
            &Op::Insert {
                values: vec![
                    ("name".into(), text("O'Brien; DROP TABLE t")),
                    ("note".into(), Value::Null),
                ],
            },
        )
        .unwrap();
        assert_eq!(
            b.sql,
            "INSERT INTO \"pub\"\"lic\".\"t\"\"x\" (\"name\", \"note\") VALUES (CAST($1 AS character varying(20)), CAST($2 AS text))"
        );
        assert_eq!(b.binds, vec![Some("O'Brien; DROP TABLE t".into()), None]);
        assert!(
            !b.summary.contains("O'Brien"),
            "values never appear in the summary"
        );
        let s = build(
            DialectKind::Sqlite,
            "main",
            "t",
            &table(),
            &Op::Insert {
                values: vec![("name".into(), text("x"))],
            },
        )
        .unwrap();
        assert_eq!(s.sql, "INSERT INTO \"t\" (\"name\") VALUES (?)");
        let defaults = build(
            DialectKind::Sqlite,
            "main",
            "t",
            &table(),
            &Op::Insert { values: vec![] },
        )
        .unwrap();
        assert_eq!(defaults.sql, "INSERT INTO \"t\" DEFAULT VALUES");
    }

    #[test]
    fn identifiers_must_come_from_the_catalog_and_generated_columns_are_protected() {
        let insert = |name: &str| {
            build(
                DialectKind::PostgreSql,
                "s",
                "t",
                &table(),
                &Op::Insert {
                    values: vec![(name.into(), text("1"))],
                },
            )
        };
        assert_eq!(insert("name\" = 1; --"), Err(CrudError::UnknownColumn));
        assert_eq!(insert("nope"), Err(CrudError::UnknownColumn));
        assert_eq!(insert("total"), Err(CrudError::ReadOnlyColumn));
        assert_eq!(insert("seq"), Err(CrudError::ReadOnlyColumn));
        assert_eq!(insert("bin"), Err(CrudError::UnsupportedType));
        assert_eq!(
            build(
                DialectKind::PostgreSql,
                "s",
                "t",
                &table(),
                &Op::Insert {
                    values: vec![("name".into(), Value::Null)]
                }
            ),
            Err(CrudError::NullNotAllowed)
        );
        assert_eq!(
            build(
                DialectKind::PostgreSql,
                "s",
                "t",
                &table(),
                &Op::Insert {
                    values: vec![("name".into(), text("a")), ("name".into(), text("b"))]
                }
            ),
            Err(CrudError::BadInput)
        );
        let huge = "x".repeat(MAX_VALUE_BYTES + 1);
        assert_eq!(insert_value(&huge), Err(CrudError::BadInput));
        assert_eq!(insert_value("a\0b"), Err(CrudError::BadInput));
    }

    fn insert_value(v: &str) -> Result<Built, CrudError> {
        build(
            DialectKind::PostgreSql,
            "s",
            "t",
            &table(),
            &Op::Insert {
                values: vec![("name".into(), text(v))],
            },
        )
    }

    #[test]
    fn update_targets_one_row_by_full_key_and_checks_previous_values() {
        let op = Op::Update {
            key: vec![("id".into(), "7".into())],
            set: vec![("name".into(), text("new")), ("note".into(), Value::Null)],
            expected: vec![("name".into(), text("old")), ("note".into(), text("was"))],
        };
        let pg = build(DialectKind::PostgreSql, "s", "t", &table(), &op).unwrap();
        assert_eq!(
            pg.sql,
            "UPDATE \"s\".\"t\" SET \"name\" = CAST($1 AS character varying(20)), \"note\" = CAST($2 AS text) WHERE \"id\" = CAST($3 AS integer) AND \"name\" IS NOT DISTINCT FROM CAST($4 AS character varying(20)) AND \"note\" IS NOT DISTINCT FROM CAST($5 AS text)"
        );
        assert_eq!(pg.binds.len(), 5);
        let lite = build(DialectKind::Sqlite, "main", "t", &table(), &op).unwrap();
        assert_eq!(
            lite.sql,
            "UPDATE \"t\" SET \"name\" = ?, \"note\" = ? WHERE \"id\" = ? AND \"name\" IS ? AND \"note\" IS ?"
        );
        // Missing key, partial key, missing expected values and no changes are refused.
        let no_key = Op::Update {
            key: vec![],
            set: vec![("name".into(), text("n"))],
            expected: vec![("name".into(), text("o"))],
        };
        assert_eq!(
            build(DialectKind::Sqlite, "main", "t", &table(), &no_key),
            Err(CrudError::KeyRequired)
        );
        let no_expected = Op::Update {
            key: vec![("id".into(), "1".into())],
            set: vec![("name".into(), text("n"))],
            expected: vec![],
        };
        assert_eq!(
            build(DialectKind::Sqlite, "main", "t", &table(), &no_expected),
            Err(CrudError::BadInput)
        );
        let empty = Op::Update {
            key: vec![("id".into(), "1".into())],
            set: vec![],
            expected: vec![],
        };
        assert_eq!(
            build(DialectKind::Sqlite, "main", "t", &table(), &empty),
            Err(CrudError::BadInput)
        );
        let on_key = Op::Update {
            key: vec![("id".into(), "1".into())],
            set: vec![("total".into(), text("1"))],
            expected: vec![("total".into(), text("0"))],
        };
        assert_eq!(
            build(DialectKind::Sqlite, "main", "t", &table(), &on_key),
            Err(CrudError::ReadOnlyColumn)
        );
    }

    #[test]
    fn delete_needs_the_whole_primary_key_and_tables_without_one_are_refused() {
        let ok = build(
            DialectKind::PostgreSql,
            "s",
            "t",
            &table(),
            &Op::Delete {
                key: vec![("id".into(), "7".into())],
            },
        )
        .unwrap();
        assert_eq!(
            ok.sql,
            "DELETE FROM \"s\".\"t\" WHERE \"id\" = CAST($1 AS integer)"
        );
        assert_eq!(ok.kind, "delete");
        for key in [
            vec![],
            vec![("name".into(), "x".into())],
            vec![("id".into(), "1".into()), ("name".into(), "x".into())],
        ] {
            assert_eq!(
                build(
                    DialectKind::PostgreSql,
                    "s",
                    "t",
                    &table(),
                    &Op::Delete { key }
                ),
                Err(CrudError::KeyRequired)
            );
        }
        let keyless = vec![col("a", "text", false, true)];
        assert_eq!(
            build(
                DialectKind::Sqlite,
                "main",
                "t",
                &keyless,
                &Op::Delete {
                    key: vec![("a".into(), "1".into())]
                }
            ),
            Err(CrudError::KeyRequired)
        );
        // Composite keys need every part.
        let composite = vec![
            col("a", "integer", true, false),
            col("b", "text", true, false),
            col("c", "text", false, true),
        ];
        assert!(
            build(
                DialectKind::Sqlite,
                "main",
                "t",
                &composite,
                &Op::Delete {
                    key: vec![("a".into(), "1".into()), ("b".into(), "x".into())]
                }
            )
            .is_ok()
        );
        assert_eq!(
            build(
                DialectKind::Sqlite,
                "main",
                "t",
                &composite,
                &Op::Delete {
                    key: vec![("a".into(), "1".into())]
                }
            ),
            Err(CrudError::KeyRequired)
        );
    }

    #[test]
    fn catalog_type_names_are_validated_before_interpolation() {
        let evil = vec![col("id", "integer); DROP TABLE t; --", true, false)];
        assert_eq!(
            build(
                DialectKind::PostgreSql,
                "s",
                "t",
                &evil,
                &Op::Delete {
                    key: vec![("id".into(), "1".into())]
                }
            ),
            Err(CrudError::UnsupportedType)
        );
    }
}
