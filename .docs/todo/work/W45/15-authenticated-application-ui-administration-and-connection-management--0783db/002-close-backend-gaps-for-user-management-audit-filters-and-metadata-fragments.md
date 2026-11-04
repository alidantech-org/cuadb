---
{id: TSK-449CDC06521C,name: "Close backend gaps for user management, audit filters and metadata fragments",type: task,status: planned,priority: high,created_at: 2026-11-04T23:18:10.617Z,updated_at: 2026-11-04T23:18:10.617Z,tags: [cuadb,phase2-ui],order: 2048}
---

# Close backend gaps for user management, audit filters and metadata fragments

## Goal
Add the small server routes the updated demo needs that the backend does not provide yet, so the Administration and Overview screens can bind to real data instead of hiding features.

## Context
Existing admin routes allow creating users, enabling or disabling them and resetting passwords (`/admin/users*`), granting and removing grants, and paging the audit log by cursor only. Missing: deleting a user, changing a role, audit filters (actor, kind, time range), and per-object metadata for functions, procedures, sequences, types, triggers and extensions. `/catalog/advanced` and `/catalog/overview` return flat lists. All routes stay admin-only where administrative, deny by default, CSRF-protected and audited without recording values. A user may not demote, disable or delete themself, and the last administrator cannot be removed. PostgreSQL exposes the full metadata set; SQLite reports only what it supports and the UI must show unsupported kinds honestly.

This task belongs to **15 — Authenticated application UI, administration and connection management**. The look, feel and density come from `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. No inline scripts or styles are allowed by the CSP, and acceptance is a manual side-by-side browser comparison plus focused PostgreSQL and SQLite Rust tests; MySQL and MariaDB are out of scope until the frontend milestone.

## Acceptance checklist
- [ ] Add admin routes to delete a user and change a role with last-administrator and self-protection rules, audit events, and tests against the auth store.
- [ ] Add audit filters (actor, event kind, since and until) to `/admin/audit` while keeping cursor paging bounded.
- [ ] Add metadata fragments for functions, procedures, sequences, types, triggers and extensions for PostgreSQL and the supported subset for SQLite, each read-only and permission-scoped.
- [ ] Cover each new route with focused PostgreSQL and SQLite tests and record the engine limits in `docs/auth.md` and the catalog documentation.

## Verification and handoff
Run the focused tests for this slice, run `bun run format:check` and `bun run check` at the group boundary, add a Task comment with the evidence and any deliberate departure from the demo, tick accepted items with `task check`, and only then mark the task completed.
