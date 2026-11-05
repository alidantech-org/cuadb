//! Opt-in, READ-ONLY catalog tests. Setup fixtures manually in a
//! disposable cuadb_fixture_* database; tests never execute DDL or DML.
use cuadb::{
    config::Config,
    db::{
        adapter::DatabaseAdapter,
        postgres::PgManager,
        types::{Availability, Capability, CatalogObjectKind, RelationId, RelationKind},
    },
};

#[tokio::test]
async fn catalog_contracts_work_without_postgres() {
    use cuadb::db::postgres::catalog::PgCatalog;
    // The adapter's kind/capabilities can be tested from a lazy pool without
    // opening a socket; this test does not attempt a database query.
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(
        "postgresql://viewer:password@127.0.0.1:1/absent?sslmode=disable",
        &config.limits,
    )
    .unwrap();
    let catalog: PgCatalog = manager.catalog();
    assert!(catalog.supports(Capability::Schemas));
    assert!(catalog.supports(Capability::Triggers));
    assert!(!catalog.supports(Capability::StableCursorPaging));
}

fn id(name: &str) -> RelationId {
    RelationId {
        namespace: "cuadb_fixture".into(),
        name: name.into(),
    }
}

#[tokio::test]
async fn disposable_catalog_fixtures() {
    let Ok(url) = std::env::var("CUADB_TEST_CATALOG_URL") else {
        return;
    };
    // Guard against accidental use of a user's real database; matching this
    // prefix is not a substitute for disposable fixtures and limited roles.
    let parsed = url::Url::parse(&url).unwrap();
    let database = parsed.path().trim_start_matches('/');
    assert!(
        database.starts_with("cuadb_fixture_"),
        "catalog fixture URL must point to cuadb_fixture_*"
    );
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(&url, &config.limits).unwrap();
    let catalog = manager.catalog();

    let schemas = catalog.list_schemas().await.unwrap();
    assert!(schemas.items.iter().any(|x| x.name == "cuadb_fixture"));
    assert!(schemas.items.iter().any(|x| x.name == "CuaDB Mixed"));
    assert!(!schemas.items.iter().any(|x| x.name == "pg_catalog"));

    let relations = catalog.list_relations("cuadb_fixture").await.unwrap();
    let find = |name: &str| {
        relations
            .items
            .iter()
            .find(|x| x.id.name == name)
            .unwrap()
            .kind
    };
    assert_eq!(find("accounts"), RelationKind::Table);
    assert_eq!(find("accounts_view"), RelationKind::View);
    assert_eq!(find("accounts_mat"), RelationKind::MaterializedView);
    assert_eq!(find("events"), RelationKind::PartitionedTable);
    assert_eq!(find("events_2026"), RelationKind::Partition);
    assert_eq!(find("foreign_stats_fixture"), RelationKind::ForeignTable);
    let quoted = catalog.list_relations("CuaDB Mixed").await.unwrap();
    assert!(quoted.items.iter().any(|r| r.id.name == "Ünicode Table"));

    let details = catalog.describe_columns(&id("orders")).await.unwrap();
    let code = details
        .items
        .iter()
        .find(|d| d.column.name == "Code")
        .unwrap();
    assert!(!code.column.nullable);
    assert!(code.column.primary_key);
    let identity = details
        .items
        .iter()
        .find(|d| d.column.name == "ID")
        .unwrap();
    assert!(identity.identity.is_some());
    let generated = details
        .items
        .iter()
        .find(|d| d.column.name == "computed")
        .unwrap();
    assert!(generated.generated.is_some());
    assert!(
        details
            .items
            .iter()
            .any(|d| d.column.name == "value" && d.domain.is_some())
    );

    let constraints = catalog.list_constraints(&id("orders")).await.unwrap();
    let fk = constraints
        .items
        .iter()
        .find(|c| c.name.contains("account_id"))
        .unwrap();
    assert_eq!(fk.references.as_ref().unwrap().name, "accounts");
    assert_eq!(fk.referenced_columns, vec!["id"]);
    let keys = catalog.list_indexes(&id("orders")).await.unwrap();
    assert!(keys.items.iter().any(|i| i.primary));
    assert!(
        keys.items
            .iter()
            .any(|i| i.columns_or_expressions.iter().any(|c| c.contains("lower")))
    );
    let partitions = catalog.list_partitions(&id("events")).await.unwrap();
    assert!(
        partitions
            .items
            .iter()
            .any(|p| p.child.name == "events_2026")
    );

    let stats = catalog.relation_statistics(&id("orders")).await.unwrap();
    assert!(matches!(stats.table_bytes, Availability::Available(_)));
    assert!(matches!(stats.index_bytes, Availability::Available(_)));
    assert!(matches!(stats.total_bytes, Availability::Available(_)));
    let parent = catalog.relation_statistics(&id("events")).await.unwrap();
    assert_eq!(parent.table_bytes, Availability::Unavailable);
    let view = catalog
        .relation_statistics(&id("accounts_view"))
        .await
        .unwrap();
    assert_eq!(view.estimated_rows, Availability::Unavailable);
    let foreign = catalog
        .relation_statistics(&id("foreign_stats_fixture"))
        .await
        .unwrap();
    assert_eq!(
        foreign.relation_kind,
        Availability::Available(RelationKind::ForeignTable)
    );
    assert_eq!(foreign.table_bytes, Availability::Unavailable);
    assert_eq!(foreign.index_bytes, Availability::Unavailable);

    let advanced = catalog.list_advanced("cuadb_fixture").await.unwrap();
    assert!(
        advanced
            .items
            .iter()
            .any(|o| o.kind == CatalogObjectKind::Function)
    );
    assert!(
        advanced
            .items
            .iter()
            .any(|o| o.kind == CatalogObjectKind::Procedure)
    );
    assert!(
        advanced
            .items
            .iter()
            .any(|o| o.kind == CatalogObjectKind::Sequence)
    );
    assert!(
        advanced
            .items
            .iter()
            .any(|o| o.kind == CatalogObjectKind::Trigger)
    );
    let overview = catalog.advanced_overview().await.unwrap();
    assert!(overview.visible_schema_count >= 2);
    assert!(overview.visible_relation_count >= 5);
    manager.close().await;
}

