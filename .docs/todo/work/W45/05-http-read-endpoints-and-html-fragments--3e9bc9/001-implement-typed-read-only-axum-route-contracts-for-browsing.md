---
{id: TSK-DC88ADC32B93,name: Implement typed read-only Axum route contracts for browsing,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:05.669Z,updated_at: 2026-11-04T11:23:01.716Z,tags: [cuadb,http],depends_on: [TSK-A3CFF0B5E267,TSK-23F3D2D0DDE1],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:23:01.716Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-DC88ADC32B93/1793791381726-start,started_at: 2026-11-04T11:23:01.750Z}]}}}
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
- [ ] Add isolated router tests with a fake adapter rather than requiring a live database.

## Canonical UI fidelity

**Mandatory reference:** `assets/cuadb-ui-demo.html`, with the complete interaction and visual contract in `.docs/docs/frontend-interactions.md`. Reproduce the exact applicable layout and behaviors rather than designing a new interface. The HTML file uses fake records; the actual implementation must render verified database-backed content with safe Rust endpoints and HTMX swaps. This requirement is part of task completion, not a future polish option.

### Reference-specific acceptance
- [x] Expose typed read-only routes for the demo's sidebar tree, selected-object toolbar, Data/Structure/Indexes segment, Overview and advanced metadata screens, with all relation identifiers and options checked server-side.
- [ ] Specify secure, cancellable, resource-bounded backend routes for the demo's CSV/JSON exports and linked-table ZIP; do not implement its unbounded JavaScript whole-table export against real data.
- [ ] Preserve the selected connection, relation, row filter, column visibility, sort and page state across the same on-screen controls shown in the reference.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:29:32.483Z · PETER IRUNGU

Implemented Axum GET contracts with typed serde query models for catalog and row routes, deny-unknown-fields parsing, duplicate scalar rejection and catalog delegation through AppState. Added global HTML no-store and browser security headers plus cache-disabled HTMX fragments and route-state tests. Export routes remain intentionally unimplemented and the reference UI state preservation is incomplete.
