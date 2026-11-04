---
{id: TSK-047C7016EEAA,name: "Expose keys, foreign keys, indexes and partition hierarchy",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:05.013Z,updated_at: 2026-11-04T07:32:05.013Z,tags: [cuadb,postgres-catalog],depends_on: [TSK-7AA22F3D0843],order: 3072}
---

# Expose keys, foreign keys, indexes and partition hierarchy

## Goal
Model PostgreSQL relationships and indexes so users can understand table references, uniqueness and sorting opportunities. Keep this metadata read-only and bounded, and support partitioned tables without double-counting data or misrepresenting inherited structure.

## Context
Primary keys and usable unique indexes are important for stable row pagination. Foreign keys may cross schemas, and index details may include expressions or predicates. The adapter should expose useful human-readable metadata and preserve links to related objects for later UI navigation.

This task belongs to **03 — PostgreSQL schemas, relations and metadata**. The group outcome is: Deliver complete, permission-aware catalog browsing for PostgreSQL and PostGIS installations. Users must find schemas, tables and views; inspect column definitions, indexes, keys, constraints and storage sizes; and understand when a statistic is approximate or unavailable. All catalog queries must be scoped, efficient and read-only.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Discover foreign key source and target columns with schema-qualified relation names.
- [ ] Return index names, uniqueness, primary status and relevant indexed columns.
- [ ] Represent partition parents and child tables with unambiguous relationships.
- [ ] Test composite keys, expression indexes and links to inaccessible related tables.
- [ ] Provide metadata usable by both Structure view and pagination planning.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
