---
{id: GOAL-GRP-2BBEC20783DB,name: "15 — Authenticated application UI, administration and connection management goal",type: documentation,extra: {created_at: 2026-11-04T23:18:06.352Z,group: GRP-2BBEC20783DB}}
---

# 15 — Authenticated application UI, administration and connection management — Goal

## Outcome
Deliver the screens and the small supporting backend that the updated `assets/cuadb-ui-demo.html` adds on top of the read-only explorer: sign-in and account handling, a principal-aware application shell with CSRF wiring, a connection and database switcher, real connection management for administrators, an Administration view (members, permissions, audit log and a read-only security status), a typed row editor, and metadata-rich Overview and object pages. The explorer groups 05 to 08 keep owning the read-only browsing surfaces; this group owns everything that depends on identity, permissions or connection management.

## Definition of done
1. **Principal-aware shell, CSRF and sign-in:** every page and fragment knows the signed-in principal, mutating requests carry the CSRF token, and login, logout and local-mode behavior match the demo.
2. **Backend gaps closed:** administrators can delete users and change roles, the audit log can be filtered, and Advanced Overview objects (functions, procedures, sequences, types, triggers, extensions) have real fragments, each with focused PostgreSQL and SQLite tests.
3. **Connection management:** administrators create, edit, test and delete profiles; secrets are encrypted, never returned, and a blank password keeps the stored one.
4. **Switchers and Administration UI:** connection and database pickers show only permitted items with access tags, and the Administration view works end to end.
5. **Row editing UI:** inserts, updates and deletes use the typed backend and respect transactions, access levels and the audit log.
Each result is verified by Rust tests for behavior and a manual side-by-side browser check against the demo at 1440x900, 1280x800 and 390x844 in light and dark.

## Boundaries
Reuse the existing Axum, Askama and HTMX stack with no frontend framework. Never weaken the CSP, CSRF or deny-by-default grants. Verify PostgreSQL and SQLite only; MySQL and MariaDB wait for the frontend milestone. Do not mutate a personal database in tests.
