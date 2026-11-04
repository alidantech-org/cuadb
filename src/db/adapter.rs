//! Adapter interface for supported database engines.
//!
//! Return-position impl Future keeps the trait dependency-free and testable;
//! an explicit adapter enum can provide dispatch once multiple engines exist.

use std::future::Future;
use super::types::{
    Capability, Column, DbError, Engine, Namespace, PageRequest, Relation, RelationId,
    RelationStats, RowPage,
};

pub trait DatabaseAdapter: Send + Sync {
    fn engine(&self) -> Engine;
    fn supports(&self, capability: Capability) -> bool;
    fn namespaces(&self) -> impl Future<Output = Result<Vec<Namespace>, DbError>> + Send;
    fn relations(&self, namespace: &str)
        -> impl Future<Output = Result<Vec<Relation>, DbError>> + Send;
    fn columns(&self, relation: &RelationId)
        -> impl Future<Output = Result<Vec<Column>, DbError>> + Send;
    fn statistics(&self, relation: &RelationId)
        -> impl Future<Output = Result<RelationStats, DbError>> + Send;
    fn rows(
        &self,
        relation: &RelationId,
        request: PageRequest,
    ) -> impl Future<Output = Result<RowPage, DbError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::{Availability, DbErrorKind};

    // This fake proves HTTP-facing code can be tested without opening a socket.
    struct Stub;
    impl DatabaseAdapter for Stub {
        fn engine(&self) -> Engine { Engine::Sqlite }
        fn supports(&self, capability: Capability) -> bool {
            matches!(capability, Capability::Views)
        }
        async fn namespaces(&self) -> Result<Vec<Namespace>, DbError> {
            Ok(vec![Namespace { name: "main".into() }])
        }
        async fn relations(&self, _: &str) -> Result<Vec<Relation>, DbError> { Ok(vec![]) }
        async fn columns(&self, _: &RelationId) -> Result<Vec<Column>, DbError> { Ok(vec![]) }
        async fn statistics(&self, _: &RelationId) -> Result<RelationStats, DbError> {
            Ok(RelationStats {
                estimated_rows: Availability::Unavailable,
                table_bytes: Availability::Unavailable,
                index_bytes: Availability::Unavailable,
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
        let id = RelationId { namespace: "main".into(), name: "items".into() };
        assert_eq!(stub.rows(&id, PageRequest { limit: 25, offset: 0 })
            .await.unwrap_err().kind, DbErrorKind::Unsupported);
    }
}
