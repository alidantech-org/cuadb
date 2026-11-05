---
{id: TSK-CB16F5548879,name: "Build the typed row editor, insert and delete flows",type: task,status: in-progress,priority: medium,created_at: 2026-11-04T23:18:11.215Z,updated_at: 2026-11-05T00:03:49.538Z,tags: [cuadb,phase2-ui],order: 6144,depends_on: [TSK-A448B23E0C59],extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-05T00:03:49.538Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-CB16F5548879/1793837029540-start,started_at: 2026-11-05T00:03:49.565Z}]}}}
---

# Build the typed row editor, insert and delete flows

## Goal
Implement the demo's row editing UI over the existing typed `/data/insert`, `/data/update` and `/data/delete` routes and the transaction controls.

## Context
Editing is visible only when the profile is writable and the principal has write access to the relation; delete requires destructive access. The inspector header offers Edit and Delete, and the toolbar offers `+ Add row`. The editor shows a typed control per column (select for enums, checkbox for booleans, textarea with live JSON validation for jsonb, datetime for timestamps, number for numeric types, comma list for arrays), a NULL toggle for nullable columns, read-only primary keys on edit, and a sticky footer with the error text. The backend enforces full-primary-key addressing, stale-row checks and bound values, so the UI never builds SQL. Deletes require a confirmation modal. The pending-change bar from `/tx/status` offers commit and rollback with the exact count. Everything is audited server-side and escaped on output.

This task belongs to **15 — Authenticated application UI, administration and connection management**. The look, feel and density come from `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. No inline scripts or styles are allowed by the CSP, and acceptance is a manual side-by-side browser comparison plus focused PostgreSQL and SQLite Rust tests; MySQL and MariaDB are out of scope until the frontend milestone.

## Acceptance checklist
- [x] Render the editor and insert form from catalog column metadata with the typed controls, NULL toggles and server-side error mapping.
- [x] Add Edit and Delete to the inspector and `+ Add row` to the toolbar, gated by capability and access level, with the delete confirmation modal.
- [ ] Show the pending-change bar with commit and rollback and test stale-row and constraint errors on PostgreSQL and SQLite.
- [ ] Verify escaping of malicious values and compare the editor with the demo in the built-in browser in light and dark.

## Verification and handoff
Run the focused tests for this slice, run `bun run format:check` and `bun run check` at the group boundary, add a Task comment with the evidence and any deliberate departure from the demo, tick accepted items with `task check`, and only then mark the task completed.

## Comments

### 2026-11-05T00:03:53.373Z · PETER IRUNGU

Progress (2026-11-05): the grid offers + Add row, and Edit and Delete in the inspector, only when CUADB_SQL_WORKSPACE is enabled, the connection is writable, the user holds edit on the relation and a primary key is visible (tests/sql_workspace_sqlite.rs). Typed controls (text, number, boolean select), NULL toggles, read-only keys and unsupported types, stale-row conflicts reported as conflicts, constraint errors mapped, delete confirmed inline, values escaped. Browser-verified on SQLite: insert, edit with NULL toggle, stale conflict, delete, search-preserving refresh. Open: item 3 (edits here are single statements, not part of a transaction, and the PostgreSQL write fixtures need CUADB_TEST_PG_WRITER_URL) and item 4 (light and dark comparison with the demo).
