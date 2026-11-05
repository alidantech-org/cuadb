---
{id: TSK-449CDC06521C,name: "Close backend gaps for user management, audit filters and metadata fragments",type: task,status: completed,priority: high,created_at: 2026-11-04T23:18:10.617Z,updated_at: 2026-11-05T00:04:00.779Z,tags: [cuadb,phase2-ui],order: 2048,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-05T00:03:48.463Z},{from: in-progress,to: completed,at: 2026-11-05T00:04:00.779Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-449CDC06521C/1793837028474-start,started_at: 2026-11-05T00:03:48.516Z,end_ref: refs/tracking/TSK-449CDC06521C/1793837040781-end,ended_at: 2026-11-05T00:04:00.779Z,summary: {files: 0,insertions: 0,deletions: 0}}]}}}
---

# Close backend gaps for user management, audit filters and metadata fragments

## Goal
Add the small server routes the updated demo needs that the backend does not provide yet, so the Administration and Overview screens can bind to real data instead of hiding features.

## Context
Existing admin routes allow creating users, enabling or disabling them and resetting passwords (`/admin/users*`), granting and removing grants, and paging the audit log by cursor only. Missing: deleting a user, changing a role, audit filters (actor, kind, time range), and per-object metadata for functions, procedures, sequences, types, triggers and extensions. `/catalog/advanced` and `/catalog/overview` return flat lists. All routes stay admin-only where administrative, deny by default, CSRF-protected and audited without recording values. A user may not demote, disable or delete themself, and the last administrator cannot be removed. PostgreSQL exposes the full metadata set; SQLite reports only what it supports and the UI must show unsupported kinds honestly.

This task belongs to **15 — Authenticated application UI, administration and connection management**. The look, feel and density come from `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. No inline scripts or styles are allowed by the CSP, and acceptance is a manual side-by-side browser comparison plus focused PostgreSQL and SQLite Rust tests; MySQL and MariaDB are out of scope until the frontend milestone.

## Acceptance checklist
- [x] Add admin routes to delete a user and change a role with last-administrator and self-protection rules, audit events, and tests against the auth store.
- [x] Add audit filters (actor, event kind, since and until) to `/admin/audit` while keeping cursor paging bounded.
- [x] Add metadata fragments for functions, procedures, sequences, types, triggers and extensions for PostgreSQL and the supported subset for SQLite, each read-only and permission-scoped.
- [x] Cover each new route with focused PostgreSQL and SQLite tests and record the engine limits in `docs/auth.md` and the catalog documentation.

## Verification and handoff
Run the focused tests for this slice, run `bun run format:check` and `bun run check` at the group boundary, add a Task comment with the evidence and any deliberate departure from the demo, tick accepted items with `task check`, and only then mark the task completed.

## Comments

### 2026-11-05T00:03:52.712Z · PETER IRUNGU

Done (2026-11-05): POST /admin/users/role and /admin/users/delete (not yourself, last active administrator protected, sessions and grants removed, transactions ended, audit events user_role_changed and user_deleted); /admin/audit filters actor, kind, outcome, since and until with unknown kinds and malformed dates ignored; GET /catalog/objects and /catalog/object plus a dedicated Overview for the Advanced explorer. Evidence: tests/auth_http.rs (roles, delete, audit filters), tests/sql_workspace_sqlite.rs (SQLite returns an empty objects fragment, input validation, schema-scoped access), tests/catalog_live_http.rs against the disposable cuadb_fixture_catalog PostgreSQL database with the SELECT-only fixture reader (overview bars, object list, definition page, bad kinds rejected). Engine limits are recorded in docs/auth.md and docs/sqlite.md.
