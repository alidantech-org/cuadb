---
{id: TSK-0DB3969BA9F6,name: Expose SQLite capabilities and integration tests honestly,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:08.304Z,updated_at: 2026-11-04T16:15:21.227Z,tags: [cuadb,sqlite],depends_on: [TSK-7D658997F68C,TSK-0AA246B508ED],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T12:46:13.975Z},{from: in-progress,to: completed,at: 2026-11-04T14:46:18.924Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-0DB3969BA9F6/1793796373977-start,started_at: 2026-11-04T12:46:13.994Z,end_ref: refs/tracking/TSK-0DB3969BA9F6/1793803578927-end,ended_at: 2026-11-04T14:46:18.924Z,summary: {files: 55,insertions: 5538,deletions: 294}}]}},links: [{type: blocks,task: TSK-0AA246B508ED}]}
---

# Expose SQLite capabilities and integration tests honestly

## Goal
Complete SQLite support with verified Info and Structure behavior that clearly describes unavailable per-table size estimates. Ensure that read-only operation works with real mounted files in Docker as well as native development.

## Context
The SQLite database file has a measurable file size, but accurate data-versus-index storage per relation may be unavailable without optional extensions. Surface known file-level metadata instead of manufacturing PostgreSQL-equivalent numbers. Test fresh fixtures and locked or WAL-mode databases carefully.

This task belongs to **10 — SQLite file-based database adapter**. The group outcome is: Enable browsing local SQLite database files securely from the same CuaDB interface. A database file must be mounted into the container or available through a configured host path; the viewer may inspect it but must never edit the database or produce write side-effects as part of ordinary browsing.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Render usable SQLite database information with explicit unavailable metrics.
- [x] Test in-memory or disposable fixture files across relevant table types.
- [x] Verify Docker bind mounts read-only and no write side effects after browsing.
- [x] Document SQLite path rules, WAL caveats and limits of available storage statistics.
- [x] Run common adapter contract tests across PostgreSQL, MySQL and SQLite.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:45:49.953Z · PETER IRUNGU

Progress evidence (2026-11-04): the adapter reports per-table size statistics unavailable rather than inventing values, and fixture tests cover the SQLite database, view, index, FK, row and unavailable size behavior. Docker read-only mount behavior and common cross-adapter contract tests remain open; WAL caveats need fuller documentation.

### 2026-11-04T14:10:06.968Z · PETER IRUNGU

Items 4,5 verified: docs/sqlite.md; tests/adapter_contract.rs runs one behavioral contract through the DatabaseAdapter trait on PostgreSQL 18, MariaDB 11.8, MySQL 8.4 and SQLite - all pass. Contract caught a real PostgreSQL defect: the row filter treated % and _ as ILIKE wildcards (fixed with escaping). Item 3 (Docker :ro bind mount in a real container) deferred to Group 12 Docker verification.

### 2026-11-04T14:46:18.733Z · PETER IRUNGU

Item 3 verified in a real container (docs/docker.md): SQLite on a -v :ro bind mount with --read-only rootfs; browsed relations/rows; SHA-256 unchanged and directory listing shows only the database file afterwards. Task complete.
