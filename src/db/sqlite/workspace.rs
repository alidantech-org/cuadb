//! SQLite SQL workspace: a read-only pool for SELECTs and an optional writer pool
//! for explicitly read-write profiles. Both use one connection so a runaway
//! statement can never starve the explorer's own pool.
//!
//! Limitation: SQLite cannot be interrupted from here; a statement that outlives
//! its deadline is abandoned (the caller gets a timeout) but keeps running on its
//! connection until it finishes. Row and byte caps still bound what is returned.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use sqlx::{
    Column, Row, Sqlite, SqlitePool, Transaction, TypeInfo, ValueRef,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow},
};

use crate::{
    config::Limits,
    db::types::{Cell, DbError, DbErrorKind},
    sql::{
        exec::{CancelHandle, ExecLimits, SqlResult, Workspace, WriteTx},
        policy::DialectKind,
    },
};

pub struct SqliteWorkspace {
    read: SqlitePool,
    write: Option<SqlitePool>,
}

impl SqliteWorkspace {
    pub fn new(path: &Path, writable: bool, limits: &Limits) -> Self {
        let base = |read_only: bool| {
            SqliteConnectOptions::new()
                .filename(path)
                .read_only(read_only)
                .create_if_missing(false)
                .foreign_keys(true)
                .busy_timeout(limits.connection_timeout)
        };
        let pool = |options: SqliteConnectOptions, query_only: bool| {
            SqlitePoolOptions::new()
                .max_connections(1)
                .min_connections(0)
                .acquire_timeout(limits.connection_timeout)
                .idle_timeout(Some(Duration::from_secs(120)))
                .after_connect(move |connection, _| {
                    Box::pin(async move {
                        if query_only {
                            sqlx::query("PRAGMA query_only=ON")
                                .execute(&mut *connection)
                                .await?;
                        }
                        Ok(())
                    })
                })
                .connect_lazy_with(options)
        };
        Self {
            read: pool(base(true), true),
            write: writable.then(|| pool(base(false), false)),
        }
    }

    pub async fn close(&self) {
        self.read.close().await;
        if let Some(write) = &self.write {
            write.close().await;
        }
    }
}

fn map(error: sqlx::Error) -> DbError {
    DbError::new(match &error {
        sqlx::Error::PoolTimedOut => DbErrorKind::TimedOut,
        sqlx::Error::Database(db) => match db.code().as_deref() {
            // 5 BUSY, 6 LOCKED
            Some("5" | "6") => DbErrorKind::TimedOut,
            // 19 CONSTRAINT (and its extended codes 1555, 2067, 787, 1299, 275)
            Some("19" | "1555" | "2067" | "787" | "1299" | "275" | "531") => DbErrorKind::Conflict,
            // 8 READONLY, 23 AUTH
            Some("8" | "23") => DbErrorKind::PermissionDenied,
            // 1 generic SQL error (syntax, missing table/column)
            Some("1") => DbErrorKind::Rejected,
            _ => DbErrorKind::Rejected,
        },
        _ => DbErrorKind::ConnectionUnavailable,
    })
}

fn decode(row: &SqliteRow, index: usize) -> Result<Cell, DbError> {
    let raw = row
        .try_get_raw(index)
        .map_err(|_| DbError::new(DbErrorKind::Internal))?;
    if raw.is_null() {
        return Ok(Cell::Null);
    }
    let class = raw.type_info().name().to_ascii_uppercase();
    let internal = |_| DbError::new(DbErrorKind::Internal);
    Ok(match class.as_str() {
        "INTEGER" | "BOOLEAN" | "INT" | "BIGINT" => {
            Cell::Integer(row.try_get::<i64, _>(index).map_err(internal)?)
        }
        "REAL" | "FLOAT" | "DOUBLE" | "NUMERIC" => {
            Cell::Decimal(row.try_get::<f64, _>(index).map_err(internal)?.to_string())
        }
        "BLOB" => {
            let bytes: Vec<u8> = row.try_get(index).map_err(internal)?;
            if bytes.len() > 256 {
                Cell::Truncated {
                    preview: format!(
                        "\\x{}",
                        bytes[..256]
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()
                    ),
                    type_name: format!("blob ({} bytes)", bytes.len()),
                }
            } else {
                Cell::Bytes(bytes)
            }
        }
        _ => {
            let text: String = row.try_get(index).map_err(internal)?;
            if text.chars().count() > 4096 {
                Cell::Truncated {
                    preview: text.chars().take(4096).collect(),
                    type_name: "text".into(),
                }
            } else {
                Cell::Text(text)
            }
        }
    })
}

