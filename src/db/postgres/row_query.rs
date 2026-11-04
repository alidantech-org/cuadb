//! Catalog-validated SQL generation for PostgreSQL row reads.
use crate::db::types::{ColumnDetail, DbError, DbErrorKind, RowOptions, SortDirection};
pub const MAX_VISIBLE_COLUMNS: usize = 24;
pub const MAX_PAGE_SIZE: u32 = 100;
pub const MAX_OFFSET: u64 = 10_000;
pub const PREVIEW_CHARACTERS: usize = 257;

fn invalid() -> DbError {
    DbError::new(DbErrorKind::InvalidIdentifier)
}
pub fn quote(name: &str) -> Result<String, DbError> {
    if name.is_empty() || name.len() > 63 || name.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
}

#[derive(Debug, Clone)]
pub struct UniqueKey {
    pub column: String,
    pub integer: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekDirection {
    Forward,
    Backward,
}
#[derive(Debug, Clone, Copy)]
pub struct Seek {
    pub value: i64,
    pub direction: SeekDirection,
}
#[derive(Debug)]
pub struct BuiltQuery {
    pub sql: String,
    pub selected: Vec<ColumnDetail>,
    pub stable: bool,
    pub keyset: bool,
    pub reverse_results: bool,
    pub truncated_columns: bool,
    pub page: u32,
    pub offset: i64,
    pub seek_value: Option<i64>,
    pub filter_value: Option<String>,
    pub spatial_geojson: bool,
}
pub fn sort_supported(native: &str) -> bool {
    let x = native.to_ascii_lowercase();
    !x.starts_with("json")
        && !x.contains("geometry")
        && !x.contains("geography")
        && !x.starts_with("xml")
        && !x.starts_with("record")
        && !x.starts_with("point")
        && !x.ends_with("[]")
        && !x.starts_with("tsvector")
}

pub fn build(
    schema: &str,
    relation: &str,
    columns: &[ColumnDetail],
    key: Option<&UniqueKey>,
    options: &RowOptions,
    max_page_size: usize,
    seek: Option<Seek>,
    postgis_schema: Option<&str>,
) -> Result<BuiltQuery, DbError> {
    if options.limit == 0
        || options.limit > MAX_PAGE_SIZE
        || options.limit as usize > max_page_size
        || (options.page > 100 && seek.is_none())
    {
        return Err(invalid());
    }
    if columns.is_empty() {
        return Err(DbError::new(DbErrorKind::Unsupported));
    }
    let truncated_columns = !options.column_selection_explicit
        && options.selected_columns.is_empty()
        && columns.len() > MAX_VISIBLE_COLUMNS;
    let mut selected: Vec<ColumnDetail> = vec![];
    if options.selected_columns.is_empty() && !options.column_selection_explicit {
        selected.extend(columns.iter().take(MAX_VISIBLE_COLUMNS).cloned());
    } else {
        if options.selected_columns.len() > MAX_VISIBLE_COLUMNS {
            return Err(invalid());
        }
        for name in &options.selected_columns {
            if selected.iter().any(|c| c.column.name == *name) {
                return Err(invalid());
            }
            selected.push(
                columns
                    .iter()
                    .find(|c| c.column.name == *name)
                    .ok_or_else(invalid)?
                    .clone(),
            );
        }
    }
    let sorted = if let Some(sort) = &options.sort {
        let col = columns
            .iter()
            .find(|c| c.column.name == sort.column)
            .ok_or_else(invalid)?;
        if !sort_supported(&col.column.native_type) {
            return Err(DbError::new(DbErrorKind::Unsupported));
        }
        Some((&sort.column, sort.direction))
    } else {
        None
    };
    let stable = key.is_some();
    let keyset = key.is_some_and(|k| k.integer)
        && sorted.is_none_or(|(name, _)| key.is_some_and(|k| name == &k.column));
    if seek.is_some() && !keyset {
        return Err(invalid());
    }

    let mut projections = Vec::new();
    for (n, col) in selected.iter().enumerate() {
        let name = format!("t.{}", quote(&col.column.name)?);
        let native = col.column.native_type.to_ascii_lowercase();
        let display = if (native.starts_with("geometry") || native.starts_with("geography"))
            && let Some(schema) = postgis_schema
        {
            format!("{}.ST_AsText({name})", quote(schema)?)
        } else {
            format!("{name}::text")
        };
        projections.push(format!(
            "CASE WHEN {name} IS NULL THEN NULL ELSE left({display},{PREVIEW_CHARACTERS}) END AS \"c{n}\""
        ));
        if options.spatial_geojson
            && (native.starts_with("geometry") || native.starts_with("geography"))
            && let Some(schema) = postgis_schema
        {
            projections.push(format!(
                "CASE WHEN {name} IS NULL THEN NULL ELSE left({}.ST_AsGeoJSON({name}),1025) END AS \"g{n}\"",
                quote(schema)?
            ));
        }
    }
    let key_projection = match key {
        Some(k) if k.integer => format!("t.{}::bigint AS \"__cuadb_key\"", quote(&k.column)?),
        _ => "NULL::bigint AS \"__cuadb_key\"".to_string(),
    };
    projections.push(key_projection);
    let source = format!("{}.{} AS t", quote(schema)?, quote(relation)?);

    let direction = sorted.map_or(SortDirection::Asc, |(_, d)| d);
    let reverse_results = seek.is_some_and(|s| s.direction == SeekDirection::Backward);
    let actual_direction = if reverse_results {
        match direction {
            SortDirection::Asc => SortDirection::Desc,
            SortDirection::Desc => SortDirection::Asc,
        }
    } else {
        direction
    };
    let dir = match actual_direction {
        SortDirection::Asc => "ASC",
        SortDirection::Desc => "DESC",
    };
    let mut order = Vec::new();
    if let Some((sort, _)) = sorted {
        order.push(format!("t.{} {dir} NULLS LAST", quote(sort)?));
    }
    if let Some(k) = key {
        if sorted.is_none_or(|(name, _)| name != &k.column) {
            let key_dir = if sorted.is_some() { "ASC" } else { dir };
            order.push(format!("t.{} {key_dir}", quote(&k.column)?));
        } else if order.is_empty() {
            order.push(format!("t.{} {dir}", quote(&k.column)?));
        }
    }
    if order.is_empty()
        && sorted.is_none()
        && let Some(first) = columns
            .iter()
            .find(|c| sort_supported(&c.column.native_type))
    {
        order.push(format!("t.{} ASC NULLS LAST", quote(&first.column.name)?));
    }

    let seek_predicate = if let Some(seek) = seek {
        let key = key.ok_or_else(invalid)?;
        let op = match (direction, seek.direction) {
            (SortDirection::Asc, SeekDirection::Forward)
            | (SortDirection::Desc, SeekDirection::Backward) => ">",
            _ => "<",
        };
        format!("t.{} {op} $1::bigint", quote(&key.column)?)
    } else {
        "$1::bigint IS NULL".to_string()
    };
    let search_values = columns
        .iter()
        .map(|col| Ok(format!("t.{}::text", quote(&col.column.name)?)))
        .collect::<Result<Vec<_>, DbError>>()?;
    let predicate = format!(
        "({seek_predicate}) AND ($4::text IS NULL OR concat_ws(' ', {}) ILIKE ('%' || $4::text || '%'))",
        search_values.join(", ")
    );
    let offset = if seek.is_some() {
        0
    } else {
        u64::from(options.page) * u64::from(options.limit)
    };
    if offset > MAX_OFFSET {
        return Err(invalid());
    }
    let order_clause = if order.is_empty() {
        String::new()
    } else {
        format!(" ORDER BY {}", order.join(", "))
    };
    let sql = format!(
        "SELECT {} FROM {source} WHERE {predicate}{order_clause} LIMIT $2::bigint OFFSET $3::bigint",
        projections.join(", ")
    );
    Ok(BuiltQuery {
        sql,
        selected,
        stable,
        keyset,
        reverse_results,
        truncated_columns,
        page: options.page,
        offset: offset as i64,
        seek_value: seek.map(|s| s.value),
        filter_value: options.filter.clone(),
        spatial_geojson: options.spatial_geojson,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::{Availability, Column, SortSpec};
    fn col(name: &str, kind: &str) -> ColumnDetail {
        ColumnDetail {
            column: Column {
                name: name.into(),
                native_type: kind.into(),
                nullable: false,
                primary_key: false,
                default_expression: None,
            },
            position: 1,
            unique_key: false,
            identity: None,
            generated: None,
            domain: None,
            enum_values: Availability::Unavailable,
        }
    }
    #[test]
    fn quotes_keywords_unicode_and_embedded_quotes() {
        assert_eq!(quote("select").unwrap(), "\"select\"");
        assert_eq!(quote("Ünicode \"x\"").unwrap(), "\"Ünicode \"\"x\"\"\"");
        assert!(quote("hello\0world").is_err());
        assert!(quote("").is_err());
        assert!(quote(&"x".repeat(64)).is_err());
    }
    #[test]
    fn accepts_only_known_identifiers_and_bound_offsets() {
        let columns = [col("select", "text"), col("ID", "integer")];
        let mut options = RowOptions {
            selected_columns: vec!["select".into()],
            sort: Some(SortSpec {
                column: "ID".into(),
                direction: SortDirection::Desc,
            }),
            ..RowOptions::default()
        };
        let key = UniqueKey {
            column: "ID".into(),
            integer: true,
        };
        let plan = build(
            "strange \"schema\"",
            "orders",
            &columns,
            Some(&key),
            &options,
            100,
            None,
            None,
        )
        .unwrap();
        assert!(plan.sql.contains("LIMIT $2::bigint OFFSET $3::bigint"));
        assert!(plan.sql.contains("t.\"ID\" DESC NULLS LAST"));
        options.sort.as_mut().unwrap().column = "ID; DROP".into();
        assert!(
            build(
                "public",
                "orders",
                &columns,
                Some(&key),
                &options,
                100,
                None,
                None,
            )
            .is_err()
        );
    }
    #[test]
    fn row_filter_is_bound_and_scoped_to_selected_columns() {
        let columns = [col("name", "text"), col("secret", "text")];
        let options = RowOptions {
            selected_columns: vec!["name".into()],
            filter: Some("%' OR true --".into()),
            ..RowOptions::default()
        };
        let plan = build("public", "t", &columns, None, &options, 100, None, None).unwrap();
        assert!(plan.sql.contains("$4::text IS NULL"));
        assert!(plan.sql.contains("t.\"name\"::text"));
        assert!(plan.sql.contains("t.\"secret\"::text"));
        assert!(!plan.sql.contains("OR true"));
        assert_eq!(plan.filter_value.as_deref(), Some("%' OR true --"));
    }
    #[test]
    fn postgis_values_use_quoted_bounded_text_conversion_only_when_available() {
        let columns = [col("shape", "geometry(Point,4326)")];
        let options = RowOptions::default();
        let spatial = build(
            "public",
            "places",
            &columns,
            None,
            &options,
            100,
            None,
            Some("geo schema"),
        )
        .unwrap();
        assert!(spatial.sql.contains("\"geo schema\".ST_AsText"));
        assert!(spatial.sql.contains("left(\"geo schema\".ST_AsText"));
        let plain = build(
            "public", "places", &columns, None, &options, 100, None, None,
        )
        .unwrap();
        assert!(plain.sql.contains("t.\"shape\"::text"));
        let mut geojson_options = options;
        geojson_options.spatial_geojson = true;
        let geojson = build(
            "public",
            "places",
            &columns,
            None,
            &geojson_options,
            100,
            None,
            Some("geo schema"),
        )
        .unwrap();
        assert!(
            geojson
                .sql
                .contains("left(\"geo schema\".ST_AsGeoJSON(t.\"shape\"),1025)")
        );
    }
    #[test]
    fn bounds_page_size_offset_and_selected_columns() {
        let columns = [col("id", "integer")];
        let mut options = RowOptions {
            limit: 101,
            ..RowOptions::default()
        };
        assert!(build("public", "t", &columns, None, &options, 100, None, None).is_err());
        options.limit = 25;
        options.page = 101;
        assert!(build("public", "t", &columns, None, &options, 100, None, None).is_err());
        options.page = 0;
        options.selected_columns = vec!["id".into(), "id".into()];
        assert!(build("public", "t", &columns, None, &options, 100, None, None).is_err());
    }
    #[test]
    fn cursor_direction_reverses_key_order() {
        let columns = [col("id", "bigint")];
        let options = RowOptions::default();
        let key = UniqueKey {
            column: "id".into(),
            integer: true,
        };
        let next = build(
            "public",
            "t",
            &columns,
            Some(&key),
            &options,
            100,
            Some(Seek {
                value: 7,
                direction: SeekDirection::Forward,
            }),
            None,
        )
        .unwrap();
        assert!(next.sql.contains("\"id\" > $1::bigint"));
        let previous = build(
            "public",
            "t",
            &columns,
            Some(&key),
            &options,
            100,
            Some(Seek {
                value: 7,
                direction: SeekDirection::Backward,
            }),
            None,
        )
        .unwrap();
        assert!(previous.sql.contains("\"id\" < $1::bigint"));
        assert!(previous.reverse_results);
    }
}
