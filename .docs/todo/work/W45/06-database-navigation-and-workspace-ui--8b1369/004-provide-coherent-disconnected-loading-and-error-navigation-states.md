---
{id: TSK-61DA50E7C274,name: "Provide coherent disconnected, loading and error navigation states",type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:06.252Z,updated_at: 2026-11-05T00:19:13.143Z,tags: [cuadb,explorer-ui],depends_on: [TSK-1C5E54BDB641,TSK-E68FB08CB73D],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:11.664Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:17.546Z},{from: blocked,to: planned,at: 2026-11-04T23:18:19.912Z},{from: planned,to: in-progress,at: 2026-11-05T00:18:59.573Z},{from: in-progress,to: completed,at: 2026-11-05T00:19:13.143Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-61DA50E7C274/1793791751666-start,started_at: 2026-11-04T11:29:11.681Z,end_ref: refs/tracking/TSK-61DA50E7C274/1793808857548-end,ended_at: 2026-11-04T16:14:17.546Z,summary: {files: 91,insertions: 10231,deletions: 706}},{base_ref: refs/tracking/TSK-61DA50E7C274/1793837939575-start,started_at: 2026-11-05T00:18:59.601Z,end_ref: refs/tracking/TSK-61DA50E7C274/1793837953146-end,ended_at: 2026-11-05T00:19:13.143Z,summary: {files: 0,insertions: 0,deletions: 0}}]}},links: [{type: blocks,task: TSK-4C7273B9E46A}]}
---

# Provide coherent disconnected, loading and error navigation states

## Goal
Make the interface trustworthy at every stage of database initialization. A user must be able to tell whether CuaDB itself is running, whether a database is connected and whether a selected table is loading or inaccessible.

## Context
The shell currently marks itself as not connected. The real interface should preserve that honesty when URLs are missing, connections are refused or the current user lacks permission. Avoid flickering status labels, stale schema trees and unlimited automatic retries.

This task belongs to **06 — Database navigation and workspace UI**. The group outcome is: Replace the demo shell with a clean, responsive database explorer focused on fast navigation. The left sidebar should display connections, schemas and relation names; the main panel should immediately show data or a meaningful empty state. Preserve the lightweight Rust-rendered design without React, Next.js or a frontend state library.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Render accurate connection indicators distinct from server health.
- [x] Expose useful recovery guidance for offline or misconfigured database connections.
- [x] Show accessible skeleton/loading and empty-content states without fake data.
- [x] Prevent stale navigation fragments after fast table switching.
- [x] Test accessibility announcements and focus retention during HTMX swaps.

## Canonical UI fidelity

**Mandatory visual specification:** `assets/cuadb-ui-demo.html` (see `.docs/docs/frontend-interactions.md`). Its sidebar, interactions, state patterns and typography are required, not decorative inspiration. Recreate the reference with Askama/HTMX, Tailwind and minimal JavaScript using actual database data; do not replace the interface with a conventional dashboard. Safety and accessible real-data states may adapt the underlying mechanism, not silently redesign the appearance.

### Reference-specific acceptance
- [x] Use the same demo shell to show disconnected, connecting, offline and forbidden database states without displaying the demo's synthetic sample records as live content.
- [x] Use compact connection badges, empty grid/sidebar regions and toast feedback instead of visually unrelated full-screen status cards.
- [x] Preserve the mobile sidebar toggle and row inspector positioning, including recovery when the selected table or connection becomes unavailable.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [x] Add states for signed-out, session-expired, no-access, disconnected profile and read-only profile, each rendered inside the existing shell regions rather than as unrelated full pages.

## Comments

### 2026-11-04T11:29:35.092Z · PETER IRUNGU

Shell separately reports app health, database connection state, catalog loading, empty catalog and safe database errors. Failure content remains inside the app shell. Full browser state/access-denied view parity requires further work.

### 2026-11-04T16:14:17.757Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.

### 2026-11-05T00:19:11.467Z · PETER IRUNGU

Phase 2 evidence (2026-11-05), built-in browser against the dedicated cuadb_fixture_catalog PostgreSQL fixture (read-only reader) and disposable SQLite data, light and dark at 1440x900, 1280x800 and 390x844 (paired with the updated demo), plus Rust tests (tests/sql_workspace_sqlite.rs, auth_http.rs, catalog_live_http.rs). Signed-out (login), session-expired (redirect), no-grants (sidebar note), disconnected or offline (dot and in-region error), read-only (footer tag) and loading (progress bar) all render inside the existing shell regions; the mobile drawer and the inspector overlay were checked at 390x844.
