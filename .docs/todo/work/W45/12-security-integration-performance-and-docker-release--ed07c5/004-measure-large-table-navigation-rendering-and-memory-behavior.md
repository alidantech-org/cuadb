---
{id: TSK-B51DF4691392,name: "Measure large-table navigation, rendering and memory behavior",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:09.345Z,updated_at: 2026-11-04T07:32:09.345Z,tags: [cuadb,release],depends_on: [TSK-54598487D0CE,TSK-8B8935A1684A],order: 4096}
---

# Measure large-table navigation, rendering and memory behavior

## Goal
Keep the viewer responsive on databases with many schemas, thousands of tables and large relations. Benchmark actual query plans and response sizes before introducing cache or concurrency complexity.

## Context
Rust alone does not make deep offset queries or unindexed sorts inexpensive. Typical bottlenecks are database round trips, large cells, HTML generation and excessive catalog enumeration. Metrics should separate database execution, rendering and total request time while avoiding record-content logging.

This task belongs to **12 — Security, integration, performance and Docker release**. The group outcome is: Prove CuaDB is safe and dependable as a local-first read-only database viewer for the supported engines. Ensure reproducible builds, meaningful automated tests, bounded query and memory behavior, documentation for real connections and a single production Docker container with one published UI port.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Create representative large-table and wide-table fixtures with safe synthetic data.
- [ ] Measure first-page, deep-page, sort, metadata and repeated HTMX navigation timings.
- [ ] Verify configured query deadlines, memory caps and bounded response payload sizes.
- [x] Record and address clear hotspots using query plans and targeted optimization.
- [x] Document practical limits and when cursor pagination is available or unavailable.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:34:18.974Z · PETER IRUNGU

Items 4,5 verified; 1-3 partial. docs/performance.md: 1M-row tables on 4 engines, first/deep page, indexed/unindexed sort, filter scan, metadata, export, 100 ms timeout. Hotspot found by timing and fixed: NULLS-LAST wrapper defeated indexes (SQLite 4 s -> 3 ms, MariaDB 206 -> 3 ms). Left open: no wide-table fixture (item 1), no repeated-HTMX-navigation or memory-use measurement (items 2,3), and query plans were not captured (PostgreSQL sort id desc 310 ms is unexplained).
