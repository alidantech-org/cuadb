//! MySQL/MariaDB catalog and bounded row reads.
use std::time::Duration;

use sqlx::{MySqlPool, Row, mysql::MySqlRow};
use tokio::sync::RwLock;
use tokio::time::timeout;

use crate::db::{
    adapter::DatabaseAdapter,
    mysql::manager::classify,
    types::{
        Availability, Capability, CatalogList, Cell, Column, ColumnDetail, Constraint,
        ConstraintKind, DbError, DbErrorKind, Engine, Index, Namespace, Overview, PageRequest,
        PartitionLink, Relation, RelationId, RelationKind, RelationStats, RowOptions, RowPage,
        RowPreview, SortDirection,
    },
};

const MAX_SCHEMAS: i64 = 300;
const MAX_COLUMNS: i64 = 512;
const MAX_DETAILS: i64 = 300;
const MAX_VISIBLE_COLUMNS: usize = 24;
const MAX_PAGE_SIZE: u32 = 1000;
const MAX_OFFSET: u64 = 10_000;
const PREVIEW_CHARS: usize = 257;

#[derive(Clone)]
pub struct MySqlCatalog {
    pool: MySqlPool,
    deadline: Duration,
    max_page_size: usize,
    engine: std::sync::Arc<RwLock<Engine>>,
}

impl MySqlCatalog {
    pub fn new(pool: MySqlPool, deadline: Duration, max_page_size: usize) -> Self {
        Self {
            pool,
            deadline,
            max_page_size,
            engine: std::sync::Arc::new(RwLock::new(Engine::MySql)),
        }
    }

    pub fn with_engine_state(mut self, engine: std::sync::Arc<RwLock<Engine>>) -> Self {
        self.engine = engine;
        self
    }

    async fn current_engine(&self) -> Engine {
        *self.engine.read().await
    }

    async fn bounded<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, sqlx::Error>>,
    ) -> Result<T, DbError> {
        timeout(self.deadline, future)
            .await
            .map_err(|_| DbError::new(DbErrorKind::TimedOut))?
            .map_err(classify)
    }

    async fn visible_relation(&self, id: &RelationId) -> Result<(), DbError> {
        let count: i64 = self.bounded(sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=? AND table_name=?",
        ).bind(&id.namespace).bind(&id.name).fetch_one(&self.pool)).await?;
        if count == 1 {
            Ok(())
        } else {
            Err(DbError::new(DbErrorKind::InvalidIdentifier))
        }
    }
}

/// Reads a result column. MySQL 8 reports information_schema names in upper case
/// and many text columns as VARBINARY; MariaDB uses lower case and VARCHAR.
trait Field: Sized {
    fn read(row: &MySqlRow, name: &str) -> Result<Self, sqlx::Error>;
}
fn text(row: &MySqlRow, name: &str) -> Result<Option<String>, sqlx::Error> {
    match row.try_get::<Option<String>, _>(name) {
        Ok(value) => Ok(value),
        Err(_) => row
            .try_get::<Option<Vec<u8>>, _>(name)
            .map(|bytes| bytes.map(|b| String::from_utf8_lossy(&b).into_owned())),
    }
}
impl Field for String {
    fn read(row: &MySqlRow, name: &str) -> Result<Self, sqlx::Error> {
        text(row, name)?.ok_or_else(|| sqlx::Error::ColumnNotFound(name.into()))
    }
}
impl Field for Option<String> {
    fn read(row: &MySqlRow, name: &str) -> Result<Self, sqlx::Error> {
        text(row, name)
    }
}
macro_rules! plain_field {
    ($($t:ty),*) => {$(
        impl Field for $t {
            fn read(row: &MySqlRow, name: &str) -> Result<Self, sqlx::Error> {
                row.try_get(name)
            }
        }
    )*};
}
plain_field!(i8, u64, Option<u64>);

fn get<T: Field>(row: &MySqlRow, field: &str) -> Result<T, DbError> {
    T::read(row, field)
        .or_else(|_| T::read(row, field.to_ascii_uppercase().as_str()))
        .map_err(|_| DbError::new(DbErrorKind::Internal))
}

fn capped<T>(mut items: Vec<T>, cap: usize) -> CatalogList<T> {
    let truncated = items.len() > cap;
    items.truncate(cap);
    CatalogList { items, truncated }
}

