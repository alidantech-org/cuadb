//! PostgreSQL SQL workspace. Reads run in a read-only transaction on the explorer's
//! read-only pool with a server-side `statement_timeout`; writes use a separate
//! writer pool that exists only for `read-write` profiles.
//!
//! Results are produced with `to_json(row)` so any column type renders without a
//! per-type decoder; numbers keep their exact text through `RawValue`.

use std::time::Instant;

use sqlx::{Executor, PgPool, Postgres, Row, Transaction};

use crate::{
    db::types::{Cell, DbError, DbErrorKind},
    sql::{
        exec::{CancelHandle, ExecLimits, SqlResult, Workspace, WriteTx},
        policy::DialectKind,
    },
};

pub struct PgWorkspace {
    read: PgPool,
    write: Option<PgPool>,
}

impl PgWorkspace {
    pub fn new(read: PgPool, write: Option<PgPool>) -> Self {
        Self { read, write }
    }

    pub async fn close(&self) {
        self.read.close().await;
        if let Some(write) = &self.write {
            write.close().await;
        }
    }
}

fn map(error: sqlx::Error) -> DbError {
    if let sqlx::Error::Database(db) = &error {
        crate::sql::exec::note_db_message(db.message());
    }
    DbError::new(match &error {
        sqlx::Error::Database(db) => match db.code().as_deref() {
            Some("57014") => DbErrorKind::TimedOut,
            Some("42501") => DbErrorKind::PermissionDenied,
            Some("25006") => DbErrorKind::PermissionDenied,
            Some(code) if code.starts_with("23") => DbErrorKind::Conflict,
            Some("40001" | "40P01" | "55P03") => DbErrorKind::Conflict,
            Some(code) if code.starts_with("42") || code.starts_with("22") => DbErrorKind::Rejected,
            Some("28P01" | "28000") => DbErrorKind::AuthenticationFailed,
            _ => DbErrorKind::Rejected,
        },
        sqlx::Error::PoolTimedOut => DbErrorKind::TimedOut,
        _ => DbErrorKind::ConnectionUnavailable,
    })
}

fn bare(sql: &str) -> &str {
    sql.trim().trim_end_matches(';').trim_end()
}

/// One JSON row token -> ordered (key, cell) pairs, numbers kept exact.
fn parse_row(json: &str) -> Result<Vec<(String, Cell)>, DbError> {
    use serde::de::{MapAccess, Visitor};
    use serde_json::value::RawValue;

    struct Pairs(Vec<(String, Box<RawValue>)>);
    impl<'de> serde::Deserialize<'de> for Pairs {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = Pairs;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("a JSON object")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pairs, A::Error> {
                    let mut out = Vec::new();
                    while let Some(entry) = map.next_entry::<String, Box<RawValue>>()? {
                        out.push(entry);
                    }
                    Ok(Pairs(out))
                }
            }
            d.deserialize_map(V)
        }
    }
    let Pairs(pairs) =
        serde_json::from_str(json).map_err(|_| DbError::new(DbErrorKind::Internal))?;
    Ok(pairs
        .into_iter()
        .map(|(key, raw)| {
            let token = raw.get();
            let cell = match token {
                "null" => Cell::Null,
                "true" => Cell::Boolean(true),
                "false" => Cell::Boolean(false),
                _ if token.starts_with('"') => {
                    let text: String = serde_json::from_str(token).unwrap_or_default();
                    if text.chars().count() > 4096 {
                        Cell::Truncated {
                            preview: text.chars().take(4096).collect(),
                            type_name: "text".into(),
                        }
                    } else {
                        Cell::Text(text)
                    }
                }
                _ if token.starts_with('{') || token.starts_with('[') => {
                    if token.len() > 4096 {
                        Cell::Truncated {
                            preview: token.chars().take(4096).collect(),
                            type_name: "json".into(),
                        }
                    } else {
                        Cell::Text(token.to_string())
                    }
                }
                _ => match token.parse::<i64>() {
                    Ok(n) => Cell::Integer(n),
                    Err(_) => Cell::Decimal(token.to_string()),
                },
            };
            (key, cell)
        })
        .collect())
}

fn cell_bytes(cell: &Cell) -> usize {
    match cell {
        Cell::Text(t) | Cell::Decimal(t) => t.len(),
        Cell::Truncated { preview, .. } => preview.len(),
        Cell::Bytes(b) => b.len(),
        _ => 8,
    }
}

fn is_explain(sql: &str) -> bool {
    sql.trim_start().to_ascii_lowercase().starts_with("explain")
}

