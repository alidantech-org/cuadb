---
{id: TSK-5CCB4090CF43,name: Implement bounded PostgreSQL pool and connection lifecycle,type: task,status: planned,priority: high,created_at: 2026-11-04T07:32:04.605Z,updated_at: 2026-11-04T07:32:04.605Z,tags: [cuadb,contracts],depends_on: [TSK-9DEA186D9800],order: 3072}
---

# Implement bounded PostgreSQL pool and connection lifecycle

## Goal
Add asynchronous PostgreSQL connections using SQLx, with bounded pool size, acquisition and connection timeouts, predictable retry behavior and explicit startup status. Avoid reconnecting on every HTMX request or creating unbounded background tasks.

## Context
The application should start in disconnected mode if no URL is supplied. If a URL exists, failed connections must produce an informative, redacted state rather than crashing or hanging the UI. Network and TLS settings need clear boundaries. Connection state should be shareable from Axum application state without mutable global variables.

This task belongs to **02 — Secure database contracts and connections**. The group outcome is: Provide one small engine-independent interface for database discovery, metadata and row retrieval. Make a PostgreSQL URL the default connection input and define capabilities before expanding to MySQL, SQLite or SQL Server. This group owns lifecycle, timeout and read-only guarantees; it must not leak SQL or vendor-specific catalog assumptions into web templates.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Create one lazily initialized or bounded connection pool for the configured database.
- [ ] Apply connection, acquisition and query timeout defaults with configurable upper bounds.
- [ ] Distinguish disabled, connecting, connected and failed states in a testable model.
- [ ] Verify retry and failure behavior without exposing raw driver errors or credentials.
- [ ] Add a live PostgreSQL fixture test behind an explicit environment gate.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
