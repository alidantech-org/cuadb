---
{id: TSK-61DA50E7C274,name: "Provide coherent disconnected, loading and error navigation states",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:06.252Z,updated_at: 2026-11-04T07:32:06.252Z,tags: [cuadb,explorer-ui],depends_on: [TSK-1C5E54BDB641,TSK-E68FB08CB73D],order: 4096}
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
- [ ] Render accurate connection indicators distinct from server health.
- [ ] Expose useful recovery guidance for offline or misconfigured database connections.
- [ ] Show accessible skeleton/loading and empty-content states without fake data.
- [ ] Prevent stale navigation fragments after fast table switching.
- [ ] Test accessibility announcements and focus retention during HTMX swaps.

## Canonical UI fidelity

**Mandatory visual specification:** `assets/cuadb-ui-demo.html` (see `.docs/docs/frontend-interactions.md`). Its sidebar, interactions, state patterns and typography are required, not decorative inspiration. Recreate the reference with Askama/HTMX, Tailwind and minimal JavaScript using actual database data; do not replace the interface with a conventional dashboard. Safety and accessible real-data states may adapt the underlying mechanism, not silently redesign the appearance.

### Reference-specific acceptance
- [ ] Use the same demo shell to show disconnected, connecting, offline and forbidden database states without displaying the demo's synthetic sample records as live content.
- [ ] Use compact connection badges, empty grid/sidebar regions and toast feedback instead of visually unrelated full-screen status cards.
- [ ] Preserve the mobile sidebar toggle and row inspector positioning, including recovery when the selected table or connection becomes unavailable.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
