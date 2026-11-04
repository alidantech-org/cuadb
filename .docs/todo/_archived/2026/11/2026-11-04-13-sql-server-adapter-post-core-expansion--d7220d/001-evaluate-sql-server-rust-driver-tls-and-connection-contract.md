---
{id: TSK-E25A9F7241A3,name: "Evaluate SQL Server Rust driver, TLS and connection contract",type: task,status: archived,priority: medium,created_at: 2026-11-04T07:32:09.649Z,updated_at: 2026-11-04T15:14:25.035Z,tags: [cuadb,mssql],depends_on: [TSK-54598487D0CE],order: 1024,extra: {status_history: [{from: planned,to: archived,at: 2026-11-04T15:14:25.035Z}]},archived_at: 2026-11-04T15:14:25.352Z}
---

# Evaluate SQL Server Rust driver, TLS and connection contract

## Goal
Validate the feasibility and security of a SQL Server adapter using an actively maintained Rust driver before committing to the implementation. Produce a clear go/no-go decision and compatible URL, timeout and authentication strategy.

## Context
SQL Server is a common database engine but is not covered by the initial SQLx driver choice. External dependencies and TLS handling need additional review. The adapter should be optional and isolated so a failure to support a particular authentication mode cannot destabilize CuaDB’s core engines.

This task belongs to **13 — SQL Server adapter (post-core expansion)**. The group outcome is: Evaluate and implement Microsoft SQL Server support after the PostgreSQL, MySQL/MariaDB and SQLite viewer is proven. Preserve the same read-only explorer, data grid and metadata contract while respecting SQL Server-specific drivers, namespaces, catalog views and pagination requirements.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Evaluate supported Rust drivers, maintenance status, TLS and licensing constraints.
- [ ] Prototype a read-only connection against a disposable SQL Server instance.
- [x] Specify supported URL format, credentials and clear unsupported auth modes.
- [x] Record capability differences and any required package/runtime changes.
- [x] Document a go/no-go decision before building user-facing SQL Server routes.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:44:56.369Z · PETER IRUNGU

Items 1,3,4,5 done (docs/sql-server.md). Driver: tiberius 0.13.0 (MIT/Apache-2.0, released 2026-09-25, rustls feature) recommended; odbc-api rejected for runtime image/licence reasons; mssql-client too new; SQLx has no MSSQL. A scratch crate with tiberius(rustls,tds73)+tokio compiled in 53 s. Item 2 (prototype against a real server) is BLOCKED: no SQL Server here; pulling the ~1.6 GB image needs ACCEPT_EULA=Y which requires the user's explicit approval. Decision: GO for the driver, NO-GO for any supported-engine claim until live validation.
