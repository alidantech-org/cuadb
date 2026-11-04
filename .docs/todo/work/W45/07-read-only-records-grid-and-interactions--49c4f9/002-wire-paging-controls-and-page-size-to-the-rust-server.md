---
{id: TSK-16AEEF4061F8,name: Wire paging controls and page size to the Rust server,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:06.572Z,updated_at: 2026-11-04T07:32:06.572Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-BE512E99EA68,TSK-8B8935A1684A],order: 2048}
---

# Wire paging controls and page size to the Rust server

## Goal
Let users move between pages and choose common bounded sizes such as 25, 50 or 100 rows. Paging must preserve the selected relation and sort while clearly indicating whether next and previous pages are available.

## Context
The backend chooses an offset or keyset strategy based on table indexes. The UI must not promise exact total pages when only approximate row statistics are available. HTMX swaps should avoid resetting unrelated sidebar state or accidentally rendering data from an older request.

This task belongs to **07 — Read-only records grid and interactions**. The group outcome is: Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Render previous/next buttons using actual backend continuation state.
- [ ] Offer bounded page-size choices without allowing arbitrary large values.
- [ ] Preserve relation, sorting and pagination cursor during HTMX navigation.
- [ ] Handle first, last and empty pages with accurate disabled states.
- [ ] Test rapid paging and stale request behavior against a fake adapter.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
