---
{id: TSK-D066361C1628,name: "Build the Administration view for members, permissions, audit log and security status",type: task,status: planned,priority: medium,created_at: 2026-11-04T23:18:11.064Z,updated_at: 2026-11-04T23:18:19.541Z,tags: [cuadb,phase2-ui],order: 5120,depends_on: [TSK-449CDC06521C,TSK-A448B23E0C59]}
---

# Build the Administration view for members, permissions, audit log and security status

## Goal
Provide the admin-only Administration screen from the demo, bound to the real user, grant and audit routes.

## Context
The Administration view has tabs for Members, Connections, Permissions, Audit log and Security. Members supports add (with the username pattern and a temporary password of at least eight characters), edit display name and role, enable or disable, and delete, with the self-protection and last-administrator rules from the backend. Permissions is a per-member, per-connection and per-database four-level control. The audit log is read-only and filterable. Security is read-only status: deployment mode, bind address and session lifetime come from operator configuration and cannot be changed in the UI. Everything reuses `UsersTemplate`, `GrantsTemplate` and `AuditTemplate` (`src/web/auth.rs`), requires administrator role server-side, and renders in the demo's compact tables with confirmation modals for destructive actions.

This task belongs to **15 — Authenticated application UI, administration and connection management**. The look, feel and density come from `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. No inline scripts or styles are allowed by the CSP, and acceptance is a manual side-by-side browser comparison plus focused PostgreSQL and SQLite Rust tests; MySQL and MariaDB are out of scope until the frontend milestone.

## Acceptance checklist
- [ ] Render the five tabs as HTMX fragments, admin-only, with the demo's table, segmented-control and hint styles.
- [ ] Wire member add, edit, enable or disable, delete and password reset to the backend with confirmation modals and inline errors.
- [ ] Wire the permission segmented control to grant and remove routes and the audit log to the filter parameters.
- [ ] Show the Security tab as read-only operator configuration and verify a non-administrator receives a denied fragment.
- [ ] Compare every tab with the demo in the built-in browser in light and dark and on mobile.

## Verification and handoff
Run the focused tests for this slice, run `bun run format:check` and `bun run check` at the group boundary, add a Task comment with the evidence and any deliberate departure from the demo, tick accepted items with `task check`, and only then mark the task completed.
