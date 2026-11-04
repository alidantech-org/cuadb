---
{id: TSK-68384A11A749,name: Define SQL statement policy and authorized execution boundary,type: task,status: completed,priority: medium,created_at: 2026-11-04T16:14:15.437Z,updated_at: 2026-11-04T16:47:56.135Z,tags: [cuadb,sql-workspace],depends_on: [TSK-FF6C70D26908,TSK-5689F3668ED7,TSK-0DB3969BA9F6],order: 1024,links: [{type: blocks,task: TSK-E5355EF46139}],extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T16:47:56.135Z}]}}
---

# Define SQL statement policy and authorized execution boundary

## Goal
Create a server-side execution boundary that classifies submitted SQL and checks the authenticated user’s permissions for the selected trusted profile. The default must remain disabled. Reject unsupported or dangerous statement classes, multi-statement payloads and resource-exhausting requests before contacting the database.

## Context
The current explorer deliberately has no arbitrary SQL endpoint. This task establishes policy before any editor UI or write operation exists. Use a maintained SQL parser and engine capability flags; never treat regex checks as a security boundary. Database grants and CuaDB authorization must both permit the requested operation.

This task belongs to **14 — SQL workspace and controlled database editing**. The group outcome is: Add a separate, explicitly authorized workspace for SQL execution and normal table data editing. Preserve the safe read-only explorer as the default experience. Every mutating action must be authenticated, permission-checked, bounded, attributable and auditable.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Add parser-backed statement classification for supported PostgreSQL, SQLite and MySQL-family dialects.
- [x] Default SQL execution to disabled and require explicit operator configuration plus user permission.
- [x] Reject multi-statements and disallowed administrative, file, network and privilege-changing operations.
- [x] Apply statement deadlines, result row/byte caps, cancellation and redacted errors.
- [x] Test parser edge cases, denied statements and per-profile permission failures.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T16:47:55.283Z · PETER IRUNGU

VERIFIED (backend). src/sql/policy.rs (sqlparser 0.59): exactly one statement; SELECT/EXPLAIN/INSERT/UPDATE/DELETE only; DDL, admin, privileges, COPY, MERGE, TRUNCATE, PRAGMA, ATTACH, EXPLAIN ANALYZE, SELECT INTO, locking reads, data-modifying CTEs, oversize (20 KB) refused; function denylist (files, network, extensions, sleep, advisory locks, set_config, sequences) wherever it appears incl. table functions; literals stripped from summaries. Default OFF (CUADB_SQL_WORKSPACE) and needs an authenticated user with a profile-wide sql grant. Execution: reads in READ ONLY transactions (a side-effecting SQL function fails: live PG test), server-side statement_timeout, row/byte caps, cancel registry (PG pg_cancel_backend verified in pg_stat_activity), redacted errors. Evidence: 7 policy unit tests (3 dialects), tests/sql_workspace_sqlite.rs (14), tests/sql_workspace_pg.rs (5, live PostgreSQL 18). LIMITATIONS: SQLite cannot interrupt a running statement (timeout/cancel answer immediately, worker finishes); MySQL/MariaDB are classification-only; the denylist is not exhaustive (read-only txns and DB privileges are the boundaries). The UI task (TSK-B18207BD473C) is untouched.
