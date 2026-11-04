//! Read-only row paging regression tests. Live tests require a disposable
//! cuadb_fixture_* database prepared by tests/fixtures/catalog.sql and rows.sql.
use cuadb::{
    config::Config,
    db::{
        postgres::PgManager,
        types::{Availability, Cell, RelationId, RowOptions, SortDirection, SortSpec},
    },
};
fn relation(name:&str)->RelationId {
    RelationId {namespace:"cuadb_fixture".into(),name:name.into()}
}
#[tokio::test]
async fn fixture_pages_have_exact_bounds_and_cursor_navigation() {
    let Ok(url)=std::env::var("CUADB_TEST_ROWS_URL") else {return;};
    let parsed=url::Url::parse(&url).unwrap();
    assert!(parsed.path().trim_start_matches('/').starts_with("cuadb_fixture_"));
    let config=Config::from_values(&[]).unwrap();
    let manager=PgManager::new(&url,&config.limits).unwrap();
    let catalog=manager.catalog();
    let empty=catalog.fetch_rows(&relation("empty_paging"),RowOptions::default())
        .await.unwrap();
    assert!(empty.rows.is_empty());
    assert!(!empty.has_more);
    let single=catalog.fetch_rows(&relation("single_paging"),RowOptions::default())
        .await.unwrap();
    assert_eq!(single.rows.len(),1);
    assert!(!single.has_more);
    let mut request=RowOptions {limit:50,..RowOptions::default()};
    let first=catalog.fetch_rows(&relation("paging"),request.clone()).await.unwrap();
    assert_eq!(first.rows.len(),50);
    assert!(first.has_more);
    assert!(first.keyset_enabled);
    assert!(matches!(first.rows[0][0],Cell::Integer(1)));
    assert!(first.previews[0][2].truncated);
    assert!(matches!(first.approximate_total,Availability::Available(_)));
    request.cursor=first.next_cursor.clone();
    let second=catalog.fetch_rows(&relation("paging"),request.clone()).await.unwrap();
    assert_eq!(second.rows.len(),50);
    assert_eq!(second.page,1);
    assert!(second.has_previous);
    assert!(matches!(second.rows[0][0],Cell::Integer(51)));
    request.cursor=second.next_cursor.clone();
    let third=catalog.fetch_rows(&relation("paging"),request.clone()).await.unwrap();
    assert_eq!(third.rows.len(),20);
    assert!(!third.has_more);
    assert!(matches!(third.rows[0][0],Cell::Integer(101)));
    request.cursor=third.previous_cursor.clone();
    let previous=catalog.fetch_rows(&relation("paging"),request.clone()).await.unwrap();
    assert_eq!(previous.rows.len(),50);
    assert!(matches!(previous.rows[0][0],Cell::Integer(51)));
    let mut sort=RowOptions::default();
    sort.sort=Some(SortSpec {
        column:"nullable_text".into(),direction:SortDirection::Asc,
    });
    let sorted=catalog.fetch_rows(&relation("paging"),sort.clone()).await.unwrap();
    assert!(sorted.stable_order);
    assert!(!sorted.keyset_enabled);
    sort.sort.as_mut().unwrap().column="not_a_column".into();
    assert!(catalog.fetch_rows(&relation("paging"),sort).await.is_err());

    let mut modified=RowOptions::default();
    modified.cursor=first.next_cursor.clone();
    assert!(catalog.fetch_rows(&relation("single_paging"),modified).await.is_err());

    let invalid=RowOptions {limit:101,..RowOptions::default()};
    assert!(catalog.fetch_rows(&relation("paging"),invalid).await.is_err());
    manager.close().await;
}
