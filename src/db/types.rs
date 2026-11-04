//! Engine-neutral descriptions. Never encode PostgreSQL catalog assumptions here.
//!
//! The contract distinguishes unsupported information from known-empty values.
//! Future engines can implement this API without changing HTML view models.

use std::fmt;

/// A named, operator-configured database connection, not a browser-supplied URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    PostgreSql,
    MySql,
    MariaDb,
    Sqlite,
    SqlServer,
}

/// Capabilities vary between database families and user permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Schemas,
    Views,
    MaterializedViews,
    Indexes,
    ForeignKeys,
    EstimatedRows,
    StorageSizes,
    SpatialTypes,
    StableCursorPaging,
}

/// Explicit presence information: do not use zero as a stand-in for unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability<T> {
    Available(T),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelationId {
    pub namespace: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    Table,
    View,
    MaterializedView,
    Partition,
    ForeignTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Namespace {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relation {
    pub id: RelationId,
    pub kind: RelationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub native_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub default_expression: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationStats {
    pub estimated_rows: Availability<i64>,
    pub table_bytes: Availability<u64>,
    pub index_bytes: Availability<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(String),
    Text(String),
    Bytes(Vec<u8>),
    Unsupported { type_name: String },
}

/// Server-enforced limits are independent of client-provided paging choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: u32,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowPage {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub has_more: bool,
    pub approximate_total: Availability<i64>,
}

/// No driver error strings are stored or displayed in this error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbErrorKind {
    InvalidIdentifier,
    PermissionDenied,
    AuthenticationFailed,
    ConnectionUnavailable,
    TimedOut,
    Unsupported,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbError {
    pub kind: DbErrorKind,
}

impl DbError {
    pub const fn new(kind: DbErrorKind) -> Self {
        Self { kind }
    }

    pub const fn code(self) -> &'static str {
        match self.kind {
            DbErrorKind::InvalidIdentifier => "invalid_identifier",
            DbErrorKind::PermissionDenied => "permission_denied",
            DbErrorKind::AuthenticationFailed => "authentication_failed",
            DbErrorKind::ConnectionUnavailable => "connection_unavailable",
            DbErrorKind::TimedOut => "database_timeout",
            DbErrorKind::Unsupported => "unsupported_capability",
            DbErrorKind::Internal => "database_error",
        }
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for DbError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Ready,
    Unavailable,
    AuthenticationFailed,
    PermissionDenied,
}

/// Safe to serialize or display: no host, username or URL fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionSnapshot {
    pub engine: Engine,
    pub status: ConnectionStatus,
    pub error_code: Option<&'static str>,
}

impl ConnectionSnapshot {
    pub const fn disconnected() -> Self {
        Self {
            engine: Engine::PostgreSql,
            status: ConnectionStatus::Disconnected,
            error_code: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_statistic_is_distinct_from_zero() {
        assert_ne!(Availability::Available(0_i64), Availability::Unavailable);
    }

    #[test]
    fn contracts_are_engine_neutral() {
        let id = RelationId {
            namespace: "sales".into(),
            name: "orders".into(),
        };
        assert_eq!(id.namespace, "sales");
        assert_eq!(RelationKind::View, RelationKind::View);
        assert_eq!(
            ConnectionSnapshot::disconnected().status,
            ConnectionStatus::Disconnected
        );
    }

    #[test]
    fn errors_never_hold_database_driver_messages() {
        let e = DbError::new(DbErrorKind::AuthenticationFailed);
        assert_eq!(e.to_string(), "authentication_failed");
    }
}
