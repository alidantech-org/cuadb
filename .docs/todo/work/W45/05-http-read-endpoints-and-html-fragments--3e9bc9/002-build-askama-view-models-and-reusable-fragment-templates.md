---
{id: TSK-FFD6A5B94574,name: Build Askama view models and reusable fragment templates,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:05.740Z,updated_at: 2026-11-04T07:32:05.740Z,tags: [cuadb,http],depends_on: [TSK-DC88ADC32B93],order: 2048}
---

# Build Askama view models and reusable fragment templates

## Goal
Create compile-time-checked templates and data-focused view models for connection state, table navigation, rows and metadata. Keep display logic outside SQL implementation and ensure all database-provided strings are safely escaped.

## Context
Askama templates are currently limited to a demo page. The real explorer should reuse consistent rendering logic for a full-page load and an HTMX partial update. Empty, restricted and offline states must be first-class view model variants rather than a tangle of template conditionals.

This task belongs to **05 — HTTP read endpoints and HTML fragments**. The group outcome is: Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Create typed view models for sidebar tree, relation header, grid and inspector tabs.
- [ ] Reuse components across full pages and HTMX responses without duplicate markup.
- [ ] Verify HTML escaping for suspicious table names, cell contents and error messages.
- [ ] Render explicit empty and inaccessible states with clear next actions.
- [ ] Keep template code free of direct driver calls or raw SQL statements.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
