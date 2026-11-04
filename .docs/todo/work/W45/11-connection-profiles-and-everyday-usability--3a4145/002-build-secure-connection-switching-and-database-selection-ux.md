---
{id: TSK-4C7273B9E46A,name: Build secure connection switching and database selection UX,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:08.583Z,updated_at: 2026-11-04T07:32:08.583Z,tags: [cuadb,profiles],depends_on: [TSK-B92248F7876E,TSK-61DA50E7C274],order: 2048}
---

# Build secure connection switching and database selection UX

## Goal
Offer an understandable connection picker, accurately update the schema tree and selected relation, and avoid retaining stale rows from a previously selected database. Keep navigation responsive without storing secrets on the client.

## Context
Switching connections changes the database trust boundary, catalog and permissions. HTMX requests must be tied to a stable configured profile ID and not accept arbitrary database hosts. Browser navigation should not display records from another profile after rapid switching or a failed connection.

This task belongs to **11 — Connection profiles and everyday usability**. The group outcome is: Allow safe switching among multiple known database connections while keeping credentials out of the browser and eliminating the need to repeatedly reconfigure the viewer. Improve the everyday explorer experience with durable, non-sensitive preferences and sensible keyboard shortcuts.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Render available trusted profile labels with clear connected and offline states.
- [ ] Reset or scope table selection, sorting and cursors when a profile changes.
- [ ] Reject unknown or removed profile identifiers without exposing connection details.
- [ ] Test rapid switching and inaccessible databases for stale response leakage.
- [ ] Document the connection selection behavior in the README.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
