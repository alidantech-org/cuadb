---
{id: TSK-A513FD340BD5,name: "Establish compact accessible layout, tokens and responsive panels",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:06.015Z,updated_at: 2026-11-04T07:32:06.015Z,tags: [cuadb,explorer-ui],depends_on: [TSK-FFD6A5B94574],order: 1024}
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
- [ ] Refactor repeated presentation into reusable Askama components and theme tokens.
- [ ] Create accessible sidebar, toolbar, content area and empty/disconnected layouts.
- [ ] Ensure responsive behavior does not hide essential table navigation controls.
- [ ] Add visible focus and dark/light theme primitives with adequate contrast.
- [ ] Verify final build uses only bundled CSS and JS assets.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
