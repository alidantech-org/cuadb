---
{id: TSK-1C5E54BDB641,name: Add fast relation-name search and navigation breadcrumbs,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:06.170Z,updated_at: 2026-11-04T23:18:19.872Z,tags: [cuadb,explorer-ui],depends_on: [TSK-5EE171C4C580],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:11.515Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:17.168Z},{from: blocked,to: planned,at: 2026-11-04T23:18:19.872Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-1C5E54BDB641/1793791751518-start,started_at: 2026-11-04T11:29:11.532Z,end_ref: refs/tracking/TSK-1C5E54BDB641/1793808857170-end,ended_at: 2026-11-04T16:14:17.168Z,summary: {files: 91,insertions: 10231,deletions: 706}}]}},links: [{type: blocks,task: TSK-61DA50E7C274}]}
---

# Add fast relation-name search and navigation breadcrumbs

## Goal
Let users locate a table or view quickly even in databases containing hundreds of relations. Provide contextual schema and relation breadcrumbs so users understand the current selection without relying on the sidebar alone.

## Context
CuaDB is not a SQL search engine. The initial search filters relation names and schemas only; it must not query table contents or full-scan the database. Large trees may need server-side filtering or lazy loading, while small lists can be filtered locally with minimal JavaScript.

This task belongs to **06 — Database navigation and workspace UI**. The group outcome is: Replace the demo shell with a clean, responsive database explorer focused on fast navigation. The left sidebar should display connections, schemas and relation names; the main panel should immediately show data or a meaningful empty state. Preserve the lightweight Rust-rendered design without React, Next.js or a frontend state library.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Search schema and relation names without reading underlying data rows.
- [ ] Preserve navigation context and selection when clearing or changing the search.
- [ ] Provide keyboard navigation and clear empty-results messaging.
- [ ] Test special characters, case sensitivity and large table lists.
- [x] Keep search requests bounded and debounce remote requests when used.

## Canonical UI fidelity

**Mandatory visual specification:** `assets/cuadb-ui-demo.html` (see `.docs/docs/frontend-interactions.md`). Its sidebar, interactions, state patterns and typography are required, not decorative inspiration. Recreate the reference with Askama/HTMX, Tailwind and minimal JavaScript using actual database data; do not replace the interface with a conventional dashboard. Safety and accessible real-data states may adapt the underlying mechanism, not silently redesign the appearance.

### Reference-specific acceptance
- [ ] Place the 'Search tables…' field and optional selection control exactly as in the demo sidebar; filter matching schema/relation names with the same compact 'Nothing matches' state.
- [ ] Keep the demo's schema-qualified table name in the small main toolbar rather than adding a new breadcrumb navigation strip.
- [ ] Support Ctrl/Cmd+K to reveal/focus/select the sidebar search and retain expansion/selection state during searches and relation changes.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [ ] Ctrl/Cmd+K focuses the sidebar search, the filter is permission-safe, and breadcrumbs show connection, database, schema and relation with the access tag.

## Comments

### 2026-11-04T11:29:34.666Z · PETER IRUNGU

Added immediate case-insensitive relation-name filtering on server-rendered catalog entries; filtering inspects names only and does not query cell data. Breadcrumbs and large-catalog server fallback remain open.

### 2026-11-04T13:51:44.854Z · PETER IRUNGU

BACKEND VERIFIED - FRONTEND ACCEPTANCE PENDING. Added literal, case-insensitive, 64-char-bounded name filter 'q' on /catalog/relations (no SQL wildcards; control chars rejected). Tests: router_fake_adapter (special chars %_ <b>, case, long/control input, 500-item truncated list) and catalog_live_http (live PG: Unicode/mixed-case names, empty result). LIMITATION: the filter runs over the adapter's bounded list (500 relations), so on truncated lists the result is flagged incomplete rather than searched server-side; search is per-schema only. Item 4 debounce and items 1-3,6-8 are UI (Ctrl+K, keyboard nav, placement).

### 2026-11-04T16:14:17.352Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.
