---
{id: TSK-E5355EF46139,name: "Implement permission-checked table create, update and delete operations",type: task,status: completed,priority: medium,created_at: 2026-11-04T16:14:15.801Z,updated_at: 2026-11-04T16:47:56.387Z,tags: [cuadb,sql-workspace],depends_on: [TSK-68384A11A749,TSK-4C7273B9E46A],order: 3072,links: [{type: blocks,task: TSK-32BD4CD19E43}],extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T16:47:56.387Z}]}}
---

# Implement permission-checked table create, update and delete operations

## Goal
Add normal table data editing through explicit typed operations rather than silently making the existing explorer grid editable. Validate relation and column identities against catalog metadata, bind values, enforce per-resource edit permissions and keep every operation within a transaction.

## Context
CRUD is distinct from raw SQL execution and should remain available only for tables and columns the adapter can safely identify. Respect generated columns, identity values, constraints, foreign keys and database errors. Do not provide bulk destructive actions without explicit bounded confirmation.

This task belongs to **14 — SQL workspace and controlled database editing**. The group outcome is: Add a separate, explicitly authorized workspace for SQL execution and normal table data editing. Preserve the safe read-only explorer as the default experience. Every mutating action must be authenticated, permission-checked, bounded, attributable and auditable.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Implement create, update and delete operations using validated catalog identifiers and bound values.
- [x] Enforce per-user, per-profile and per-relation edit permissions on the server.
- [x] Respect generated/identity columns, constraints and adapter capability limitations.
- [x] Require explicit confirmation and affected-row bounds for destructive operations.
- [x] Test constraint failures, stale row versions and unauthorized mutation attempts.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T16:47:55.430Z · PETER IRUNGU

VERIFIED (backend). /data/insert|update|delete (src/sql/crud.rs + src/web/sql.rs): catalog-validated identifiers, bound values cast to catalog types, generated/ALWAYS-identity/binary columns refused, one-row edits by full primary key, previous-value stale check, delete needs confirm=schema.relation, affected rows must be exactly 1 else rollback + 409, per-relation edit grant, read-write profile required even for admins. Evidence: 5 unit tests, SQLite and live PostgreSQL tests for constraint failures (UNIQUE, NOT NULL, FK), stale rows, unauthorized relations/profiles, hostile identifiers. Limitation: no bulk edits and no composite-key UI contract yet.
