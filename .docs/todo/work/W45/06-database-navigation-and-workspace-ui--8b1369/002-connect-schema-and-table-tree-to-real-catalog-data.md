---
{id: TSK-5EE171C4C580,name: Connect schema and table tree to real catalog data,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:06.091Z,updated_at: 2026-11-04T07:32:06.091Z,tags: [cuadb,explorer-ui],depends_on: [TSK-A513FD340BD5,TSK-5FC58B7B6E27,TSK-9890D0090922],order: 2048}
---

# Connect schema and table tree to real catalog data

## Goal
Show accessible databases or configured connections, schemas and supported relation types in a collapsible tree. Clicking a table should select it, update the main panel and preserve clear visual focus without a full-page reload.

## Context
The PostgreSQL adapter supplies typed relation metadata, while the sidebar must not expose inaccessible objects or raw catalog rows. Schemas can be expanded on demand, and common types need distinct labels. Name filtering should be quick and should not force loading every record from every relation.

This task belongs to **06 — Database navigation and workspace UI**. The group outcome is: Replace the demo shell with a clean, responsive database explorer focused on fast navigation. The left sidebar should display connections, schemas and relation names; the main panel should immediately show data or a meaningful empty state. Preserve the lightweight Rust-rendered design without React, Next.js or a frontend state library.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Load available schema nodes and relation lists through real adapter-backed routes.
- [ ] Support expand/collapse and distinguish tables, views and partition parents.
- [ ] Implement empty schema and permission-denied states without broken tree nodes.
- [ ] Preserve selected relation and active tab across HTMX navigation.
- [ ] Test tree behavior with many relations and unusual names.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
