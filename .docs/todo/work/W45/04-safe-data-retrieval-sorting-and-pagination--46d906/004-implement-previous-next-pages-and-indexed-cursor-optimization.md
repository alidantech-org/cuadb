---
{id: TSK-8B8935A1684A,name: Implement previous/next pages and indexed cursor optimization,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:05.474Z,updated_at: 2026-11-04T11:14:06.159Z,tags: [cuadb,postgres-rows],depends_on: [TSK-84DCF1B4A953],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:14:05.714Z},{from: in-progress,to: completed,at: 2026-11-04T11:14:06.159Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-8B8935A1684A/1793790845717-start,started_at: 2026-11-04T11:14:05.734Z,end_ref: refs/tracking/TSK-8B8935A1684A/1793790846160-end,ended_at: 2026-11-04T11:14:06.159Z,summary: {files: 0,insertions: 0,deletions: 0}}]}}}
---

# Implement previous/next pages and indexed cursor optimization

## Goal
Provide predictable page navigation using bounded offsets for ordinary tables and keyset paging when a suitable index and sort order exist. Keep page tokens opaque, validated and scoped to the chosen relation and sort.

## Context
Deep OFFSET is increasingly expensive on large tables. Cursor-based reads can improve performance but require a stable unique ordering and careful handling of NULL values. This task should introduce an explicit pagination strategy rather than promising keyset navigation on tables that cannot support it.

This task belongs to **04 — Safe data retrieval, sorting and pagination**. The group outcome is: Implement the core read-only viewing capability with bounded, predictable SQL and correct typed values. A user must be able to open any permitted PostgreSQL table, read a page, sort an allowed column and navigate safely without requesting the entire relation. This group is the performance and query-security center of the project.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Implement previous/next states and safe bounds for requested page numbers or cursors.
- [x] Choose keyset pagination only when a suitable unique key and ordering are available.
- [x] Validate or sign opaque cursors to prevent alteration across relations or sorts.
- [x] Test empty, last-page, duplicate-key, NULL and changing-row scenarios.
- [x] Document consistency limitations when concurrent database writes occur.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:14:05.999Z · PETER IRUNGU

Verified empty/last-page states, bounded offset fallback, eligible integer unique-key cursor paging, previous/next navigation, 256-bit OS-random opaque tokens, scope/tamper rejection, expiry/restart behavior by construction, and documented concurrent-write limitations.
