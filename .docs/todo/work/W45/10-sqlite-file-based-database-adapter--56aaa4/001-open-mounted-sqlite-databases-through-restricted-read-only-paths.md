---
{id: TSK-F71C30D38C2C,name: Open mounted SQLite databases through restricted read-only paths,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:07.968Z,updated_at: 2026-11-04T12:33:38.494Z,tags: [cuadb,sqlite],depends_on: [TSK-A3CFF0B5E267,TSK-55D20E7D39E9],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T12:33:38.494Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-F71C30D38C2C/1793795618496-start,started_at: 2026-11-04T12:33:38.512Z}]}}}
---

# Open mounted SQLite databases through restricted read-only paths

## Goal
Accept a configured SQLite URL or file path only within an allowed mounted directory and open the database in read-only mode. Reject arbitrary filesystem traversal, unsupported URI options and paths outside configured roots.

## Context
SQLite differs from network databases because the file is the database. A local web viewer must not become a general file browser for the host or container. Enforce canonical path checks, reject symlink escape where feasible, and document read-only mount patterns for Docker and native development.

This task belongs to **10 — SQLite file-based database adapter**. The group outcome is: Enable browsing local SQLite database files securely from the same CuaDB interface. A database file must be mounted into the container or available through a configured host path; the viewer may inspect it but must never edit the database or produce write side-effects as part of ordinary browsing.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Validate configured SQLite URLs against an explicit allowed data root.
- [x] Open the file without creating a new database or allowing modifications.
- [ ] Test traversal attempts, missing files, symlinks and permission failures.
- [ ] Document read-only Docker bind mounts and limitations with WAL databases.
- [x] Keep SQLite file paths and sensitive filesystem details out of the browser.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:33:38.598Z · PETER IRUNGU

Implemented SQLite URL allow-root canonical path resolution, bounded read-only SQLx pool, query_only session guard, shared adapter metadata and row paging. Focused cargo test --test sqlite_catalog passes against the local disposable fixture, including traversal rejection, FK/index/schema metadata, read-only connection, dynamic cell types and honest unavailable sizes. Caveats: offset paging only, virtual-table-specific metadata unsupported, mounted Docker runtime not yet smoke-tested.

### 2026-11-04T12:34:12.937Z · PETER IRUNGU

Current checkpoint: Group10 SQLite slice passes cargo test --test sqlite_catalog (2 tests), cargo check, cargo fmt and bun run format:check; task validate passes. Broader Groups 05–13 remain in progress, with UI parity and other adapter/release work outstanding. Do not mark this task or goal complete until the remaining checklist and release verification are addressed.

### 2026-11-04T12:45:49.483Z · PETER IRUNGU

Progress evidence (2026-11-04): explicit canonical allow-root validation, read_only(true)/create_if_missing(false), and query_only=ON are implemented. cargo test --lib db::sqlite::tests passed 2 tests including denied INSERT and symlink escape; cargo test --test sqlite_catalog passed 2 fixture tests; bun run format:check and task validate passed. Still open: permission-failure coverage, read-only Docker bind-mount smoke, explicit WAL guidance, and full end-to-end behavior.
