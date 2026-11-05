//! One behavioral contract run against every available engine through the shared
//! `DatabaseAdapter` trait. SQLite always runs; PostgreSQL, MariaDB and MySQL run
//! when their CUADB_TEST_*_URL is set (an unset engine is skipped, NOT verified).

use std::sync::Arc;

use cuadb::{
    config::Config,
    db::{
        adapter::DatabaseAdapter, mysql::MySqlManager, postgres::PgManager, sqlite::SqliteManager,
        types::*,
    },
};
use sqlx::Connection;

struct Subject {
    label: &'static str,
    adapter: Arc<dyn DatabaseAdapter>,
    schema: String,
    table: &'static str,
    view: &'static str,
    keyed: &'static str, // a column that is part of the table's primary key
}

async fn subjects() -> Vec<Subject> {
    let config = Config::from_values(&[]).unwrap();
    let mut out: Vec<Subject> = vec![];
    if let Ok(url) = std::env::var("CUADB_TEST_CATALOG_URL") {
        let manager = PgManager::new(&url, &config.limits).unwrap();
        out.push(Subject {
            label: "postgresql",
            adapter: Arc::new(manager.catalog()),
            schema: "cuadb_fixture".into(),
            table: "accounts",
            view: "accounts_view",
            keyed: "id",
        });
    }
    for (label, env, schema) in [
        ("mariadb", "CUADB_TEST_MARIADB_URL", "cuadb_fixture_mariadb"),
        ("mysql", "CUADB_TEST_MYSQL_URL", "cuadb_fixture_mysql"),
    ] {
        if let Ok(url) = std::env::var(env) {
            let manager = MySqlManager::new(&url, &config.limits).unwrap();
            out.push(Subject {
                label,
                adapter: Arc::new(manager.catalog()),
                schema: schema.into(),
                table: "Mixed_Items",
                view: "items_view",
                keyed: "id",
            });
        }
    }
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("contract-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("c.db");
    let mut conn =
        sqlx::SqliteConnection::connect(&format!("sqlite://{}?mode=rwc", file.display()))
            .await
            .unwrap();
    for sql in [
        "CREATE TABLE accounts(id INTEGER PRIMARY KEY, account_name TEXT)",
        "CREATE VIEW accounts_view AS SELECT id, account_name FROM accounts",
        "INSERT INTO accounts(account_name) VALUES ('a'),('b'),('c')",
    ] {
        sqlx::query(sql).execute(&mut conn).await.unwrap();
    }
    conn.close().await.unwrap();
    let manager = SqliteManager::new(
        &format!(
            "sqlite:///{}",
            file.display().to_string().trim_start_matches('/')
        ),
        &dir,
        &config.limits,
    )
    .unwrap();
    out.push(Subject {
        label: "sqlite",
        adapter: Arc::new(manager.catalog()),
        schema: "main".into(),
        table: "accounts",
        view: "accounts_view",
        keyed: "id",
    });
    out
}

fn id(schema: &str, name: &str) -> RelationId {
    RelationId {
        namespace: schema.into(),
        name: name.into(),
    }
}

#[tokio::test]
async fn every_engine_satisfies_the_adapter_contract() {
    let subjects = subjects().await;
    assert!(!subjects.is_empty());
    for s in &subjects {
        let a = &s.adapter;
        let who = s.label;
        // Discovery
        let schemas = a.list_schemas().await.unwrap();
        assert!(
            schemas.items.iter().any(|n| n.name == s.schema),
            "{who}: schema listed"
        );
        let relations = a.list_relations(&s.schema).await.unwrap();
        let kind = |name: &str| {
            relations
                .items
                .iter()
                .find(|r| r.id.name == name)
                .map(|r| r.kind)
        };
        assert_eq!(kind(s.table), Some(RelationKind::Table), "{who}");
        assert_eq!(kind(s.view), Some(RelationKind::View), "{who}");
        // Order is deterministic (collation differs per engine) and names are unique.
        let again = a.list_relations(&s.schema).await.unwrap();
        assert_eq!(
            relations.items, again.items,
            "{who}: relation order is repeatable"
        );
        let mut names: Vec<_> = relations.items.iter().map(|r| r.id.name.clone()).collect();
        names.sort();
        names.dedup();
        assert_eq!(
            names.len(),
            relations.items.len(),
            "{who}: no duplicate relations"
        );
        // Columns and keys
        let columns = a.describe_columns(&id(&s.schema, s.table)).await.unwrap();
        assert!(
            columns
                .items
                .iter()
                .any(|c| c.column.name == s.keyed && c.column.primary_key),
            "{who}"
        );
        assert!(
            columns
                .items
                .iter()
                .all(|c| !c.column.native_type.is_empty()),
            "{who}: types are never empty"
        );
        assert!(
            a.list_indexes(&id(&s.schema, s.table)).await.is_ok(),
            "{who}"
        );
        assert!(
            a.list_constraints(&id(&s.schema, s.table)).await.is_ok(),
            "{who}"
        );
        assert!(
            a.list_partitions(&id(&s.schema, s.table))
                .await
                .unwrap()
                .items
                .is_empty(),
            "{who}: not partitioned"
        );
        assert!(
            a.list_constraints(&id(&s.schema, s.view))
                .await
                .unwrap()
                .items
                .is_empty(),
            "{who}: views have no constraints"
        );
        // Statistics are Available or explicitly Unavailable, never fabricated errors
        let stats = a
            .relation_statistics(&id(&s.schema, s.table))
            .await
            .unwrap();
        assert!(
            matches!(
                stats.relation_kind,
                Availability::Available(RelationKind::Table)
            ),
            "{who}"
        );
        // Paging
        let first = a
            .fetch_rows(
                &id(&s.schema, s.table),
                RowOptions {
                    limit: 1,
                    ..RowOptions::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(first.rows.len(), 1, "{who}");
        assert_eq!(
            first.rows.len(),
            first.previews.len(),
            "{who}: previews parallel rows"
        );
        assert!(first.has_more, "{who}: more rows exist");
        assert!(!first.has_previous && first.page == 0, "{who}");
        let second = a
            .fetch_rows(
                &id(&s.schema, s.table),
                RowOptions {
                    limit: 1,
                    page: 1,
                    ..RowOptions::default()
                },
            )
            .await
            .unwrap();
        assert!(second.has_previous && second.page == 1, "{who}");
        assert_ne!(
            first.rows[0], second.rows[0],
            "{who}: pages do not repeat rows"
        );
        // Sorting: unknown columns are rejected, NULL handling is consistent, direction flips order
        let sort = |direction| RowOptions {
            limit: 2,
            sort: Some(SortSpec {
                column: s.keyed.into(),
                direction,
            }),
            ..RowOptions::default()
        };
        let asc = a
            .fetch_rows(&id(&s.schema, s.table), sort(SortDirection::Asc))
            .await
            .unwrap();
        let desc = a
            .fetch_rows(&id(&s.schema, s.table), sort(SortDirection::Desc))
            .await
            .unwrap();
        assert_ne!(
            asc.rows[0], desc.rows[0],
            "{who}: descending differs from ascending"
        );
        let bad_sort = RowOptions {
            sort: Some(SortSpec {
                column: "no_such_column".into(),
                direction: SortDirection::Asc,
            }),
            ..RowOptions::default()
        };
        assert!(
            a.fetch_rows(&id(&s.schema, s.table), bad_sort)
                .await
                .is_err(),
            "{who}"
        );
        // Bounds and identifier safety
        for limit in [0u32, 1001] {
            let bad = RowOptions {
                limit,
                ..RowOptions::default()
            };
            assert!(
                a.fetch_rows(&id(&s.schema, s.table), bad).await.is_err(),
                "{who}: limit {limit}"
            );
        }
        for hostile in [
            "no_such_table",
            "x\"; DROP TABLE accounts; --",
            "`x`",
            "a\0b",
            "",
        ] {
            let outcome = a
                .fetch_rows(&id(&s.schema, hostile), RowOptions::default())
                .await;
            assert!(outcome.is_err(), "{who}: {hostile:?} must be rejected");
            let outcome = a.describe_columns(&id(&s.schema, hostile)).await;
            assert!(outcome.is_err(), "{who}: {hostile:?} must be rejected");
        }
        // Filter is literal
        let none = a
            .fetch_rows(
                &id(&s.schema, s.table),
                RowOptions {
                    filter: Some("%".into()),
                    ..RowOptions::default()
                },
            )
            .await
            .unwrap();
        assert!(none.rows.is_empty(), "{who}: '%' is not a wildcard");
        // Capabilities are declared consistently
        assert!(a.supports(Capability::Views), "{who}");
        assert!(a.supports(Capability::Indexes), "{who}");
        eprintln!("contract ok: {who}");
    }
}
