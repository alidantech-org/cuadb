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

## Canonical UI fidelity

The committed `assets/cuadb-ui-demo.html` is the binding reference. Implement the exact applicable control positions, popup states, theme/size options and behavior as required by `.docs/docs/frontend-interactions.md`. Demo profiles and records are illustrative only; real credentials stay server-side and real data is fetched by safe, read-only Rust endpoints. Do not treat visual similarity alone as completion if an existing demo control does not work.

### Reference-specific acceptance
- [ ] Reproduce every existing demo settings control: Simple/Advanced; Auto/Light/Dark; Mist/Slate/Sand/Mono; accent presets/custom color; Sharp/Soft/Round corners; text S/M/L and Sans/Serif/Mono and monospace data.
- [ ] Reproduce Grid choices XS/S/M/L, Rows/Grid/None lines, NULL/∅/Blank, row numbers, type labels, striped rows, wrapping and thousands separators; Sidebar icon mode, rows/size counts and unit; and per-table sort/hidden-column memory.
- [ ] Match the demo's grouped compact settings popover with segmented controls, toggles, reset-this-table/reset-all and immediate visual application with non-sensitive persistence.
- [ ] Keep settings, Escape and Ctrl/Cmd+K navigation functional after HTMX swaps with no additional frontend framework.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [ ] The settings popover matches the updated demo (Account block with Sign out and Administration, Explorer, Theme, Type, Grid, Sidebar, Tables) and preferences persist only non-sensitive choices.
