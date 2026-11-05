//! Adapter interface for supported database engines.
//!
//! Return-position impl Future keeps the trait dependency-free and testable;
//! an explicit adapter enum can provide dispatch once multiple engines exist.

use super::types::{
    Capability, CatalogList, CatalogObject, Column, ColumnDetail, Constraint, DbError, Engine,
    Index, Namespace, Overview, PageRequest, PartitionLink, Relation, RelationId, RelationStats,
    RowOptions, RowPage,
};
#[async_trait::async_trait]
pub trait DatabaseAdapter: Send + Sync {
    fn engine(&self) -> Engine;
    async fn detected_engine(&self) -> Engine {
        self.engine()
    }
    fn sort_supported(&self, native_type: &str) -> bool {
        let _ = native_type;
        true
    }
    fn supports(&self, capability: Capability) -> bool;
    async fn namespaces(&self) -> Result<Vec<Namespace>, DbError>;
    async fn relations(&self, namespace: &str) -> Result<Vec<Relation>, DbError>;
    async fn columns(&self, relation: &RelationId) -> Result<Vec<Column>, DbError>;
    async fn statistics(&self, relation: &RelationId) -> Result<RelationStats, DbError>;
    /// Optional extended metadata. Other engines explicitly return Unsupported.
    async fn column_details(&self, _: &RelationId) -> Result<CatalogList<ColumnDetail>, DbError> {
        Err(DbError::new(super::types::DbErrorKind::Unsupported))
    }
    async fn constraints(&self, _: &RelationId) -> Result<CatalogList<Constraint>, DbError> {
        Err(DbError::new(super::types::DbErrorKind::Unsupported))
    }
    async fn indexes(&self, _: &RelationId) -> Result<CatalogList<Index>, DbError> {
        Err(DbError::new(super::types::DbErrorKind::Unsupported))
    }
    async fn partitions(&self, _: &RelationId) -> Result<CatalogList<PartitionLink>, DbError> {
        Err(DbError::new(super::types::DbErrorKind::Unsupported))
    }
    async fn advanced_objects(&self, _: &str) -> Result<CatalogList<CatalogObject>, DbError> {
        Err(DbError::new(super::types::DbErrorKind::Unsupported))
    }
    async fn overview(&self) -> Result<Overview, DbError> {
        Err(DbError::new(super::types::DbErrorKind::Unsupported))
    }
    /// Extended paging, optional on other database engines.
    async fn page(&self, _: &RelationId, _: RowOptions) -> Result<RowPage, DbError> {
        Err(DbError::new(super::types::DbErrorKind::Unsupported))
    }
    async fn rows(&self, relation: &RelationId, request: PageRequest) -> Result<RowPage, DbError>;

