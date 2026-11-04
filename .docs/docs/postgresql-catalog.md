---
{id: DOC-POSTGRESQL-CATALOG,name: PostgreSQL catalog discovery,type: documentation}
---

# PostgreSQL catalog discovery (Group 03)

CuaDB uses **fixed, read-only pg_catalog queries** over the single
operator-configured SQLx pool. It never accepts raw SQL, user-supplied connection
targets, or unquoted SQL identifiers. Schema and relation names are bound
parameters, and HTML fragments are auto-escaped by Askama. The app still does
not fetch table **rows**: row browsing belongs to the next roadmap group.

## Available metadata

- `GET /catalog/schemas` lists accessible non-system schemas, including empty
  schemas. A connected role must have schema `USAGE`.
- `GET /catalog/relations?schema=...` lists SELECT-visible tables, partition
  children/parents, foreign tables, views and materialized views. Objects are
  sorted by exact case-sensitive PostgreSQL identifiers.
- `GET /catalog/structure?schema=...&relation=...` describes ordered columns,
  native types (including PostGIS), nullability, defaults, identities, generated
  columns, PK/unique membership, domain and enum labels, constraints and visible
  parent/child partition edges.
- `GET /catalog/indexes?schema=...&relation=...` shows unique, PK, partial,
  expression and other index definitions and validity flags.
- `GET /catalog/stats?schema=...&relation=...` reports actual PostgreSQL
  **bytes** from `pg_table_size`, `pg_indexes_size` and
  `pg_total_relation_size`, and **approximate** `pg_class.reltuples` (never
  `COUNT(*)`). Views, partitioned parents and foreign tables return explicit
  unavailable values rather than deceptive zeros. Partition leaf storage is
  reported individually; the parent does not silently sum children.
- `GET /catalog/advanced?schema=...` lists visible routines (definitions
  clipped to 4096 characters), procedures, sequences, user types and
  non-internal triggers; no execution or `nextval` occurs. Definitions
  unavailable to the adapter are marked as such. PostgreSQL privilege visibility
  is not synonymous with permission to SELECT all data.
- `GET /catalog/overview` shows installed extensions, catalog-based **estimated**
  storage (`relpages × block_size`) by schema and largest visible relations, and
  bounded visible-object counts. Values are explicitly approximate; they are
  not equivalent to the exact per-relation size endpoint.

The current shell includes a small HTMX catalog panel for real metadata
inspection. Final Simple/Advanced layout parity with
`assets/cuadb-ui-demo.html` remains owned by later frontend roadmap tasks;
do not mistake these minimal fragments for the final reference UI.

Every list has a fixed cap (schemas 300, relations/advanced 500,
columns 512, keys/indexes/partitions 300, overview 20) and marks truncation
instead of silently suggesting completeness. `/catalog/overview` caps its
visible relation count to 5000. Every query uses the configured
`CUADB_QUERY_TIMEOUT_MS` deadline (default 15000 ms) and the shared bounded
SQLx pool; no full-table data scans or background fan-out occur. HTTP requests
retain the existing concurrency and 30-second global deadline.

Known limitations: catalog visibility and PostGIS type formats vary by role;
schema USAGE or table SELECT does not necessarily grant access to every
dependent reference. Inaccessible related tables are not linked by name.
Foreign tables lack local storage statistics. Materialized views with
`WITH NO DATA` may have no usable statistics. Unanalyzed objects can have
unavailable or stale `reltuples`. Function definitions are display-only and
may be truncated. Large catalogs are capped rather than silently clipped;
server-side search/pagination are future extensions.

## Opt-in fixture testing: PostgreSQL 18 and PostGIS 3.6

**NEVER run fixture setup against an existing personal or production
database.** The SQL files contain DROP SCHEMA, CREATE TABLE and INSERT.
They enforce a dedicated database-name prefix `cuadb_fixture_` and must be
run by an operator with a separate disposable database. The application and
Rust integration tests themselves issue **SELECT queries only**.

Example using locally available PostgreSQL administration tools:

```bash
# As a fixture DB administrator; never replace this with a personal database.
createdb -h 127.0.0.1 -U postgres cuadb_fixture_catalog

# The script refuses databases whose name does not start cuadb_fixture_.
psql -X -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres \
  -d cuadb_fixture_catalog -f tests/fixtures/catalog.sql

# Optional only on the disposable PostGIS instance:
psql -X -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres \
  -d cuadb_fixture_catalog -f tests/fixtures/catalog_postgis.sql
```

For best security, provision a fixture reader role and explicit grants. Run
these **manually, as the fixture administrator** after setup, adapting your
deployment's authentication policy:

```sql
CREATE ROLE cuadb_fixture_reader LOGIN PASSWORD 'A_UNIQUE_FIXTURE_PASSWORD';
GRANT CONNECT ON DATABASE cuadb_fixture_catalog TO cuadb_fixture_reader;
GRANT USAGE ON SCHEMA cuadb_fixture, "CuaDB Mixed" TO cuadb_fixture_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA cuadb_fixture, "CuaDB Mixed"
  TO cuadb_fixture_reader;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA cuadb_fixture
  TO cuadb_fixture_reader;
GRANT USAGE ON TYPE cuadb_fixture.review_state, cuadb_fixture.nonnegative
  TO cuadb_fixture_reader;
```

Grant access to `places` after applying the PostGIS fixture. Do not grant
SUPERUSER, CREATE, or DELETE capabilities to the viewer. Materialized-view
access and routine EXECUTE permissions depend on your fixture roles; grant
them only on the disposable fixture if you want those nodes visible.

Run opt-in Rust tests **after** creating fixtures. The URL must name a
database starting with `cuadb_fixture_`; tests enforce this prefix.
The PostGIS and denied-permission tests run only if their separate optional\nvariables are set. Create the denied-permission role with CONNECT to the\nfixture database but without schema USAGE or table SELECT; point\n\`CUADB_TEST_CATALOG_DENIED_URL\` at that role.

```bash
export CUADB_TEST_CATALOG_URL='postgresql://cuadb_fixture_reader:...@127.0.0.1:5432/cuadb_fixture_catalog?sslmode=disable'
cargo test --locked --test postgres_catalog -- --nocapture

# Optional: after loading PostGIS schema and adjusting grants
export CUADB_TEST_CATALOG_POSTGIS_URL="$CUADB_TEST_CATALOG_URL"
cargo test --locked --test postgres_catalog disposable_postgis_metadata_fixture
```

If only the existing PostgreSQL/PostGIS container is available, create a
**separate disposable database there**; do not use your existing
`postgres` database or run fixture DDL from CuaDB. The tests check
mixed-case/Unicode identifiers, a domain/enum, identity/generated columns,
composite PK/FK, expression and partial indexes, schema visibility,
partitions, empty/unavailable statistics, and read-only advanced metadata.

Ordinary commands (`bun run check`, `bun run test`, `bun run smoke`) remain
independent of running PostgreSQL unless fixture-specific environment
variables are deliberately set. The standard smoke test still checks server
liveness and disconnected status.
