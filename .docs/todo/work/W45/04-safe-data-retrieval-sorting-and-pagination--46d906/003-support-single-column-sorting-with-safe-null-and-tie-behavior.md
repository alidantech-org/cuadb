---
{id: TSK-84DCF1B4A953,name: Support single-column sorting with safe null and tie behavior,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:05.404Z,updated_at: 2026-11-04T11:14:05.513Z,tags: [cuadb,postgres-rows],depends_on: [TSK-23B4836BB3AE,TSK-047C7016EEAA],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:14:04.978Z},{from: in-progress,to: completed,at: 2026-11-04T11:14:05.513Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-84DCF1B4A953/1793790844981-start,started_at: 2026-11-04T11:14:04.997Z,end_ref: refs/tracking/TSK-84DCF1B4A953/1793790845515-end,ended_at: 2026-11-04T11:14:05.513Z,summary: {files: 0,insertions: 0,deletions: 0}}]}}}
---

# Support single-column sorting with safe null and tie behavior

## Goal
Allow ascending and descending sorting on valid columns while preserving deterministic ties where feasible. Make sorting parameters explicit and ensure the UI cannot request arbitrary expressions or issue unbounded database work.

## Context
PostgreSQL exposes NULLS FIRST/LAST behavior and case-sensitive identifiers. To avoid duplicates across page boundaries, a unique key may be added as a tie-breaker when compatible. For unsupported or expensive sorts, return a clear user-facing explanation instead of silently sorting data in the browser.

This task belongs to **04 — Safe data retrieval, sorting and pagination**. The group outcome is: Implement the core read-only viewing capability with bounded, predictable SQL and correct typed values. A user must be able to open any permitted PostgreSQL table, read a page, sort an allowed column and navigate safely without requesting the entire relation. This group is the performance and query-security center of the project.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Accept only catalog-listed columns and an explicit ascending or descending direction.
- [x] Define NULL ordering and unique tie-breaking behavior with repeatable SQL generation.
- [x] Test mixed NULL values, duplicate sort values, text and timestamp columns.
- [x] Enforce row-size limits, query timeout and rejected unsupported sort requests.
- [x] Document behavior when an index is unavailable or ordering cannot be stable.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:14:05.291Z · PETER IRUNGU

Verified ASC/DESC allowlisted sorting, explicit NULLS LAST SQL, unique-key tie-breaking, rejected unsupported/unknown columns, bounded execution, and documentation. Unit, HTTP, live PostgreSQL fixture, and live descending-sort HTTP checks passed.
