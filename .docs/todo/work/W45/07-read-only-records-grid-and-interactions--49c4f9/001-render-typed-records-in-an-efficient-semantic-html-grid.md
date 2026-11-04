---
{id: TSK-BE512E99EA68,name: Render typed records in an efficient semantic HTML grid,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:06.442Z,updated_at: 2026-11-04T11:29:41.419Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-8DB2E738839F,TSK-5EE171C4C580],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:41.419Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-BE512E99EA68/1793791781420-start,started_at: 2026-11-04T11:29:41.432Z}]}}}
---

# Render typed records in an efficient semantic HTML grid

## Goal
Turn adapter row pages into a clean table with stable column ordering, readable type formatting and clear NULL values. Keep the layout usable with dozens of columns and avoid loading unnecessary data on the client.

## Context
Database tables frequently have mixed scalar, JSON and binary columns. Large content should be abbreviated in a predictable way and accessible on demand. HTML output must always escape values, and the selected relation name must be shown explicitly so users know which data they are viewing.

This task belongs to **07 — Read-only records grid and interactions**. The group outcome is: Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Render row headers, column labels, typed values and explicit NULL markers safely.
- [x] Provide horizontal scrolling and sticky headers on narrow or wide tables.
- [ ] Show accessible empty-table and unsupported-type states without fake rows.
- [x] Bound initial rendered cell content and preserve a path for expanded inspection.
- [ ] Verify XSS escaping and layout behavior with fixture records.

## Canonical UI fidelity

The **required frontend specification** is `assets/cuadb-ui-demo.html` and the comparison contract in `.docs/docs/frontend-interactions.md`. Match the demo's visible interface, controls and applicable interactions accurately, using real database data and safe read-only backend pagination/export behavior instead of the prototype's synthetic arrays. No generic table-grid replacement or materially different layout is acceptable without user-approved documentation.

### Reference-specific acceptance
- [ ] Replicate the demo grid's sticky headers and row-number rail, 24px compact default rows, small column type labels, primary-key dot, fine row/cell borders, horizontal scrolling and numeric alignment.
- [ ] Match demo-specific NULL, boolean/status pill, text/JSON/array/timestamp styles, row hover, selection highlighting and ellipsis, all populated by escaped real records.
- [ ] Support the same user-selectable row densities, horizontal/grid/no border styles, striped rows, text wrapping, thousands separators, visible row numbers and type labels.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:22:43.035Z · PETER IRUNGU

Implemented initial shared read-only grid view with typed bounded cells, paging links and sizes, sortable headers, search, inspector detail/copy and accessibility semantics. Canonical demo acceptance remains incomplete: no column picker/export and visual screenshot comparison not yet demonstrated; leave checklist unchecked where these apply.

### 2026-11-04T12:45:50.105Z · PETER IRUNGU

Progress evidence (2026-11-04): semantic table headers/rows and typed/null cells are rendered; sticky headers, horizontal scrolling and bounded previews are implemented. Focused rows/adapter tests passed and bun run format:check passed. The canonical grid remains incomplete: full selected-row inspector, export actions, FK navigation, preference controls, screenshot parity and several accessibility interactions are not accepted.

### 2026-11-04T13:46:53.726Z · PETER IRUNGU

Live finding: tests/postgres_rows.rs filter case expected a 'row 11' value absent from tests/fixtures/rows.sql, and had never run live. Test changed to filter on the fixture's unique value; live paging/cursor/sort/filter test now passes on PostgreSQL 18.
