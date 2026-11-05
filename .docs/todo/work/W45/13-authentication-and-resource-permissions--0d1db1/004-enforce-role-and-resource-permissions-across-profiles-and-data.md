---
{id: TSK-FF6C70D26908,name: Enforce role and resource permissions across profiles and data,type: task,status: completed,priority: medium,created_at: 2026-11-04T16:14:15.031Z,updated_at: 2026-11-05T01:44:28.767Z,tags: [cuadb,auth],depends_on: [TSK-D91E3CD3D4C6],order: 4096,links: [{type: blocks,task: TSK-54598487D0CE}],extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T16:33:02.085Z}]}}
---

# Enforce role and resource permissions across profiles and data

## Goal
Add deny-by-default authorization for administrators, read-only users and explicitly permitted editors. Permissions must scope users to trusted connection profiles, schemas, relations and allowed actions so one user cannot browse or modify another user’s resources.

## Context
A single global role is insufficient once CuaDB supports multiple connections and controlled data editing. Resolve permissions on the server for every route and operation, including catalog navigation, rows, exports, profile switching and SQL execution. Database grants remain an independent security boundary.

This task belongs to **13 — Authentication and resource permissions**. The group outcome is: Add a secure local administrator bootstrap, user accounts, sessions and deny-by-default permissions before CuaDB exposes protected multi-user or write-capable features. Authorization must be scoped to trusted connection profiles and database resources.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Define roles and explicit view, export, edit and SQL-execution permissions.
- [x] Scope grants to connection profiles and optionally schemas or relations.
- [x] Enforce authorization in every relevant HTTP and HTMX handler, not only the UI.
- [x] Default to deny and prevent privilege escalation through profile or identifier tampering.
- [x] Add positive and negative tests for cross-profile and per-resource access.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T16:33:01.055Z · PETER IRUNGU

VERIFIED (backend). Roles admin/editor/viewer with ceilings, grants (user, profile, schema?, relation?, actions), deny by default, enforced in schemas/relations/structure/indexes/stats/data/export/overview/advanced/connection handlers; lists filtered; related ZIP never follows or reveals non-exportable relations; admin endpoints admin-only; escalation and cross-profile tests. tests/auth_http.rs, src/auth/rbac.rs unit tests. Edit and sql actions exist in the model and are enforced by Group 14. Limitation: grants are not yet scoped per column.
