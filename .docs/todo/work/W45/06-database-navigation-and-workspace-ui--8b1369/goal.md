---
{id: GOAL-GRP-51BFCC8B1369,name: 06 — Database navigation and workspace UI goal,type: documentation,extra: {created_at: 2026-11-04T07:32:05.939Z,group: GRP-51BFCC8B1369}}
---

## Phase gate

This is Phase 2 frontend work. Do not start or continue implementation until the PostgreSQL + SQLite backend groups (01–05 and 10) are accepted. The backend must provide real, stable data and HTML-fragment contracts first. Do not add MySQL/MariaDB checks to frontend tasks. Keep the committed `assets/cuadb-ui-demo.html` as the binding reference and perform focused visual acceptance at the group boundary, not repeated full screenshot matrices for every small change.
# 06 — Database navigation and workspace UI — Goal

## Outcome
Replace the demo shell with a clean, responsive database explorer focused on fast navigation. The left sidebar should display connections, schemas and relation names; the main panel should immediately show data or a meaningful empty state. Preserve the lightweight Rust-rendered design without React, Next.js or a frontend state library.

## Definition of done
Acceptance of **06 — Database navigation and workspace UI** requires evidence for these results:
1. **Establish compact accessible layout, tokens and responsive panels:** Create a cohesive app frame with navigation, contextual toolbar, connection status and primary content area. Optimize for desktop and laptop widths, while preserving a useful fallback on narrow screens and accessible keyboard focus. Verification includes: Refactor repeated presentation into reusable Askama components and theme tokens.
2. **Connect schema and table tree to real catalog data:** Show accessible databases or configured connections, schemas and supported relation types in a collapsible tree. Clicking a table should select it, update the main panel and preserve clear visual focus without a full-page reload. Verification includes: Load available schema nodes and relation lists through real adapter-backed routes.
3. **Add fast relation-name search and navigation breadcrumbs:** Let users locate a table or view quickly even in databases containing hundreds of relations. Provide contextual schema and relation breadcrumbs so users understand the current selection without relying on the sidebar alone. Verification includes: Search schema and relation names without reading underlying data rows.
4. **Provide coherent disconnected, loading and error navigation states:** Make the interface trustworthy at every stage of database initialization. A user must be able to tell whether CuaDB itself is running, whether a database is connected and whether a selected table is loading or inaccessible. Verification includes: Render accurate connection indicators distinct from server health.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.

## Mandatory visual acceptance

The canonical frontend specification for this group is `assets/cuadb-ui-demo.html`, reinforced by `.docs/docs/frontend-interactions.md`. It is a binding acceptance baseline for all related tasks, not an alternative design sketch. Completion requires a close visual replica of the standalone demonstration's 264px sidebar, narrow resizer, search field, nested Simple/Advanced explorer, icons, bottom connection/settings area, compact typography, theme colors and <=760px drawer layout. Compare matched screenshots in light and dark modes at desktop and mobile widths. The demo is a requirement, not a source of optional design inspiration; material changes need approval. All relevant tasks must pass a side-by-side comparison against the same browser viewport, mode and selected object state, and must not be marked complete based on a different layout.

## Phase 2 update (2026-11-05)

The owner accepted the PostgreSQL + SQLite backend gate (`.docs/docs/backend-gate-decision.md`) and started Phase 2. The gate wording above is historical. The updated `assets/cuadb-ui-demo.html` is the look-and-feel baseline and may be improved where it makes the product better, and the work must follow the CSP, CSRF and manual browser-verification rules in `.docs/docs/frontend-interactions.md`.
