---
{id: TSK-CFEA5F712D4B,name: Implement efficient PostgreSQL table size and statistics inspection,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:05.080Z,updated_at: 2026-11-04T11:13:41.836Z,tags: [cuadb,postgres-catalog],depends_on: [TSK-047C7016EEAA],order: 4096,extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T11:13:41.836Z}]}}
---

# Implement efficient PostgreSQL table size and statistics inspection

## Goal
Expose total relation storage, table bytes, index bytes and estimated rows without running expensive full scans. Make estimates clearly distinguishable from exact measurements and avoid misleading totals for partitioned or foreign tables.

## Context
PostgreSQL provides pg_total_relation_size, pg_table_size, pg_indexes_size and pg_stat* catalog views. Partitioned parents, empty tables and un-analyzed relations need special handling. Displaying estimated row counts from catalog statistics is preferable to forcing COUNT(*) on large production datasets.

This task belongs to **03 — PostgreSQL schemas, relations and metadata**. The group outcome is: Deliver complete, permission-aware catalog browsing for PostgreSQL and PostGIS installations. Users must find schemas, tables and views; inspect column definitions, indexes, keys, constraints and storage sizes; and understand when a statistic is approximate or unavailable. All catalog queries must be scoped, efficient and read-only.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Report available table, index and total size metrics with correct units and nullability.
- [x] Return estimated row count with an explicit approximate indicator.
- [x] Handle partitions, materialized views and foreign relations without false totals.
- [x] Avoid automatic exact row counts and bound metadata query execution times.
- [x] Verify expected metrics on populated, empty and unanalyzed fixture tables.

## UI-demo backend support

The real frontend must match the canonical committed `assets/cuadb-ui-demo.html`. This backend/release task must expose the safe metadata or data behavior required by the reference rather than forcing the frontend to fake its controls. The complete cross-layer fidelity contract is `.docs/docs/frontend-interactions.md`.

### Demo-related acceptance
- [x] Supply the demo's read-only Advanced Overview with extension list, size-by-schema, largest relation sizes and aggregate object counts using bounded PostgreSQL catalog queries.
- [x] Mark missing or estimated values honestly, including partition and view edge cases, instead of copying the synthetic figures in assets/cuadb-ui-demo.html.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:13:41.661Z · PETER IRUNGU

Verified locally on 2026-11-04 against disposable PostgreSQL 18.6 using a dedicated non-superuser reader. cargo test --locked --test postgres_catalog passed, including mixed-case/Unicode identifiers, relation kinds, columns, constraints, expression indexes, partitions, statistics, advanced objects, denied permissions, and PostGIS metadata. bun run test also passed without opt-in database variables.
