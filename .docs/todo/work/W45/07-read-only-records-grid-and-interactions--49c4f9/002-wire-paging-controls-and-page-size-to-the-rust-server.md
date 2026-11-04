---
{id: TSK-16AEEF4061F8,name: Wire paging controls and page size to the Rust server,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:06.572Z,updated_at: 2026-11-04T23:18:19.988Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-BE512E99EA68,TSK-8B8935A1684A],order: 2048,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:41.555Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:18.312Z},{from: blocked,to: planned,at: 2026-11-04T23:18:19.988Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-16AEEF4061F8/1793791781557-start,started_at: 2026-11-04T11:29:41.570Z,end_ref: refs/tracking/TSK-16AEEF4061F8/1793808858314-end,ended_at: 2026-11-04T16:14:18.312Z,summary: {files: 91,insertions: 10231,deletions: 706}}]}},links: [{type: blocks,task: TSK-812F816785FD}]}
---

# Wire paging controls and page size to the Rust server

## Goal
Let users move between pages and choose common bounded sizes such as 25, 50 or 100 rows. Paging must preserve the selected relation and sort while clearly indicating whether next and previous pages are available.

## Context
The backend chooses an offset or keyset strategy based on table indexes. The UI must not promise exact total pages when only approximate row statistics are available. HTMX swaps should avoid resetting unrelated sidebar state or accidentally rendering data from an older request.

This task belongs to **07 — Read-only records grid and interactions**. The group outcome is: Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Render previous/next buttons using actual backend continuation state.
- [x] Offer bounded page-size choices without allowing arbitrary large values.
- [x] Preserve relation, sorting and pagination cursor during HTMX navigation.
- [x] Handle first, last and empty pages with accurate disabled states.
- [ ] Test rapid paging and stale request behavior against a fake adapter.

## Canonical UI fidelity

The **required frontend specification** is `assets/cuadb-ui-demo.html` and the comparison contract in `.docs/docs/frontend-interactions.md`. Match the demo's visible interface, controls and applicable interactions accurately, using real database data and safe read-only backend pagination/export behavior instead of the prototype's synthetic arrays. No generic table-grid replacement or materially different layout is acceptable without user-approved documentation.

### Reference-specific acceptance
- [ ] Match the demo's bottom footer: narrow ‹/› buttons, displayed row range, visible-column count, 25/50/100 rows-per-page select (default 25) and green 'SELECT only' tag.
- [ ] Render actual limited server-side page state instead of inferring exact totals from catalog estimates or fetching all rows; preserve demo positioning and styling.
- [ ] Keep sidebar, toolbar, hidden-column choices and selected relation stable during HTMX pagination, without changing unrelated regions.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [ ] Page size and previous/next remain bounded server fetches, and paging cancels any open row editor after confirming unsaved changes.

## Comments

### 2026-11-04T13:56:03.346Z · PETER IRUNGU

BACKEND VERIFIED - FRONTEND ACCEPTANCE PENDING. Items 1-4 verified live: Previous/Next from real continuation state, ranges 1-50/51-100/101-120/0-0, disabled buttons on first/last/empty, size/page/cursor bounds rejected (400), sort+size+column selection preserved in nav links. Item 5 left open: rapid paging is covered live and slow-adapter timeouts with a fake, but dropping stale responses is browser behavior (hx-sync) - unverified until Phase B. 6-8 UI.

### 2026-11-04T16:14:18.517Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.