fn quote(identifier: &str) -> Result<String, DbError> {
    if identifier.is_empty() || identifier.len() > 64 || identifier.chars().any(char::is_control) {
        return Err(DbError::new(DbErrorKind::InvalidIdentifier));
    }
    Ok(format!("`{}`", identifier.replace('`', "``")))
}

fn sort_supported(native_type: &str) -> bool {
    let native = native_type.to_ascii_lowercase();
    !native.contains("json")
        && !native.contains("blob")
        && !native.contains("binary")
        && !native.contains("geometry")
        && !native.contains("text")
        && !native.contains("point")
}

fn is_binary(native_type: &str) -> bool {
    let native = native_type.to_ascii_lowercase();
    native.contains("blob") || native.contains("binary") || native.starts_with("bit")
}

fn cell(raw: Option<String>, native_type: &str) -> (Cell, RowPreview) {
    let Some(raw) = raw else {
        return (
            Cell::Null,
            RowPreview {
                text: "NULL".into(),
                truncated: false,
                type_name: native_type.into(),
                detail: None,
            },
        );
    };
    let mut preview = raw.chars().take(256).collect::<String>();
    let truncated = raw.chars().count() > 256;
    if truncated && !is_binary(native_type) {
        return (
            Cell::Truncated {
                preview: preview.clone(),
                type_name: native_type.into(),
            },
            RowPreview {
                text: preview,
                truncated: true,
                type_name: native_type.into(),
                detail: None,
            },
        );
    }
    let native = native_type.to_ascii_lowercase();
    let value = if is_binary(native_type) {
        decode_hex(&raw).map_or_else(
            || Cell::Unsupported {
                type_name: native_type.into(),
            },
            Cell::Bytes,
        )
    } else if native.starts_with("tinyint(1)") || native == "boolean" || native == "bool" {
        match raw.as_str() {
            "1" | "true" => Cell::Boolean(true),
            "0" | "false" => Cell::Boolean(false),
            _ => Cell::Unsupported {
                type_name: native_type.into(),
            },
        }
    } else if native.starts_with("tinyint")
        || native.starts_with("smallint")
        || native.starts_with("mediumint")
        || native.starts_with("int")
        || native.starts_with("bigint")
    {
        raw.parse::<i64>()
            .map(Cell::Integer)
            .unwrap_or_else(|_| Cell::Unsupported {
                type_name: native_type.into(),
            })
    } else if native.starts_with("decimal")
        || native.starts_with("numeric")
        || native.starts_with("float")
        || native.starts_with("double")
    {
        Cell::Decimal(raw.clone())
    } else if truncated {
        Cell::Truncated {
            preview: preview.clone(),
            type_name: native_type.into(),
        }
    } else {
        Cell::Text(raw)
    };
    if let Cell::Bytes(bytes) = &value {
        preview = format!("{} bytes", bytes.len());
    }
    (
        value,
        RowPreview {
            text: preview,
            truncated,
            type_name: native_type.into(),
            detail: None,
        },
    )
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|part| {
            let pair = std::str::from_utf8(part).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect()
}

#[async_trait::async_trait]
impl DatabaseAdapter for MySqlCatalog {
    fn engine(&self) -> Engine {
        Engine::MySql
    }

    async fn detected_engine(&self) -> Engine {
        self.current_engine().await
    }

    fn sort_supported(&self, native_type: &str) -> bool {
        sort_supported(native_type)
    }

    fn supports(&self, capability: Capability) -> bool {
        matches!(
            capability,
            Capability::Schemas
                | Capability::Views
                | Capability::Indexes
                | Capability::ForeignKeys
                | Capability::EstimatedRows
                | Capability::StorageSizes
                | Capability::StableCursorPaging
        )
    }

    async fn namespaces(&self) -> Result<Vec<Namespace>, DbError> {
        Ok(self.list_schemas().await?.items)
    }

    async fn relations(&self, namespace: &str) -> Result<Vec<Relation>, DbError> {
        Ok(self.list_relations(namespace).await?.items)
    }

    async fn columns(&self, relation: &RelationId) -> Result<Vec<Column>, DbError> {
        Ok(self
            .describe_columns(relation)
            .await?
            .items
            .into_iter()
            .map(|detail| detail.column)
            .collect())
    }

    async fn statistics(&self, relation: &RelationId) -> Result<RelationStats, DbError> {
        self.relation_statistics(relation).await
    }

    async fn column_details(
        &self,
        relation: &RelationId,
    ) -> Result<CatalogList<ColumnDetail>, DbError> {
        self.describe_columns(relation).await
    }

    async fn constraints(&self, relation: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        self.list_constraints(relation).await
    }

    async fn partitions(
        &self,
        relation: &RelationId,
    ) -> Result<CatalogList<PartitionLink>, DbError> {
        self.visible_relation(relation).await?;
        let rows = self
            .bounded(
                sqlx::query(
                    "SELECT partition_name, partition_method, partition_description \
                     FROM information_schema.partitions \
                     WHERE table_schema=? AND table_name=? AND partition_name IS NOT NULL \
                     ORDER BY partition_ordinal_position LIMIT ?",
                )
                .bind(&relation.namespace)
                .bind(&relation.name)
                .bind(MAX_DETAILS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        let items = rows
            .iter()
            .map(|row| {
                let name: String = get(row, "partition_name")?;
                let method: Option<String> = get(row, "partition_method")?;
                let description: Option<String> = get(row, "partition_description")?;
                Ok(PartitionLink {
                    parent: relation.clone(),
                    child: RelationId {
                        namespace: relation.namespace.clone(),
                        name,
                    },
                    bound: Some(format!(
                        "{} {}",
                        method.unwrap_or_default(),
                        description.unwrap_or_default()
                    ))
                    .map(|bound| bound.trim().to_string())
                    .filter(|bound| !bound.is_empty()),
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        Ok(capped(items, MAX_DETAILS as usize))
    }

    async fn indexes(&self, relation: &RelationId) -> Result<CatalogList<Index>, DbError> {
        self.list_indexes(relation).await
    }

    async fn overview(&self) -> Result<Overview, DbError> {
        Err(DbError::new(DbErrorKind::Unsupported))
    }

    async fn page(&self, relation: &RelationId, options: RowOptions) -> Result<RowPage, DbError> {
        self.fetch_rows(relation, options).await
    }

    async fn rows(&self, relation: &RelationId, request: PageRequest) -> Result<RowPage, DbError> {
        if request.limit == 0 || !request.offset.is_multiple_of(u64::from(request.limit)) {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let page = u32::try_from(request.offset / u64::from(request.limit))
            .map_err(|_| DbError::new(DbErrorKind::InvalidIdentifier))?;
        self.fetch_rows(
            relation,
            RowOptions {
                limit: request.limit,
                page,
                ..RowOptions::default()
            },
        )
        .await
    }

    async fn list_schemas(&self) -> Result<CatalogList<Namespace>, DbError> {
        let rows = self
            .bounded(
                sqlx::query(
                    "SELECT schema_name FROM information_schema.schemata \
             WHERE schema_name NOT IN ('information_schema','mysql','performance_schema','sys') \
             ORDER BY schema_name LIMIT ?",
                )
                .bind(MAX_SCHEMAS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|row| get(row, "schema_name").map(|name| Namespace { name }))
                .collect::<Result<Vec<_>, _>>()?,
            MAX_SCHEMAS as usize,
        ))
    }

    async fn list_relations(&self, namespace: &str) -> Result<CatalogList<Relation>, DbError> {
        let rows = self
            .bounded(
                sqlx::query(
                    "SELECT table_name, table_type FROM information_schema.tables \
             WHERE table_schema=? AND table_type IN ('BASE TABLE','VIEW') \
             ORDER BY table_name",
                )
                .bind(namespace)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|row| {
                    let name: String = get(row, "table_name")?;
                    let kind: String = get(row, "table_type")?;
                    Ok(Relation {
                        id: RelationId {
                            namespace: namespace.into(),
                            name,
                        },
                        kind: if kind == "VIEW" {
                            RelationKind::View
                        } else {
                            RelationKind::Table
                        },
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            usize::MAX,
        ))
    }

    async fn describe_columns(
        &self,
        id: &RelationId,
    ) -> Result<CatalogList<ColumnDetail>, DbError> {
        self.visible_relation(id).await?;
        let rows = self.bounded(sqlx::query(
            "SELECT column_name, column_type, is_nullable, column_default, column_key, extra, generation_expression, ordinal_position \
             FROM information_schema.columns WHERE table_schema=? AND table_name=? \
             ORDER BY ordinal_position LIMIT ?",
        ).bind(&id.namespace).bind(&id.name).bind(MAX_COLUMNS + 1).fetch_all(&self.pool)).await?;
        Ok(capped(
            rows.iter()
                .map(|row| {
                    let name: String = get(row, "column_name")?;
                    let native_type: String = get(row, "column_type")?;
                    let nullable: String = get(row, "is_nullable")?;
                    let column_key: String = get(row, "column_key")?;
                    let extra: String = get(row, "extra")?;
                    let generated: Option<String> = get(row, "generation_expression")?;
                    let position: u64 = get(row, "ordinal_position")?;
                    let default_expression: Option<String> = get(row, "column_default")?;
                    Ok(ColumnDetail {
                        column: Column {
                            name,
                            native_type,
                            nullable: nullable == "YES",
                            primary_key: column_key == "PRI",
                            default_expression,
                        },
                        position: i16::try_from(position).unwrap_or(i16::MAX),
                        unique_key: matches!(column_key.as_str(), "PRI" | "UNI"),
                        identity: extra
                            .contains("auto_increment")
                            .then(|| "auto_increment".into()),
                        generated: generated
                            .filter(|value| !value.is_empty())
                            .or_else(|| extra.contains("GENERATED").then(|| extra.clone())),
                        domain: None,
                        enum_values: Availability::Unavailable,
                        comment: None,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_COLUMNS as usize,
        ))
    }

    async fn list_indexes(&self, id: &RelationId) -> Result<CatalogList<Index>, DbError> {
        self.visible_relation(id).await?;
        let rows = self
            .bounded(
                sqlx::query(
                    "SELECT index_name, non_unique, seq_in_index, column_name, index_type \
             FROM information_schema.statistics WHERE table_schema=? AND table_name=? \
             ORDER BY index_name, seq_in_index LIMIT ?",
                )
                .bind(&id.namespace)
                .bind(&id.name)
                .bind(MAX_DETAILS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        let mut grouped: Vec<Index> = Vec::new();
        for row in &rows {
            let name: String = get(row, "index_name")?;
            let non_unique: i8 = get(row, "non_unique")?;
            let column_name: Option<String> = get(row, "column_name")?;
            let primary = name == "PRIMARY";
            if let Some(index) = grouped.iter_mut().find(|index| index.name == name) {
                if let Some(column) = column_name {
                    index.columns_or_expressions.push(column);
                }
            } else {
                grouped.push(Index {
                    name,
                    columns_or_expressions: column_name.into_iter().collect(),
                    unique: non_unique == 0,
                    primary,
                    valid: true,
                    predicate: None,
                    definition: get::<String>(row, "index_type")?,
                });
            }
        }
        Ok(capped(grouped, MAX_DETAILS as usize))
    }

    async fn list_constraints(&self, id: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        self.visible_relation(id).await?;
        let rows = self
            .bounded(
                sqlx::query(
                    "SELECT tc.constraint_name, tc.constraint_type, kcu.column_name, \
             kcu.referenced_table_schema, kcu.referenced_table_name, kcu.referenced_column_name \
             FROM information_schema.table_constraints tc \
             LEFT JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_schema=kcu.constraint_schema AND tc.table_name=kcu.table_name \
              AND tc.constraint_name=kcu.constraint_name \
             WHERE tc.table_schema=? AND tc.table_name=? \
             ORDER BY tc.constraint_name, kcu.ordinal_position LIMIT ?",
                )
                .bind(&id.namespace)
                .bind(&id.name)
                .bind(MAX_DETAILS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        let mut constraints: Vec<Constraint> = Vec::new();
        for row in &rows {
            let name: String = get(row, "constraint_name")?;
            let kind: String = get(row, "constraint_type")?;
            let kind = match kind.as_str() {
                "PRIMARY KEY" => ConstraintKind::Primary,
                "UNIQUE" => ConstraintKind::Unique,
                "FOREIGN KEY" => ConstraintKind::ForeignKey,
                "CHECK" => ConstraintKind::Check,
                _ => continue,
            };
            let column: Option<String> = get(row, "column_name")?;
            let target_schema: Option<String> = get(row, "referenced_table_schema")?;
            let target_name: Option<String> = get(row, "referenced_table_name")?;
            let target_column: Option<String> = get(row, "referenced_column_name")?;
            if let Some(existing) = constraints.iter_mut().find(|item| item.name == name) {
                if let Some(column) = column {
                    existing.columns.push(column);
                }
                if let Some(column) = target_column {
                    existing.referenced_columns.push(column);
                }
            } else {
                let references = target_schema
                    .zip(target_name)
                    .map(|(namespace, name)| RelationId { namespace, name });
                constraints.push(Constraint {
                    name,
                    kind,
                    columns: column.into_iter().collect(),
                    references,
                    referenced_columns: target_column.into_iter().collect(),
                    definition: String::new(),
                });
            }
        }
        Ok(capped(constraints, MAX_DETAILS as usize))
    }

    async fn relation_statistics(&self, id: &RelationId) -> Result<RelationStats, DbError> {
        let row = self.bounded(sqlx::query("SELECT table_type, table_rows, data_length, index_length FROM information_schema.tables WHERE table_schema=? AND table_name=? LIMIT 1").bind(&id.namespace).bind(&id.name).fetch_optional(&self.pool)).await?.ok_or_else(|| DbError::new(DbErrorKind::InvalidIdentifier))?;
        let table_type: String = get(&row, "table_type")?;
        let rows: Option<u64> = get(&row, "table_rows")?;
        let data: Option<u64> = get(&row, "data_length")?;
        let indexes: Option<u64> = get(&row, "index_length")?;
        let table_bytes = data.map_or(Availability::Unavailable, Availability::Available);
        let index_bytes = indexes.map_or(Availability::Unavailable, Availability::Available);
        let total_bytes = data
            .zip(indexes)
            .map_or(Availability::Unavailable, |(a, b)| {
                Availability::Available(a.saturating_add(b))
            });
        Ok(RelationStats {
            relation_kind: Availability::Available(if table_type == "VIEW" {
                RelationKind::View
            } else {
                RelationKind::Table
            }),
            estimated_rows: rows
                .and_then(|n| i64::try_from(n).ok())
                .map_or(Availability::Unavailable, Availability::Available),
            table_bytes,
            index_bytes,
            total_bytes,
        })
    }

    async fn fetch_rows(&self, id: &RelationId, options: RowOptions) -> Result<RowPage, DbError> {
        if options.limit == 0
            || options.limit > MAX_PAGE_SIZE
            || options.limit as usize > self.max_page_size
            || options.page > 100
            || options.cursor.is_some()
            || options.spatial_geojson
        {
            return Err(DbError::new(DbErrorKind::Unsupported));
        }
        let offset = u64::from(options.page) * u64::from(options.limit);
        if offset > MAX_OFFSET {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let details = self.describe_columns(id).await?;
        if details.truncated {
            return Err(DbError::new(DbErrorKind::Unsupported));
        }
        let columns = details.items;
        let selected = if options.selected_columns.is_empty() && !options.column_selection_explicit
        {
            columns
                .iter()
                .take(MAX_VISIBLE_COLUMNS)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            if options.selected_columns.len() > MAX_VISIBLE_COLUMNS {
                return Err(DbError::new(DbErrorKind::InvalidIdentifier));
            }
            options
                .selected_columns
                .iter()
                .map(|name| {
                    columns
                        .iter()
                        .find(|column| column.column.name == *name)
                        .cloned()
                        .ok_or_else(|| DbError::new(DbErrorKind::InvalidIdentifier))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut names = Vec::new();
        for column in &selected {
            if names.contains(&column.column.name) {
                return Err(DbError::new(DbErrorKind::InvalidIdentifier));
            }
            names.push(column.column.name.clone());
        }
        let mut projections = Vec::new();
        for (i, column) in selected.iter().enumerate() {
            let q = quote(&column.column.name)?;
            let expr = if is_binary(&column.column.native_type) {
                // Bound the transfer in the database: never HEX() a whole blob.
                if column
                    .column
                    .native_type
                    .to_ascii_lowercase()
                    .starts_with("bit")
                {
                    format!("HEX({q})")
                } else {
                    format!("HEX(SUBSTRING({q}, 1, {PREVIEW_CHARS}))")
                }
            } else {
                format!("LEFT(CAST({q} AS CHAR), {PREVIEW_CHARS})")
            };
            projections.push(format!("{expr} AS `c{i}`"));
        }
        if projections.is_empty() {
            projections.push("1 AS `__cuadb_empty`".into());
        }
        let mut predicate = String::new();
        if let Some(filter) = options.filter.as_deref() {
            if filter.is_empty() || filter.len() > 128 {
                return Err(DbError::new(DbErrorKind::InvalidIdentifier));
            }
            let search = columns
                .iter()
                .map(|column| quote(&column.column.name).map(|q| format!("CAST({q} AS CHAR)")))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            predicate = format!(" WHERE CONCAT_WS(' ', {search}) LIKE ? ESCAPE '!'");
        }
        let order = if let Some(sort) = &options.sort {
            let column = columns
                .iter()
                .find(|column| column.column.name == sort.column)
                .ok_or_else(|| DbError::new(DbErrorKind::InvalidIdentifier))?;
            if !sort_supported(&column.column.native_type) {
                return Err(DbError::new(DbErrorKind::Unsupported));
            }
            // NULLs last in both directions, matching the PostgreSQL adapter.
            // Skipped for NOT NULL columns so an index can serve the sort.
            let nulls_last = if column.column.nullable {
                format!("({}) IS NULL, ", quote(&sort.column)?)
            } else {
                String::new()
            };
            let mut order = format!(
                " ORDER BY {nulls_last}{} {}",
                quote(&sort.column)?,
                if sort.direction == SortDirection::Asc {
                    "ASC"
                } else {
                    "DESC"
                }
            );
            let primary = columns
                .iter()
                .filter(|column| column.column.primary_key && column.column.name != sort.column)
                .map(|column| quote(&column.column.name))
                .collect::<Result<Vec<_>, _>>()?;
            if !primary.is_empty() {
                order.push_str(", ");
                order.push_str(&primary.join(", "));
            }
            order
        } else {
            let primary = columns
                .iter()
                .filter(|column| column.column.primary_key)
                .map(|column| quote(&column.column.name))
                .collect::<Result<Vec<_>, _>>()?;
            if primary.is_empty() {
                String::new()
            } else {
                format!(" ORDER BY {}", primary.join(", "))
            }
        };
        let query = format!(
            "SELECT {} FROM {}.{}{}{} LIMIT ? OFFSET ?",
            projections.join(", "),
            quote(&id.namespace)?,
            quote(&id.name)?,
            predicate,
            order
        );
        let mut q = sqlx::query(&query);
        if let Some(filter) = options.filter.as_deref() {
            // Literal substring: neutralize LIKE wildcards and the escape character.
            let literal = filter
                .replace('!', "!!")
                .replace('%', "!%")
                .replace('_', "!_");
            q = q.bind(format!("%{literal}%"));
        }
        let rows = self
            .bounded(
                q.bind(i64::from(options.limit) + 1)
                    .bind(offset as i64)
                    .fetch_all(&self.pool),
            )
            .await?;
        let has_more = rows.len() > options.limit as usize;
        let mut values = Vec::new();
        let mut previews = Vec::new();
        for row in rows.iter().take(options.limit as usize) {
            let mut cells = Vec::new();
            let mut cell_previews = Vec::new();
            for (i, column) in selected.iter().enumerate() {
                let value: Option<String> = get(row, &format!("c{i}"))?;
                let (cell, preview) = cell(value, &column.column.native_type);
                cells.push(cell);
                cell_previews.push(preview);
            }
            values.push(cells);
            previews.push(cell_previews);
        }
        let statistics = self.relation_statistics(id).await?;
        let has_key = columns.iter().any(|column| column.column.primary_key);
        Ok(RowPage {
            available_columns: columns.iter().map(|column| column.column.clone()).collect(),
            columns: selected.into_iter().map(|column| column.column).collect(),
            rows: values,
            previews,
            has_more,
            has_previous: options.page > 0,
            next_cursor: None,
            previous_cursor: None,
            stable_order: has_key,
            keyset_enabled: false,
            page: options.page,
            offset,
            approximate_total: if options.filter.is_some() {
                Availability::Unavailable
            } else {
                statistics.estimated_rows
            },
            truncated_columns: options.selected_columns.is_empty()
                && columns.len() > MAX_VISIBLE_COLUMNS,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_mysql_identifiers_and_decodes_cells_safely() {
        assert_eq!(quote("a`b").unwrap(), "`a``b`");
        assert!(quote("x\0y").is_err());
        assert_eq!(cell(Some("42".into()), "int").0, Cell::Integer(42));
        assert_eq!(
            cell(Some("00ff".into()), "blob").0,
            Cell::Bytes(vec![0, 255])
        );
        assert_eq!(cell(None, "varchar(20)").0, Cell::Null);
    }
}
