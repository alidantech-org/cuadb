---
{id: TSK-EF78B53DDF3C,name: "Add SQL Server bounded rows, sorting and honest Info metadata",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:09.946Z,updated_at: 2026-11-04T07:32:09.946Z,tags: [cuadb,mssql],depends_on: [TSK-C00E0A305361],order: 3072}
---

# Add SQL Server bounded rows, sorting and honest Info metadata

## Goal
Complete basic SQL Server browsing with paginated data, safe column sorting and available table-level metadata, using the same grid and inspector templates already supported by other engines.

## Context
SQL Server paging uses different syntax and can require ORDER BY. Type conversions, large values and available storage statistics may differ from PostgreSQL. The adapter must define reliable sort behavior, apply query deadlines and expose metrics only when they can be retrieved with the configured privileges.

This task belongs to **13 — SQL Server adapter (post-core expansion)**. The group outcome is: Evaluate and implement Microsoft SQL Server support after the PostgreSQL, MySQL/MariaDB and SQLite viewer is proven. Preserve the same read-only explorer, data grid and metadata contract while respecting SQL Server-specific drivers, namespaces, catalog views and pagination requirements.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Fetch bounded pages and enforce relation/column allowlists for sorting.
- [ ] Normalize common scalar and large-value types into the shared cell model.
- [ ] Report supported constraints and Info statistics with clear availability flags.
- [ ] Exercise real end-to-end browser flows using SQL Server fixture data.
- [ ] Document supported driver/server versions and unresolved engine limitations.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
