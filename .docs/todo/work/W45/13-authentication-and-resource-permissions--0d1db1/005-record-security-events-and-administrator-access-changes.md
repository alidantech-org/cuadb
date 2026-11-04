---
{id: TSK-92AB61E4D8C0,name: Record security events and administrator access changes,type: task,status: planned,priority: medium,created_at: 2026-11-04T16:14:15.204Z,updated_at: 2026-11-04T16:15:04.590Z,tags: [cuadb,auth],depends_on: [TSK-FF6C70D26908],order: 5120,links: [{type: blocks,task: TSK-CAE8F98AF33E}]}
---

# Record security events and administrator access changes

## Goal
Provide a minimal audit trail for security-sensitive actions so operators can understand who accessed CuaDB and who changed permissions. Record login outcomes, session revocation, profile access and permission changes without storing passwords, connection URLs, SQL secrets or database row contents.

## Context
Auditability is required before enabling controlled write operations. Keep events append-only from application code, bound retention and avoid turning audit logs into a second copy of database contents. Administrative actions must be attributable to an authenticated user.

This task belongs to **13 — Authentication and resource permissions**. The group outcome is: Add a secure local administrator bootstrap, user accounts, sessions and deny-by-default permissions before CuaDB exposes protected multi-user or write-capable features. Authorization must be scoped to trusted connection profiles and database resources.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Record authentication outcomes, session revocations and permission changes.
- [ ] Record profile access and later SQL/edit actions with user and resource identifiers.
- [ ] Redact credentials, raw connection strings, query parameters and row contents.
- [ ] Restrict audit viewing to authorized administrators and bound retention/queries.
- [ ] Test audit coverage for denied access and successful privileged actions.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.
