---
{id: GRP-2BBEC20783DB,name: "15 — Authenticated application UI, administration and connection management",type: documentation,extra: {created_at: 2026-11-04T23:18:06.352Z}}
---

# 15 — Authenticated application UI, administration and connection management — Plan

## Implementation approach
Start with the shell: add a principal view model and the CSRF meta tag and header hook to the base layout, then build sign-in, sign-out and the Account block. Next add the small backend gaps (user delete and role change, audit filters, metadata fragments) so later UI tasks bind to real routes. Then build connection management, first the encrypted profile store and test endpoint, then the picker and modal UI. Finally build the Administration view and the row editor. Each task is one vertical slice: route and view model in Rust, an Askama partial, a delegated handler in `assets/js/app.js`, tokens in `assets/css/app.css`, and tests.

## Milestones
1. Shell, CSRF and sign-in work in local and authenticated modes.
2. Backend gaps merged with PostgreSQL and SQLite tests.
3. Connection profiles can be managed end to end.
4. Administration view and row editor complete.
5. Browser acceptance recorded for every task.

## Risks and safeguards
Secret storage needs an operator-supplied encryption key; agree its source with the owner before wiring it. The demo's simulated behavior (any password, client audit, runtime mode toggle) must never reach production code. The CSP forbids inline styles and scripts, so every demo shortcut that relies on them must be rewritten. Authorization must stay in the server request path for fragments, exports and profile switching alike.

## Verification
Run the focused Rust tests per slice and the root quality commands at group boundaries. Compare with the demo in the built-in browser, record evidence in a Task comment, then tick the checklist with `task check` and close the task.
