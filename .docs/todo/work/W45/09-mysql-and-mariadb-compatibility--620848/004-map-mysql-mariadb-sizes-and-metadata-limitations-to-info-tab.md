---
{id: TSK-5689F3668ED7,name: Map MySQL/MariaDB sizes and metadata limitations to Info tab,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:07.816Z,updated_at: 2026-11-04T14:05:35.463Z,tags: [cuadb,mysql],depends_on: [TSK-242B6B658703,TSK-0AA246B508ED],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T12:46:13.382Z},{from: in-progress,to: completed,at: 2026-11-04T14:05:35.463Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-5689F3668ED7/1793796373384-start,started_at: 2026-11-04T12:46:13.411Z,end_ref: refs/tracking/TSK-5689F3668ED7/1793801135465-end,ended_at: 2026-11-04T14:05:35.463Z,summary: {files: 32,insertions: 2403,deletions: 73}}]}}}
---

# Map MySQL/MariaDB sizes and metadata limitations to Info tab

## Goal
Display accurate available metadata such as estimated rows, storage and index bytes while making engine-specific limitations visible. The UI should not pretend exact PostgreSQL-equivalent statistics are always available.

## Context
Storage engines and server versions can report estimates differently, and INFORMATION_SCHEMA values may be stale or missing. Table and index sizes must be labeled according to the underlying engine semantics. The core capability interface should make these distinctions explicit without special-casing templates.

This task belongs to **09 — MySQL and MariaDB compatibility**. The group outcome is: Bring MySQL and MariaDB into CuaDB through a separate adapter while preserving the same read-only navigation and data-grid contract. Reuse common models and browser components, but implement engine-specific discovery, identifier quoting, data decoding and statistics rather than assuming PostgreSQL SQL syntax works everywhere.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Read supported table and index size metrics without costly full-table scans.
- [x] Label approximate row counts and unsupported storage values honestly.
- [x] Verify behavior with views, empty tables and different storage engines where available.
- [x] Keep the Structure and Info tabs reusable across PostgreSQL and MySQL adapters.
- [x] Document MySQL/MariaDB-specific caveats and validated database versions.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:20:01.631Z · PETER IRUNGU

Implemented INFORMATION_SCHEMA TABLE_ROWS, DATA_LENGTH and INDEX_LENGTH metrics and common availability-aware statistics display. Values are engine-reported estimates that may be stale or absent; no full scans. MariaDB fixture verified estimated rows. Caveats added to docs/mysql-mariadb.md; MySQL 8 and alternate storage engines remain unverified.

### 2026-11-04T13:46:53.624Z · PETER IRUNGU

Live finding (MySQL 8.4.13 vs MariaDB 11.8.9): MySQL reports information_schema column names upper-case and text as VARBINARY, which made every catalog call fail with Internal on MySQL. Fixed in src/db/mysql/catalog.rs (Field trait with case/varbinary fallback). tests/mysql_catalog.rs now runs independent live tests for MariaDB (CUADB_TEST_MARIADB_URL) and MySQL (CUADB_TEST_MYSQL_URL): both pass against the cuadb_fixture_* databases. A second Mixed_Items row was added via the operator account (idempotent insert) so has_more paging is exercised.

### 2026-11-04T14:04:42.372Z · PETER IRUNGU

Item 3 verified on both engines: view structure, empty InnoDB table, MyISAM statistics, PRIMARY index and FK constraint rendering. See docs/mysql-mariadb.md for caveats and validated versions.
