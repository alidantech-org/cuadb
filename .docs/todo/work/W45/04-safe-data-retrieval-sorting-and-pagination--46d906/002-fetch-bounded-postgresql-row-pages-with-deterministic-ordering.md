---
{id: TSK-23B4836BB3AE,name: Fetch bounded PostgreSQL row pages with deterministic ordering,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:05.334Z,updated_at: 2026-11-04T07:32:05.334Z,tags: [cuadb,postgres-rows],depends_on: [TSK-EC8CC62AD5CD,TSK-5CCB4090CF43],order: 2048}
---

# Fetch bounded PostgreSQL row pages with deterministic ordering

## Goal
Return a small set of records for a permitted relation without loading its full contents or computing exact totals. Preserve a stable column order and make explicit when the relation lacks a reliable key for deterministic navigation.

## Context
The grid defaults to fifty records per page and must cap requested page size at a configured maximum. PostgreSQL query results may contain NULL, large TOAST values and extension types. The first iteration should favor correctness, read-only enforcement and predictable failure handling over premature optimization.

This task belongs to **04 — Safe data retrieval, sorting and pagination**. The group outcome is: Implement the core read-only viewing capability with bounded, predictable SQL and correct typed values. A user must be able to open any permitted PostgreSQL table, read a page, sort an allowed column and navigate safely without requesting the entire relation. This group is the performance and query-security center of the project.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Fetch only requested columns and a bounded number of rows using stable ordering where possible.
- [ ] Apply statement timeouts and max page size before submitting SQL to PostgreSQL.
- [ ] Represent empty tables, permission errors and unsupported types without crashes.
- [ ] Add tests for 0, 1, 50 and more than 50 rows with deterministic order assertions.
- [ ] Record safe slow-query diagnostics without logging cell values or secrets.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
