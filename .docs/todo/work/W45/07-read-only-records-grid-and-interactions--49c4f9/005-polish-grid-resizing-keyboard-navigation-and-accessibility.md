---
{id: TSK-470D81309D89,name: "Polish grid resizing, keyboard navigation and accessibility",type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:06.902Z,updated_at: 2026-11-05T00:19:13.955Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-812F816785FD,TSK-ABE2D51C83F3],order: 5120,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:42.035Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:19.361Z},{from: blocked,to: planned,at: 2026-11-04T23:18:20.106Z},{from: planned,to: in-progress,at: 2026-11-05T00:19:00.686Z},{from: in-progress,to: completed,at: 2026-11-05T00:19:13.955Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-470D81309D89/1793791782037-start,started_at: 2026-11-04T11:29:42.052Z,end_ref: refs/tracking/TSK-470D81309D89/1793808859363-end,ended_at: 2026-11-04T16:14:19.361Z,summary: {files: 91,insertions: 10231,deletions: 706}},{base_ref: refs/tracking/TSK-470D81309D89/1793837940689-start,started_at: 2026-11-05T00:19:00.718Z,end_ref: refs/tracking/TSK-470D81309D89/1793837953958-end,ended_at: 2026-11-05T00:19:13.956Z,summary: {files: 0,insertions: 0,deletions: 0}}]}},links: [{type: blocks,task: TSK-A7B0DFC23314}]}
---

# Polish grid resizing, keyboard navigation and accessibility

## Goal
Make dense database browsing comfortable with the keyboard and usable for assistive technology. Keep client-side behavior lightweight and resilient across HTMX fragment replacement.

## Context
Resizable columns and shortcuts improve a developer tool, but custom browser behavior must not interfere with native table semantics. Delegate events from stable containers so swapped rows continue to work. Do not add a complete frontend framework or local data state store for these small interactions.

This task belongs to **07 — Read-only records grid and interactions**. The group outcome is: Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Support focused table navigation, copy shortcuts and escape-to-close behavior.
- [x] Add optional column width adjustments without breaking responsive overflow.
- [x] Keep hover, focus, loading and error states readable in dark and light modes.
- [x] Verify event handlers still work after multiple HTMX swaps.
- [x] Test keyboard-only navigation and automated accessibility checks where feasible.

## Canonical UI fidelity

The **required frontend specification** is `assets/cuadb-ui-demo.html` and the comparison contract in `.docs/docs/frontend-interactions.md`. Match the demo's visible interface, controls and applicable interactions accurately, using real database data and safe read-only backend pagination/export behavior instead of the prototype's synthetic arrays. No generic table-grid replacement or materially different layout is acceptable without user-approved documentation.

### Reference-specific acceptance
- [x] Match sidebar split-resizing, double-click reset, popover click-away dismissal, Escape, Ctrl/Cmd+K sidebar search, mobile hamburger and local feedback toasts.
- [x] Keep all browser event handlers working after HTMX swaps, with keyboard accessibility, appropriate focus behavior and the demo's compact control appearance.
- [x] Compare reference/app screenshots for default grid, selected-row inspector, Advanced navigation and mobile widths; document any nontrivial deviations rather than marking complete.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [x] Column resizing, keyboard row and cell navigation, visible focus and screen-reader labels cover the grid, inspector, popovers and modals (focus trap in `#modal`, focus return on close).

## Comments

### 2026-11-04T16:14:19.507Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.

### 2026-11-05T00:19:12.222Z · PETER IRUNGU

Phase 2 evidence (2026-11-05), built-in browser against the dedicated cuadb_fixture_catalog PostgreSQL fixture (read-only reader) and disposable SQLite data, light and dark at 1440x900, 1280x800 and 390x844 (paired with the updated demo), plus Rust tests (tests/sql_workspace_sqlite.rs, auth_http.rs, catalog_live_http.rs). Splitter drag and double-click reset, popovers closed by click-away and Escape, Ctrl/Cmd+K, drawer, toasts, column resize by dragging a header edge (double-click fits), arrow-key row navigation, Enter opens the inspector, Ctrl/Cmd+C copies the focused row, handlers work after repeated swaps. No automated accessibility scanner was run (not available offline); keyboard-only use was exercised by hand.
