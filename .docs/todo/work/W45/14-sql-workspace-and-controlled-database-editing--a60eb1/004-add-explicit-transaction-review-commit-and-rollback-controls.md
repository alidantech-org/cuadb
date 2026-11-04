---
{id: TSK-32BD4CD19E43,name: "Add explicit transaction review, commit and rollback controls",type: task,status: completed,priority: medium,created_at: 2026-11-04T16:14:15.966Z,updated_at: 2026-11-04T16:47:56.599Z,tags: [cuadb,sql-workspace],depends_on: [TSK-E5355EF46139],order: 4096,links: [{type: blocks,task: TSK-CAE8F98AF33E}],extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T16:47:56.599Z}]}}
---

# Add explicit transaction review, commit and rollback controls

## Goal
Make write behavior understandable and recoverable by exposing explicit transaction boundaries for supported editing workflows. Users must see the target profile, pending changes and affected-row estimates before committing, with a clear rollback path for uncommitted work.

## Context
Database changes are irreversible once committed unless the database itself provides recovery. Avoid hidden auto-commit behavior for multi-step edits and never hold transactions open indefinitely. Respect each engine’s transaction semantics and surface unsupported capabilities honestly.

This task belongs to **14 — SQL workspace and controlled database editing**. The group outcome is: Add a separate, explicitly authorized workspace for SQL execution and normal table data editing. Preserve the safe read-only explorer as the default experience. Every mutating action must be authenticated, permission-checked, bounded, attributable and auditable.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Provide explicit transaction begin, commit and rollback handling for supported edit flows.
- [x] Show the target profile, operation summary and affected-row count before commit.
- [x] Apply transaction deadlines and roll back on cancellation, disconnect or validation failure.
- [x] Prevent profile switching while a transaction is pending unless it is safely rolled back.
- [x] Test rollback, commit failures, timeouts and concurrent row changes.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T16:47:55.583Z · PETER IRUNGU

VERIFIED (backend contract; UI is separate). /tx/begin|status|commit|rollback: server-held transactions (max 2 per user, one profile at a time, private to owner), status lists profile, remaining time and each pending change with affected-row count and no values, commit requires expected_changes equal to the pending count, TTL (60 s default) with a 5 s sweeper, rollback on failed step, logout, user disable and dead sessions, profile switching refused while pending. Evidence: src/sql/exec.rs unit tests; SQLite tests (rollback, failed commit via deferred FK leaves nothing, privacy, logout, session revocation sweep, two-profile switching refusal, single-writer wait times out cleanly); live PG tests (commit, rollback, aborted tx, row-lock timeout bounded, stale check after concurrent commit). Limitation: SQLite allows one writer connection, so a second begin fails after the connection timeout.
