---
{id: TSK-D066361C1628,name: "Build the Administration view for members, permissions, audit log and security status",type: task,status: in-progress,priority: medium,created_at: 2026-11-04T23:18:11.064Z,updated_at: 2026-11-05T00:03:48.958Z,tags: [cuadb,phase2-ui],order: 5120,depends_on: [TSK-449CDC06521C,TSK-A448B23E0C59],extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T23:45:43.886Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-D066361C1628/1793835943888-start,started_at: 2026-11-04T23:45:43.908Z}]}}}
---

# Build the Administration view for members, permissions, audit log and security status

## Goal
Provide the admin-only Administration screen from the demo, bound to the real user, grant and audit routes.

## Context
The Administration view has tabs for Members, Connections, Permissions, Audit log and Security. Members supports add (with the username pattern and a temporary password of at least eight characters), edit display name and role, enable or disable, and delete, with the self-protection and last-administrator rules from the backend. Permissions is a per-member, per-connection and per-database four-level control. The audit log is read-only and filterable. Security is read-only status: deployment mode, bind address and session lifetime come from operator configuration and cannot be changed in the UI. Everything reuses `UsersTemplate`, `GrantsTemplate` and `AuditTemplate` (`src/web/auth.rs`), requires administrator role server-side, and renders in the demo's compact tables with confirmation modals for destructive actions.

This task belongs to **15 — Authenticated application UI, administration and connection management**. The look, feel and density come from `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. No inline scripts or styles are allowed by the CSP, and acceptance is a manual side-by-side browser comparison plus focused PostgreSQL and SQLite Rust tests; MySQL and MariaDB are out of scope until the frontend milestone.

## Acceptance checklist
- [x] Render the five tabs as HTMX fragments, admin-only, with the demo's table, segmented-control and hint styles.
- [x] Wire member add, edit, enable or disable, delete and password reset to the backend with confirmation modals and inline errors.
- [x] Wire the permission segmented control to grant and remove routes and the audit log to the filter parameters.
- [x] Show the Security tab as read-only operator configuration and verify a non-administrator receives a denied fragment.
- [ ] Compare every tab with the demo in the built-in browser in light and dark and on mobile.

## Verification and handoff
Run the focused tests for this slice, run `bun run format:check` and `bun run check` at the group boundary, add a Task comment with the evidence and any deliberate departure from the demo, tick accepted items with `task check`, and only then mark the task completed.

## Comments

### 2026-11-04T23:45:44.177Z · PETER IRUNGU

Progress (2026-11-05): /admin (panel), /admin/users (Members with add, enable or disable, reset password), /admin/grants (member picker, grants table, add with four access levels, remove), /admin/audit (localized times, older events) and /admin/security (read-only operator configuration) are built as HTMX fragments with the demo's tables and styles. Tests cover admin-only access (anonymous redirected, non-admin 403), no inline code, the HX-Trigger refresh after changes and the policy message for weak passwords. Browser-verified: creating a member, the permissions grant flow, the audit and security tabs. Open: the Connections tab (needs the connection management backend), editing a role and deleting a member (backend does not offer them yet), audit filters, confirmation modals and the light/dark/mobile comparison.

### 2026-11-05T00:03:53.055Z · PETER IRUNGU

Progress (2026-11-05): all five tabs exist (Members, Connections, Permissions, Audit log, Security) with role change, delete, enable/disable and password reset, grants, filtered audit and read-only security status; the connection dialog and removal are verified in the browser. Destructive actions use the browser's native confirm (hx-confirm) rather than a custom modal. Item 5 (light, dark and mobile comparison against the demo) is still open.
