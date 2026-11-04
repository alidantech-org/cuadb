---
{id: TSK-5FC58B7B6E27,name: "Discover accessible PostgreSQL schemas, tables and views",type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:04.875Z,updated_at: 2026-11-04T11:13:40.181Z,tags: [cuadb,postgres-catalog],depends_on: [TSK-55D20E7D39E9],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T09:49:34.759Z},{from: in-progress,to: completed,at: 2026-11-04T11:13:40.181Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-5FC58B7B6E27/1793785774763-start,started_at: 2026-11-04T09:49:34.790Z,end_ref: refs/tracking/TSK-5FC58B7B6E27/1793790820198-end,ended_at: 2026-11-04T11:13:40.182Z,summary: {files: 34,insertions: 3806,deletions: 10}}]}}}
---

# Discover accessible PostgreSQL schemas, tables and views

## Goal
Return a stable tree of visible schemas and supported relation types for the sidebar, including ordinary tables, views, materialized views and partitions. Hide noise by default while retaining a clear path to inspect relevant non-default schemas.

## Context
PostgreSQL URLs connect to one database, not the whole server. Catalog discovery should use pg_namespace, pg_class and ownership/privilege checks. Sorting and filtering should be stable; tables with unusual Unicode or mixed-case names must remain navigable without losing their exact identifier spelling.

This task belongs to **03 — PostgreSQL schemas, relations and metadata**. The group outcome is: Deliver complete, permission-aware catalog browsing for PostgreSQL and PostGIS installations. Users must find schemas, tables and views; inspect column definitions, indexes, keys, constraints and storage sizes; and understand when a statistic is approximate or unavailable. All catalog queries must be scoped, efficient and read-only.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] List accessible user schemas without assuming public is the only namespace.
- [x] Discover tables, views, materialized views and partitioned parents with correct types.
- [x] Sort and filter names safely, preserving exact identifiers and handling empty schemas.
- [x] Test denied permissions, quoted names, system schemas and table type distinctions.
- [x] Expose stable typed objects through the adapter instead of direct template SQL.

## UI-demo backend support

The real frontend must match the canonical committed `assets/cuadb-ui-demo.html`. This backend/release task must expose the safe metadata or data behavior required by the reference rather than forcing the frontend to fake its controls. The complete cross-layer fidelity contract is `.docs/docs/frontend-interactions.md`.

### Demo-related acceptance
- [x] Provide optional, capability-aware discovery for Advanced-mode read-only Functions, Procedures, Sequences, Types and Triggers tree groups and metadata-only detail screens from assets/cuadb-ui-demo.html; never execute functions or procedures.
- [x] Expose schema-qualified object names, definitions and counts only when visible to the connected database role, and provide a clear unavailable state on unsupported engines.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:13:39.967Z · PETER IRUNGU

Verified locally on 2026-11-04 against disposable PostgreSQL 18.6 using a dedicated non-superuser reader. cargo test --locked --test postgres_catalog passed, including mixed-case/Unicode identifiers, relation kinds, columns, constraints, expression indexes, partitions, statistics, advanced objects, denied permissions, and PostGIS metadata. bun run test also passed without opt-in database variables.
