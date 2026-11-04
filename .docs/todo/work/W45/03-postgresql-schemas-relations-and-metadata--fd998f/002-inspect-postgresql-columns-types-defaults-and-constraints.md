---
{id: TSK-7AA22F3D0843,name: "Inspect PostgreSQL columns, types, defaults and constraints",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:04.943Z,updated_at: 2026-11-04T07:32:04.943Z,tags: [cuadb,postgres-catalog],depends_on: [TSK-5FC58B7B6E27],order: 2048}
---

# Inspect PostgreSQL columns, types, defaults and constraints

## Goal
Provide a complete column structure model including native type, nullability, default expression, generated/identity status and primary or unique key indicators. The user should understand table shape without opening a SQL console.

## Context
PostgreSQL uses catalog metadata for arrays, domains, enums and extension types. Avoid flattening all types into a generic string that loses meaningful information. The UI should receive safe display-ready metadata, while backend representation retains stable identifiers for later sort and cell decoding.

This task belongs to **03 — PostgreSQL schemas, relations and metadata**. The group outcome is: Deliver complete, permission-aware catalog browsing for PostgreSQL and PostGIS installations. Users must find schemas, tables and views; inspect column definitions, indexes, keys, constraints and storage sizes; and understand when a statistic is approximate or unavailable. All catalog queries must be scoped, efficient and read-only.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Return ordered column names, data types, nullable flags and default expressions.
- [ ] Identify primary keys, identity/generated columns, domains and enum information.
- [ ] Handle views and atypical extension types without panics or false metadata.
- [ ] Test composite and mixed-case identifiers against PostgreSQL fixture tables.
- [ ] Label unsupported details explicitly rather than returning empty invented values.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
