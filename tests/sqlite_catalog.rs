use cuadb::{
    config::Config,
    db::{
        adapter::DatabaseAdapter,
        sqlite::{SqliteManager, resolve_path},
        types::*,
    },
};
use std::path::Path;
const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sqlite-data");
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/sqlite-data/cuadb_fixture_sqlite.db"
);
#[test]
fn sqlite_paths_are_canonical_and_confined() {
    let root = Path::new(FIXTURE_DIR);
    assert!(
        resolve_path(
            &format!("sqlite:///{}", FIXTURE.trim_start_matches('/')),
            root
        )
        .is_ok()
    );
    assert!(resolve_path("sqlite:////etc/passwd", root).is_err());
    assert!(resolve_path("sqlite:////tmp/missing.db", root).is_err());
}
#[tokio::test]
async fn sqlite_fixture_catalog_rows_and_read_only() {
    let config = Config::from_values(&[]).unwrap();
    let root = Path::new(FIXTURE_DIR);
    let url = format!("sqlite:///{}", FIXTURE.trim_start_matches('/'));
    let manager = SqliteManager::new(&url, root, &config.limits).unwrap();
    assert_eq!(manager.snapshot().await.status, ConnectionStatus::Ready);
    let c = manager.catalog();
    assert_eq!(c.engine(), Engine::Sqlite);
    assert!(!c.supports(Capability::StorageSizes));
    let rel = c.list_relations("main").await.unwrap();
    assert!(
        rel.items
            .iter()
            .any(|x| x.id.name == "item_names" && x.kind == RelationKind::View)
    );
    let id = RelationId {
        namespace: "main".into(),
        name: "items".into(),
    };
    let cols = c.describe_columns(&id).await.unwrap();
    assert!(
        cols.items
            .iter()
            .any(|x| x.column.name == "id" && x.column.primary_key)
    );
    assert_eq!(c.list_indexes(&id).await.unwrap().items.len(), 1);
    assert_eq!(
        c.list_constraints(&RelationId {
            namespace: "main".into(),
            name: "child".into()
        })
        .await
        .unwrap()
        .items
        .len(),
        1
    );
    let page = c
        .fetch_rows(
            &id,
            RowOptions {
                limit: 1,
                ..RowOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert!(page.has_more);
    assert!(matches!(page.rows[0][0], Cell::Integer(1)));
    assert!(matches!(page.rows[0][4], Cell::Bytes(ref value) if value == &[0, 255]));
    assert!(matches!(
        c.statistics(&id).await.unwrap().table_bytes,
        Availability::Unavailable
    ));
    assert_eq!(
        c.fetch_rows(
            &id,
            RowOptions {
                limit: 1,
                sort: Some(SortSpec {
                    column: "nope".into(),
                    direction: SortDirection::Asc
                }),
                ..RowOptions::default()
            }
        )
        .await
        .unwrap_err()
        .kind,
        DbErrorKind::InvalidIdentifier
    );
    manager.close().await;
}
