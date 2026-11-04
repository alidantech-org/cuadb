//! Permission-scoped PostgreSQL catalog queries.
//!
//! All SQL is fixed at compile time. Browser-supplied identifiers are bound
//! as values, never interpolated as SQL. These queries never inspect row data.
use std::{future::Future, time::Duration};

use sqlx::{PgPool, Row, postgres::PgRow};
use tokio::time::timeout;

use crate::db::{
    adapter::DatabaseAdapter,
    types::{
        Availability, Capability, CatalogList, CatalogObject, CatalogObjectKind, Column,
        ColumnDetail, Constraint, ConstraintKind, DbError, DbErrorKind, Engine, Index, Namespace,
        Overview, PageRequest, PartitionLink, Relation, RelationId, RelationKind, RelationStats,
        RowOptions, RowPage, SchemaSize,
    },
};

use super::manager::classify;

/// Explicit caps; the UI displays `truncated` rather than claiming completeness.
const MAX_SCHEMAS: i64 = 300;
const MAX_RELATIONS: i64 = 500;
const MAX_COLUMNS: i64 = 512;
const MAX_DETAILS: i64 = 300;
const MAX_OVERVIEW: i64 = 20;

#[derive(Clone)]
pub struct PgCatalog {
    pub(super) pool: PgPool,
    pub(super) deadline: Duration,
    pub(super) max_page_size: usize,
    pub(super) cursor_store: std::sync::Arc<std::sync::Mutex<super::row_cursor::CursorStore>>,
}

fn internal(_: sqlx::Error) -> DbError {
    DbError::new(DbErrorKind::Internal)
}
fn get<T>(row: &PgRow, field: &str) -> Result<T, DbError>
where
    T: for<'r> sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
{
    row.try_get(field).map_err(internal)
}
fn capped<T>(mut items: Vec<T>, limit: i64) -> CatalogList<T> {
    let truncated = items.len() > limit as usize;
    items.truncate(limit as usize);
    CatalogList { items, truncated }
}
fn supported(kind: &str, partition: bool) -> Result<RelationKind, DbError> {
    match (kind, partition) {
        ("r", true) => Ok(RelationKind::Partition),
        ("r", false) => Ok(RelationKind::Table),
        ("p", _) => Ok(RelationKind::PartitionedTable),
        ("v", _) => Ok(RelationKind::View),
        ("m", _) => Ok(RelationKind::MaterializedView),
        ("f", _) => Ok(RelationKind::ForeignTable),
        _ => Err(DbError::new(DbErrorKind::Unsupported)),
    }
}
fn available_bytes(value: Option<i64>) -> Availability<u64> {
    match value.and_then(|n| u64::try_from(n).ok()) {
        Some(value) => Availability::Available(value),
        None => Availability::Unavailable,
    }
}
impl PgCatalog {
    fn user_schema(namespace: &str) -> Result<(), DbError> {
        // System catalogs are intentionally not browser-selectable.
        if namespace.starts_with("pg_") || namespace == "information_schema" {
            return Err(DbError::new(DbErrorKind::InvalidIdentifier));
        }
        Ok(())
    }

    async fn bounded<T, F>(&self, future: F) -> Result<T, DbError>
    where
        F: Future<Output = Result<T, sqlx::Error>>,
    {
        timeout(self.deadline, future)
            .await
            .map_err(|_| DbError::new(DbErrorKind::TimedOut))?
            .map_err(classify)
    }

    /// Resolve a schema-qualified identifier without ever quoting or
    /// interpolating names into SQL. Missing/invisible objects are indistinct.
    pub(super) async fn relation_oid(&self, id: &RelationId) -> Result<i64, DbError> {
        Self::user_schema(&id.namespace)?;
        let row = self
            .bounded(
                sqlx::query(
                    r#"SELECT c.oid::bigint AS oid
                FROM pg_catalog.pg_class c
                JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = $1 AND c.relname = $2
                  AND c.relkind IN ('r','p','v','m','f')
                  AND pg_catalog.has_schema_privilege(n.oid, 'USAGE')
                  AND pg_catalog.has_table_privilege(c.oid, 'SELECT')"#,
                )
                .bind(&id.namespace)
                .bind(&id.name)
                .fetch_optional(&self.pool),
            )
            .await?;
        match row {
            Some(row) => get(&row, "oid"),
            None => Err(DbError::new(DbErrorKind::InvalidIdentifier)),
        }
    }