    async fn list_schemas(&self) -> Result<CatalogList<Namespace>, DbError> {
        Ok(CatalogList {
            items: self.namespaces().await?,
            truncated: false,
        })
    }
    async fn list_relations(&self, namespace: &str) -> Result<CatalogList<Relation>, DbError> {
        Ok(CatalogList {
            items: self.relations(namespace).await?,
            truncated: false,
        })
    }
    async fn describe_columns(
        &self,
        relation: &RelationId,
    ) -> Result<CatalogList<ColumnDetail>, DbError> {
        self.column_details(relation).await
    }
    async fn list_constraints(
        &self,
        relation: &RelationId,
    ) -> Result<CatalogList<Constraint>, DbError> {
        self.constraints(relation).await
    }
    async fn list_indexes(&self, relation: &RelationId) -> Result<CatalogList<Index>, DbError> {
        self.indexes(relation).await
    }
    async fn list_partitions(
        &self,
        relation: &RelationId,
    ) -> Result<CatalogList<PartitionLink>, DbError> {
        self.partitions(relation).await
    }
    async fn list_advanced(&self, namespace: &str) -> Result<CatalogList<CatalogObject>, DbError> {
        self.advanced_objects(namespace).await
    }
    async fn advanced_overview(&self) -> Result<Overview, DbError> {
        self.overview().await
    }
    async fn relation_statistics(&self, relation: &RelationId) -> Result<RelationStats, DbError> {
        self.statistics(relation).await
    }
    async fn fetch_rows(
        &self,
        relation: &RelationId,
        options: RowOptions,
    ) -> Result<RowPage, DbError> {
        self.page(relation, options).await
    }
    async fn postgis_schema(&self) -> Result<Option<String>, DbError> {
        Ok(None)
    }
    /// Databases of this server the connection's role may connect to.
    async fn list_databases(&self) -> Result<Vec<String>, DbError> {
        Err(DbError::new(crate::db::types::DbErrorKind::Unsupported))
    }
    /// Estimated rows and size of the tables of a schema (statistics only).
    async fn relation_sizes(
        &self,
        _namespace: &str,
    ) -> Result<Vec<crate::db::types::RelationSize>, DbError> {
        Err(DbError::new(crate::db::types::DbErrorKind::Unsupported))
    }
    /// Table count and estimated size per visible schema (statistics only).
    async fn schema_summaries(&self) -> Result<Vec<crate::db::types::SchemaSummary>, DbError> {
        Err(DbError::new(crate::db::types::DbErrorKind::Unsupported))
    }
    /// One cell, addressed by the relation's full primary key. Engines without support
    /// report `Unsupported` and the sheet keeps the bounded value it already has.
    async fn cell_value(
        &self,
        _relation: &RelationId,
        _column: &str,
        _key: &[(String, String)],
    ) -> Result<crate::db::types::CellValue, DbError> {
        Err(DbError::new(crate::db::types::DbErrorKind::Unsupported))
    }
    /// Whether the given ordered columns identify at most one row.
    async fn is_unique_key(
        &self,
        _relation: &RelationId,
        _columns: &[String],
    ) -> Result<bool, DbError> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::{Availability, DbErrorKind};

    // This fake proves HTTP-facing code can be tested without opening a socket.
    struct Stub;
    #[async_trait::async_trait]
    impl DatabaseAdapter for Stub {
        fn engine(&self) -> Engine {
            Engine::Sqlite
        }
        fn supports(&self, capability: Capability) -> bool {
            matches!(capability, Capability::Views)
        }
        async fn namespaces(&self) -> Result<Vec<Namespace>, DbError> {
            Ok(vec![Namespace {
                name: "main".into(),
            }])
        }
        async fn relations(&self, _: &str) -> Result<Vec<Relation>, DbError> {
            Ok(vec![])
        }
        async fn columns(&self, _: &RelationId) -> Result<Vec<Column>, DbError> {
            Ok(vec![])
        }
        async fn statistics(&self, _: &RelationId) -> Result<RelationStats, DbError> {
            Ok(RelationStats {
                relation_kind: Availability::Unavailable,
                estimated_rows: Availability::Unavailable,
                table_bytes: Availability::Unavailable,
                index_bytes: Availability::Unavailable,
                total_bytes: Availability::Unavailable,
            })
        }
        async fn rows(&self, _: &RelationId, _: PageRequest) -> Result<RowPage, DbError> {
            Err(DbError::new(DbErrorKind::Unsupported))
        }
    }

    #[tokio::test]
    async fn stub_adapter_uses_common_contract() {
        let stub = Stub;
        assert_eq!(stub.engine(), Engine::Sqlite);
        assert!(stub.supports(Capability::Views));
        assert!(!stub.supports(Capability::StorageSizes));
        assert_eq!(stub.namespaces().await.unwrap()[0].name, "main");
        let id = RelationId {
            namespace: "main".into(),
            name: "items".into(),
        };
        assert_eq!(
            stub.rows(
                &id,
                PageRequest {
                    limit: 25,
                    offset: 0
                }
            )
            .await
            .unwrap_err()
            .kind,
            DbErrorKind::Unsupported
        );
    }
}
