---
{id: TSK-54598487D0CE,name: Establish disposable database fixtures and end-to-end acceptance tests,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:09.208Z,updated_at: 2026-11-04T07:32:09.208Z,tags: [cuadb,release],depends_on: [TSK-725618F4E684,TSK-470D81309D89,TSK-5689F3668ED7,TSK-0DB3969BA9F6],order: 3072}
---

# Establish disposable database fixtures and end-to-end acceptance tests

## Goal
Verify that the actual browser routes, adapter queries and templates work together for PostgreSQL, MySQL, MariaDB and SQLite. Tests must not modify an existing user database and should be explicit about which fixtures are required.

## Context
A unit-level fake adapter proves HTTP behavior but cannot establish real catalog compatibility. Use disposable instances, seeded schemas and a least-privilege viewer role. End-to-end checks should cover schema browsing, first page, next page, sorting, structure, info and a connection failure path for each supported engine.

This task belongs to **12 — Security, integration, performance and Docker release**. The group outcome is: Prove CuaDB is safe and dependable as a local-first read-only database viewer for the supported engines. Ensure reproducible builds, meaningful automated tests, bounded query and memory behavior, documentation for real connections and a single production Docker container with one published UI port.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Add isolated PostgreSQL/PostGIS, MySQL/MariaDB and SQLite integration fixtures.
- [ ] Run query and metadata acceptance tests with a restricted read-only user.
- [ ] Exercise real HTMX routes and verify sanitized HTML output and paging state.
- [ ] Keep ordinary unit tests runnable offline without Docker or databases.
- [ ] Document fixture cleanup, local commands and any optional test dependencies.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
