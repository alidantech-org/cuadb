---
{id: TSK-E09AA19D0352,name: "Harden browser responses, headers and fragment cache behavior",type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:05.812Z,updated_at: 2026-11-04T23:31:30.427Z,tags: [cuadb,http],depends_on: [TSK-FFD6A5B94574],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:07.459Z},{from: in-progress,to: completed,at: 2026-11-04T23:31:30.427Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-E09AA19D0352/1793791747476-start,started_at: 2026-11-04T11:29:07.497Z,end_ref: refs/tracking/TSK-E09AA19D0352/1793835090430-end,ended_at: 2026-11-04T23:31:30.427Z,summary: {files: 129,insertions: 22027,deletions: 959}}]}}}
---

# Harden browser responses, headers and fragment cache behavior

## Goal
Protect local database content from accidental exposure through HTTP caches, browser history snapshots and cross-origin interactions. Keep web access on localhost by default and document the boundary for any later remote deployment.

## Context
CuaDB is a browser-based interface to sensitive database values. Even read-only responses may contain personal or proprietary information. HTMX must not persist data snapshots unexpectedly, while CSS and JS should be bundled locally. Security headers should permit intended HTMX behavior without opening permissive script or embedding policies.

This task belongs to **05 — HTTP read endpoints and HTML fragments**. The group outcome is: Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Add no-store cache behavior to all authenticated or database-bearing HTML responses.
- [x] Define CSP, clickjacking and MIME-sniffing protections compatible with bundled assets.
- [x] Disable HTMX history caching of sensitive database fragments and test navigation behavior.
- [x] Verify that unsafe origins or methods cannot trigger unauthorized future state changes.
- [x] Add response security regression tests and document local-only deployment assumptions.

## Canonical UI fidelity

**Mandatory reference:** `assets/cuadb-ui-demo.html`, with the complete interaction and visual contract in `.docs/docs/frontend-interactions.md`. Reproduce the exact applicable layout and behaviors rather than designing a new interface. The HTML file uses fake records; the actual implementation must render verified database-backed content with safe Rust endpoints and HTMX swaps. This requirement is part of task completion, not a future polish option.

### Reference-specific acceptance
- [x] Keep database-bearing HTML no-store and disable sensitive HTMX history snapshots without breaking the demo's interaction pattern, visual states, popovers or row inspector.
- [x] Allow local persistence only for non-sensitive demo settings and per-table sort/hidden-column names; never save cell values, raw SQL, connection credentials or URLs.
- [x] Serve all CSS, HTMX and browser JavaScript from the Rust app with content security headers that do not break supported popup, copy, search or preference behavior.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [x] A test asserts that rendered pages and fragments contain no inline script, `style=` attribute or `on*=` handler, so the CSP (`script-src 'self'; style-src 'self'`) holds for every new screen.
- [x] Authenticated fragments keep `Cache-Control: no-store` and the login page and static assets keep their intended cache behavior.

## Comments

### 2026-11-04T11:29:33.423Z · PETER IRUNGU

Verified global no-store for HTML/database routes, CSP, frame denial, nosniff and referrer policy via regression tests. Disabled HTMX history snapshots for database fragments and set historyCacheSize to zero. CSS, JS and HTMX remain same-origin bundled assets.

### 2026-11-04T13:50:13.907Z · PETER IRUNGU

Backend items 1-5, 8 already checked. Items 6-7 (history snapshots with demo interaction; local persistence of non-sensitive settings) are UI - Phase B.

### 2026-11-04T23:31:30.252Z · PETER IRUNGU

Phase 2 evidence (2026-11-05): tests/auth_http.rs now asserts the explorer, login and catalog fragments contain no style attribute, style/script element or inline handler (assert_csp_clean). Referrer-Policy changed from no-referrer to same-origin because no-referrer made browsers send Origin: null on the login form post, which the origin check refused (found in the real browser; regression test asserts the header). Explorer and auth pages keep no-store; htmx historyCacheSize stays 0 and includeIndicatorStyles is off so htmx injects no inline style. theme.js stores only display preferences; no cell values, SQL or credentials are persisted. Verified in the built-in browser against a disposable SQLite database in light and dark: popovers, inspector, copy and search work under the strict CSP with no console errors.