fn cell_bytes(cell: &Cell) -> usize {
    match cell {
        Cell::Text(t) | Cell::Decimal(t) => t.len(),
        Cell::Bytes(b) => b.len() * 2,
        Cell::Truncated { preview, .. } => preview.len(),
        _ => 8,
    }
}

/// Collect rows from an already-fetched stream within the caps.
fn collect(rows: Vec<SqliteRow>, limits: &ExecLimits) -> Result<SqlResult, DbError> {
    let mut out = SqlResult {
        columns: vec![],
        rows: vec![],
        truncated: false,
        affected: None,
        elapsed_ms: 0,
    };
    let mut bytes = 0usize;
    for (n, row) in rows.iter().enumerate() {
        if n == 0 {
            out.columns = row.columns().iter().map(|c| c.name().to_string()).collect();
        }
        if out.rows.len() >= limits.max_rows {
            out.truncated = true;
            break;
        }
        let cells = (0..row.columns().len())
            .map(|i| decode(row, i))
            .collect::<Result<Vec<_>, _>>()?;
        bytes += cells.iter().map(cell_bytes).sum::<usize>();
        if bytes > limits.max_bytes {
            out.truncated = true;
            break;
        }
        out.rows.push(cells);
    }
    Ok(out)
}

/// Remove trailing semicolons/whitespace so the statement can be wrapped.
fn bare(sql: &str) -> &str {
    sql.trim().trim_end_matches(';').trim_end()
}

#[async_trait::async_trait]
impl Workspace for SqliteWorkspace {
    fn dialect(&self) -> DialectKind {
        DialectKind::Sqlite
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
        // Bound the work SQLite does, not just what we keep: fetch at most max_rows + 1.
        let wrapped = format!(
            "SELECT * FROM (\n{}\n) LIMIT {}",
            bare(sql),
            limits.max_rows + 1
        );
        let run = async {
            let rows = sqlx::query(&wrapped)
                .fetch_all(&self.read)
                .await
                .map_err(map)?;
            collect(rows, limits)
        };
        let outcome = tokio::select! {
            result = tokio::time::timeout(limits.timeout, run) => match result {
                Ok(inner) => inner,
                Err(_) => Err(DbError::new(DbErrorKind::TimedOut)),
            },
            _ = cancel.cancelled() => Err(DbError::new(DbErrorKind::Cancelled)),
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
        let tx = tokio::time::timeout(limits.timeout, pool.begin())
            .await
            .map_err(|_| DbError::new(DbErrorKind::TimedOut))?
            .map_err(map)?;
        Ok(Box::new(SqliteTx {
            tx: Some(tx),
            timeout: limits.timeout,
        }))
    }
}

struct SqliteTx {
    tx: Option<Transaction<'static, Sqlite>>,
    timeout: Duration,
}

#[async_trait::async_trait]
impl WriteTx for SqliteTx {
    async fn execute(&mut self, sql: &str, binds: &[Option<String>]) -> Result<u64, DbError> {
        let tx = self
            .tx
            .as_mut()
            .ok_or(DbError::new(DbErrorKind::Internal))?;
        let mut query = sqlx::query(sql);
        for bind in binds {
            query = query.bind(bind.clone());
        }
        let done = tokio::time::timeout(self.timeout, query.execute(&mut **tx))
            .await
            .map_err(|_| DbError::new(DbErrorKind::TimedOut))?
            .map_err(map)?;
        Ok(done.rows_affected())
    }

    async fn query(&mut self, sql: &str, max_rows: usize) -> Result<SqlResult, DbError> {
        let tx = self
            .tx
            .as_mut()
            .ok_or(DbError::new(DbErrorKind::Internal))?;
        let wrapped = format!("SELECT * FROM (\n{}\n) LIMIT {}", bare(sql), max_rows + 1);
        let rows = tokio::time::timeout(self.timeout, sqlx::query(&wrapped).fetch_all(&mut **tx))
            .await
            .map_err(|_| DbError::new(DbErrorKind::TimedOut))?
            .map_err(map)?;
        collect(
            rows,
            &ExecLimits {
                max_rows,
                ..ExecLimits::default()
            },
        )
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
