---
{id: TSK-CAE8F98AF33E,name: Audit SQL execution and database mutations,type: task,status: completed,priority: medium,created_at: 2026-11-04T16:14:16.138Z,updated_at: 2026-11-04T16:47:56.816Z,tags: [cuadb,sql-workspace],depends_on: [TSK-32BD4CD19E43,TSK-92AB61E4D8C0],order: 5120,links: [{type: blocks,task: TSK-54598487D0CE}],extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T16:47:56.816Z}]}}
---

# Audit SQL execution and database mutations

## Goal
Make every SQL execution and CRUD mutation attributable and reviewable. Store the authenticated actor, profile, operation category, timestamp, outcome and bounded metadata without copying sensitive row values or credentials into the audit log.

## Context
The SQL workspace is a privileged feature. Audit events complement database-native logs and must not expose secrets or create an unbounded second data store. Failed and denied attempts are as important as successful writes.

This task belongs to **14 — SQL workspace and controlled database editing**. The group outcome is: Add a separate, explicitly authorized workspace for SQL execution and normal table data editing. Preserve the safe read-only explorer as the default experience. Every mutating action must be authenticated, permission-checked, bounded, attributable and auditable.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Record authorized and denied SQL execution attempts with actor, profile and outcome.
- [x] Record CRUD and transaction commit/rollback outcomes without storing row contents.
- [x] Redact secrets and bound query text or store only a safe normalized statement summary.
- [x] Restrict audit access and document retention and export behavior.
- [x] Test audit events for successful, failed, denied and rolled-back operations.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T16:47:55.714Z · PETER IRUNGU

VERIFIED. Events sql_execute, data_change, tx_begin/commit/rollback with actor, profile, outcome ok/denied/failed and normalized value-free summaries (no literals, no row values, bounded to 200 chars), denied attempts by non-granted users recorded as access_denied; audit view admin-only, bounded, retained 90 days, no export endpoint (documented in docs/auth.md, docs/sql-workspace.md). tests/sql_workspace_sqlite.rs and sql_workspace_pg.rs assert success, failure, denial and rollback paths and that secrets never appear.
