---
{id: TSK-5689F3668ED7,name: Map MySQL/MariaDB sizes and metadata limitations to Info tab,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:07.816Z,updated_at: 2026-11-04T07:32:07.816Z,tags: [cuadb,mysql],depends_on: [TSK-242B6B658703,TSK-0AA246B508ED],order: 4096}
---

# Map MySQL/MariaDB sizes and metadata limitations to Info tab

## Goal
Display accurate available metadata such as estimated rows, storage and index bytes while making engine-specific limitations visible. The UI should not pretend exact PostgreSQL-equivalent statistics are always available.

## Context
Storage engines and server versions can report estimates differently, and INFORMATION_SCHEMA values may be stale or missing. Table and index sizes must be labeled according to the underlying engine semantics. The core capability interface should make these distinctions explicit without special-casing templates.

This task belongs to **09 — MySQL and MariaDB compatibility**. The group outcome is: Bring MySQL and MariaDB into CuaDB through a separate adapter while preserving the same read-only navigation and data-grid contract. Reuse common models and browser components, but implement engine-specific discovery, identifier quoting, data decoding and statistics rather than assuming PostgreSQL SQL syntax works everywhere.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Read supported table and index size metrics without costly full-table scans.
- [ ] Label approximate row counts and unsupported storage values honestly.
- [ ] Verify behavior with views, empty tables and different storage engines where available.
- [ ] Keep the Structure and Info tabs reusable across PostgreSQL and MySQL adapters.
- [ ] Document MySQL/MariaDB-specific caveats and validated database versions.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
