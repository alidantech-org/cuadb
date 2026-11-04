---
{id: TSK-BE512E99EA68,name: Render typed records in an efficient semantic HTML grid,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:06.442Z,updated_at: 2026-11-04T07:32:06.442Z,tags: [cuadb,data-grid-ui],depends_on: [TSK-8DB2E738839F,TSK-5EE171C4C580],order: 1024}
---

# Render typed records in an efficient semantic HTML grid

## Goal
Turn adapter row pages into a clean table with stable column ordering, readable type formatting and clear NULL values. Keep the layout usable with dozens of columns and avoid loading unnecessary data on the client.

## Context
Database tables frequently have mixed scalar, JSON and binary columns. Large content should be abbreviated in a predictable way and accessible on demand. HTML output must always escape values, and the selected relation name must be shown explicitly so users know which data they are viewing.

This task belongs to **07 — Read-only records grid and interactions**. The group outcome is: Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Render row headers, column labels, typed values and explicit NULL markers safely.
- [ ] Provide horizontal scrolling and sticky headers on narrow or wide tables.
- [ ] Show accessible empty-table and unsupported-type states without fake rows.
- [ ] Bound initial rendered cell content and preserve a path for expanded inspection.
- [ ] Verify XSS escaping and layout behavior with fixture records.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
