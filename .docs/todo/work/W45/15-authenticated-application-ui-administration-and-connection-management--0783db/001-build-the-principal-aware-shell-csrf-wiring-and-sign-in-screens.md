---
{id: TSK-A448B23E0C59,name: "Build the principal-aware shell, CSRF wiring and sign-in screens",type: task,status: planned,priority: high,created_at: 2026-11-04T23:18:10.466Z,updated_at: 2026-11-04T23:18:10.466Z,tags: [cuadb,phase2-ui],order: 1024}
---

# Build the principal-aware shell, CSRF wiring and sign-in screens

## Goal
Make the application shell aware of who is signed in and make every state-changing request work under authentication. Today only the users page receives the CSRF token, and the explorer template has no user, role or mode context, so every HTMX POST from the explorer would be rejected once authentication is enforced.

## Context
The backend already provides `/login`, `/logout`, `/account/password`, the `Principal` extractor and CSRF checks (`src/web/auth.rs`). The base layout is `templates/base.html` and the explorer page is `templates/pages/explorer.html`. The demo's login card, Account block and local-mode behavior are the visual reference. In trusted local mode (no administrator yet) the explorer stays open, hides Sign out and shows a trusted-local label. The CSP forbids inline script and style, so the CSRF token travels in a meta tag and the bundled script adds the `x-csrf-token` header to HTMX requests. Passwords are never echoed and the first-sign-in password change from the backend must be supported.

This task belongs to **15 — Authenticated application UI, administration and connection management**. The look, feel and density come from `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. No inline scripts or styles are allowed by the CSP, and acceptance is a manual side-by-side browser comparison plus focused PostgreSQL and SQLite Rust tests; MySQL and MariaDB are out of scope until the frontend milestone.

## Acceptance checklist
- [ ] Expose a principal view model (display name, role, local or authenticated mode, CSRF token, capability flags) to the base layout and the explorer page.
- [ ] Add the CSRF meta tag and an `htmx:configRequest` hook in `assets/js/app.js`, covered by a Rust test that a POST from a rendered page succeeds under enforced auth.
- [ ] Restyle the login page to the demo's login card with inline error text and no demo-account chips, and handle disabled accounts, lockout and forced password change.
- [ ] Add the Account block to the settings popover with Sign out, and hide it in trusted local mode.
- [ ] Compare login, signed-in and local-mode shells with the demo in the built-in browser in light and dark at desktop and mobile widths.

## Verification and handoff
Run the focused tests for this slice, run `bun run format:check` and `bun run check` at the group boundary, add a Task comment with the evidence and any deliberate departure from the demo, tick accepted items with `task check`, and only then mark the task completed.
