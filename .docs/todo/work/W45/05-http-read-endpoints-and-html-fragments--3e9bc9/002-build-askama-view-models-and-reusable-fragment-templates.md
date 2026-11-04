---
{id: TSK-FFD6A5B94574,name: Build Askama view models and reusable fragment templates,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:05.740Z,updated_at: 2026-11-04T16:14:59.295Z,tags: [cuadb,http],depends_on: [TSK-DC88ADC32B93],order: 2048,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:25:00.389Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-FFD6A5B94574/1793791500391-start,started_at: 2026-11-04T11:25:00.407Z}]}},links: [{type: blocks,task: TSK-0AA246B508ED}]}
---

# Build Askama view models and reusable fragment templates

## Goal
Create compile-time-checked templates and data-focused view models for connection state, table navigation, rows and metadata. Keep display logic outside SQL implementation and ensure all database-provided strings are safely escaped.

## Context
Askama templates are currently limited to a demo page. The real explorer should reuse consistent rendering logic for a full-page load and an HTMX partial update. Empty, restricted and offline states must be first-class view model variants rather than a tangle of template conditionals.

This task belongs to **05 — HTTP read endpoints and HTML fragments**. The group outcome is: Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Create typed view models for sidebar tree, relation header, grid and inspector tabs.
- [ ] Reuse components across full pages and HTMX responses without duplicate markup.
- [x] Verify HTML escaping for suspicious table names, cell contents and error messages.
- [x] Render explicit empty and inaccessible states with clear next actions.
- [x] Keep template code free of direct driver calls or raw SQL statements.

## Canonical UI fidelity

**Mandatory reference:** `assets/cuadb-ui-demo.html`, with the complete interaction and visual contract in `.docs/docs/frontend-interactions.md`. Reproduce the exact applicable layout and behaviors rather than designing a new interface. The HTML file uses fake records; the actual implementation must render verified database-backed content with safe Rust endpoints and HTMX swaps. This requirement is part of task completion, not a future polish option.

### Reference-specific acceptance
- [x] Map Askama pages/fragments directly to demo regions: #side/#tree, bottom #cbtn/#spop, #rs splitter, #mainbody, .bar toolbar, .gw grid, #insp detail panel and .foot footer.
- [ ] Render both Simple and Advanced explorer layouts and the database Overview, table Data/Structure/Indexes and read-only metadata object detail screens with the demo's compact component hierarchy.
- [x] Retain the demo's visual layout instead of introducing an unrelated global header, generic dashboard or alternate navigation model.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [ ] Every full page and fragment receives a principal view model (display name, role, local-or-authenticated mode, CSRF token, SQL-workspace and write capability flags) and shared popover, modal and toast partials exist.
- [x] No template emits `style=` attributes or inline handlers; bar widths, swatches and density use classes, `data-*` attributes or CSS variables set from bundled JS.
- [ ] Fragments for the new surfaces (account block, database switcher, connection modal, admin tabs, row editor, SQL workspace) are Askama partials, each with empty, loading, denied and error states.

## Comments

### 2026-11-04T11:29:33.158Z · PETER IRUNGU

Introduced reusable escaped catalog fragments and compact app shell regions for schema navigation, main content, connection state, loading and empty/error messaging. Tested suspicious markup escaping and disconnected state. The full demo toolbar, grid, inspector and metadata object views remain incomplete.

### 2026-11-04T13:50:13.631Z · PETER IRUNGU

Item 2 PARTIAL: Data/Structure/Indexes tab nav extracted to templates/components/relation_tabs.html (structure + indexes pages). catalog.html and data_page.html still carry their own nav markup (data_page differs: Data is a button). Items 6-8 are UI (Phase B).

### 2026-11-04T23:31:43.029Z · PETER IRUNGU

Phase 2 progress (2026-11-05): explorer.html, schema_tree, relation_tree, data_page, structure_page, indexes_page, catalog, profile_list, settings and login now emit the demo regions (#side/#tree, #cbtn/#spop, #rs, #mainbody, .bar, .gw, #insp, .foot) with no global header, and tests assert no inline style or script. Still open: item 2 (some components still duplicated), item 7 (Advanced Overview and metadata object pages need the new backend metadata in Group 15), item 9 (principal view model reaches only the explorer and login pages) and item 11 (new-surface partials).
