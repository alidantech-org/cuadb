# Running the tests

Everything under `cargo test` runs offline except the live-database tests, which **return early (and so prove nothing)** when their `CUADB_TEST_*_URL` is unset. Always check that the engines you care about were actually exercised.

```bash
cargo test --locked                      # offline + whatever env vars are set
bun run format:check && bun run check && bun run test
task validate && bun run task:verify
```

## Live fixtures

Use dedicated databases whose names start with `cuadb_fixture_` and a SELECT-only reader account. Fixture SQL is additive and idempotent and is run by a separate fixture *operator* account, never by the reader and never by root:

| Engine | Fixture SQL | Reader URL variable |
|---|---|---|
| PostgreSQL 18 + PostGIS | `tests/fixtures/catalog.sql` (schema is **destructive**: it drops/recreates `cuadb_fixture`, run only on a fresh fixture DB), `rows.sql`, `catalog_postgis.sql`, `rich_values.sql`, `perf.sql` | `CUADB_TEST_CATALOG_URL`, `CUADB_TEST_ROWS_URL`, `CUADB_TEST_CATALOG_POSTGIS_URL`, `CUADB_TEST_LOCAL_TRUST_URL` |
| PostgreSQL without PostGIS | `tests/fixtures/plain_postgres.sql` (run in a separate `cuadb_fixture_*` database with no PostGIS extension) | `CUADB_TEST_CATALOG_PLAIN_URL` |
| MariaDB 11.8 | `tests/fixtures/mysql-catalog.sql`, `mysql-rich.sql`, `perf-mysql.sql` | `CUADB_TEST_MARIADB_URL` |
| MySQL 8.4 | same files | `CUADB_TEST_MYSQL_URL` |
| PostgreSQL writes (SQL workspace) | `tests/fixtures/rw_scratch.sql` (additive scratch schema `cuadb_rw_scratch`, owned by the fixture operator; tests insert/delete only their own rows) | `CUADB_TEST_PG_WRITER_URL` (the operator URL, never a reader) |
| SQLite | generated per test under `target/tmp`; committed file `tests/fixtures/sqlite-data/` | none |

Do **not** set `CUADB_TEST_POSTGRES_URL` to a trust-authenticated server: that test expects a wrong password to be rejected. For `CUADB_TEST_CATALOG_DENIED_URL`, use a separate disposable database, run `tests/fixtures/catalog_denied.sql` as its operator, grant the restricted reader CONNECT on the database only, and leave the `cuadb_fixture` schema ungranted. The normal catalog fixture includes a `file_fdw` foreign table used only for metadata and storage-availability checks; the CuaDB reader does not need server-file access.

Example (local containers, passwordless loopback readers):

```bash
export CUADB_TEST_CATALOG_URL='postgresql://cuadb_fixture_reader@127.0.0.1:5432/cuadb_fixture_catalog'
export CUADB_TEST_ROWS_URL="$CUADB_TEST_CATALOG_URL" CUADB_TEST_CATALOG_POSTGIS_URL="$CUADB_TEST_CATALOG_URL" CUADB_TEST_LOCAL_TRUST_URL="$CUADB_TEST_CATALOG_URL"
export CUADB_TEST_MARIADB_URL='mariadb://cuadb_reader@127.0.0.1:3306/cuadb_fixture_mariadb'
export CUADB_TEST_MYSQL_URL='mysql://cuadb_reader@127.0.0.1:3307/cuadb_fixture_mysql'
```

Cleanup: nothing needs cleanup; the tests never write to databases. Fixture tables are left in place. Temporary SQLite files live in `target/tmp` (removed by `cargo clean`). The 1M-row tables take a few hundred MB; drop them manually as the fixture owner if you no longer need `tests/performance_live.rs`.

## Test map
`router_fake_adapter` (router with an in-memory adapter), `adapter_contract` (one contract across four engines), `*_live_http` and `rich_live_http` (real routes against live engines), `sqlite_live`, `profiles_http`, `security_negative`, `performance_live`, `export_live`.

## Docker verification

Manual; results and commands are in [docker.md](docker.md).
