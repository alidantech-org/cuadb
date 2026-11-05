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
    Routines,
    Sequences,
    UserTypes,
    Triggers,
    PartitionHierarchy,
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
    PartitionedTable,
    View,
    MaterializedView,
    Partition,
    ForeignTable,
    /// SQLite `CREATE VIRTUAL TABLE` (FTS, R-Tree, ...); backed by a module, not a plain table.
    VirtualTable,
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
    pub relation_kind: Availability<RelationKind>,
    /// Approximate from pg_class.reltuples, not SELECT COUNT(*).
    pub estimated_rows: Availability<i64>,
    pub table_bytes: Availability<u64>,
    pub index_bytes: Availability<u64>,
    pub total_bytes: Availability<u64>,
}

/// A bounded list never silently implies completeness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogList<T> {
    pub items: Vec<T>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDetail {
    pub column: Column,
    pub position: i16,
    pub unique_key: bool,
    pub identity: Option<String>,
    pub generated: Option<String>,
    pub domain: Option<String>,
    pub enum_values: Availability<Vec<String>>,
    /// Column comment as stored by the engine (PostgreSQL `COMMENT ON COLUMN`).
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    Primary,
    Unique,
    ForeignKey,
    Check,
    Exclusion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub name: String,
    pub kind: ConstraintKind,
    pub columns: Vec<String>,
    /// Only populated when the referenced relation is visible to the reader.
    pub references: Option<RelationId>,
    pub referenced_columns: Vec<String>,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub name: String,
    pub columns_or_expressions: Vec<String>,
    pub unique: bool,
    pub primary: bool,
    pub valid: bool,
    pub predicate: Option<String>,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionLink {
    pub parent: RelationId,
    pub child: RelationId,
    pub bound: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogObjectKind {
    Function,
    Procedure,
    Sequence,
    UserType,
    Trigger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogObject {
    pub kind: CatalogObjectKind,
    pub namespace: String,
    pub name: String,
    /// Server-rendered inspector text; never callable through HTTP.
    pub definition: Availability<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaSize {
    pub namespace: String,
    /// Derived from relpages, not an exact aggregated on-disk measurement.
    pub estimated_bytes: Availability<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overview {
    pub extensions: Vec<String>,
    pub schema_sizes: CatalogList<SchemaSize>,
    pub largest_relations: CatalogList<(RelationId, Availability<u64>)>,
    /// Counts are bounded to visible objects and may be incomplete.
    pub visible_relation_count: usize,
    pub visible_schema_count: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Null,
    Boolean(bool),
    Integer(i64),
    Decimal(String),
    Text(String),
    Bytes(Vec<u8>),
    Truncated { preview: String, type_name: String },
    Unsupported { type_name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortSpec {
    pub column: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowOptions {
    pub limit: u32,
    pub page: u32,
    pub selected_columns: Vec<String>,
    pub column_selection_explicit: bool,
    pub sort: Option<SortSpec>,
    pub cursor: Option<String>,
    /// Optional literal substring matched against catalogued relation columns.
    pub filter: Option<String>,
    /// Request bounded GeoJSON inspector details for PostGIS values.
    pub spatial_geojson: bool,
}
impl Default for RowOptions {
    fn default() -> Self {
        Self {
            limit: 25,
            page: 0,
            selected_columns: vec![],
            column_selection_explicit: false,
            sort: None,
            cursor: None,
            filter: None,
            spatial_geojson: false,
        }
    }
}

/// An explicit preview marker prevents displaying truncated data as complete.
#[derive(Debug, Clone, PartialEq)]
pub struct RowPreview {
    pub text: String,
    pub truncated: bool,
    pub type_name: String,
    pub detail: Option<String>,
}

/// Server-enforced limits are independent of client-provided paging choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: u32,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowPage {
    pub available_columns: Vec<Column>,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub has_more: bool,
    pub approximate_total: Availability<i64>,
    pub previews: Vec<Vec<RowPreview>>,
    pub has_previous: bool,
    pub next_cursor: Option<String>,
    pub previous_cursor: Option<String>,
    pub stable_order: bool,
    pub keyset_enabled: bool,
    pub page: u32,
    pub offset: u64,
    pub truncated_columns: bool,
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
    /// The statement itself was refused by the database (syntax, missing column, ...).
    Rejected,
    /// A constraint or concurrent change prevented the operation.
    Conflict,
    /// The user cancelled a running statement.
    Cancelled,
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
            DbErrorKind::Rejected => "statement_rejected",
            DbErrorKind::Conflict => "conflict",
            DbErrorKind::Cancelled => "cancelled",
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
    pub const fn unavailable() -> Self {
        Self {
            engine: Engine::PostgreSql,
            status: ConnectionStatus::Unavailable,
            error_code: None,
        }
    }

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

/// The whole text of one cell (bounded), for the row sheet's "load full value".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellValue {
    /// `None` for SQL NULL.
    pub text: Option<String>,
    /// The value was longer than [`MAX_CELL_CHARS`] and was cut.
    pub truncated: bool,
}

/// Longest cell text returned by a single-cell fetch.
pub const MAX_CELL_CHARS: usize = 65_536;
