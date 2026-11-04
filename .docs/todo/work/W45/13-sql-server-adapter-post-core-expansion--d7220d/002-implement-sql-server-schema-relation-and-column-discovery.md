---
{id: TSK-C00E0A305361,name: "Implement SQL Server schema, relation and column discovery",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:09.798Z,updated_at: 2026-11-04T07:32:09.798Z,tags: [cuadb,mssql],depends_on: [TSK-E25A9F7241A3],order: 2048}
---

# Implement SQL Server schema, relation and column discovery

## Goal
Expose SQL Server databases and schemas, tables and views, columns, keys and indexes through CuaDB’s adapter interface. Do not force SQL Server catalog behavior into PostgreSQL-only models.

## Context
SQL Server has distinct catalog tables, identifier quoting conventions and permissions for metadata visibility. The user should see supported objects without being granted administration rights. Missing permissions should appear as clear unavailable states rather than generic 500 errors.

This task belongs to **13 — SQL Server adapter (post-core expansion)**. The group outcome is: Evaluate and implement Microsoft SQL Server support after the PostgreSQL, MySQL/MariaDB and SQLite viewer is proven. Preserve the same read-only explorer, data grid and metadata contract while respecting SQL Server-specific drivers, namespaces, catalog views and pagination requirements.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Discover accessible schemas, tables and views with correct identifiers.
- [ ] Return native types, nullable columns, keys and indexes where permitted.
- [ ] Use SQL Server-safe quoted identifiers and validated metadata lookups.
- [ ] Handle case-insensitive and mixed-case collations correctly in navigation.
- [ ] Verify against disposable fixtures under a restricted read-only login.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
