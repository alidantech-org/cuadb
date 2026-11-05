---
{id: TSK-812F816785FD,name: Connect sortable headers with clear direction and stable state,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:06.702Z,updated_at: 2026-11-05T00:19:13.726Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-16AEEF4061F8,TSK-84DCF1B4A953],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:41.718Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:18.745Z},{from: blocked,to: planned,at: 2026-11-04T23:18:20.029Z},{from: planned,to: in-progress,at: 2026-11-05T00:19:00.419Z},{from: in-progress,to: completed,at: 2026-11-05T00:19:13.726Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-812F816785FD/1793791781721-start,started_at: 2026-11-04T11:29:41.736Z,end_ref: refs/tracking/TSK-812F816785FD/1793808858747-end,ended_at: 2026-11-04T16:14:18.745Z,summary: {files: 91,insertions: 10231,deletions: 706}},{base_ref: refs/tracking/TSK-812F816785FD/1793837940422-start,started_at: 2026-11-05T00:19:00.454Z,end_ref: refs/tracking/TSK-812F816785FD/1793837953729-end,ended_at: 2026-11-05T00:19:13.726Z,summary: {files: 0,insertions: 0,deletions: 0}}]}},links: [{type: blocks,task: TSK-470D81309D89}]}
---

# Connect sortable headers with clear direction and stable state

## Goal
Allow one-column ascending or descending sorting with a clear indicator, keyboard support and bounded server-side queries. The UI must never sort just the current page locally while implying that the whole table is sorted.

## Context
Sorting is an explicit backend operation and may be slow for unindexed columns. The grid should announce the chosen direction, reset page position when needed and retain the table selection. Unsupported columns should not look interactive or trigger broken query paths.

This task belongs to **07 — Read-only records grid and interactions**. The group outcome is: Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Generate sort links only for backend-approved column identifiers.
- [x] Expose aria-sort and visible ascending/descending state on active headers.
- [x] Reset or adapt paging state when sorting changes.
- [x] Render a safe error and restore usable controls when a sort is rejected or timed out.
- [x] Test sort transitions, NULL-heavy fixtures and fast repeated header clicks.

## Canonical UI fidelity

The **required frontend specification** is `assets/cuadb-ui-demo.html` and the comparison contract in `.docs/docs/frontend-interactions.md`. Match the demo's visible interface, controls and applicable interactions accurately, using real database data and safe read-only backend pagination/export behavior instead of the prototype's synthetic arrays. No generic table-grid replacement or materially different layout is acceptable without user-approved documentation.

### Reference-specific acceptance
- [x] Make each table header click cycle ascending, descending and unsorted just like the demo, with the same small arrow next to the active column.
- [x] Sort the full selected relation through a validated backend query, not merely the loaded page; preserve page-size and reset continuation state correctly.
- [x] Keep column type annotations, compact hover/focus styles and accessible aria-sort consistent with the reference across repeated HTMX requests.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [x] Sort headers expose `aria-sort`, reset paging and keep hidden-column and sort preferences per table, remembered only when the preference is on.

## Comments

### 2026-11-04T13:56:03.197Z · PETER IRUNGU

BACKEND VERIFIED - FRONTEND ACCEPTANCE PENDING. Items 1,3,5 verified live (PostgreSQL 18, tests/rows_live_http.rs): sort links only for sortable types, links reset page/cursor, NULLs always last (explicit NULLS LAST, both directions), repeatable order, 120/120 unique rows across sorted pages, 16 concurrent mixed-direction requests each return their own aria-sort. Defect fixed: repeated column= params (emitted by the app's own links and column picker) failed with 'invalid type: string, expected a sequence'; new raw-query parser (src/web/rows.rs split_query) now used by /catalog/data and /catalog/export. Items 2,4 (visible state, restoring controls after a rejected sort in a browser), 6,7(UI cycle),8 pending Phase B. Limitation: sorted paging uses offset paging (keyset only for unsorted/unique-key scans).

### 2026-11-04T16:14:18.900Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.

### 2026-11-05T00:19:12.045Z · PETER IRUNGU

Phase 2 evidence (2026-11-05), built-in browser against the dedicated cuadb_fixture_catalog PostgreSQL fixture (read-only reader) and disposable SQLite data, light and dark at 1440x900, 1280x800 and 390x844 (paired with the updated demo), plus Rust tests (tests/sql_workspace_sqlite.rs, auth_http.rs, catalog_live_http.rs). Header links cycle ascending, descending, unsorted with the arrow and aria-sort; sorting is a validated server query over the whole relation; remembered per table when 'Remember hidden columns and sort' is on (verified by leaving and returning); a rejected sort leaves the grid in place with a toast.
