//! SQLite metadata and bounded read-only row adapter.
use super::classify;
use crate::db::{adapter::DatabaseAdapter, types::*};
use sqlx::{Row, Sqlite, SqlitePool, sqlite::SqliteRow};
use std::time::Duration;
use tokio::time::timeout;
const MAX_RELATIONS: i64 = 500;
const MAX_COLUMNS: i64 = 512;
const MAX_DETAILS: i64 = 300;
const MAX_VISIBLE: usize = 24;
const MAX_PAGE: u32 = 100;
const MAX_OFFSET: u64 = 10_000;
/// Bytes/characters fetched per text or blob cell (one over the 256 display limit).
const PREVIEW: usize = 257;
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
    /// Relation kind and whether it has a rowid; errors if it is not a visible user relation.
    async fn meta(&self, id: &RelationId) -> Result<(RelationKind, bool), DbError> {
        if id.namespace != "main" {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let row = self
            .run(
                sqlx::query(
                    "SELECT type, wr FROM pragma_table_list WHERE schema='main' AND name=? \
                     AND type IN ('table','view','virtual') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' LIMIT 1",
                )
                .bind(&id.name)
                .fetch_optional(&self.pool),
            )
            .await?
            .ok_or_else(|| DbError::new(DbErrorKind::InvalidIdentifier))?;
        let kind: String = get(&row, "type")?;
        let without_rowid: i64 = get(&row, "wr")?;
        Ok(match kind.as_str() {
            "view" => (RelationKind::View, false),
            "virtual" => (RelationKind::VirtualTable, false),
            _ => (RelationKind::Table, without_rowid == 0),
        })
    }
    async fn visible(&self, id: &RelationId) -> Result<(), DbError> {
        self.meta(id).await.map(|_| ())
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
            // 1 = hidden column of a virtual table; 2/3 = VIRTUAL/STORED generated column.
            if hidden == 1 {
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
                generated: match hidden {
                    2 => Some("VIRTUAL".into()),
                    3 => Some("STORED".into()),
                    _ => None,
                },
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
/// Declared type, plus the actual storage class when SQLite's dynamic typing
/// stored something the declared affinity would not predict.
fn storage_label(declared: &str, class: &str) -> String {
    let upper = declared.to_ascii_uppercase();
    let expected: &[&str] = if declared == "dynamic" {
        &[]
    } else if upper.contains("INT") {
        &["integer"]
    } else if upper.contains("CHAR") || upper.contains("CLOB") || upper.contains("TEXT") {
        &["text"]
    } else if upper.contains("BLOB") {
        &["blob"]
    } else if upper.contains("REAL") || upper.contains("FLOA") || upper.contains("DOUB") {
        &["real", "integer"]
    } else {
        &["integer", "real"]
    };
    if class == "null" || expected.contains(&class) {
        declared.to_string()
    } else {
        format!("{declared} · {class}")
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
        let (kind, _) = self.meta(id).await?;
        Ok(RelationStats {
            relation_kind: Availability::Available(kind),
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
        let rows = self
            .run(
                sqlx::query(
                    "SELECT name, type FROM pragma_table_list WHERE schema='main' \
                     AND type IN ('table','view','virtual') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
                     ORDER BY name LIMIT ?",
                )
                .bind(MAX_RELATIONS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
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
                        kind: match ty.as_str() {
                            "view" => RelationKind::View,
                            "virtual" => RelationKind::VirtualTable,
                            _ => RelationKind::Table,
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
        let (_, has_rowid) = self.meta(id).await?;
        let mut projections = Vec::new();
        for (i, c) in selected.iter().enumerate() {
            let q = quote(&c.column.name)?;
            // Bound text/blob transfer in SQLite itself; the length column reveals truncation.
            projections.push(format!(
                "CASE WHEN typeof({q}) IN ('text','blob') THEN substr({q},1,{PREVIEW}) ELSE {q} END AS c{i}, \
                 CASE WHEN typeof({q}) IN ('text','blob') THEN length({q}) END AS l{i}, typeof({q}) AS t{i}"
            ));
        }
        if projections.is_empty() {
            projections.push("1 AS __cuadb_empty".into());
        }
        // Deterministic ties: primary key columns, else rowid when the table has one.
        let rowid_usable = has_rowid
            && !columns.iter().any(|c| {
                matches!(
                    c.column.name.to_ascii_lowercase().as_str(),
                    "rowid" | "_rowid_" | "oid"
                )
            });
        let mut tie_break = columns
            .iter()
            .filter(|c| c.column.primary_key)
            .map(|c| quote(&c.column.name))
            .collect::<Result<Vec<_>, _>>()?;
        if tie_break.is_empty() && rowid_usable {
            tie_break.push("rowid".into());
        }
        let stable_order = !tie_break.is_empty();
        let order = if let Some(s) = &o.sort {
            let column = columns
                .iter()
                .find(|c| c.column.name == s.column)
                .ok_or_else(|| DbError::new(DbErrorKind::InvalidIdentifier))?;
            if !self.sort_supported(&column.column.native_type) {
                return Err(DbError::new(DbErrorKind::Unsupported));
            }
            let q = quote(&s.column)?;
            let mut order = format!(
                " ORDER BY ({q} IS NULL), {q} {}",
                if s.direction == SortDirection::Asc {
                    "ASC"
                } else {
                    "DESC"
                }
            );
            for key in tie_break.iter().filter(|key| **key != q) {
                order.push_str(&format!(", {key}"));
            }
            order
        } else if stable_order {
            format!(" ORDER BY {}", tie_break.join(", "))
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
            q = q.bind(format!(
                "%{}%",
                f.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            ))
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
                let class: String = r
                    .try_get(format!("t{i}").as_str())
                    .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                let total: Option<i64> = r
                    .try_get(format!("l{i}").as_str())
                    .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                let type_name = storage_label(&c.column.native_type, &class);
                let (v, mut p) = match class.as_str() {
                    "null" => cell(None, &type_name),
                    "blob" => {
                        let bytes: Vec<u8> = r
                            .try_get(format!("c{i}").as_str())
                            .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                        let total = total.unwrap_or(bytes.len() as i64);
                        let truncated = total as usize > bytes.len();
                        let text = if truncated {
                            format!("{total} bytes (first {} shown)", bytes.len())
                        } else {
                            format!("{total} bytes")
                        };
                        let cell = if truncated {
                            Cell::Truncated {
                                preview: format!(
                                    "\\x{}",
                                    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
                                ),
                                type_name: type_name.clone(),
                            }
                        } else {
                            Cell::Bytes(bytes)
                        };
                        (
                            cell,
                            RowPreview {
                                text,
                                truncated,
                                type_name: type_name.clone(),
                                detail: None,
                            },
                        )
                    }
                    "integer" => {
                        let n: i64 = r
                            .try_get(format!("c{i}").as_str())
                            .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                        let boolean = c.column.native_type.eq_ignore_ascii_case("boolean")
                            || c.column.native_type.eq_ignore_ascii_case("bool");
                        (
                            if boolean && (n == 0 || n == 1) {
                                Cell::Boolean(n == 1)
                            } else {
                                Cell::Integer(n)
                            },
                            RowPreview {
                                text: n.to_string(),
                                truncated: false,
                                type_name: type_name.clone(),
                                detail: None,
                            },
                        )
                    }
                    "real" => {
                        let n: f64 = r
                            .try_get(format!("c{i}").as_str())
                            .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                        let text = n.to_string();
                        (
                            Cell::Decimal(text.clone()),
                            RowPreview {
                                text,
                                truncated: false,
                                type_name: type_name.clone(),
                                detail: None,
                            },
                        )
                    }
                    _ => {
                        let raw: String = r
                            .try_get(format!("c{i}").as_str())
                            .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                        let (mut cell_value, preview) = cell(Some(raw), &type_name);
                        // `cell` guesses numbers from text; a text-class value stays text.
                        if let Cell::Integer(_) | Cell::Decimal(_) | Cell::Boolean(_) = cell_value {
                            cell_value = Cell::Text(preview.text.clone());
                        }
                        (cell_value, preview)
                    }
                };
                p.type_name = type_name;
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
            stable_order,
            keyset_enabled: false,
            page: o.page,
            offset,
            approximate_total: Availability::Unavailable,
            truncated_columns: o.selected_columns.is_empty() && columns.len() > MAX_VISIBLE,
        })
    }
}