    pub async fn list_schemas(&self) -> Result<CatalogList<Namespace>, DbError> {
        let rows = self
            .bounded(
                sqlx::query(
                    r#"SELECT n.nspname::text AS name
                FROM pg_catalog.pg_namespace n
                WHERE left(n.nspname,3) <> 'pg_'
                  AND n.nspname <> 'information_schema'
                  AND pg_catalog.has_schema_privilege(n.oid, 'USAGE')
                ORDER BY n.nspname COLLATE "C"
                LIMIT $1"#,
                )
                .bind(MAX_SCHEMAS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|r| {
                    Ok(Namespace {
                        name: get(r, "name")?,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_SCHEMAS,
        ))
    }

    pub async fn list_relations(&self, namespace: &str) -> Result<CatalogList<Relation>, DbError> {
        Self::user_schema(namespace)?;
        let rows = self
            .bounded(
                sqlx::query(
                    r#"SELECT c.relname::text AS name, c.relkind::text AS kind,
                      c.relispartition AS partition
                FROM pg_catalog.pg_class c
                JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = $1 AND c.relkind IN ('r','p','v','m','f')
                  AND left(n.nspname,3) <> 'pg_'
                  AND n.nspname <> 'information_schema'
                  AND pg_catalog.has_schema_privilege(n.oid, 'USAGE')
                  AND pg_catalog.has_table_privilege(c.oid, 'SELECT')
                ORDER BY c.relname COLLATE "C", c.oid
                LIMIT $2"#,
                )
                .bind(namespace)
                .bind(MAX_RELATIONS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|r| {
                    let kind: String = get(r, "kind")?;
                    let partition = get(r, "partition")?;
                    Ok(Relation {
                        id: RelationId {
                            namespace: namespace.to_string(),
                            name: get(r, "name")?,
                        },
                        kind: supported(&kind, partition)?,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_RELATIONS,
        ))
    }

    pub async fn describe_columns(
        &self,
        id: &RelationId,
    ) -> Result<CatalogList<ColumnDetail>, DbError> {
        let oid = self.relation_oid(id).await?;
        let rows = self
            .bounded(
                sqlx::query(
                    r#"SELECT a.attnum::int2 AS position, a.attname::text AS name,
                      pg_catalog.format_type(a.atttypid, a.atttypmod) AS native_type,
                      NOT a.attnotnull AS nullable,
                      pg_catalog.pg_get_expr(def.adbin, def.adrelid) AS default_expression,
                      EXISTS (SELECT 1 FROM pg_catalog.pg_constraint con
                              WHERE con.conrelid = a.attrelid AND con.contype='p'
                                AND a.attnum = ANY(con.conkey)) AS primary_key,
                      EXISTS (SELECT 1 FROM pg_catalog.pg_constraint con
                              WHERE con.conrelid = a.attrelid AND con.contype='u'
                                AND a.attnum = ANY(con.conkey)) AS unique_key,
                      CASE a.attidentity::text WHEN 'a' THEN 'ALWAYS' WHEN 'd' THEN 'BY DEFAULT' ELSE NULL END AS identity,
                      CASE a.attgenerated::text WHEN 's' THEN 'STORED' WHEN 'v' THEN 'VIRTUAL' ELSE NULL END AS generated,
                      CASE WHEN ty.typtype='d' THEN
                          pg_catalog.format_type(ty.oid, NULL) ELSE NULL END AS domain,
                      CASE WHEN ty.typtype='e' THEN ARRAY(
                        SELECT e.enumlabel::text FROM pg_catalog.pg_enum e
                        WHERE e.enumtypid=ty.oid ORDER BY e.enumsortorder
                      ) ELSE NULL END AS enum_values
                FROM pg_catalog.pg_attribute a
                JOIN pg_catalog.pg_type ty ON ty.oid=a.atttypid
                LEFT JOIN pg_catalog.pg_attrdef def
                     ON def.adrelid=a.attrelid AND def.adnum=a.attnum
                WHERE a.attrelid::bigint=$1 AND a.attnum>0 AND NOT a.attisdropped
                ORDER BY a.attnum LIMIT $2"#,
                )
                .bind(oid)
                .bind(MAX_COLUMNS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|r| {
                    let enum_values: Option<Vec<String>> = get(r, "enum_values")?;
                    Ok(ColumnDetail {
                        column: Column {
                            name: get(r, "name")?,
                            native_type: get(r, "native_type")?,
                            nullable: get(r, "nullable")?,
                            primary_key: get(r, "primary_key")?,
                            default_expression: get(r, "default_expression")?,
                        },
                        position: get(r, "position")?,
                        unique_key: get(r, "unique_key")?,
                        identity: get(r, "identity")?,
                        generated: get(r, "generated")?,
                        domain: get(r, "domain")?,
                        enum_values: enum_values
                            .map_or(Availability::Unavailable, Availability::Available),
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_COLUMNS,
        ))
    }

    pub async fn list_constraints(
        &self,
        id: &RelationId,
    ) -> Result<CatalogList<Constraint>, DbError> {
        let oid = self.relation_oid(id).await?;
        let rows = self
            .bounded(
                sqlx::query(
                    r#"SELECT con.conname::text AS name, con.contype::text AS kind,
                      pg_catalog.pg_get_constraintdef(con.oid, true) AS definition,
                      ARRAY(SELECT a.attname::text
                            FROM unnest(con.conkey) WITH ORDINALITY AS k(attnum, pos)
                            JOIN pg_catalog.pg_attribute a
                              ON a.attrelid=con.conrelid AND a.attnum=k.attnum
                            ORDER BY k.pos) AS columns,
                      CASE WHEN target.oid IS NOT NULL
                         AND pg_catalog.has_schema_privilege(tns.oid,'USAGE')
                         AND pg_catalog.has_table_privilege(target.oid,'SELECT')
                         THEN tns.nspname::text ELSE NULL END AS target_schema,
                      CASE WHEN target.oid IS NOT NULL
                         AND pg_catalog.has_schema_privilege(tns.oid,'USAGE')
                         AND pg_catalog.has_table_privilege(target.oid,'SELECT')
                         THEN target.relname::text ELSE NULL END AS target_name,
                      CASE WHEN target.oid IS NOT NULL
                         AND pg_catalog.has_schema_privilege(tns.oid,'USAGE')
                         AND pg_catalog.has_table_privilege(target.oid,'SELECT')
                         THEN ARRAY(SELECT a.attname::text
                                    FROM unnest(con.confkey) WITH ORDINALITY AS k(attnum,pos)
                                    JOIN pg_catalog.pg_attribute a
                                      ON a.attrelid=con.confrelid AND a.attnum=k.attnum
                                    ORDER BY k.pos)
                         ELSE ARRAY[]::text[] END AS target_columns
                FROM pg_catalog.pg_constraint con
                LEFT JOIN pg_catalog.pg_class target ON target.oid=con.confrelid
                LEFT JOIN pg_catalog.pg_namespace tns ON tns.oid=target.relnamespace
                WHERE con.conrelid::bigint=$1
                  AND con.contype IN ('p','u','f','c','x')
                ORDER BY con.conname COLLATE "C" LIMIT $2"#,
                )
                .bind(oid)
                .bind(MAX_DETAILS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|r| {
                    let kind: String = get(r, "kind")?;
                    let kind = match kind.as_str() {
                        "p" => ConstraintKind::Primary,
                        "u" => ConstraintKind::Unique,
                        "f" => ConstraintKind::ForeignKey,
                        "c" => ConstraintKind::Check,
                        "x" => ConstraintKind::Exclusion,
                        _ => return Err(DbError::new(DbErrorKind::Unsupported)),
                    };
                    let target_schema: Option<String> = get(r, "target_schema")?;
                    let target_name: Option<String> = get(r, "target_name")?;
                    let target_visible = target_schema.is_some() && target_name.is_some();
                    // pg_get_constraintdef can include an otherwise inaccessible
                    // referenced table name. Do not render that raw definition.
                    let definition = if kind == ConstraintKind::ForeignKey && !target_visible {
                        "Foreign key references a relation not visible to this role".to_string()
                    } else {
                        get(r, "definition")?
                    };
                    Ok(Constraint {
                        name: get(r, "name")?,
                        kind,
                        columns: get(r, "columns")?,
                        references: target_schema
                            .zip(target_name)
                            .map(|(namespace, name)| RelationId { namespace, name }),
                        referenced_columns: get(r, "target_columns")?,
                        definition,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_DETAILS,
        ))
    }

    pub async fn list_indexes(&self, id: &RelationId) -> Result<CatalogList<Index>, DbError> {
        let oid = self.relation_oid(id).await?;
        let rows = self
            .bounded(
                sqlx::query(
                    r#"SELECT idx.relname::text AS name, i.indisunique AS unique_flag,
                      i.indisprimary AS primary_flag, i.indisvalid AS valid,
                      pg_catalog.pg_get_indexdef(i.indexrelid) AS definition,
                      pg_catalog.pg_get_expr(i.indpred, i.indrelid) AS predicate,
                      ARRAY(SELECT pg_catalog.pg_get_indexdef(i.indexrelid,k.pos,true)
                            FROM generate_series(1,i.indnkeyatts::int) AS k(pos)
                            ORDER BY k.pos) AS columns
                FROM pg_catalog.pg_index i
                JOIN pg_catalog.pg_class idx ON idx.oid=i.indexrelid
                WHERE i.indrelid::bigint=$1
                ORDER BY idx.relname COLLATE "C" LIMIT $2"#,
                )
                .bind(oid)
                .bind(MAX_DETAILS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|r| {
                    Ok(Index {
                        name: get(r, "name")?,
                        unique: get(r, "unique_flag")?,
                        primary: get(r, "primary_flag")?,
                        valid: get(r, "valid")?,
                        predicate: get(r, "predicate")?,
                        definition: get(r, "definition")?,
                        columns_or_expressions: get(r, "columns")?,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_DETAILS,
        ))
    }

    pub async fn list_partitions(
        &self,
        id: &RelationId,
    ) -> Result<CatalogList<PartitionLink>, DbError> {
        let oid = self.relation_oid(id).await?;
        let rows = self
            .bounded(
                sqlx::query(
                    r#"SELECT pn.nspname::text AS parent_schema, p.relname::text AS parent_name,
                      cn.nspname::text AS child_schema, c.relname::text AS child_name,
                      pg_catalog.pg_get_expr(c.relpartbound,c.oid,true) AS bound
                FROM pg_catalog.pg_inherits inh
                JOIN pg_catalog.pg_class p ON p.oid=inh.inhparent
                JOIN pg_catalog.pg_namespace pn ON pn.oid=p.relnamespace
                JOIN pg_catalog.pg_class c ON c.oid=inh.inhrelid
                JOIN pg_catalog.pg_namespace cn ON cn.oid=c.relnamespace
                WHERE (inh.inhparent::bigint=$1 OR inh.inhrelid::bigint=$1)
                  AND c.relispartition
                  AND pg_catalog.has_schema_privilege(pn.oid,'USAGE')
                  AND pg_catalog.has_schema_privilege(cn.oid,'USAGE')
                  AND pg_catalog.has_table_privilege(p.oid,'SELECT')
                  AND pg_catalog.has_table_privilege(c.oid,'SELECT')
                ORDER BY pn.nspname, p.relname, cn.nspname, c.relname LIMIT $2"#,
                )
                .bind(oid)
                .bind(MAX_DETAILS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|r| {
                    Ok(PartitionLink {
                        parent: RelationId {
                            namespace: get(r, "parent_schema")?,
                            name: get(r, "parent_name")?,
                        },
                        child: RelationId {
                            namespace: get(r, "child_schema")?,
                            name: get(r, "child_name")?,
                        },
                        bound: get(r, "bound")?,
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_DETAILS,
        ))
    }

    pub async fn relation_statistics(&self, id: &RelationId) -> Result<RelationStats, DbError> {
        let oid = self.relation_oid(id).await?;
        let row = self
            .bounded(
                sqlx::query(
                    r#"SELECT c.relkind::text AS relation_kind,
                      CASE WHEN c.relkind IN ('r','m') AND c.reltuples>=0
                         THEN c.reltuples::bigint ELSE NULL END AS rows_estimate,
                      CASE WHEN c.relkind IN ('r','m')
                         THEN pg_catalog.pg_table_size(c.oid) ELSE NULL END AS table_bytes,
                      CASE WHEN c.relkind IN ('r','m')
                         THEN pg_catalog.pg_indexes_size(c.oid) ELSE NULL END AS index_bytes,
                      CASE WHEN c.relkind IN ('r','m')
                         THEN pg_catalog.pg_total_relation_size(c.oid)
                         ELSE NULL END AS total_bytes
                FROM pg_catalog.pg_class c WHERE c.oid::bigint=$1"#,
                )
                .bind(oid)
                .fetch_one(&self.pool),
            )
            .await?;
        let estimate: Option<i64> = get(&row, "rows_estimate")?;
        let kind: String = get(&row, "relation_kind")?;
        Ok(RelationStats {
            relation_kind: Availability::Available(match kind.as_str() {
                "r" => RelationKind::Table,
                "p" => RelationKind::PartitionedTable,
                "v" => RelationKind::View,
                "m" => RelationKind::MaterializedView,
                "f" => RelationKind::ForeignTable,
                _ => return Err(DbError::new(DbErrorKind::Unsupported)),
            }),
            estimated_rows: estimate.map_or(Availability::Unavailable, Availability::Available),
            table_bytes: available_bytes(get(&row, "table_bytes")?),
            index_bytes: available_bytes(get(&row, "index_bytes")?),
            total_bytes: available_bytes(get(&row, "total_bytes")?),
        })
    }

    /// Return the trusted namespace for PostGIS functions, if the extension is
    /// installed in this database. The namespace is quoted before SQL use.
    pub async fn postgis_schema(&self) -> Result<Option<String>, DbError> {
        self.bounded(
            sqlx::query_scalar::<_, String>(
                r#"SELECT n.nspname::text
                   FROM pg_catalog.pg_extension e
                   JOIN pg_catalog.pg_namespace n ON n.oid=e.extnamespace
                   WHERE e.extname='postgis' LIMIT 1"#,
            )
            .fetch_optional(&self.pool),
        )
        .await
    }

    /// Metadata only. Never invoke routines or read sequence values.
    pub async fn list_advanced(
        &self,
        namespace: &str,
    ) -> Result<CatalogList<CatalogObject>, DbError> {
        Self::user_schema(namespace)?;
        // Select at most 501 visible object OIDs before decompiling
        // procedure/trigger bodies. This bounds expensive source retrieval
        // even when PostGIS installs thousands of public functions.
        let rows = self
            .bounded(
                sqlx::query(
                    r#"WITH visible AS (
                SELECT CASE p.prokind WHEN 'p' THEN 'procedure'
                       ELSE 'function' END::text AS kind,
                       n.nspname::text AS namespace,
                       (p.proname || '(' ||
                        pg_catalog.pg_get_function_identity_arguments(p.oid) || ')')::text AS name,
                       p.oid AS object_oid
                FROM pg_catalog.pg_proc p
                JOIN pg_catalog.pg_namespace n ON n.oid=p.pronamespace
                WHERE n.nspname=$1 AND p.prokind IN ('f','p')
                  AND pg_catalog.has_schema_privilege(n.oid,'USAGE')
                  AND pg_catalog.has_function_privilege(p.oid,'EXECUTE')
                UNION ALL
                SELECT 'sequence', n.nspname::text, c.relname::text, c.oid
                FROM pg_catalog.pg_class c
                JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=$1 AND c.relkind='S'
                  AND pg_catalog.has_schema_privilege(n.oid,'USAGE')
                  AND (pg_catalog.has_sequence_privilege(c.oid,'SELECT')
                       OR pg_catalog.has_sequence_privilege(c.oid,'USAGE'))
                UNION ALL
                SELECT 'type', n.nspname::text, t.typname::text, t.oid
                FROM pg_catalog.pg_type t
                JOIN pg_catalog.pg_namespace n ON n.oid=t.typnamespace
                WHERE n.nspname=$1 AND t.typtype IN ('e','d','c')
                  AND (t.typtype <> 'c' OR EXISTS (
                       SELECT 1 FROM pg_catalog.pg_class c
                       WHERE c.oid=t.typrelid AND c.relkind='c'))
                  AND pg_catalog.has_schema_privilege(n.oid,'USAGE')
                  AND pg_catalog.has_type_privilege(t.oid,'USAGE')
                UNION ALL
                SELECT 'trigger', n.nspname::text,
                       (c.relname || '.' || tr.tgname)::text, tr.oid
                FROM pg_catalog.pg_trigger tr
                JOIN pg_catalog.pg_class c ON c.oid=tr.tgrelid
                JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=$1 AND NOT tr.tgisinternal
                  AND pg_catalog.has_schema_privilege(n.oid,'USAGE')
                  AND pg_catalog.has_table_privilege(c.oid,'SELECT')
            )
            SELECT chosen.kind, chosen.namespace, chosen.name,
                   CASE WHEN chosen.kind IN ('function','procedure')
                       THEN left(pg_catalog.pg_get_functiondef(chosen.object_oid),4096)
                        WHEN chosen.kind='trigger'
                       THEN left(pg_catalog.pg_get_triggerdef(chosen.object_oid,true),4096)
                        ELSE NULL END::text AS definition
            FROM (
                SELECT kind,namespace,name,object_oid FROM visible
                ORDER BY kind,name COLLATE "C" LIMIT $2
            ) chosen
            ORDER BY chosen.kind,chosen.name COLLATE "C""#,
                )
                .bind(namespace)
                .bind(MAX_RELATIONS + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        Ok(capped(
            rows.iter()
                .map(|r| {
                    let kind: String = get(r, "kind")?;
                    let kind = match kind.as_str() {
                        "function" => CatalogObjectKind::Function,
                        "procedure" => CatalogObjectKind::Procedure,
                        "sequence" => CatalogObjectKind::Sequence,
                        "type" => CatalogObjectKind::UserType,
                        "trigger" => CatalogObjectKind::Trigger,
                        _ => return Err(DbError::new(DbErrorKind::Unsupported)),
                    };
                    let def: Option<String> = get(r, "definition")?;
                    Ok(CatalogObject {
                        kind,
                        namespace: get(r, "namespace")?,
                        name: get(r, "name")?,
                        definition: def.map_or(Availability::Unavailable, Availability::Available),
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_RELATIONS,
        ))
    }

    /// Estimated overview sizes (relpages * block_size). No table-wide scans.
    pub async fn advanced_overview(&self) -> Result<Overview, DbError> {
        let schemas = self.list_schemas().await?;
        let extensions = self.bounded(sqlx::query(
            "SELECT extname::text AS name FROM pg_catalog.pg_extension ORDER BY extname LIMIT 65"
        ).fetch_all(&self.pool)).await?
            .iter().take(64).map(|r| get(r,"name"))
            .collect::<Result<Vec<String>,DbError>>()?;
        let largest = self
            .bounded(
                sqlx::query(
                    r#"SELECT n.nspname::text AS namespace, c.relname::text AS name,
                      GREATEST(c.relpages::bigint,0)
                        * pg_catalog.current_setting('block_size')::bigint AS bytes_estimate
                FROM pg_catalog.pg_class c
                JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE c.relkind IN ('r','m')
                  AND left(n.nspname,3) <> 'pg_'
                  AND n.nspname<>'information_schema'
                  AND pg_catalog.has_schema_privilege(n.oid,'USAGE')
                  AND pg_catalog.has_table_privilege(c.oid,'SELECT')
                ORDER BY bytes_estimate DESC, n.nspname, c.relname
                LIMIT $1"#,
                )
                .bind(MAX_OVERVIEW + 1)
                .fetch_all(&self.pool),
            )
            .await?;
        let largest = capped(
            largest
                .iter()
                .map(|r| {
                    Ok((
                        RelationId {
                            namespace: get(r, "namespace")?,
                            name: get(r, "name")?,
                        },
                        available_bytes(get(r, "bytes_estimate")?),
                    ))
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_OVERVIEW,
        );
        let sizes = self.bounded(sqlx::query(
            r#"SELECT n.nspname::text AS namespace,
                      SUM(GREATEST(c.relpages::bigint,0)
                          * pg_catalog.current_setting('block_size')::bigint)::bigint AS bytes_estimate
                FROM pg_catalog.pg_class c
                JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE c.relkind IN ('r','m')
                  AND left(n.nspname,3) <> 'pg_'
                  AND n.nspname<>'information_schema'
                  AND pg_catalog.has_schema_privilege(n.oid,'USAGE')
                  AND pg_catalog.has_table_privilege(c.oid,'SELECT')
                GROUP BY n.nspname
                ORDER BY bytes_estimate DESC, n.nspname LIMIT $1"#
        ).bind(MAX_SCHEMAS+1).fetch_all(&self.pool)).await?;
        let sizes = capped(
            sizes
                .iter()
                .map(|r| {
                    Ok(SchemaSize {
                        namespace: get(r, "namespace")?,
                        estimated_bytes: available_bytes(get(r, "bytes_estimate")?),
                    })
                })
                .collect::<Result<Vec<_>, DbError>>()?,
            MAX_SCHEMAS,
        );
        // One bounded catalog count, never an N+1 query per schema.
        let visible = self
            .bounded(
                sqlx::query(
                    r#"SELECT c.oid::bigint AS oid
                FROM pg_catalog.pg_class c
                JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE c.relkind IN ('r','p','v','m','f')
                  AND left(n.nspname,3) <> 'pg_'
                  AND n.nspname <> 'information_schema'
                  AND pg_catalog.has_schema_privilege(n.oid,'USAGE')
                  AND pg_catalog.has_table_privilege(c.oid,'SELECT')
                LIMIT 5001"#,
                )
                .fetch_all(&self.pool),
            )
            .await?;
        let visible_relation_count = visible.len().min(5000);
        let truncated = schemas.truncated
            || sizes.truncated
            || largest.truncated
            || visible.len() > 5000
            || extensions.len() == 64;
        Ok(Overview {
            extensions,
            schema_sizes: sizes,
            largest_relations: largest,
            visible_relation_count,
            visible_schema_count: schemas.items.len(),
            truncated,
        })
    }
}

// RPITIT: consumers of the engine-neutral interface never access pg_catalog SQL.
#[async_trait::async_trait]
impl DatabaseAdapter for PgCatalog {
    fn engine(&self) -> Engine {
        Engine::PostgreSql
    }

    fn sort_supported(&self, native_type: &str) -> bool {
        super::row_query::sort_supported(native_type)
    }
    fn supports(&self, capability: Capability) -> bool {
        matches!(
            capability,
            Capability::Schemas
                | Capability::Views
                | Capability::MaterializedViews
                | Capability::Indexes
                | Capability::ForeignKeys
                | Capability::EstimatedRows
                | Capability::StorageSizes
                | Capability::SpatialTypes
                | Capability::Routines
                | Capability::Sequences
                | Capability::UserTypes
                | Capability::Triggers
                | Capability::PartitionHierarchy
        )
    }
    async fn namespaces(&self) -> Result<Vec<Namespace>, DbError> {
        Ok(self.list_schemas().await?.items)
    }
    async fn relations(&self, namespace: &str) -> Result<Vec<Relation>, DbError> {
        Ok(self.list_relations(namespace).await?.items)
    }
    async fn columns(&self, id: &RelationId) -> Result<Vec<Column>, DbError> {
        Ok(self
            .describe_columns(id)
            .await?
            .items
            .into_iter()
            .map(|d| d.column)
            .collect())
    }
    async fn statistics(&self, id: &RelationId) -> Result<RelationStats, DbError> {
        self.relation_statistics(id).await
    }
    async fn column_details(&self, id: &RelationId) -> Result<CatalogList<ColumnDetail>, DbError> {
        self.describe_columns(id).await
    }
    async fn constraints(&self, id: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        self.list_constraints(id).await
    }
    async fn indexes(&self, id: &RelationId) -> Result<CatalogList<Index>, DbError> {
        self.list_indexes(id).await
    }
    async fn partitions(&self, id: &RelationId) -> Result<CatalogList<PartitionLink>, DbError> {
        self.list_partitions(id).await
    }
    async fn advanced_objects(
        &self,
        namespace: &str,
    ) -> Result<CatalogList<CatalogObject>, DbError> {
        self.list_advanced(namespace).await
    }
    async fn overview(&self) -> Result<Overview, DbError> {
        self.advanced_overview().await
    }
    async fn page(
        &self,
        id: &RelationId,
        opts: crate::db::types::RowOptions,
    ) -> Result<RowPage, DbError> {
        self.fetch_rows(id, opts).await
    }
    async fn rows(&self, id: &RelationId, opts: PageRequest) -> Result<RowPage, DbError> {
        self.offset_page(id, opts).await
    }
    async fn list_schemas(&self) -> Result<CatalogList<Namespace>, DbError> {
        PgCatalog::list_schemas(self).await
    }
    async fn list_relations(&self, namespace: &str) -> Result<CatalogList<Relation>, DbError> {
        PgCatalog::list_relations(self, namespace).await
    }
    async fn describe_columns(
        &self,
        id: &RelationId,
    ) -> Result<CatalogList<ColumnDetail>, DbError> {
        PgCatalog::describe_columns(self, id).await
    }
    async fn list_constraints(&self, id: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        PgCatalog::list_constraints(self, id).await
    }
    async fn list_indexes(&self, id: &RelationId) -> Result<CatalogList<Index>, DbError> {
        PgCatalog::list_indexes(self, id).await
    }
    async fn list_partitions(
        &self,
        id: &RelationId,
    ) -> Result<CatalogList<PartitionLink>, DbError> {
        PgCatalog::list_partitions(self, id).await
    }
    async fn list_advanced(&self, namespace: &str) -> Result<CatalogList<CatalogObject>, DbError> {
        PgCatalog::list_advanced(self, namespace).await
    }
    async fn advanced_overview(&self) -> Result<Overview, DbError> {
        PgCatalog::advanced_overview(self).await
    }
    async fn relation_statistics(&self, id: &RelationId) -> Result<RelationStats, DbError> {
        PgCatalog::relation_statistics(self, id).await
    }
    async fn fetch_rows(&self, id: &RelationId, opts: RowOptions) -> Result<RowPage, DbError> {
        PgCatalog::fetch_rows(self, id, opts).await
    }
    async fn postgis_schema(&self) -> Result<Option<String>, DbError> {
        PgCatalog::postgis_schema(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relation_kinds_preserve_partitions_and_views() {
        assert_eq!(
            supported("p", false).unwrap(),
            RelationKind::PartitionedTable
        );
        assert_eq!(supported("r", true).unwrap(), RelationKind::Partition);
        assert_eq!(
            supported("m", false).unwrap(),
            RelationKind::MaterializedView
        );
        assert_eq!(supported("f", false).unwrap(), RelationKind::ForeignTable);
        assert!(supported("S", false).is_err());
    }
    #[test]
    fn system_schemas_are_never_navigable() {
        assert!(PgCatalog::user_schema("pg_catalog").is_err());
        assert!(PgCatalog::user_schema("pg_toast").is_err());
        assert!(PgCatalog::user_schema("information_schema").is_err());
        assert!(PgCatalog::user_schema("public").is_ok());
        assert!(PgCatalog::user_schema("CuaDB Mixed").is_ok());
    }
    #[test]
    fn truncation_is_explicit_and_stable() {
        assert_eq!(
            capped(vec![1, 2, 3], 2),
            CatalogList {
                items: vec![1, 2],
                truncated: true
            }
        );
        assert_eq!(available_bytes(None), Availability::Unavailable);
        assert_eq!(available_bytes(Some(-1)), Availability::Unavailable);
        assert_eq!(available_bytes(Some(0)), Availability::Available(0));
    }
}
