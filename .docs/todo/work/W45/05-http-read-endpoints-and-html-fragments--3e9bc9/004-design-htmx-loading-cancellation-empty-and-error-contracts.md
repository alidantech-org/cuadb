---
{id: TSK-9890D0090922,name: "Design HTMX loading, cancellation, empty and error contracts",type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:05.887Z,updated_at: 2026-11-04T16:15:20.401Z,tags: [cuadb,http],depends_on: [TSK-E09AA19D0352],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:07.621Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-9890D0090922/1793791747624-start,started_at: 2026-11-04T11:29:07.636Z}]}},links: [{type: blocks,task: TSK-A513FD340BD5}]}
---

# Design HTMX loading, cancellation, empty and error contracts

## Goal
Make asynchronous data navigation predictable during slow queries, network interruptions and permission failures. Stale responses should not overwrite the currently selected table, and errors should render in a consistent, accessible way.

## Context
Switching between tables, sorting and clicking next rapidly may produce overlapping requests. The UI must avoid showing mixed results for different relations. Distinguish empty dataset, connection offline, timeout, denied permission and invalid request. Browser-only interactions should remain available if the database is temporarily unreachable.

This task belongs to **05 — HTTP read endpoints and HTML fragments**. The group outcome is: Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Define swap targets, request synchronization and stable loading indicators per fragment.
- [x] Provide safe fragments for timeout, validation, permission and disconnected states.
- [ ] Ensure stale responses cannot replace a newer selected relation or sort.
- [x] Test full-page and HTMX failure behavior with simulated slow fake adapters.
- [x] Document retry and refresh actions without automatic unbounded polling.

## Canonical UI fidelity

**Mandatory reference:** `assets/cuadb-ui-demo.html`, with the complete interaction and visual contract in `.docs/docs/frontend-interactions.md`. Reproduce the exact applicable layout and behaviors rather than designing a new interface. The HTML file uses fake records; the actual implementation must render verified database-backed content with safe Rust endpoints and HTMX swaps. This requirement is part of task completion, not a future polish option.

### Reference-specific acceptance
- [ ] Keep loading, empty, offline, invalid-object and access-denied fragments inside the demo's sidebar, toolbar, grid and inspector regions instead of showing a visually different full-screen error UI.
- [ ] Synchronize HTMX requests so rapid relation selection, search, pagination, sorting and profile changes never display stale data in the reference grid.
- [ ] Match the demo's dismissible popovers, transient toast feedback, selected-row close control and disabled control treatment while adding focus/accessible state announcements.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [ ] HTMX requests send the `x-csrf-token` header (from a layout meta tag) and a session-expired `hx-redirect` lands on the login page without leaving a half-rendered shell.
- [ ] Permission-denied (403), session-expired and read-only-profile responses render in the same shell region as the demo's inline messages, and stale responses never overwrite a newer selection.

## Comments

### 2026-11-04T11:29:33.819Z · PETER IRUNGU

Established stable sidebar/tree/main targets, HTMX loading indicator, disabled history caching, accessible status and safe error fragment behavior. Existing request timeout drops handler futures. Stale selection ordering, broader UI states and fake-adapter slow-route tests remain open.

### 2026-11-04T13:12:04.975Z · PETER IRUNGU

Backend partial: slow fake adapter is cancelled by the request timeout (router_fake_adapter.rs). HTMX-vs-full-page failure variants, stale-response handling and retry docs still open.

### 2026-11-04T13:50:13.779Z · PETER IRUNGU

Items 4,5 verified (router_fake_adapter: identical in-region error fragment for full-page and HX-Request, slow adapter timeout; README retry section). Items 3,7: hx-sync=this:replace added on all catalog panel roots and asserted in a test; real abort behavior is browser-only and UNVERIFIED - left open for Phase B. Items 6,8 UI.
