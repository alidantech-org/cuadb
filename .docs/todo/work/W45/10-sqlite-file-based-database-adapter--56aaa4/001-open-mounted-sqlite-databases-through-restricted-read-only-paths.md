---
{id: TSK-F71C30D38C2C,name: Open mounted SQLite databases through restricted read-only paths,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:07.968Z,updated_at: 2026-11-04T07:32:07.968Z,tags: [cuadb,sqlite],depends_on: [TSK-A3CFF0B5E267,TSK-55D20E7D39E9],order: 1024}
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
- [ ] Validate configured SQLite URLs against an explicit allowed data root.
- [ ] Open the file without creating a new database or allowing modifications.
- [ ] Test traversal attempts, missing files, symlinks and permission failures.
- [ ] Document read-only Docker bind mounts and limitations with WAL databases.
- [ ] Keep SQLite file paths and sensitive filesystem details out of the browser.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
