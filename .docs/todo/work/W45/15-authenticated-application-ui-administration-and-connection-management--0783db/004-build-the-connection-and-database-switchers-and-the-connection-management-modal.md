---
{id: TSK-6E55E65FD4B1,name: Build the connection and database switchers and the connection management modal,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T23:18:10.916Z,updated_at: 2026-11-05T00:03:49.206Z,tags: [cuadb,phase2-ui],order: 4096,depends_on: [TSK-91DF6AB91FA3,TSK-A448B23E0C59],extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-05T00:03:49.206Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-6E55E65FD4B1/1793837029208-start,started_at: 2026-11-05T00:03:49.236Z}]}}}
---

# Build the connection and database switchers and the connection management modal

## Goal
Implement the bottom-of-sidebar connection and database pickers and the administrator's connection modal exactly in the demo's style, driven by the real profile and grant data.

## Context
The pickers list only profiles and databases the principal may see and show the access tag (none, read, write, destructive) per database. Choosing another profile resets the tree, selection, open editor and any transaction through the existing server logic. The admin modal comes from the demo (`connModal`) and talks to the endpoints from the connection management task; it supports create, edit, test and remove, disables the engines the backend does not support yet, and keeps the password field blank on edit with the hint that a blank value keeps the saved password. The last remaining connection cannot be removed. Everything uses HTMX fragments, delegated handlers and no inline styles. Reference: `ProfileRow` and `ProfileListTemplate` in `src/web/mod.rs`.

This task belongs to **15 — Authenticated application UI, administration and connection management**. The look, feel and density come from `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. No inline scripts or styles are allowed by the CSP, and acceptance is a manual side-by-side browser comparison plus focused PostgreSQL and SQLite Rust tests; MySQL and MariaDB are out of scope until the frontend milestone.

## Acceptance checklist
- [ ] Render the connection picker, database picker and access tags from real grants, with the demo's popovers, keyboard and Escape behavior.
- [x] Build the connection modal with create, edit, test and remove flows, inline validation messages and focus trapping.
- [x] Verify server responses never contain connection URLs or passwords and add a Rust test for the picker fragments.
- [ ] Compare pickers and modal with the demo in the built-in browser in light and dark at 1440x900, 1280x800 and 390x844.

## Verification and handoff
Run the focused tests for this slice, run `bun run format:check` and `bun run check` at the group boundary, add a Task comment with the evidence and any deliberate departure from the demo, tick accepted items with `task check`, and only then mark the task completed.

## Comments

### 2026-11-05T00:03:53.223Z · PETER IRUNGU

Progress (2026-11-05): the connection picker lists only the connections the user may see and the dialog (create, edit, test, remove) works with inline messages, Escape, a Tab focus trap and focus restore. No separate database picker or per-database access tags yet: a connection is one connection plus one database in the current backend, and grants are per connection, schema and relation. Item 1 and the screenshot matrix (item 4) remain open.
