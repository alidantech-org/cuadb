---
{id: TSK-1C5E54BDB641,name: Add fast relation-name search and navigation breadcrumbs,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:06.170Z,updated_at: 2026-11-04T07:32:06.170Z,tags: [cuadb,explorer-ui],depends_on: [TSK-5EE171C4C580],order: 3072}
---

# Add fast relation-name search and navigation breadcrumbs

## Goal
Let users locate a table or view quickly even in databases containing hundreds of relations. Provide contextual schema and relation breadcrumbs so users understand the current selection without relying on the sidebar alone.

## Context
CuaDB is not a SQL search engine. The initial search filters relation names and schemas only; it must not query table contents or full-scan the database. Large trees may need server-side filtering or lazy loading, while small lists can be filtered locally with minimal JavaScript.

This task belongs to **06 — Database navigation and workspace UI**. The group outcome is: Replace the demo shell with a clean, responsive database explorer focused on fast navigation. The left sidebar should display connections, schemas and relation names; the main panel should immediately show data or a meaningful empty state. Preserve the lightweight Rust-rendered design without React, Next.js or a frontend state library.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Search schema and relation names without reading underlying data rows.
- [ ] Preserve navigation context and selection when clearing or changing the search.
- [ ] Provide keyboard navigation and clear empty-results messaging.
- [ ] Test special characters, case sensitivity and large table lists.
- [ ] Keep search requests bounded and debounce remote requests when used.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
