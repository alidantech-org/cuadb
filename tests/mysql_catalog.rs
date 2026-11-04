//! Opt-in integration checks against a disposable MariaDB fixture.
use cuadb::{
    config::Config,
    db::{
        adapter::DatabaseAdapter,
        mysql::MySqlManager,
        types::{Capability, Cell, RelationId, RelationKind, RowOptions, SortDirection, SortSpec},
    },
};

#[tokio::test]
async fn adapter_contract_without_mysql_network() {
    let config = Config::from_values(&[]).unwrap();
    let manager = MySqlManager::new(
        "mysql://reader:fixture@127.0.0.1:1/cuadb_fixture_unreachable",
        &config.limits,
    )
    .unwrap();
    let catalog = manager.catalog();
    assert_eq!(catalog.engine(), cuadb::db::types::Engine::MySql);
    assert!(catalog.supports(Capability::Schemas));
    assert!(catalog.supports(Capability::StorageSizes));
    assert!(!catalog.supports(Capability::SpatialTypes));
    manager.close().await;
}

#[tokio::test]
async fn disposable_mariadb_catalog_and_rows() {
    let Ok(url) = std::env::var("CUADB_TEST_MARIADB_URL") else {
        return;
    };
    let parsed = url::Url::parse(&url).unwrap();
    assert!(
        parsed
            .path()
            .trim_start_matches('/')
            .starts_with("cuadb_fixture_"),
        "must use disposable cuadb_fixture database"
    );
    let config = Config::from_values(&[]).unwrap();
    let manager = MySqlManager::new(&url, &config.limits).unwrap();
    assert!(manager.snapshot().await.status == cuadb::db::types::ConnectionStatus::Ready);
    let catalog = manager.catalog();
    let schemas = catalog.list_schemas().await.unwrap();
    assert!(
        schemas
            .items
            .iter()
            .any(|schema| schema.name == "cuadb_fixture_mariadb")
    );
    let relations = catalog
        .list_relations("cuadb_fixture_mariadb")
        .await
        .unwrap();
    assert!(
        relations
            .items
            .iter()
            .any(|relation| relation.id.name == "Mixed_Items"
                && relation.kind == RelationKind::Table)
    );
    assert!(relations.items.iter().any(|relation| relation.id.name == "items_view" && relation.kind == RelationKind::View));
    let id = RelationId {
        namespace: "cuadb_fixture_mariadb".into(),
        name: "Mixed_Items".into(),
    };
    let columns = catalog.describe_columns(&id).await.unwrap();
    assert!(
        columns
            .items
            .iter()
            .any(|column| column.column.name == "id" && column.column.primary_key)
    );
    assert!(columns.items.iter().any(|column| column.identity.is_some()));
    assert!(
        columns
            .items
            .iter()
            .any(|column| column.generated.is_some())
    );
    let constraints = catalog
        .list_constraints(&RelationId {
            namespace: "cuadb_fixture_mariadb".into(),
            name: "child".into(),
        })
        .await
        .unwrap();
    assert!(constraints.items.iter().any(|constraint| {
        constraint
            .references
            .as_ref()
            .is_some_and(|id| id.name == "Mixed_Items")
    }));
    let rows = catalog
        .fetch_rows(
            &id,
            RowOptions {
                limit: 1,
                sort: Some(SortSpec {
                    column: "id".into(),
                    direction: SortDirection::Asc,
                }),
                ..RowOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(rows.rows.len(), 1);
    assert!(rows.has_more);
    assert!(matches!(rows.rows[0][0], Cell::Integer(1)));
    let stats = catalog.relation_statistics(&id).await.unwrap();
    assert!(matches!(
        stats.estimated_rows,
        cuadb::db::types::Availability::Available(_)
    ));
    manager.close().await;
}
