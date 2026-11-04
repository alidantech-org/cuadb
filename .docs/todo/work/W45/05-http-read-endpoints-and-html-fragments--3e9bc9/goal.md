---
{id: GOAL-GRP-63970A3E9BC9,name: 05 — HTTP read endpoints and HTML fragments goal,type: documentation,extra: {created_at: 2026-11-04T07:32:05.597Z,group: GRP-63970A3E9BC9}}
---

---
{id: GOAL-GRP-63970A3E9BC9,name: 05 — HTTP read endpoints and HTML fragments goal,type: documentation,extra: {created_at: 2026-11-04T07:32:05.597Z,group: GRP-63970A3E9BC9}}
---

# 05 — HTTP read endpoints and HTML fragments — Goal

## Outcome
Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.

## Definition of done
Acceptance of **05 — HTTP read endpoints and HTML fragments** requires evidence for these results:
1. **Implement typed read-only Axum route contracts for browsing:** Design and implement HTTP GET endpoints for connection status, schemas, tables, data rows, structure and info. Routes should delegate to adapters, return stable status codes and support both direct navigation and HTMX requests. Verification includes: Define documented route paths and typed path/query request parsing.
2. **Build Askama view models and reusable fragment templates:** Create compile-time-checked templates and data-focused view models for connection state, table navigation, rows and metadata. Keep display logic outside SQL implementation and ensure all database-provided strings are safely escaped. Verification includes: Create typed view models for sidebar tree, relation header, grid and inspector tabs.
3. **Harden browser responses, headers and fragment cache behavior:** Protect local database content from accidental exposure through HTTP caches, browser history snapshots and cross-origin interactions. Keep web access on localhost by default and document the boundary for any later remote deployment. Verification includes: Add no-store cache behavior to all authenticated or database-bearing HTML responses.
4. **Design HTMX loading, cancellation, empty and error contracts:** Make asynchronous data navigation predictable during slow queries, network interruptions and permission failures. Stale responses should not overwrite the currently selected table, and errors should render in a consistent, accessible way. Verification includes: Define swap targets, request synchronization and stable loading indicators per fragment.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.

## Mandatory visual acceptance

The canonical frontend specification for this group is `assets/cuadb-ui-demo.html`, reinforced by `.docs/docs/frontend-interactions.md`. It is a binding acceptance baseline for all related tasks, not an alternative design sketch. HTTP pages and HTMX fragments must map to the actual demonstration regions, including the small main toolbar, Data/Structure/Indexes selector, sidebar, grid, inspector, Overview and browser popovers. The same layout must survive slow, empty, permission-restricted and disconnected states; new generic interface shells are not accepted. Server-side query and export endpoints must preserve the demo controls while enforcing access controls, timeouts, resource bounds and confidential browser output. All relevant tasks must pass a side-by-side comparison against the same browser viewport, mode and selected object state, and must not be marked complete based on a different layout.
