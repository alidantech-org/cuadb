---
{id: TSK-B2D3EC3E1F6B,name: "Discover SQLite tables, views, columns and key constraints",type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:08.084Z,updated_at: 2026-11-04T16:15:21.057Z,tags: [cuadb,sqlite],depends_on: [TSK-F71C30D38C2C],order: 2048,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T12:46:13.556Z},{from: in-progress,to: completed,at: 2026-11-04T14:10:07.324Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-B2D3EC3E1F6B/1793796373558-start,started_at: 2026-11-04T12:46:13.573Z,end_ref: refs/tracking/TSK-B2D3EC3E1F6B/1793801407326-end,ended_at: 2026-11-04T14:10:07.325Z,summary: {files: 36,insertions: 3513,deletions: 186}}]}},links: [{type: blocks,task: TSK-8EC2E48964BC}]}
---

# Discover SQLite tables, views, columns and key constraints

## Goal
Map SQLite schema objects into the common explorer without inventing server databases or PostgreSQL namespaces. Show columns, declared types, primary and foreign keys and indexes where SQLite metadata supports them.

## Context
SQLite uses sqlite_schema and PRAGMA calls for metadata. Dynamic typing, WITHOUT ROWID tables and virtual tables need careful handling. An attached database namespace should be shown only when deliberately supported and not imply that arbitrary ATTACH statements can be sent through the browser.

This task belongs to **10 — SQLite file-based database adapter**. The group outcome is: Enable browsing local SQLite database files securely from the same CuaDB interface. A database file must be mounted into the container or available through a configured host path; the viewer may inspect it but must never edit the database or produce write side-effects as part of ordinary browsing.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] List ordinary tables, supported views and virtual tables with accurate type distinctions.
- [x] Return columns, declared types, primary keys, foreign keys and indexes.
- [x] Handle WITHOUT ROWID and generated columns without assuming rowid exists.
- [x] Avoid introducing an arbitrary SQL, ATTACH or schema-modification endpoint.
- [x] Test unusual SQLite names and empty database files with disposable fixtures.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:45:49.641Z · PETER IRUNGU

Progress evidence (2026-11-04): adapter implementation covers ordinary table/view discovery, table_xinfo columns, foreign keys and indexes, with no browser SQL/ATTACH/write route. The disposable SQLite fixture integration test passed and checks view, primary key, index and FK metadata. Still open: virtual-table distinction, generated/WITHOUT ROWID coverage, unusual names and empty-file fixtures.

### 2026-11-04T14:10:06.692Z · PETER IRUNGU

VERIFIED (tests/sqlite_live.rs): tables, views and VIRTUAL tables distinguished (new RelationKind::VirtualTable), FTS shadow tables hidden, WITHOUT ROWID ordered by PK, generated columns listed as VIRTUAL/STORED (they were previously silently dropped by the hidden>=2 filter - fixed), unusual names (reserved words, quotes, semicolons, Unicode/emoji), empty zero-byte database file, hostile names rejected, only 'main' schema reachable.