async fn run_read(
    conn: &mut sqlx::PgConnection,
    sql: &str,
    limits: &ExecLimits,
) -> Result<SqlResult, DbError> {
    let statement = bare(sql);
    if is_explain(statement) {
        let rows = sqlx::query(statement)
            .fetch_all(&mut *conn)
            .await
            .map_err(map)?;
        let mut out = SqlResult {
            columns: vec!["QUERY PLAN".into()],
            rows: vec![],
            truncated: false,
            affected: None,
            elapsed_ms: 0,
        };
        for row in rows.iter().take(limits.max_rows) {
            out.rows.push(vec![Cell::Text(
                row.try_get::<String, _>(0).unwrap_or_default(),
            )]);
        }
        return Ok(out);
    }
    // Column names (in order, duplicates included) without running the statement.
    let described = Executor::describe(&mut *conn, statement)
        .await
        .map_err(map)?;
    let columns: Vec<String> = described
        .columns()
        .iter()
        .map(|c| sqlx::Column::name(c).to_string())
        .collect();
    let wrapped = format!(
        "SELECT to_json(q)::text FROM (\n{statement}\n) q LIMIT {}",
        limits.max_rows + 1
    );
    let rows = sqlx::query_scalar::<_, String>(&wrapped)
        .fetch_all(&mut *conn)
        .await
        .map_err(map)?;
    let mut out = SqlResult {
        columns,
        rows: vec![],
        truncated: false,
        affected: None,
        elapsed_ms: 0,
    };
    let mut bytes = 0usize;
    for json in rows {
        if out.rows.len() >= limits.max_rows {
            out.truncated = true;
            break;
        }
        let pairs = parse_row(&json)?;
        let cells: Vec<Cell> = pairs.into_iter().map(|(_, cell)| cell).collect();
        bytes += cells.iter().map(cell_bytes).sum::<usize>();
        if bytes > limits.max_bytes {
            out.truncated = true;
            break;
        }
        out.rows.push(cells);
    }
    Ok(out)
}

#[async_trait::async_trait]
impl Workspace for PgWorkspace {
    fn dialect(&self) -> DialectKind {
        DialectKind::PostgreSql
    }

    fn writable(&self) -> bool {
        self.write.is_some()
    }

    async fn query(
        &self,
        sql: &str,
        limits: &ExecLimits,
        cancel: &CancelHandle,
    ) -> Result<SqlResult, DbError> {
        let started = Instant::now();
        let work = async {
            let mut tx = self.read.begin().await.map_err(map)?;
            sqlx::query("SET TRANSACTION READ ONLY")
                .execute(&mut *tx)
                .await
                .map_err(map)?;
            let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
                .fetch_one(&mut *tx)
                .await
                .map_err(map)?;
            *cancel.backend_pid.lock().unwrap_or_else(|p| p.into_inner()) = Some(i64::from(pid));
            sqlx::query("SELECT pg_catalog.set_config('statement_timeout', $1, true)")
                .bind(limits.timeout.as_millis().to_string())
                .execute(&mut *tx)
                .await
                .map_err(map)?;
            let result = run_read(&mut tx, sql, limits).await;
            let _ = tx.rollback().await;
            result
        };
        let outcome = tokio::select! {
            result = tokio::time::timeout(limits.timeout + std::time::Duration::from_secs(2), work) => match result {
                Ok(inner) => inner,
                Err(_) => Err(DbError::new(DbErrorKind::TimedOut)),
            },
            _ = cancel.cancelled() => {
                let pid = *cancel.backend_pid.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(pid) = pid {
                    let _ = sqlx::query("SELECT pg_cancel_backend($1)").bind(pid as i32).execute(&self.read).await;
                }
                Err(DbError::new(DbErrorKind::Cancelled))
            }
        };
        outcome.map(|mut result| {
            result.elapsed_ms = started.elapsed().as_millis() as u64;
            result
        })
    }

    async fn begin(&self, limits: &ExecLimits) -> Result<Box<dyn WriteTx>, DbError> {
        let pool = self
            .write
            .as_ref()
            .ok_or(DbError::new(DbErrorKind::PermissionDenied))?;
        let mut tx = pool.begin().await.map_err(map)?;
        sqlx::query("SELECT pg_catalog.set_config('statement_timeout', $1, true)")
            .bind(limits.timeout.as_millis().to_string())
            .execute(&mut *tx)
            .await
            .map_err(map)?;
        sqlx::query("SELECT pg_catalog.set_config('lock_timeout', $1, true)")
            .bind(limits.timeout.as_millis().to_string())
            .execute(&mut *tx)
            .await
            .map_err(map)?;
        Ok(Box::new(PgTx { tx: Some(tx) }))
    }
}

struct PgTx {
    tx: Option<Transaction<'static, Postgres>>,
}

#[async_trait::async_trait]
impl WriteTx for PgTx {
    async fn execute(&mut self, sql: &str, binds: &[Option<String>]) -> Result<u64, DbError> {
        let tx = self
            .tx
            .as_mut()
            .ok_or(DbError::new(DbErrorKind::Internal))?;
        let mut query = sqlx::query(sql);
        for bind in binds {
            query = query.bind(bind.clone());
        }
        // A statement error aborts a PostgreSQL transaction; the caller rolls it back.
        Ok(query.execute(&mut **tx).await.map_err(map)?.rows_affected())
    }

    async fn query(&mut self, sql: &str, max_rows: usize) -> Result<SqlResult, DbError> {
        let tx = self
            .tx
            .as_mut()
            .ok_or(DbError::new(DbErrorKind::Internal))?;
        run_read(
            tx,
            sql,
            &ExecLimits {
                max_rows,
                ..ExecLimits::default()
            },
        )
        .await
    }

    async fn commit(mut self: Box<Self>) -> Result<(), DbError> {
        match self.tx.take() {
            Some(tx) => tx.commit().await.map_err(map),
            None => Err(DbError::new(DbErrorKind::Internal)),
        }
    }

    async fn rollback(mut self: Box<Self>) -> Result<(), DbError> {
        match self.tx.take() {
            Some(tx) => tx.rollback().await.map_err(map),
            None => Ok(()),
        }
    }
}
