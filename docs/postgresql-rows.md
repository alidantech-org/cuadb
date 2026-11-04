# PostgreSQL row paging (Group 04)

This milestone adds a **read-only, bounded HTML data page** at
`GET /catalog/data`, using permission-scoped metadata and the existing
operator-configured SQLx pool. There is no SQL editor or write endpoint.
The final grid and inspector layout remains governed by
`assets/cuadb-ui-demo.html` in later frontend tasks.

## Request examples

```text
/catalog/data?schema=public&relation=orders
/catalog/data?schema=public&relation=orders&size=50&page=1
/catalog/data?schema=public&relation=orders&sort=id&direction=desc
/catalog/data?schema=public&relation=orders&column=id&column=name
```

Exact-case schema and relation names are required. Sorting needs both
`sort` and `direction=asc|desc`. Repeated `column` selects a subset,
not arbitrary SQL. Size defaults to 25 and is capped at 100, with at most
24 selected columns and 256 bytes per rendered cell.

An optional `filter` is capped at 128 bytes and bound as a value. PostgreSQL
matches it against the text forms of catalogued columns before paging. This
may scan the relation, so the configured statement deadline remains in force.
Filtered row-count estimates are reported unavailable instead of reusing the
whole-relation estimate. Filtering is rejected when column metadata is
truncated, avoiding a misleading partial search.

When PostGIS geometry or geography columns are present, their native type
label includes the declared subtype and SRID and the grid uses bounded WKT
previews. An explicit **Show GeoJSON details** control requests GeoJSON only
for the visible page; each inspector value is capped at 1024 characters and
is not reprojected. Plain PostgreSQL databases omit the control. This does
not perform map rendering or bulk geometry conversion.

The catalog endpoints are `GET /catalog/schemas`, `/catalog/relations?schema=…`,
`/catalog/structure?schema=…&relation=…`, `/catalog/indexes`, `/catalog/stats`,
`/catalog/advanced?schema=…`, and `/catalog/overview`. Query strings are
deserialized into closed typed request models; repeated scalar fields and
unknown parameters are rejected. These routes currently render HTML for both
direct navigation and HTMX swaps. `/catalog/data` supports repeated `column`
parameters and bounded sort/page state; no request accepts a database URL or SQL.

PostgreSQL converts each selected cell to a maximum 257-character text
preview before network transfer; Rust further limits its UTF-8 byte length.
NULL, empty and truncated data have separate representations. Numeric,
boolean, UUID, timestamp, bytea, JSON and extension values carry type labels.
Large values are never silently presented as complete. Unknown conversions
use a safe type-marked fallback rather than a panic.

The application checks schema USAGE and relation SELECT privileges first,
then matches column names exactly against the live catalog. PostgreSQL
identifier quoting applies only to that validated allowlist. Values, cursor
keys, page limits and offsets use SQL bind parameters. No browser value can
become a raw SQL statement or a new connection target.

## Sorting and pagination

Numbered pages use a maximum of 10,000 OFFSET rows. A non-partial,
single-column unique NOT NULL key provides stable tie-breaking.
Integer keys additionally permit next/previous indexed keyset navigation.
Server-issued 256-bit random cursors expire after five minutes and are
validated against the exact relation, column selection, sort, page size and
filter.
A bounded in-memory registry holds up to 1024 tokens; restarting the server
invalidates tokens. Cursor values contain no database keys or cell values.

Other sorts use bounded OFFSET. NULLS LAST is explicit. Sorting JSON,
geometry, arrays and other unsupported types is rejected. Without a usable
unique key the UI warns that ordering is only best-effort. Concurrent writes
may still change row placement across requests; CuaDB does not claim
cross-request snapshot isolation or run an automatic COUNT(*).
Estimated totals are marked approximate, and values from PostGIS and
unknown types may be displayed as text previews only.

## Export boundary and supported scope

The `src/db/exports.rs` contract defines strict caps (10,000 rows,
16 MiB, eight linked relations, depth two) for future CSV, JSON and
related-table ZIP exports. **No whole-table or ZIP export endpoint is
enabled in this wave.** A later streaming implementation must first pass
the same catalog allowlists, permissions, timeouts, cancellation and
response-size rules. It must not collect a whole database table in memory.

## Opt-in fixture checks

Only a disposable database whose name begins `cuadb_fixture_` may be used.
First apply `tests/fixtures/catalog.sql`, then `tests/fixtures/rows.sql`
in that **separate fixture database** as an administrator. The SQL is
destructive and must never run against the existing `postgres` database.
Grant SELECT on the fixture tables to a dedicated non-admin reader.

```bash
# Fixture setup is manual and destructive to the disposable test database.
export CUADB_TEST_ROWS_URL='postgresql://cuadb_fixture_reader:...@127.0.0.1:5432/cuadb_fixture_catalog?sslmode=disable'
cargo test --locked --test postgres_rows
```

The Rust integration tests execute SELECT only, including empty, single,
50-row and 120-row navigation, previous/next cursors, NULLs, mixed sort
values, large Unicode, numerics and bytea. Normal unit and smoke tests
never require PostgreSQL. To verify the HTML fragment locally, run the
server with a reader URL and open a permitted relation using
`/catalog/data?schema=cuadb_fixture&relation=paging`.

The implementation was verified locally against PostgreSQL 18.6 with a
dedicated non-superuser reader. The same disposable database was also tested
with PostGIS enabled. Permission-denied catalog access, cursor tampering and
live HTTP next-page navigation were exercised separately.
