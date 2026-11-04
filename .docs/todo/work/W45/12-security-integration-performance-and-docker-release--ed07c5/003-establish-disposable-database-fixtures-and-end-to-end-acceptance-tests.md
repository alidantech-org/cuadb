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
- [x] Add isolated PostgreSQL/PostGIS, MySQL/MariaDB and SQLite integration fixtures.
- [x] Run query and metadata acceptance tests with a restricted read-only user.
- [x] Exercise real HTMX routes and verify sanitized HTML output and paging state.
- [x] Keep ordinary unit tests runnable offline without Docker or databases.
- [x] Document fixture cleanup, local commands and any optional test dependencies.

## UI-demo backend support

The real frontend must match the canonical committed `assets/cuadb-ui-demo.html`. This backend/release task must expose the safe metadata or data behavior required by the reference rather than forcing the frontend to fake its controls. The complete cross-layer fidelity contract is `.docs/docs/frontend-interactions.md`.

### Demo-related acceptance
- [ ] Add visual-regression acceptance against assets/cuadb-ui-demo.html at 1440×900, 1280×800 and 390×844, covering light/dark, Simple/Advanced, Data/Structure/Indexes, Overview, inspector and popovers with deterministic fixtures.
- [ ] Exercise every reference interaction, including search/sort/page, connection switch, settings persistence, safe CSV/JSON and linked ZIP exports, mobile menu, theme changes and Ctrl/Cmd+K/Escape behavior.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:34:18.833Z · PETER IRUNGU

Items 1-5 verified (BACKEND). Fixtures: PostgreSQL/PostGIS (catalog, rows, rich_values, spatial_edges, perf_big), MariaDB and MySQL (Mixed_Items, rich_values, perf_big, empty_t, MyISAM), SQLite generated per test. docs/testing.md has commands, env vars, additive/idempotent rules and cleanup. 'cargo test' with no CUADB_TEST_* variables passes 154 tests offline. With fixtures all live suites pass (see commits). Items 6-7 are visual/browser acceptance: PENDING Phase B.
