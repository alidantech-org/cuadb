//! SQLite metadata and bounded read-only row adapter.
use super::classify;
use crate::db::{adapter::DatabaseAdapter, types::*};
use sqlx::{Row, Sqlite, SqlitePool, TypeInfo, ValueRef, sqlite::SqliteRow};
use std::time::Duration;
use tokio::time::timeout;
const MAX_RELATIONS: i64 = 500;
const MAX_COLUMNS: i64 = 512;
const MAX_DETAILS: i64 = 300;
const MAX_VISIBLE: usize = 24;
const MAX_PAGE: u32 = 100;
const MAX_OFFSET: u64 = 10_000;
#[derive(Clone)]
pub struct SqliteCatalog {
    pool: SqlitePool,
    deadline: Duration,
    max_page: usize,
}
impl SqliteCatalog {
    pub fn new(pool: SqlitePool, deadline: Duration, max_page: usize) -> Self {
        Self {
            pool,
            deadline,
            max_page,
        }
    }
    async fn run<T>(
        &self,
        f: impl std::future::Future<Output = Result<T, sqlx::Error>>,
    ) -> Result<T, DbError> {
        timeout(self.deadline, f)
            .await
            .map_err(|_| DbError::new(DbErrorKind::TimedOut))?
            .map_err(classify)
    }
    async fn visible(&self, id: &RelationId) -> Result<(), DbError> {
        let n: i64 = self
            .run(
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE name=? AND type IN ('table','view')",
                )
                .bind(&id.name)
                .fetch_one(&self.pool),
            )
            .await?;
        if n == 1 {
            Ok(())
        } else {
            Err(DbError::new(DbErrorKind::InvalidIdentifier))
        }
    }
    async fn details(&self, id: &RelationId) -> Result<CatalogList<ColumnDetail>, DbError> {
        self.visible(id).await?;
        let rows = self
            .run(
                sqlx::query(&format!("PRAGMA table_xinfo({})", quote(&id.name)?))
                    .fetch_all(&self.pool),
            )
            .await?;
        let mut out = Vec::new();
        for r in &rows {
            let hidden: i64 = r.try_get("hidden").unwrap_or(0);
            if hidden >= 2 {
                continue;
            }
            let name: String = get(r, "name")?;
            let ty: Option<String> = get(r, "type")?;
            let notnull: i64 = get(r, "notnull")?;
            let pk: i64 = get(r, "pk")?;
            let cid: i64 = get(r, "cid")?;
            let default_expression: Option<String> = get(r, "dflt_value")?;
            out.push(ColumnDetail {
                column: Column {
                    name,
                    native_type: ty
                        .filter(|x| !x.is_empty())
                        .unwrap_or_else(|| "dynamic".into()),
                    nullable: notnull == 0 && pk == 0,
                    primary_key: pk > 0,
                    default_expression,
                },
                position: i16::try_from(cid + 1).unwrap_or(i16::MAX),
                unique_key: pk > 0,
                identity: None,
                generated: None,
                domain: None,
                enum_values: Availability::Unavailable,
            })
        }
        Ok(capped(out, MAX_COLUMNS as usize))
    }
    async fn constraints_inner(&self, id: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        self.visible(id).await?;
        let rows = self
            .run(
                sqlx::query(&format!("PRAGMA foreign_key_list({})", quote(&id.name)?))
                    .fetch_all(&self.pool),
            )
            .await?;
        let mut out = Vec::<Constraint>::new();
        for r in &rows {
            let idnum: i64 = get(r, "id")?;
            let from: String = get(r, "from")?;
            let to: Option<String> = get(r, "to")?;
            let table: String = get(r, "table")?;
            let name = format!("foreign_key_{idnum}");
            if let Some(x) = out.iter_mut().find(|x| x.name == name) {
                x.columns.push(from);
                if let Some(to) = to {
                    x.referenced_columns.push(to)
                }
            } else {
                out.push(Constraint {
                    name,
                    kind: ConstraintKind::ForeignKey,
                    columns: vec![from],
                    references: Some(RelationId {
                        namespace: "main".into(),
                        name: table,
                    }),
                    referenced_columns: to.into_iter().collect(),
                    definition: String::new(),
                })
            }
        }
        Ok(capped(out, MAX_DETAILS as usize))
    }
    async fn indexes_inner(&self, id: &RelationId) -> Result<CatalogList<Index>, DbError> {
        self.visible(id).await?;
        let rows = self
            .run(
                sqlx::query(&format!("PRAGMA index_list({})", quote(&id.name)?))
                    .fetch_all(&self.pool),
            )
            .await?;
        let mut out = Vec::new();
        for r in rows {
            let name: String = get(&r, "name")?;
            let unique: i64 = get(&r, "unique")?;
            let cols = self
                .run(
                    sqlx::query(&format!("PRAGMA index_info({})", quote(&name)?))
                        .fetch_all(&self.pool),
                )
                .await?;
            let columns = cols
                .iter()
                .filter_map(|x| x.try_get::<Option<String>, _>("name").ok().flatten())
                .collect();
            out.push(Index {
                name,
                columns_or_expressions: columns,
                unique: unique != 0,
                primary: false,
                valid: true,
                predicate: None,
                definition: "SQLite index".into(),
            })
        }
        Ok(capped(out, MAX_DETAILS as usize))
    }
}
fn get<T>(r: &SqliteRow, n: &str) -> Result<T, DbError>
where
    T: for<'r> sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite>,
{
    r.try_get(n)
        .map_err(|_| DbError::new(DbErrorKind::Internal))
}
fn quote(s: &str) -> Result<String, DbError> {
    if s.is_empty() || s.len() > 255 || s.chars().any(char::is_control) {
        return Err(DbError::new(DbErrorKind::InvalidIdentifier));
    }
    Ok(format!("\"{}\"", s.replace('"', "\"\"")))
}
fn capped<T>(mut v: Vec<T>, n: usize) -> CatalogList<T> {
    let truncated = v.len() > n;
    v.truncate(n);
    CatalogList {
        items: v,
        truncated,
    }
}
fn cell(r: Option<String>, ty: &str) -> (Cell, RowPreview) {
    let Some(v) = r else {
        return (
            Cell::Null,
            RowPreview {
                text: "NULL".into(),
                truncated: false,
                type_name: ty.into(),
                detail: None,
            },
        );
    };
    let truncated = v.chars().count() > 256;
    let text = v.chars().take(256).collect::<String>();
    let cell = if truncated {
        Cell::Truncated {
            preview: text.clone(),
            type_name: ty.into(),
        }
    } else if matches!(v.as_str(), "0" | "1")
        && matches!(ty.to_ascii_lowercase().as_str(), "boolean" | "bool")
    {
        Cell::Boolean(v == "1")
    } else if let Ok(n) = v.parse::<i64>() {
        Cell::Integer(n)
    } else if v.parse::<f64>().is_ok() {
        Cell::Decimal(v)
    } else {
        Cell::Text(v)
    };
    (
        cell,
        RowPreview {
            text,
            truncated,
            type_name: ty.into(),
            detail: None,
        },
    )
}
#[async_trait::async_trait]
impl DatabaseAdapter for SqliteCatalog {
    fn engine(&self) -> Engine {
        Engine::Sqlite
    }
    fn supports(&self, c: Capability) -> bool {
        matches!(
            c,
            Capability::Views | Capability::Indexes | Capability::ForeignKeys
        )
    }
    fn sort_supported(&self, native_type: &str) -> bool {
        let native = native_type.to_ascii_lowercase();
        !native.contains("blob") && !native.contains("binary") && !native.contains("geometry")
    }
    async fn namespaces(&self) -> Result<Vec<Namespace>, DbError> {
        Ok(vec![Namespace {
            name: "main".into(),
        }])
    }
    async fn relations(&self, ns: &str) -> Result<Vec<Relation>, DbError> {
        Ok(self.list_relations(ns).await?.items)
    }
    async fn columns(&self, id: &RelationId) -> Result<Vec<Column>, DbError> {
        Ok(self
            .details(id)
            .await?
            .items
            .into_iter()
            .map(|x| x.column)
            .collect())
    }
    async fn statistics(&self, id: &RelationId) -> Result<RelationStats, DbError> {
        self.visible(id).await?;
        let row = self
            .run(
                sqlx::query("SELECT type FROM sqlite_schema WHERE name=? LIMIT 1")
                    .bind(&id.name)
                    .fetch_one(&self.pool),
            )
            .await?;
        let kind: String = get(&row, "type")?;
        Ok(RelationStats {
            relation_kind: Availability::Available(if kind == "view" {
                RelationKind::View
            } else {
                RelationKind::Table
            }),
            estimated_rows: Availability::Unavailable,
            table_bytes: Availability::Unavailable,
            index_bytes: Availability::Unavailable,
            total_bytes: Availability::Unavailable,
        })
    }
    async fn column_details(&self, id: &RelationId) -> Result<CatalogList<ColumnDetail>, DbError> {
        self.details(id).await
    }
    async fn constraints(&self, id: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        self.constraints_inner(id).await
    }
    async fn indexes(&self, id: &RelationId) -> Result<CatalogList<Index>, DbError> {
        self.indexes_inner(id).await
    }
    /// SQLite has no partitioning: a visible relation has an empty (not unsupported) list.
    async fn partitions(&self, id: &RelationId) -> Result<CatalogList<PartitionLink>, DbError> {
        self.visible(id).await?;
        Ok(CatalogList {
            items: vec![],
            truncated: false,
        })
    }
    async fn page(&self, id: &RelationId, o: RowOptions) -> Result<RowPage, DbError> {
        self.fetch_rows(id, o).await
    }
    async fn rows(&self, id: &RelationId, r: PageRequest) -> Result<RowPage, DbError> {
        if r.limit == 0 || !r.offset.is_multiple_of(u64::from(r.limit)) {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let page = u32::try_from(r.offset / u64::from(r.limit))
            .map_err(|_| DbError::new(DbErrorKind::InvalidIdentifier))?;
        self.fetch_rows(
            id,
            RowOptions {
                limit: r.limit,
                page,
                ..RowOptions::default()
            },
        )
        .await
    }
    async fn list_schemas(&self) -> Result<CatalogList<Namespace>, DbError> {
        Ok(CatalogList {
            items: vec![Namespace {
                name: "main".into(),
            }],
            truncated: false,
        })
    }
    async fn list_relations(&self, ns: &str) -> Result<CatalogList<Relation>, DbError> {
        if ns != "main" {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let rows=self.run(sqlx::query("SELECT name,type FROM sqlite_schema WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT ?").bind(MAX_RELATIONS+1).fetch_all(&self.pool)).await?;
        Ok(capped(
            rows.iter()
                .map(|r| {
                    let name: String = get(r, "name")?;
                    let ty: String = get(r, "type")?;
                    Ok(Relation {
                        id: RelationId {
                            namespace: "main".into(),
                            name,
                        },
                        kind: if ty == "view" {
                            RelationKind::View
                        } else {
                            RelationKind::Table
                        },
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_RELATIONS as usize,
        ))
    }
    async fn describe_columns(
        &self,
        id: &RelationId,
    ) -> Result<CatalogList<ColumnDetail>, DbError> {
        self.details(id).await
    }
    async fn list_constraints(&self, id: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        self.constraints_inner(id).await
    }
    async fn list_indexes(&self, id: &RelationId) -> Result<CatalogList<Index>, DbError> {
        self.indexes_inner(id).await
    }
    async fn relation_statistics(&self, id: &RelationId) -> Result<RelationStats, DbError> {
        self.statistics(id).await
    }
    async fn fetch_rows(&self, id: &RelationId, o: RowOptions) -> Result<RowPage, DbError> {
        if o.limit == 0
            || o.limit > MAX_PAGE
            || o.limit as usize > self.max_page
            || o.page > 100
            || o.cursor.is_some()
            || o.spatial_geojson
        {
            return Err(DbError::new(DbErrorKind::Unsupported));
        }
        let offset = u64::from(o.limit) * u64::from(o.page);
        if offset > MAX_OFFSET {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let details = self.details(id).await?;
        let columns = details.items;
        if details.truncated && (o.filter.is_some() || !o.selected_columns.is_empty()) {
            return Err(DbError::new(DbErrorKind::Unsupported));
        }
        let selected = if o.selected_columns.is_empty() && !o.column_selection_explicit {
            columns
                .iter()
                .take(MAX_VISIBLE)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            o.selected_columns
                .iter()
                .map(|n| {
                    columns
                        .iter()
                        .find(|c| c.column.name == *n)
                        .cloned()
                        .ok_or_else(|| DbError::new(DbErrorKind::InvalidIdentifier))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        if selected.len() > MAX_VISIBLE {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let mut projections = Vec::new();
        for (i, c) in selected.iter().enumerate() {
            projections.push(format!("{} AS c{i}", quote(&c.column.name)?))
        }
        if projections.is_empty() {
            projections.push("1 AS __cuadb_empty".into());
        }
        let order = if let Some(s) = &o.sort {
            if !columns.iter().any(|c| c.column.name == s.column) {
                return Err(DbError::new(DbErrorKind::InvalidIdentifier));
            }
            format!(
                " ORDER BY {} {}",
                quote(&s.column)?,
                if s.direction == SortDirection::Asc {
                    "ASC"
                } else {
                    "DESC"
                }
            )
        } else {
            String::new()
        };
        let mut sql = format!("SELECT {} FROM {}", projections.join(","), quote(&id.name)?);
        if let Some(filter) = &o.filter {
            if filter.is_empty() || filter.len() > 128 {
                return Err(DbError::new(DbErrorKind::InvalidIdentifier));
            }
            let expr = columns
                .iter()
                .map(|c| quote(&c.column.name).map(|q| format!("CAST({q} AS TEXT)")))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ' ', ");
            sql.push_str(&format!(" WHERE concat({expr}) LIKE ? ESCAPE '\\'"))
        }
        sql.push_str(&order);
        sql.push_str(" LIMIT ? OFFSET ?");
        let mut q = sqlx::query(&sql);
        if let Some(f) = &o.filter {
            q = q.bind(format!("%{}%", f.replace('%', "\\%").replace('_', "\\_")))
        }
        let rows = self
            .run(
                q.bind(i64::from(o.limit) + 1)
                    .bind(offset as i64)
                    .fetch_all(&self.pool),
            )
            .await?;
        let more = rows.len() > o.limit as usize;
        let mut values = Vec::new();
        let mut previews = Vec::new();
        for r in rows.iter().take(o.limit as usize) {
            let mut cells = Vec::new();
            let mut ps = Vec::new();
            for (i, c) in selected.iter().enumerate() {
                let value = r
                    .try_get_raw(format!("c{i}").as_str())
                    .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                let (v, p) = if value.is_null() {
                    cell(None, &c.column.native_type)
                } else {
                    let type_name = value.type_info().name().to_ascii_uppercase();
                    match type_name.as_str() {
                        "BLOB" => {
                            let bytes: Vec<u8> = r
                                .try_get(format!("c{i}").as_str())
                                .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                            let preview = format!("{} bytes", bytes.len());
                            (
                                Cell::Bytes(bytes),
                                RowPreview {
                                    text: preview,
                                    truncated: false,
                                    type_name: c.column.native_type.clone(),
                                    detail: None,
                                },
                            )
                        }
                        "INTEGER" | "BOOLEAN" => {
                            let n: i64 = r
                                .try_get(format!("c{i}").as_str())
                                .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                            if type_name == "BOOLEAN" {
                                (
                                    Cell::Boolean(n != 0),
                                    RowPreview {
                                        text: n.to_string(),
                                        truncated: false,
                                        type_name: c.column.native_type.clone(),
                                        detail: None,
                                    },
                                )
                            } else {
                                (
                                    Cell::Integer(n),
                                    RowPreview {
                                        text: n.to_string(),
                                        truncated: false,
                                        type_name: c.column.native_type.clone(),
                                        detail: None,
                                    },
                                )
                            }
                        }
                        "REAL" => {
                            let n: f64 = r
                                .try_get(format!("c{i}").as_str())
                                .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                            let text = n.to_string();
                            (
                                Cell::Decimal(text.clone()),
                                RowPreview {
                                    text,
                                    truncated: false,
                                    type_name: c.column.native_type.clone(),
                                    detail: None,
                                },
                            )
                        }
                        _ => {
                            let raw: String = r
                                .try_get(format!("c{i}").as_str())
                                .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                            cell(Some(raw), &c.column.native_type)
                        }
                    }
                };
                cells.push(v);
                ps.push(p)
            }
            values.push(cells);
            previews.push(ps)
        }
        Ok(RowPage {
            available_columns: columns.iter().map(|column| column.column.clone()).collect(),
            columns: selected.into_iter().map(|x| x.column).collect(),
            rows: values,
            previews,
            has_more: more,
            has_previous: o.page > 0,
            next_cursor: None,
            previous_cursor: None,
            stable_order: false,
            keyset_enabled: false,
            page: o.page,
            offset,
            approximate_total: Availability::Unavailable,
            truncated_columns: o.selected_columns.is_empty() && columns.len() > MAX_VISIBLE,
        })
    }
}
