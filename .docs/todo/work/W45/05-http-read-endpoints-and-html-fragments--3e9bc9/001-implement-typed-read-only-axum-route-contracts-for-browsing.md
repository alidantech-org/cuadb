---
{id: TSK-DC88ADC32B93,name: Implement typed read-only Axum route contracts for browsing,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:05.669Z,updated_at: 2026-11-05T00:19:12.449Z,tags: [cuadb,http],depends_on: [TSK-A3CFF0B5E267,TSK-23F3D2D0DDE1],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:23:01.716Z},{from: in-progress,to: completed,at: 2026-11-05T00:19:12.449Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-DC88ADC32B93/1793791381726-start,started_at: 2026-11-04T11:23:01.750Z,end_ref: refs/tracking/TSK-DC88ADC32B93/1793837952452-end,ended_at: 2026-11-05T00:19:12.449Z,summary: {files: 143,insertions: 26821,deletions: 932}}]}}}
---

# Implement typed read-only Axum route contracts for browsing

## Goal
Design and implement HTTP GET endpoints for connection status, schemas, tables, data rows, structure and info. Routes should delegate to adapters, return stable status codes and support both direct navigation and HTMX requests.

## Context
The existing router serves a shell and a status fragment. Future handlers must not issue inline vendor-specific SQL or pass raw database driver objects into templates. Each route needs a validated relation identifier and clear output semantics, including what to do when a table becomes inaccessible between requests.

This task belongs to **05 — HTTP read endpoints and HTML fragments**. The group outcome is: Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Define documented route paths and typed path/query request parsing.
- [x] Delegate discovery and row reads to the adapter through application state.
- [x] Reject invalid identifiers, directions, page sizes and unsupported relation types.
- [x] Support full-page requests and HTMX fragment responses where appropriate.
- [x] Add isolated router tests with a fake adapter rather than requiring a live database.

## Canonical UI fidelity

**Mandatory reference:** `assets/cuadb-ui-demo.html`, with the complete interaction and visual contract in `.docs/docs/frontend-interactions.md`. Reproduce the exact applicable layout and behaviors rather than designing a new interface. The HTML file uses fake records; the actual implementation must render verified database-backed content with safe Rust endpoints and HTMX swaps. This requirement is part of task completion, not a future polish option.

### Reference-specific acceptance
- [x] Expose typed read-only routes for the demo's sidebar tree, selected-object toolbar, Data/Structure/Indexes segment, Overview and advanced metadata screens, with all relation identifiers and options checked server-side.
- [x] Specify secure, cancellable, resource-bounded backend routes for the demo's CSV/JSON exports and linked-table ZIP; do not implement its unbounded JavaScript whole-table export against real data.
- [x] Preserve the selected connection, relation, row filter, column visibility, sort and page state across the same on-screen controls shown in the reference.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:29:32.483Z · PETER IRUNGU

Implemented Axum GET contracts with typed serde query models for catalog and row routes, deny-unknown-fields parsing, duplicate scalar rejection and catalog delegation through AppState. Added global HTML no-store and browser security headers plus cache-disabled HTMX fragments and route-state tests. Export routes remain intentionally unimplemented and the reference UI state preservation is incomplete.

### 2026-11-04T13:12:04.840Z · PETER IRUNGU

Item 5 verified: tests/router_fake_adapter.rs drives the real router with an in-memory adapter (AppState::with_adapter): escaping, relation listing, permission-denied (403/in-region fragment), no driver text leak, slow adapter cut off by request timeout. Items 7-8 remain open (export routes not implemented; state preservation is UI).

### 2026-11-04T13:14:12.788Z · PETER IRUNGU

Item 7 PARTIAL: bounded CSV/JSON export route /catalog/export implemented (row 10k/byte 16MiB caps, formula-safe CSV, attachment+no-store, timeout cancellation, tests in router_fake_adapter.rs, documented in README). Linked-table ZIP not implemented; walk not yet verified against a live adapter. Item stays open.

### 2026-11-04T13:50:13.490Z · PETER IRUNGU

Item 7 VERIFIED: /catalog/export serves CSV, JSON and related-table ZIP (zip =2.6.1, stored, no default features). Caps: 10k rows/relation, 16 MiB, 8 relations, depth<=2; independent validation; formula-safe CSV; cancellable via request timeout. Evidence: tests/router_fake_adapter.rs (14 pass) and tests/export_live.rs (3 pass against live PostgreSQL 18 cuadb_fixture_catalog, reader role). LIMITATIONS: output buffered in memory within the cap (not streamed); ZIP follows outgoing FKs only; export walk NOT verified live on MySQL/MariaDB/SQLite. Item 8 (state across on-screen controls) is UI - BACKEND VERIFIED - FRONTEND ACCEPTANCE PENDING.

### 2026-11-05T00:19:10.937Z · PETER IRUNGU

Phase 2 evidence (2026-11-05), built-in browser against the dedicated cuadb_fixture_catalog PostgreSQL fixture (read-only reader) and disposable SQLite data, light and dark at 1440x900, 1280x800 and 390x844 (paired with the updated demo), plus Rust tests (tests/sql_workspace_sqlite.rs, auth_http.rs, catalog_live_http.rs). Selected connection, relation, row filter, chosen columns, sort and page size travel in every navigation link and hidden form field; verified through search, sort, paging and the connection picker.
