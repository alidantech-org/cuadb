---
{id: TSK-11615BE5E97B,name: "Improve PostgreSQL JSON, arrays, binary and extension value display",type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:07.257Z,updated_at: 2026-11-05T00:19:29.891Z,tags: [cuadb,inspectors],depends_on: [TSK-8DB2E738839F,TSK-ABE2D51C83F3],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:48.837Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:20.301Z},{from: blocked,to: planned,at: 2026-11-04T23:18:20.217Z},{from: planned,to: in-progress,at: 2026-11-05T00:19:29.891Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-11615BE5E97B/1793791788839-start,started_at: 2026-11-04T11:29:48.856Z,end_ref: refs/tracking/TSK-11615BE5E97B/1793808860303-end,ended_at: 2026-11-04T16:14:20.301Z,summary: {files: 91,insertions: 10231,deletions: 706}},{base_ref: refs/tracking/TSK-11615BE5E97B/1793837969894-start,started_at: 2026-11-05T00:19:29.920Z}]}},links: [{type: blocks,task: TSK-58382C325F5F}]}
---

# Improve PostgreSQL JSON, arrays, binary and extension value display

## Goal
Make complex PostgreSQL values readable through structured previews and safe detail formatting. Types such as jsonb, UUID, arrays, enum labels, timestamp with time zone and bytea should retain important semantics without bloating row pages.

## Context
The generic row model provides typed cells, but individual presentation strategies differ. JSON should be indented in a detail panel; binary data should show size or a limited encoding preview; timestamps should not silently lose zone information. Unknown extension types require a predictable fallback.

This task belongs to **08 — Structure, statistics and PostgreSQL rich values**. The group outcome is: Complete the table inspection experience through Structure and Info panels and careful handling of PostgreSQL-specific value types. Users should inspect data definitions and storage without complex administration screens, and PostgreSQL/PostGIS objects should remain readable even when they do not map to common scalar types.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Add bounded formatting for JSON/JSONB, arrays, enums and bytea.
- [x] Preserve date/time zone, exact numeric and NULL semantics in displayed values.
- [x] Prevent database strings from rendering as unsanitized HTML.
- [ ] Test deeply nested JSON, large arrays, invalid UTF-8 representations and binary values.
- [x] Document type-specific rendering behavior and size limits.

## Canonical UI fidelity

The canonical frontend is `assets/cuadb-ui-demo.html`, described in `.docs/docs/frontend-interactions.md`. Match its actual visible control labels, compact placement and selected-object behavior. In particular, the source uses **Data / Structure / Indexes** in its table toolbar and a separate Advanced **Overview**, so the older plan's independent `Info` tab terminology is superseded. The production UI uses safe real metadata rather than hard-coded statistics. User approval is required for material visual changes.

### Reference-specific acceptance
- [x] Use the same compact typed grid cells and selected-row inspector as the demo for JSONB, arrays, timestamps, UUID, inet, enums, binary and custom values; preserve multiline detail formatting.
- [x] Integrate the demo's data-cell monospace setting, NULL display setting, ellipsis and wrapping without creating a new inspection layout.
- [x] Escape all values and apply strict payload/cell limits so visual fidelity remains safe with large JSON and unusual extension types.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [ ] JSON, arrays, enums, uuid, inet and timestamps use the demo's pill and monospace treatments, and the editor reuses the same typed rendering rules.

## Comments

### 2026-11-04T14:00:06.145Z · PETER IRUNGU

Items 1,8 verified live: sizes/estimated rows/type from the adapter, 'Engine-reported estimate; not an exact COUNT(*)' captions, partitioned parent shows Unavailable, no COUNT scan. Left open: 2 (UI explanation placement), 3 (refresh action is the same bounded read; no UI control), 4 (no foreign-table / permission-restricted-stats fixture), 5 (capability flags for other engines), 6,7 UI.

### 2026-11-04T14:47:17.915Z · PETER IRUNGU

CORRECTION/UPDATE: this is the rich-value task. Evidence (tests/rich_live_http.rs, live PostgreSQL 18): items 1,2,3,5,8 verified - bounded JSON/array/enum/bytea, exact numeric text, NULL vs empty, timestamptz microseconds in session zone (UTC), markup escaped and control/bidi characters shown as \u{XXXX}, README 'Rich values' section. Item 4 partial (20k-element JSON/array, 20 KB bytea tested; deeply nested JSON not). Earlier comment on this task wrongly described storage statistics; those belong to TSK-0AA246B508ED. Items 6,7 are UI.

### 2026-11-04T16:14:20.462Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.

### 2026-11-05T00:19:39.373Z · PETER IRUNGU

Phase 2 (2026-11-05): JSON, arrays, uuid, inet, timestamps, bytea and numeric render in the demo's compact monospace and pill styles and the inspector keeps multiline structure; the monospace, NULL and wrap settings apply. Item 4 (deeply nested JSON test) and item 9 (user-defined enums get no special pill, and the editor is not driven by these rules) stay open.
