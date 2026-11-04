---
{id: TSK-0AA246B508ED,name: Implement Table Info tab with honest statistics and storage,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:07.130Z,updated_at: 2026-11-04T07:32:07.130Z,tags: [cuadb,inspectors],depends_on: [TSK-CFEA5F712D4B,TSK-FFD6A5B94574],order: 2048}
---

# Implement Table Info tab with honest statistics and storage

## Goal
Display useful measurements including estimated rows, table data bytes, index bytes, combined size and relation type. Every value must be clearly identified as exact, estimated or unavailable where applicable.

## Context
PostgreSQL pg_stat and size functions expose useful metrics but have important semantics for partitioned and foreign tables. Expensive exact counts are excluded from default navigation. Cached values may be stale; the interface should offer a lightweight explicit refresh action.

This task belongs to **08 — Structure, statistics and PostgreSQL rich values**. The group outcome is: Complete the table inspection experience through Structure and Info panels and careful handling of PostgreSQL-specific value types. Users should inspect data definitions and storage without complex administration screens, and PostgreSQL/PostGIS objects should remain readable even when they do not map to common scalar types.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Render adapter-provided sizes, estimated rows and relation classification clearly.
- [ ] Explain estimates, unknown values and partition-related size caveats in the UI.
- [ ] Provide a bounded refresh action without forcing SELECT COUNT(*) over large tables.
- [ ] Test empty, partitioned, foreign and permission-restricted fixture scenarios.
- [ ] Use the same capability flags that other database adapters will expose.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
