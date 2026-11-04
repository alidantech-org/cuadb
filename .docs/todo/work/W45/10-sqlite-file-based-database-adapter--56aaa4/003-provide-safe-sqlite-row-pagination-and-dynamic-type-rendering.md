---
{id: TSK-7D658997F68C,name: Provide safe SQLite row pagination and dynamic type rendering,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:08.195Z,updated_at: 2026-11-04T07:32:08.195Z,tags: [cuadb,sqlite],depends_on: [TSK-B2D3EC3E1F6B,TSK-8B8935A1684A],order: 3072}
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
- [ ] Implement validated table and column resolution with safe SQLite identifier quoting.
- [ ] Retrieve bounded pages with clear first/last states and sorting controls.
- [ ] Handle rowid, WITHOUT ROWID and unstable ordering explicitly.
- [ ] Display SQLite storage classes including NULL, integer, real, text and blob.
- [ ] Test large blobs, mixed-typed columns and readonly connection behavior.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
