---
{id: TSK-ABE2D51C83F3,name: Add safe expanded cell detail and copy actions,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:06.797Z,updated_at: 2026-11-05T00:19:29.059Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-BE512E99EA68,TSK-8DB2E738839F],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:41.886Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:19.078Z},{from: blocked,to: planned,at: 2026-11-04T23:18:20.066Z},{from: planned,to: in-progress,at: 2026-11-05T00:19:29.059Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-ABE2D51C83F3/1793791781888-start,started_at: 2026-11-04T11:29:41.902Z,end_ref: refs/tracking/TSK-ABE2D51C83F3/1793808859080-end,ended_at: 2026-11-04T16:14:19.078Z,summary: {files: 91,insertions: 10231,deletions: 706}},{base_ref: refs/tracking/TSK-ABE2D51C83F3/1793837969062-start,started_at: 2026-11-05T00:19:29.087Z}]}},links: [{type: blocks,task: TSK-11615BE5E97B}]}
---

# Add safe expanded cell detail and copy actions

## Goal
Let users inspect JSON, long text, arrays and other complex values without cluttering the grid. A detail drawer should show the complete supported value when within limits and clearly mark truncation or unavailable content.

## Context
Many database records include substantial JSON or serialized payloads. A naïve renderer would send megabytes per page. The detail request must remain read-only, identify the exact row and column safely and handle records with no stable row key honestly. Clipboard actions can use vanilla JavaScript without server calls.

This task belongs to **07 — Read-only records grid and interactions**. The group outcome is: Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Build an accessible reusable detail panel with formatted text and JSON when supported.
- [ ] Use bounded detail fetches tied to validated relation, column and row identity.
- [x] Show truncation and privacy-safe fallbacks for huge or unsupported values.
- [x] Implement local copy actions without exposing credentials or unsafe HTML.
- [x] Test large JSON, control characters, Unicode and missing-key row scenarios.

## Canonical UI fidelity

The **required frontend specification** is `assets/cuadb-ui-demo.html` and the comparison contract in `.docs/docs/frontend-interactions.md`. Match the demo's visible interface, controls and applicable interactions accurately, using real database data and safe read-only backend pagination/export behavior instead of the prototype's synthetic arrays. No generic table-grid replacement or materially different layout is acceptable without user-approved documentation.

### Reference-specific acceptance
- [x] On row click open the demo's 310px right-side inspector with sticky heading, close action, every column/type/value and readable multiline structured values; adapt to a mobile overlay at the reference breakpoint.
- [x] Make foreign-key values navigable to permitted related tables and support double-click-to-copy cell values with demo-style toast feedback.
- [x] Match every reference Export popover option: download current-view CSV, whole-table CSV, whole-table JSON, copy as CSV and Advanced-mode linked-table ZIP with an include-related-tables switch.
- [x] Whole-table or linked exports must use permission-checked, cancellable and bounded server-side streaming or explicit approved limits; they must not materialize a large database table into browser memory as the demo does.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [x] The 310px inspector shows all columns safely and, when permitted, offers Edit and Delete in its header; it is an overlay at 760px and narrower and Escape closes it.
- [x] Double-click copies a cell, large JSON, array and binary values open a bounded detail view, and nothing here starts a database query.

## Comments

### 2026-11-04T14:00:06.274Z · PETER IRUNGU

Items 1,2,3,5 verified live (rich_values fixture, additive/idempotent tests/fixtures/rich_values.sql): jsonb, arrays with NULL, enum, bytea (hex), exact numeric, timestamptz microseconds, uuid, inet; 20k-element JSON/array, 20KB bytea and 100KB text keep page <120KB. Defect fixed: control chars and bidi overrides (U+202E etc.) were passed raw into HTML; now shown as visible \u{XXXX}. Item 4 partial: deep nesting and invalid UTF-8 not tested (PostgreSQL rejects invalid UTF-8 in text). 6-9 UI.

### 2026-11-04T14:47:17.773Z · PETER IRUNGU

CORRECTION: earlier ticks on items 1 and 2 were made against the wrong task (I was verifying rich-value display, which is TSK-11615BE5E97B). Un-ticked. Current evidence for THIS task: item 3 (truncation markers, 'Unsupported' and 'GeoJSON detail exceeds' fallbacks) and item 5 (large JSON/array, control chars and bidi, Unicode, key-less SQLite heap tables; live tests) hold. Item 1: a detail aside with pretty-printed small JSON exists in data_page.html but accessibility and look are unverified. Item 2: there is no separate detail fetch - the inspector uses bounded per-cell detail embedded in the data page (no row-identity lookup); whether that satisfies the item is a design decision for the user. Items 4,6-9 need the browser.

### 2026-11-04T16:14:19.228Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.

### 2026-11-05T00:19:38.819Z · PETER IRUNGU

Phase 2 (2026-11-05): inspector, double-click copy, foreign-key links, the Export menu (page CSV, whole-table CSV and JSON, copy as CSV, ZIP with linked tables 1 or 2 hops, all bounded and server side) and Edit/Delete in the inspector are verified. Item 2 stays open: the inspector shows bounded values embedded in the page (truncated ones are marked) rather than fetching each cell by row identity; a full-cell fetch route would be new backend work.
