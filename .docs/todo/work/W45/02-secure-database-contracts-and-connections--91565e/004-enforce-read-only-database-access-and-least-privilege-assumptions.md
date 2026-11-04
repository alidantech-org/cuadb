---
{id: TSK-55D20E7D39E9,name: Enforce read-only database access and least-privilege assumptions,type: task,status: planned,priority: high,created_at: 2026-11-04T07:32:04.669Z,updated_at: 2026-11-04T07:32:04.669Z,tags: [cuadb,contracts],depends_on: [TSK-5CCB4090CF43],order: 4096}
---

# Enforce read-only database access and least-privilege assumptions

## Goal
Make it materially difficult for a bug or browser request to mutate a connected database. Every supported query path must be constrained to introspection and SELECT operations, using least-privilege credentials and transaction/session limits as additional safeguards.

## Context
Disabling buttons is not a security boundary. PostgreSQL read-only transaction defaults and a dedicated SELECT-only role should be considered separately; catalog and statistics permissions may vary. Views and security-definer functions require care, and read-only transactions do not replace a properly restricted database account.

This task belongs to **02 — Secure database contracts and connections**. The group outcome is: Provide one small engine-independent interface for database discovery, metadata and row retrieval. Make a PostgreSQL URL the default connection input and define capabilities before expanding to MySQL, SQLite or SQL Server. This group owns lifecycle, timeout and read-only guarantees; it must not leak SQL or vendor-specific catalog assumptions into web templates.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Document a least-privilege PostgreSQL role with schema and table read permissions.
- [ ] Enforce read-only transaction/session behavior where the driver and engine permit it.
- [ ] Reject unsupported statement types and avoid any user-controlled SQL execution endpoint.
- [ ] Test that error paths and connection lifecycle cannot silently enable writes.
- [ ] Explain how RLS, denied tables and metadata visibility appear to the browser.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
