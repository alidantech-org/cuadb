# MySQL and MariaDB support

CuaDB accepts one trusted `CUADB_DATABASE_URL` using `mysql://` or `mariadb://`. Loopback connections may omit a password and default to TLS disabled; remote connections require a password and default to certificate and hostname verification. Only an explicit `ssl-mode` URL option is accepted. The MariaDB scheme is normalized internally for SQLx while the connection status reports the detected server flavor.

The adapter applies bounded pool sizes and query deadlines, starts sessions in read-only transaction mode, quotes only catalog-discovered identifiers, and reads databases, tables, views, columns, indexes, constraints, rows and storage estimates from `information_schema`. Production credentials must still use a server-side SELECT-only role; session read-only mode is defense in depth. Row counts and storage values are engine-reported estimates and may be absent or stale. Views generally have no storage metrics. Keyset cursors and PostGIS values are not supported on MySQL/MariaDB. Large binary values and text are bounded for display.

## Validated versions and fixtures

Both engines are tested independently with a SELECT/SHOW VIEW-only reader account:

| Engine | Version validated | Env var | Fixture database |
|---|---|---|---|
| MariaDB | 11.8.9 | `CUADB_TEST_MARIADB_URL` | `cuadb_fixture_mariadb` |
| MySQL | 8.4.13 | `CUADB_TEST_MYSQL_URL` | `cuadb_fixture_mysql` |

Fixture SQL is additive and idempotent (`tests/fixtures/mysql-catalog.sql`, `tests/fixtures/mysql-rich.sql`; run as a fixture owner, never as root). Database names must start with `cuadb_fixture_`. Tests: `tests/mysql_catalog.rs` (catalog/rows contracts) and `tests/mysql_live_http.rs` (HTTP routes: failures, paging/sorting, rich values, literal filter, views/empty/MyISAM, hidden schemas). A test whose env var is unset returns early and is **not** evidence.

## Engine differences handled

- MySQL 8 returns `information_schema` column names in upper case and many text columns as `VARBINARY`; MariaDB returns lower case `VARCHAR`. The adapter reads either (`Field` in `src/db/mysql/catalog.rs`).
- `JSON` is native in MySQL and an alias for `LONGTEXT` in MariaDB; both display as text. Values come through `CAST(... AS CHAR)` truncated in SQL to the preview length.
- Binary values (`BLOB`, `VARBINARY`, `BIT`) are fetched as hex truncated in SQL (never the full blob) and shown as `\x…`.
- NULLs always sort last, matching PostgreSQL. Row filter is a literal substring (`LIKE` wildcards and `!` are escaped).
- Partition metadata comes from `information_schema.partitions`; ordinary tables return an empty list.
- Statistics (`table_rows`, sizes) are estimates, absent for views, and for MyISAM/InnoDB differ in accuracy. Keyset cursors and spatial types are unsupported: paging is offset-based (max 100 pages of 100 rows).

## Limitations

No fixture for an empty database or for an account that lacks all privileges exists; permission restriction was verified only for hidden system schemas (`mysql`, `sys`, `information_schema`, `performance_schema`). TLS-enabled connections, `ssl-mode` negotiation and MySQL 5.7 / older MariaDB were not exercised.
