---
{id: TSK-E68FB08CB73D,name: Expose safe connection status and operator diagnostics,type: task,status: planned,priority: high,created_at: 2026-11-04T07:32:04.734Z,updated_at: 2026-11-04T07:32:04.734Z,tags: [cuadb,contracts],depends_on: [TSK-55D20E7D39E9],order: 5120}
---

# Expose safe connection status and operator diagnostics

## Goal
Provide browser-safe connection health and meaningful failure categories without embedding secrets, stack traces or SQL statements in rendered templates. Separate server liveness from actual database reachability to keep the app usable when its database is offline.

## Context
The existing /health endpoint deliberately checks only the Rust service. A distinct connection-status route can indicate supported driver, database name when safe, reachability and whether credentials are missing. It must not publish raw connection URLs or grant visitors the ability to scan arbitrary hosts.

This task belongs to **02 — Secure database contracts and connections**. The group outcome is: Provide one small engine-independent interface for database discovery, metadata and row retrieval. Make a PostgreSQL URL the default connection input and define capabilities before expanding to MySQL, SQLite or SQL Server. This group owns lifecycle, timeout and read-only guarantees; it must not leak SQL or vendor-specific catalog assumptions into web templates.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Return safe status categories such as disconnected, ready, unavailable and insufficient permissions.
- [ ] Keep /health independent of database availability and fast under network failure.
- [ ] Use redacted operator logs with stable error codes for troubleshooting.
- [ ] Test no configured URL, wrong password, denied schema and temporary network loss.
- [ ] Integrate the status contract with a future HTMX connection panel.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