#[tokio::test]
async fn disposable_postgis_metadata_fixture() {
    let Ok(url) = std::env::var("CUADB_TEST_CATALOG_POSTGIS_URL") else {
        return;
    };
    let parsed = url::Url::parse(&url).unwrap();
    assert!(
        parsed
            .path()
            .trim_start_matches('/')
            .starts_with("cuadb_fixture_")
    );
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(&url, &config.limits).unwrap();
    let catalog = manager.catalog();
    let columns = catalog.describe_columns(&id("places")).await.unwrap();
    assert!(
        columns
            .items
            .iter()
            .any(|c| c.column.native_type.contains("geometry"))
    );
    let indexes = catalog.list_indexes(&id("places")).await.unwrap();
    assert!(indexes.items.iter().any(|i| i.name == "places_gist"));
    let spatial_rows = catalog
        .fetch_rows(&id("places"), cuadb::db::types::RowOptions::default())
        .await
        .unwrap();
    assert!(matches!(
        &spatial_rows.rows[0][1],
        cuadb::db::types::Cell::Text(value) if value.starts_with("POINT(")
    ));
    let statistics = catalog.relation_statistics(&id("places")).await.unwrap();
    assert!(matches!(
        statistics.relation_kind,
        cuadb::db::types::Availability::Available(cuadb::db::types::RelationKind::Table)
    ));
    let overview = catalog.advanced_overview().await.unwrap();
    assert!(overview.extensions.iter().any(|e| e == "postgis"));
    assert!(catalog.postgis_schema().await.unwrap().is_some());
    let geojson_rows = catalog
        .fetch_rows(
            &id("places"),
            cuadb::db::types::RowOptions {
                spatial_geojson: true,
                ..cuadb::db::types::RowOptions::default()
            },
        )
        .await
        .unwrap();
    assert!(
        geojson_rows.previews[0][1]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("\"coordinates\""))
    );
    manager.close().await;
}

#[tokio::test]
async fn plain_postgres_fixture_reports_postgis_as_unavailable() {
    let Ok(url) = std::env::var("CUADB_TEST_CATALOG_PLAIN_URL") else {
        return;
    };
    let parsed = url::Url::parse(&url).unwrap();
    assert!(
        parsed
            .path()
            .trim_start_matches('/')
            .starts_with("cuadb_fixture_")
    );
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(&url, &config.limits).unwrap();
    let catalog = manager.catalog();
    assert!(catalog.postgis_schema().await.unwrap().is_none());
    let overview = catalog.advanced_overview().await.unwrap();
    assert!(
        !overview
            .extensions
            .iter()
            .any(|extension| extension == "postgis")
    );
    manager.close().await;
}

#[tokio::test]
async fn denied_fixture_never_lists_inaccessible_relations() {
    let Ok(url) = std::env::var("CUADB_TEST_CATALOG_DENIED_URL") else {
        return;
    };
    let parsed = url::Url::parse(&url).unwrap();
    assert!(
        parsed
            .path()
            .trim_start_matches('/')
            .starts_with("cuadb_fixture_")
    );
    let config = Config::from_values(&[]).unwrap();
    let manager = PgManager::new(&url, &config.limits).unwrap();
    let catalog = manager.catalog();
    let namespaces = catalog.list_schemas().await.unwrap();
    assert!(!namespaces.items.iter().any(|n| n.name == "cuadb_fixture"));
    assert!(
        catalog
            .list_relations("cuadb_fixture")
            .await
            .unwrap()
            .items
            .is_empty()
    );
    let hidden = catalog.describe_columns(&id("orders")).await;
    assert!(hidden.is_err());
    manager.close().await;
}
