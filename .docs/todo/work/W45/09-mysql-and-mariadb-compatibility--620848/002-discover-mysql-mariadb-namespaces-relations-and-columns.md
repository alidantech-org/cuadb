---
{id: TSK-A7B0DFC23314,name: "Discover MySQL/MariaDB namespaces, relations and columns",type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:07.605Z,updated_at: 2026-11-04T12:46:12.997Z,tags: [cuadb,mysql],depends_on: [TSK-0D56383DBA25],order: 2048,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T12:46:12.997Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-A7B0DFC23314/1793796373007-start,started_at: 2026-11-04T12:46:13.031Z}]}}}
---

# Discover MySQL/MariaDB namespaces, relations and columns

## Goal
Map engine catalog metadata into the common database explorer tree. Users should browse available databases, tables and views, then inspect column types, defaults, indexes, primary keys and foreign-key references without a SQL editor.

## Context
MySQL typically treats a database as the namespace equivalent; do not present fictitious PostgreSQL-style nested schemas. Preserve exact identifier case where the server permits it and account for collation and filesystem case-sensitivity differences. Metadata must reflect the connected role’s visibility.

This task belongs to **09 — MySQL and MariaDB compatibility**. The group outcome is: Bring MySQL and MariaDB into CuaDB through a separate adapter while preserving the same read-only navigation and data-grid contract. Reuse common models and browser components, but implement engine-specific discovery, identifier quoting, data decoding and statistics rather than assuming PostgreSQL SQL syntax works everywhere.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] List accessible databases, tables and views with engine-correct object types.
- [x] Return columns, defaults, nullable flags, primary keys and foreign-key relationships.
- [x] Handle identifier quoting and case behavior without SQL injection risk.
- [x] Report unsupported metadata capability rather than fabricating fields.
- [ ] Test mixed-case, empty and permission-restricted fixture databases.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:20:01.334Z · PETER IRUNGU

Implemented MySQL/MariaDB information_schema schema, relation, column, index, constraint and foreign-key discovery. A disposable MariaDB 11.8 integration fixture verifies mixed-case table and view discovery, generated/identity columns and FK references. SQL uses bound catalog values and escaped backtick identifiers. MySQL 8 fixture remains unavailable; the related test is environment-gated by CUADB_TEST_MARIADB_URL.

### 2026-11-04T14:04:42.068Z · PETER IRUNGU

Item 5 left open: mixed-case names and hidden-schema (mysql/sys/information_schema/performance_schema) restriction verified on both engines; no fixture for an EMPTY database or a no-privilege account. Defect fixed: Structure tab returned 400 for EVERY MySQL/MariaDB relation because partitions() fell back to Unsupported; now implemented from information_schema.partitions. Same latent defect fixed for SQLite (empty list).
