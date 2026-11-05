//! Permission-scoped PostgreSQL row page execution.
use super::{
    catalog::PgCatalog,
    manager::classify,
    row_cursor::{Anchor, Scope},
    row_query::{self, Seek, SeekDirection, UniqueKey},
};
use crate::db::types::{
    Availability, Cell, DbError, DbErrorKind, PageRequest, RelationId, RowOptions, RowPage,
    RowPreview,
};
use sqlx::Row;
use tokio::time::timeout;
const MAX_PREVIEW_BYTES: usize = 256;
impl PgCatalog {
    async fn unique_key(&self, id: &RelationId) -> Result<Option<UniqueKey>, DbError> {
        let oid = self.relation_oid(id).await?;
        let result = timeout(
            self.deadline,
            sqlx::query(
                r#"SELECT a.attname::text AS name,t.typname::text AS type_name
              FROM pg_catalog.pg_index i
              JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS k(attnum,ord) ON k.ord=1
              JOIN pg_catalog.pg_attribute a
                 ON a.attrelid=i.indrelid AND a.attnum=k.attnum
              JOIN pg_catalog.pg_type t ON t.oid=a.atttypid
              WHERE i.indrelid::bigint=$1
                AND i.indisunique AND i.indisvalid AND i.indisready
                AND i.indnkeyatts=1 AND i.indpred IS NULL
                AND i.indexprs IS NULL AND a.attnotnull
              ORDER BY i.indisprimary DESC,i.indexrelid LIMIT 1"#,
            )
            .bind(oid)
            .fetch_optional(&self.pool),
        )
        .await
        .map_err(|_| DbError::new(DbErrorKind::TimedOut))?
        .map_err(classify)?;
        result
            .map(|r| {
                let column: String = r
                    .try_get("name")
                    .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                let kind: String = r
                    .try_get("type_name")
                    .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                Ok(UniqueKey {
                    column,
                    integer: matches!(kind.as_str(), "int2" | "int4" | "int8"),
                })
            })
            .transpose()
    }
    pub async fn fetch_rows(
        &self,
        id: &RelationId,
        mut options: RowOptions,
    ) -> Result<RowPage, DbError> {
        if options.limit == 0
            || options.limit > super::row_query::MAX_PAGE_SIZE
            || options.limit as usize > self.max_page_size
            || options.page > 100
        {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let scope = Scope::new(id, &options);
        let anchor = if let Some(token) = options.cursor.as_deref() {
            if options.page != 0 {
                return Err(DbError::new(DbErrorKind::InvalidIdentifier));
            }
            Some(
                self.cursor_store
                    .lock()
                    .map_err(|_| DbError::new(DbErrorKind::Internal))?
                    .resolve(token, &scope)?,
            )
        } else {
            None
        };
        if let Some(anchor) = anchor {
            options.page = anchor.page;
        }
        let details = self.describe_columns(id).await?;
        if details.truncated && options.filter.is_some() {
            return Err(DbError::new(DbErrorKind::Unsupported));
        }
        if details.truncated && options.selected_columns.is_empty() {
            return Err(DbError::new(DbErrorKind::Unsupported));
        }
        let key = self.unique_key(id).await?;
        let seek = anchor.map(|a| Seek {
            value: a.value,
            direction: a.direction,
        });
        let has_spatial = details.items.iter().any(|column| {
            let native = column.column.native_type.to_ascii_lowercase();
            row_query::is_spatial_type(&native)
        });
        let postgis_schema = if has_spatial {
            self.postgis_schema().await?
        } else {
            None
        };
        if options.spatial_geojson && (!has_spatial || postgis_schema.is_none()) {
            return Err(DbError::new(DbErrorKind::Unsupported));
        }
        let plan = row_query::build(
            &id.namespace,
            &id.name,
            &details.items,
            key.as_ref(),
            &options,
            self.max_page_size,
            seek,
            postgis_schema.as_deref(),
        )?;
        let began = std::time::Instant::now();
        let records = timeout(
            self.deadline,
            sqlx::query(&plan.sql)
                .bind(plan.seek_value)
                .bind(i64::from(options.limit) + 1)
                .bind(plan.offset)
                .bind(plan.filter_value.as_deref())
                .fetch_all(&self.pool),
        )
        .await
        .map_err(|_| DbError::new(DbErrorKind::TimedOut))?
        .map_err(classify)?;
        if began.elapsed().as_millis() >= 500 {
            eprintln!(
                "level=warn event=slow_row_page elapsed_ms={}",
                began.elapsed().as_millis()
            );
        }
        let more = records.len() > options.limit as usize;
        let mut items = Vec::new();
        for record in records.into_iter().take(options.limit as usize) {
            let key: Option<i64> = record
                .try_get("__cuadb_key")
                .map_err(|_| DbError::new(DbErrorKind::Internal))?;
            let mut cells = Vec::new();
            let mut previews = Vec::new();
            for (i, col) in plan.selected.iter().enumerate() {
                let raw: Option<String> = record
                    .try_get(format!("c{i}").as_str())
                    .map_err(|_| DbError::new(DbErrorKind::Internal))?;
                let (cell, mut preview) = normalize(raw.as_deref(), &col.column.native_type);
                if plan.spatial_geojson
                    && row_query::is_spatial_type(&col.column.native_type)
                    && let Some(raw_json) = record
                        .try_get::<Option<String>, _>(format!("g{i}").as_str())
                        .map_err(|_| DbError::new(DbErrorKind::Internal))?
                {
                    preview.detail = Some(if raw_json.len() > 1024 {
                        "GeoJSON detail exceeds the 1024 character display limit.".into()
                    } else if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw_json) {
                        serde_json::to_string_pretty(&value).unwrap_or(raw_json)
                    } else {
                        "GeoJSON detail is unavailable for this value.".into()
                    });
                }
                cells.push(cell);
                previews.push(preview);
            }
            items.push((key, cells, previews));
        }
        if plan.reverse_results {
            items.reverse();
        }
        let has_more = if plan.reverse_results {
            !items.is_empty()
        } else {
            more
        };
        let has_previous = if plan.reverse_results {
            more
        } else {
            options.page > 0
        };
        let mut next_cursor = None;
        let mut previous_cursor = None;
        if plan.keyset && !items.is_empty() {
            let mut registry = self
                .cursor_store
                .lock()
                .map_err(|_| DbError::new(DbErrorKind::Internal))?;
            if has_more
                && options.page < u32::MAX - 1
                && let Some(key) = items.last().and_then(|(key, _, _)| *key)
            {
                next_cursor = Some(registry.issue(
                    scope.clone(),
                    Anchor {
                        value: key,
                        direction: SeekDirection::Forward,
                        page: options.page + 1,
                    },
                )?);
            }
            if has_previous && let Some(key) = items.first().and_then(|(key, _, _)| *key) {
                previous_cursor = Some(registry.issue(
                    scope,
                    Anchor {
                        value: key,
                        direction: SeekDirection::Backward,
                        page: options.page.saturating_sub(1),
                    },
                )?);
            }
        }
        let stats = self.relation_statistics(id).await.ok();
        let estimate = stats.map_or(Availability::Unavailable, |s| s.estimated_rows);
        let mut rows = Vec::new();
        let mut previews = Vec::new();
        for (_, cells, view) in items {
            rows.push(cells);
            previews.push(view);
        }
        let available_columns = details.items.iter().map(|c| c.column.clone()).collect();
        Ok(RowPage {
            available_columns,
            columns: plan.selected.into_iter().map(|c| c.column).collect(),
            rows,
            previews,
            has_more,
            has_previous,
            next_cursor,
            previous_cursor,
            stable_order: plan.stable,
            keyset_enabled: plan.keyset,
            page: options.page,
            offset: u64::from(options.page) * u64::from(options.limit),
            approximate_total: estimate,
            truncated_columns: plan.truncated_columns,
        })
    }
    pub async fn offset_page(
        &self,
        id: &RelationId,
        request: PageRequest,
    ) -> Result<RowPage, DbError> {
        if request.limit == 0 || !request.offset.is_multiple_of(u64::from(request.limit)) {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        let page = request.offset / u64::from(request.limit);
        let page = u32::try_from(page).map_err(|_| DbError::new(DbErrorKind::InvalidIdentifier))?;
        self.fetch_rows(
            id,
            RowOptions {
                limit: request.limit,
                page,
                ..RowOptions::default()
            },
        )
        .await
    }
}

/// SQL trims to at most 257 characters; this final UTF-8 byte cap
/// independently prevents unusually wide characters from growing responses.
pub fn normalize(raw: Option<&str>, native: &str) -> (Cell, RowPreview) {
    let kind = native.to_ascii_lowercase();
    let Some(value) = raw else {
        return (
            Cell::Null,
            RowPreview {
                text: "NULL".into(),
                truncated: false,
                type_name: native.into(),
                detail: None,
            },
        );
    };
    let mut end = 0;
    for ch in value.chars() {
        if end + ch.len_utf8() > MAX_PREVIEW_BYTES {
            break;
        }
        end += ch.len_utf8();
    }
    let bounded = &value[..end];
    let truncated = end < value.len() || value.chars().count() > 256;
    let preview = RowPreview {
        text: bounded.into(),
        truncated,
        type_name: native.into(),
        detail: None,
    };
    if truncated {
        return (
            Cell::Truncated {
                preview: bounded.into(),
                type_name: native.into(),
            },
            preview,
        );
    }
    let cell = match kind.as_str() {
        "boolean" | "bool" => match bounded {
            "true" => Cell::Boolean(true),
            "false" => Cell::Boolean(false),
            _ => Cell::Unsupported {
                type_name: native.into(),
            },
        },
        "smallint" | "integer" | "bigint" => bounded
            .parse::<i64>()
            .map(Cell::Integer)
            .unwrap_or_else(|_| Cell::Unsupported {
                type_name: native.into(),
            }),
        "real" | "double precision" => Cell::Decimal(bounded.into()),
        "bytea" => decode_bytea(bounded).map_or_else(
            || Cell::Unsupported {
                type_name: native.into(),
            },
            Cell::Bytes,
        ),
        _ if kind.starts_with("numeric") || kind.starts_with("decimal") => {
            Cell::Decimal(bounded.into())
        }
        _ => Cell::Text(bounded.into()),
    };
    (cell, preview)
}
fn decode_bytea(input: &str) -> Option<Vec<u8>> {
    let hex = input.strip_prefix("\\x")?;
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        bytes.push(u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()?);
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn null_empty_and_numeric_are_distinct() {
        assert_eq!(normalize(None, "text").0, Cell::Null);
        assert_eq!(normalize(Some(""), "text").0, Cell::Text("".into()));
        assert_eq!(normalize(Some("42"), "integer").0, Cell::Integer(42));
        assert_eq!(normalize(Some("true"), "boolean").0, Cell::Boolean(true));
        assert_eq!(
            normalize(Some("19.250"), "numeric(8,3)").0,
            Cell::Decimal("19.250".into())
        );
    }
    #[test]
    fn utf8_and_binary_have_byte_bounds() {
        let (cell, preview) = normalize(Some(&"ø".repeat(300)), "text");
        assert!(matches!(cell, Cell::Truncated { .. }));
        assert!(preview.text.len() <= MAX_PREVIEW_BYTES);
        assert!(preview.truncated);
        let (binary, _) = normalize(Some("\\x00ff"), "bytea");
        assert_eq!(binary, Cell::Bytes(vec![0, 255]));
    }
    #[test]
    fn timestamps_uuid_and_extension_values_are_safe_text() {
        for kind in ["timestamp with time zone", "uuid", "geometry"] {
            let (cell, preview) = normalize(Some("<script>"), kind);
            assert!(matches!(cell, Cell::Text(_)));
            assert_eq!(preview.text, "<script>");
        }
        let (cell, _) = normalize(Some("not-int"), "integer");
        assert!(matches!(cell, Cell::Unsupported { .. }));
    }
}
