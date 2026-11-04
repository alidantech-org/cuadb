---
{id: TSK-725618F4E684,name: Validate catalog coverage against PostgreSQL 18 and PostGIS,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:05.147Z,updated_at: 2026-11-04T11:13:42.263Z,tags: [cuadb,postgres-catalog],depends_on: [TSK-CFEA5F712D4B],order: 5120,extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T11:13:42.263Z}]}}
---

# Validate catalog coverage against PostgreSQL 18 and PostGIS

## Goal
Prove the catalog adapter is usable with the existing Fedora postgis/postgis:18-3.6 image and conventional PostgreSQL deployments. Record supported object behavior and failure modes without depending on a developer machine for ordinary unit tests.

## Context
The current target is PostgreSQL 18 with a PostGIS extension available, but compatibility must not depend on geography tables existing. Live integration tests should use opt-in fixture URLs and a dedicated disposable test schema or database. Never run destructive test setup on the user’s production database.

This task belongs to **03 — PostgreSQL schemas, relations and metadata**. The group outcome is: Deliver complete, permission-aware catalog browsing for PostgreSQL and PostGIS installations. Users must find schemas, tables and views; inspect column definitions, indexes, keys, constraints and storage sizes; and understand when a statistic is approximate or unavailable. All catalog queries must be scoped, efficient and read-only.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Create repeatable integration fixtures for tables, views, indexes, constraints and partitions.
- [x] Run opt-in tests with a disposable PostgreSQL instance and least-privilege reader.
- [x] Verify catalog behavior on a PostGIS-enabled database and one without extensions.
- [x] Document version assumptions, unsupported objects and permission-dependent outcomes.
- [x] Keep local unit tests passing when PostgreSQL is not installed.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:13:42.096Z · PETER IRUNGU

Verified locally on 2026-11-04 against disposable PostgreSQL 18.6 using a dedicated non-superuser reader. cargo test --locked --test postgres_catalog passed, including mixed-case/Unicode identifiers, relation kinds, columns, constraints, expression indexes, partitions, statistics, advanced objects, denied permissions, and PostGIS metadata. bun run test also passed without opt-in database variables.
