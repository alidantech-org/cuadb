---
{id: TSK-5860FD5C106E,name: "Polish non-sensitive preferences, navigation and keyboard shortcuts",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:08.707Z,updated_at: 2026-11-04T07:32:08.707Z,tags: [cuadb,profiles],depends_on: [TSK-4C7273B9E46A,TSK-470D81309D89],order: 3072}
---

# Polish non-sensitive preferences, navigation and keyboard shortcuts

## Goal
Remember convenient presentation preferences such as theme, page size and sidebar width without persisting database rows, raw SQL or credentials in the browser. Make common explorer actions easy to discover and use.

## Context
This task is about UX state, not connection or record storage. Any client-side settings should be strictly allowlisted and small. Global shortcuts should not interfere with browser or text-input behavior, while focus management must remain correct after HTMX swaps.

This task belongs to **11 — Connection profiles and everyday usability**. The group outcome is: Allow safe switching among multiple known database connections while keeping credentials out of the browser and eliminating the need to repeatedly reconfigure the viewer. Improve the everyday explorer experience with durable, non-sensitive preferences and sensible keyboard shortcuts.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Persist theme and selected grid presentation settings without saving database values.
- [ ] Add discoverable shortcuts for search, refresh and navigation where accessible.
- [ ] Ensure preference defaults work when local browser storage is unavailable.
- [ ] Test across page reloads, HTMX swaps and keyboard-only navigation.
- [ ] Verify that no secrets, row snapshots or connection URLs enter persisted settings.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
