---
{id: TSK-5EE171C4C580,name: Connect schema and table tree to real catalog data,type: task,status: blocked,priority: medium,created_at: 2026-11-04T07:32:06.091Z,updated_at: 2026-11-04T16:14:56.886Z,tags: [cuadb,explorer-ui],depends_on: [TSK-A513FD340BD5,TSK-5FC58B7B6E27,TSK-9890D0090922],order: 2048,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:11.351Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:16.782Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-5EE171C4C580/1793791751353-start,started_at: 2026-11-04T11:29:11.368Z,end_ref: refs/tracking/TSK-5EE171C4C580/1793808856783-end,ended_at: 2026-11-04T16:14:16.782Z,summary: {files: 91,insertions: 10231,deletions: 706}}]}},links: [{type: blocks,task: TSK-BE512E99EA68}]}
---

# Connect schema and table tree to real catalog data

## Goal
Show accessible databases or configured connections, schemas and supported relation types in a collapsible tree. Clicking a table should select it, update the main panel and preserve clear visual focus without a full-page reload.

## Context
The PostgreSQL adapter supplies typed relation metadata, while the sidebar must not expose inaccessible objects or raw catalog rows. Schemas can be expanded on demand, and common types need distinct labels. Name filtering should be quick and should not force loading every record from every relation.

This task belongs to **06 — Database navigation and workspace UI**. The group outcome is: Replace the demo shell with a clean, responsive database explorer focused on fast navigation. The left sidebar should display connections, schemas and relation names; the main panel should immediately show data or a meaningful empty state. Preserve the lightweight Rust-rendered design without React, Next.js or a frontend state library.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Load available schema nodes and relation lists through real adapter-backed routes.
- [ ] Support expand/collapse and distinguish tables, views and partition parents.
- [x] Implement empty schema and permission-denied states without broken tree nodes.
- [ ] Preserve selected relation and active tab across HTMX navigation.
- [x] Test tree behavior with many relations and unusual names.

## Canonical UI fidelity

**Mandatory visual specification:** `assets/cuadb-ui-demo.html` (see `.docs/docs/frontend-interactions.md`). Its sidebar, interactions, state patterns and typography are required, not decorative inspiration. Recreate the reference with Askama/HTMX, Tailwind and minimal JavaScript using actual database data; do not replace the interface with a conventional dashboard. Safety and accessible real-data states may adapt the underlying mechanism, not silently redesign the appearance.

### Reference-specific acceptance
- [ ] Match Simple explorer schema/table/view tree and Advanced explorer Overview plus grouped Tables, Views, Functions, Procedures, Sequences, Types and Triggers, filtered by actual adapter capabilities and permissions.
- [ ] Preserve the demo's icon mapping, 24px dense rows, expand chevrons, schema/group counts, selectable table rows, relation-linked highlight and optional nested column previews.
- [ ] Support metadata-only routine/type/sequence/trigger detail screens; do not offer procedure execution, writes or fabricated server objects.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:29:34.362Z · PETER IRUNGU

Schema fragment is loaded through HTMX from actual PostgreSQL adapter routes. Relation names carry safe navigation URLs and are filtered client-side without reading row data. Collapsible grouped schema tree and keyboard selection behavior remain incomplete.

### 2026-11-04T13:51:44.968Z · PETER IRUNGU

Items 3,5 verified: permission-denied and empty states at route level, unusual/Unicode names live, 500-relation truncated tree. Fixed defect: truncation note was only rendered for empty lists (templates/components/relation_tree.html) - now shown whenever the list is cut off. Items 2,4,6,7,8 UI/expansion state: pending Phase B.

### 2026-11-04T16:14:16.969Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.
