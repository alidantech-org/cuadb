---
{id: TSK-470D81309D89,name: "Polish grid resizing, keyboard navigation and accessibility",type: task,status: blocked,priority: medium,created_at: 2026-11-04T07:32:06.902Z,updated_at: 2026-11-04T16:15:21.389Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-812F816785FD,TSK-ABE2D51C83F3],order: 5120,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:42.035Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:19.361Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-470D81309D89/1793791782037-start,started_at: 2026-11-04T11:29:42.052Z,end_ref: refs/tracking/TSK-470D81309D89/1793808859363-end,ended_at: 2026-11-04T16:14:19.361Z,summary: {files: 91,insertions: 10231,deletions: 706}}]}},links: [{type: blocks,task: TSK-A7B0DFC23314}]}
---

# Polish grid resizing, keyboard navigation and accessibility

## Goal
Make dense database browsing comfortable with the keyboard and usable for assistive technology. Keep client-side behavior lightweight and resilient across HTMX fragment replacement.

## Context
Resizable columns and shortcuts improve a developer tool, but custom browser behavior must not interfere with native table semantics. Delegate events from stable containers so swapped rows continue to work. Do not add a complete frontend framework or local data state store for these small interactions.

This task belongs to **07 — Read-only records grid and interactions**. The group outcome is: Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Support focused table navigation, copy shortcuts and escape-to-close behavior.
- [ ] Add optional column width adjustments without breaking responsive overflow.
- [ ] Keep hover, focus, loading and error states readable in dark and light modes.
- [ ] Verify event handlers still work after multiple HTMX swaps.
- [ ] Test keyboard-only navigation and automated accessibility checks where feasible.

## Canonical UI fidelity

The **required frontend specification** is `assets/cuadb-ui-demo.html` and the comparison contract in `.docs/docs/frontend-interactions.md`. Match the demo's visible interface, controls and applicable interactions accurately, using real database data and safe read-only backend pagination/export behavior instead of the prototype's synthetic arrays. No generic table-grid replacement or materially different layout is acceptable without user-approved documentation.

### Reference-specific acceptance
- [ ] Match sidebar split-resizing, double-click reset, popover click-away dismissal, Escape, Ctrl/Cmd+K sidebar search, mobile hamburger and local feedback toasts.
- [ ] Keep all browser event handlers working after HTMX swaps, with keyboard accessibility, appropriate focus behavior and the demo's compact control appearance.
- [ ] Compare reference/app screenshots for default grid, selected-row inspector, Advanced navigation and mobile widths; document any nontrivial deviations rather than marking complete.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T16:14:19.507Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.
