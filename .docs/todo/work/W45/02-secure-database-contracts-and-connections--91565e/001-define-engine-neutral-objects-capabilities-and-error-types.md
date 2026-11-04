---
{id: TSK-A3CFF0B5E267,name: "Define engine-neutral objects, capabilities and error types",type: task,status: planned,priority: high,created_at: 2026-11-04T07:32:04.467Z,updated_at: 2026-11-04T07:32:04.467Z,tags: [cuadb,contracts],depends_on: [TSK-23F3D2D0DDE1],order: 1024}
---

# Define engine-neutral objects, capabilities and error types

## Goal
Establish minimal data contracts for connections, namespaces, relations, columns, keys, statistics, pages and typed cell values. The interface must expose unavailable capabilities explicitly instead of pretending that all SQL engines implement the same metadata.

## Context
PostgreSQL exposes schemas and a rich catalog, MySQL commonly represents databases as namespaces, and SQLite is file-backed. These differences should be represented behind a focused adapter contract so the templates can display a consistent explorer. Avoid generic ORM layers, user-generated SQL and premature support code for unavailable drivers.

This task belongs to **02 — Secure database contracts and connections**. The group outcome is: Provide one small engine-independent interface for database discovery, metadata and row retrieval. Make a PostgreSQL URL the default connection input and define capabilities before expanding to MySQL, SQLite or SQL Server. This group owns lifecycle, timeout and read-only guarantees; it must not leak SQL or vendor-specific catalog assumptions into web templates.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Define typed relation identifiers and feature flags without engine-specific catalog leakage.
- [ ] Specify paged-row and metadata response models with explicit unavailable-statistic markers.
- [ ] Create adapter boundary tests using a fake or stub implementation.
- [ ] Document semantics of namespace and database switching across supported engines.
- [ ] Keep compilation independent of a running PostgreSQL server.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
