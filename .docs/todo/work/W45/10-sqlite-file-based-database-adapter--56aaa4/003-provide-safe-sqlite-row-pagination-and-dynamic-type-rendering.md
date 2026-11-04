---
{id: TSK-7D658997F68C,name: Provide safe SQLite row pagination and dynamic type rendering,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:08.195Z,updated_at: 2026-11-04T14:10:07.494Z,tags: [cuadb,sqlite],depends_on: [TSK-B2D3EC3E1F6B,TSK-8B8935A1684A],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T12:46:13.778Z},{from: in-progress,to: completed,at: 2026-11-04T14:10:07.494Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-7D658997F68C/1793796373780-start,started_at: 2026-11-04T12:46:13.806Z,end_ref: refs/tracking/TSK-7D658997F68C/1793801407497-end,ended_at: 2026-11-04T14:10:07.495Z,summary: {files: 36,insertions: 3513,deletions: 186}}]}}}
---

# Provide safe SQLite row pagination and dynamic type rendering

## Goal
Read bounded pages from SQLite files with stable sorting where possible, using engine-correct identifier quoting and row identity rules. Preserve the common grid interface while accurately representing SQLite’s dynamic per-value types.

## Context
SQLite permits varying storage classes within a column and may use rowid or an explicit primary key depending on table shape. Query generation must not assume PostgreSQL syntax, and sorting should be deterministic only when a unique key can be established. Expanded values remain size-limited and HTML-escaped.

This task belongs to **10 — SQLite file-based database adapter**. The group outcome is: Enable browsing local SQLite database files securely from the same CuaDB interface. A database file must be mounted into the container or available through a configured host path; the viewer may inspect it but must never edit the database or produce write side-effects as part of ordinary browsing.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Implement validated table and column resolution with safe SQLite identifier quoting.
- [x] Retrieve bounded pages with clear first/last states and sorting controls.
- [x] Handle rowid, WITHOUT ROWID and unstable ordering explicitly.
- [x] Display SQLite storage classes including NULL, integer, real, text and blob.
- [x] Test large blobs, mixed-typed columns and readonly connection behavior.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:45:49.805Z · PETER IRUNGU

Progress evidence (2026-11-04): SQLite row queries use catalog-resolved quoted identifiers, bounded pages and dynamic SQLite value decoding. cargo test --test sqlite_catalog passed, including bounded page and integer/blob checks; read-only mutation denial passed in the SQLite unit tests. Still open: sorting/mixed-type/large-blob edge fixtures and explicit WITHOUT ROWID/stability behavior.

### 2026-11-04T14:10:06.820Z · PETER IRUNGU

VERIFIED (tests/sqlite_live.rs): actual storage class shown with a label when it differs from declared affinity (INTEGER · text, dynamic · integer); 1 MiB blob never loaded (substr/length in SQLite, bounded prefix + true length); deterministic order (PK, else rowid; WITHOUT ROWID by PK; views report unstable); NULLs last both directions; literal filter; read-only connection rejects writes. Previously unbounded blob fetch and unordered offset paging fixed.
