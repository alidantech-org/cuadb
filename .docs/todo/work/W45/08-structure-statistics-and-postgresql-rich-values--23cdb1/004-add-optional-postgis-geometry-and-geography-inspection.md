---
{id: TSK-58382C325F5F,name: Add optional PostGIS geometry and geography inspection,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:07.352Z,updated_at: 2026-11-05T00:19:30.142Z,tags: [cuadb,inspectors],depends_on: [TSK-11615BE5E97B,TSK-725618F4E684],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:48.994Z},{from: in-progress,to: blocked,at: 2026-11-04T16:14:20.643Z},{from: blocked,to: planned,at: 2026-11-04T23:18:20.260Z},{from: planned,to: in-progress,at: 2026-11-05T00:19:30.142Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-58382C325F5F/1793791788996-start,started_at: 2026-11-04T11:29:49.010Z,end_ref: refs/tracking/TSK-58382C325F5F/1793808860645-end,ended_at: 2026-11-04T16:14:20.644Z,summary: {files: 91,insertions: 10231,deletions: 706}},{base_ref: refs/tracking/TSK-58382C325F5F/1793837970144-start,started_at: 2026-11-05T00:19:30.171Z}]}},links: [{type: blocks,task: TSK-A7B0DFC23314}]}
---

# Add optional PostGIS geometry and geography inspection

## Goal
Detect supported PostGIS types and show geometry subtype, SRID and bounded spatial representations on demand. Keep spatial data optional, read-only and safe even when geometry columns contain large or complex objects.

## Context
The existing Fedora database image is postgis/postgis:18-3.6, but an extension is installed per database and may not exist in every connected database. PostGIS metadata can describe geometry/geography types; GeoJSON conversion should be explicit and bounded, avoiding automatic full-table transformations or a heavy mapping UI.

This task belongs to **08 — Structure, statistics and PostgreSQL rich values**. The group outcome is: Complete the table inspection experience through Structure and Info panels and careful handling of PostgreSQL-specific value types. Users should inspect data definitions and storage without complex administration screens, and PostgreSQL/PostGIS objects should remain readable even when they do not map to common scalar types.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Detect PostGIS extension availability and geometry/geography columns per database.
- [x] Expose safe geometry type, SRID and optional bounded GeoJSON detail.
- [ ] Handle invalid, huge and non-4326 spatial values without crashes or misleading coordinates.
- [x] Do not introduce map rendering or expensive bulk geometry conversion by default.
- [ ] Test on both PostGIS-enabled and plain PostgreSQL fixture databases.

## Canonical UI fidelity

The canonical frontend is `assets/cuadb-ui-demo.html`, described in `.docs/docs/frontend-interactions.md`. Match its actual visible control labels, compact placement and selected-object behavior. In particular, the source uses **Data / Structure / Indexes** in its table toolbar and a separate Advanced **Overview**, so the older plan's independent `Info` tab terminology is superseded. The production UI uses safe real metadata rather than hard-coded statistics. User approval is required for material visual changes.

### Reference-specific acceptance
- [x] Surface geometry/geography types and bounded type, SRID and GeoJSON content within the demo's existing grid and right-side row inspector, not in a separate map-oriented page.
- [x] Retain the demo's Data/Structure/Indexes header and compact table layout when PostGIS is installed or absent.
- [x] Compare the spatial-data inspector presentation to the demo's row-detail style, recording safety and capability exceptions explicitly.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [x] PostGIS inspection stays optional and hidden when the extension is absent; if no PostGIS fixture exists the item stays open and is recorded as a limitation.

## Comments

### 2026-11-04T14:00:06.424Z · PETER IRUNGU

Items 1,2,4 verified live on PostGIS 3.6: extension + geometry/geography detection, type/SRID in column type, opt-in bounded GeoJSON (>~1KB refused), non-4326 labelled with CRS, POINT EMPTY preserved, no maps/bulk conversion. Item 3 partial (invalid geometry values not covered). Item 5 BLOCKED: no PostgreSQL fixture without PostGIS exists locally. 6-8 UI.

### 2026-11-04T16:14:20.772Z · PETER IRUNGU

Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.

### 2026-11-05T00:19:39.555Z · PETER IRUNGU

Phase 2 (2026-11-05): with the PostGIS fixture, geometry columns show type and SRID, the inspector shows the geometry text and bounded GeoJSON, the header and layout are unchanged, and nothing spatial appears without the extension. Items 3 (invalid and huge geometries) and 5 (a plain PostgreSQL fixture) stay open.
