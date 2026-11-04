---
{id: TSK-A513FD340BD5,name: "Establish compact accessible layout, tokens and responsive panels",type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:06.015Z,updated_at: 2026-11-04T11:29:11.209Z,tags: [cuadb,explorer-ui],depends_on: [TSK-FFD6A5B94574],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:11.209Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-A513FD340BD5/1793791751211-start,started_at: 2026-11-04T11:29:11.231Z}]}}}
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
- [ ] Create accessible sidebar, toolbar, content area and empty/disconnected layouts.
- [ ] Ensure responsive behavior does not hide essential table navigation controls.
- [ ] Add visible focus and dark/light theme primitives with adequate contrast.
- [ ] Verify final build uses only bundled CSS and JS assets.

## Canonical UI fidelity

**Mandatory visual specification:** `assets/cuadb-ui-demo.html` (see `.docs/docs/frontend-interactions.md`). Its sidebar, interactions, state patterns and typography are required, not decorative inspiration. Recreate the reference with Askama/HTMX, Tailwind and minimal JavaScript using actual database data; do not replace the interface with a conventional dashboard. Safety and accessible real-data states may adapt the underlying mechanism, not silently redesign the appearance.

### Reference-specific acceptance
- [ ] Match the demo's full-height workspace composition with no separate global header: default 264px sidebar, 5px splitter, bottom connection/settings row, compact main toolbar and dense content grid.
- [ ] Use the demo's light/dark neutral surfaces and teal accent, 12.5px default font, 6px corner radius, subtle 26px controls and CSS color/density tokens instead of arbitrary Tailwind defaults.
- [ ] Implement 180–520px draggable sidebar with double-click reset to 264px and responsive sidebar and inspector treatment at or below 760px.
- [ ] Capture matched reference-versus-app screenshots at 1440×900, 1280×800 and 390×844 in light and dark themes; request approval for any material visual deviation.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:29:34.077Z · PETER IRUNGU

Replaced generic shell/header layout with compact responsive two-panel explorer shell, canonical sidebar/search/main areas, resize rail, settings affordance, theme tokens and mobile drawer breakpoint. Detailed visual comparison and full preference/menu controls remain incomplete.

### 2026-11-04T12:22:43.338Z · PETER IRUNGU

Initial accessible shell, responsive drawer, resizer persistence and real schema tree implemented. The canonical demo requires additional Simple/Advanced navigation, exact connection/settings controls and screenshot comparison; keep acceptance unchecked until those are delivered.
