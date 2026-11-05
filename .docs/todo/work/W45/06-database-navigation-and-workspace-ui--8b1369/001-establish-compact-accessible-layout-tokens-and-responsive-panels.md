---
{id: TSK-A513FD340BD5,name: "Establish compact accessible layout, tokens and responsive panels",type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:06.015Z,updated_at: 2026-11-05T00:19:28.541Z,tags: [cuadb,explorer-ui],depends_on: [TSK-FFD6A5B94574],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:11.209Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:16.418Z},{from: blocked,to: planned,at: 2026-11-04T23:18:19.787Z},{from: planned,to: in-progress,at: 2026-11-05T00:19:28.541Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-A513FD340BD5/1793791751211-start,started_at: 2026-11-04T11:29:11.231Z,end_ref: refs/tracking/TSK-A513FD340BD5/1793808856428-end,ended_at: 2026-11-04T16:14:16.418Z,summary: {files: 91,insertions: 10231,deletions: 706}},{base_ref: refs/tracking/TSK-A513FD340BD5/1793837968543-start,started_at: 2026-11-05T00:19:28.577Z}]}},links: [{type: blocks,task: TSK-5EE171C4C580}]}
---

# Establish compact accessible layout, tokens and responsive panels

## Goal
Create a cohesive app frame with navigation, contextual toolbar, connection status and primary content area. Optimize for desktop and laptop widths, while preserving a useful fallback on narrow screens and accessible keyboard focus.

## Context
The starter uses utility classes directly inside one page. As the viewer grows, common colors, spacing, hover/focus states and dense table styles should be centralized. Tailwind builds must include all Askama templates, and specialized CSS can handle grid overflow and resizable panels without complex JS libraries.

This task belongs to **06 — Database navigation and workspace UI**. The group outcome is: Replace the demo shell with a clean, responsive database explorer focused on fast navigation. The left sidebar should display connections, schemas and relation names; the main panel should immediately show data or a meaningful empty state. Preserve the lightweight Rust-rendered design without React, Next.js or a frontend state library.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Refactor repeated presentation into reusable Askama components and theme tokens.
- [x] Create accessible sidebar, toolbar, content area and empty/disconnected layouts.
- [x] Ensure responsive behavior does not hide essential table navigation controls.
- [x] Add visible focus and dark/light theme primitives with adequate contrast.
- [x] Verify final build uses only bundled CSS and JS assets.

## Canonical UI fidelity

**Mandatory visual specification:** `assets/cuadb-ui-demo.html` (see `.docs/docs/frontend-interactions.md`). Its sidebar, interactions, state patterns and typography are required, not decorative inspiration. Recreate the reference with Askama/HTMX, Tailwind and minimal JavaScript using actual database data; do not replace the interface with a conventional dashboard. Safety and accessible real-data states may adapt the underlying mechanism, not silently redesign the appearance.

### Reference-specific acceptance
- [x] Match the demo's full-height workspace composition with no separate global header: default 264px sidebar, 5px splitter, bottom connection/settings row, compact main toolbar and dense content grid.
- [x] Use the demo's light/dark neutral surfaces and teal accent, 12.5px default font, 6px corner radius, subtle 26px controls and CSS color/density tokens instead of arbitrary Tailwind defaults.
- [x] Implement 180–520px draggable sidebar with double-click reset to 264px and responsive sidebar and inspector treatment at or below 760px.
- [x] Capture matched reference-versus-app screenshots at 1440×900, 1280×800 and 390×844 in light and dark themes; request approval for any material visual deviation.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [x] Port the demo's new tokens and components: danger colour token, `.tag.m-none|read|write|destructive` access tags, `#modal` scaffold, login card, form field and `textarea`/`select` styles, `.btn.dg` and error text.
- [ ] The sidebar footer shows the connection picker, the database picker and the settings gear exactly as in the updated demo, with the SQL workspace button available only when the capability is enabled.

## Comments

### 2026-11-04T11:29:34.077Z · PETER IRUNGU

Replaced generic shell/header layout with compact responsive two-panel explorer shell, canonical sidebar/search/main areas, resize rail, settings affordance, theme tokens and mobile drawer breakpoint. Detailed visual comparison and full preference/menu controls remain incomplete.

### 2026-11-04T12:22:43.338Z · PETER IRUNGU

Initial accessible shell, responsive drawer, resizer persistence and real schema tree implemented. The canonical demo requires additional Simple/Advanced navigation, exact connection/settings controls and screenshot comparison; keep acceptance unchecked until those are delivered.

### 2026-11-04T16:14:16.603Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.

### 2026-11-04T23:31:32.524Z · PETER IRUNGU

Phase 2 progress (2026-11-05): the shell now ports the updated demo tokens and components (app.css is built from the demo CSS), with sidebar search, schema tree, bottom connection picker and settings gear, 5px splitter (drag verified 264 to 360 and double-click reset to 264, persisted), mobile drawer at 760px and narrower, light/dark/palette preferences via theme.js, login card, modal scaffold and danger styles. Verified in the built-in browser against disposable SQLite data at about 1280x800 light and dark and 390x844 (drawer, data grid, toolbar wrap). NOT done and left unticked: item 9 (paired screenshots at 1440x900, 1280x800 and 390x844 in both themes) and item 11 (database picker). Decision: a profile is one connection plus one database in the current backend, so a separate database picker is deferred until the adapter can list databases; the connection picker is built.

### 2026-11-05T00:19:38.433Z · PETER IRUNGU

Phase 2 (2026-11-05): item 9 done, with all six app combinations (1440x900, 1280x800 and 390x844, light and dark) captured and the demo compared side by side at 1440 light, 1280 dark and 390. Item 11 stays open: there is no separate database picker because one connection is one database in the current backend and adapters cannot list databases. Owner decision needed: accept that deviation, or ask for the adapter work.
