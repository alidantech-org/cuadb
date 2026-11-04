---
{id: TSK-ABE2D51C83F3,name: Add safe expanded cell detail and copy actions,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:06.797Z,updated_at: 2026-11-04T11:29:41.886Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-BE512E99EA68,TSK-8DB2E738839F],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:41.886Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-ABE2D51C83F3/1793791781888-start,started_at: 2026-11-04T11:29:41.902Z}]}}}
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
- [x] Use bounded detail fetches tied to validated relation, column and row identity.
- [x] Show truncation and privacy-safe fallbacks for huge or unsupported values.
- [ ] Implement local copy actions without exposing credentials or unsafe HTML.
- [x] Test large JSON, control characters, Unicode and missing-key row scenarios.

## Canonical UI fidelity

The **required frontend specification** is `assets/cuadb-ui-demo.html` and the comparison contract in `.docs/docs/frontend-interactions.md`. Match the demo's visible interface, controls and applicable interactions accurately, using real database data and safe read-only backend pagination/export behavior instead of the prototype's synthetic arrays. No generic table-grid replacement or materially different layout is acceptable without user-approved documentation.

### Reference-specific acceptance
- [ ] On row click open the demo's 310px right-side inspector with sticky heading, close action, every column/type/value and readable multiline structured values; adapt to a mobile overlay at the reference breakpoint.
- [ ] Make foreign-key values navigable to permitted related tables and support double-click-to-copy cell values with demo-style toast feedback.
- [ ] Match every reference Export popover option: download current-view CSV, whole-table CSV, whole-table JSON, copy as CSV and Advanced-mode linked-table ZIP with an include-related-tables switch.
- [ ] Whole-table or linked exports must use permission-checked, cancellable and bounded server-side streaming or explicit approved limits; they must not materialize a large database table into browser memory as the demo does.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:00:06.274Z · PETER IRUNGU

Items 1,2,3,5 verified live (rich_values fixture, additive/idempotent tests/fixtures/rich_values.sql): jsonb, arrays with NULL, enum, bytea (hex), exact numeric, timestamptz microseconds, uuid, inet; 20k-element JSON/array, 20KB bytea and 100KB text keep page <120KB. Defect fixed: control chars and bidi overrides (U+202E etc.) were passed raw into HTML; now shown as visible \u{XXXX}. Item 4 partial: deep nesting and invalid UTF-8 not tested (PostgreSQL rejects invalid UTF-8 in text). 6-9 UI.
