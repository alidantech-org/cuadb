---
{id: TSK-58382C325F5F,name: Add optional PostGIS geometry and geography inspection,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:07.352Z,updated_at: 2026-11-04T11:29:48.994Z,tags: [cuadb,inspectors],depends_on: [TSK-11615BE5E97B,TSK-725618F4E684],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:48.994Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-58382C325F5F/1793791788996-start,started_at: 2026-11-04T11:29:49.010Z}]}}}
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
- [ ] Surface geometry/geography types and bounded type, SRID and GeoJSON content within the demo's existing grid and right-side row inspector, not in a separate map-oriented page.
- [ ] Retain the demo's Data/Structure/Indexes header and compact table layout when PostGIS is installed or absent.
- [ ] Compare the spatial-data inspector presentation to the demo's row-detail style, recording safety and capability exceptions explicitly.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:00:06.424Z · PETER IRUNGU

Items 1,2,4 verified live on PostGIS 3.6: extension + geometry/geography detection, type/SRID in column type, opt-in bounded GeoJSON (>~1KB refused), non-4326 labelled with CRS, POINT EMPTY preserved, no maps/bulk conversion. Item 3 partial (invalid geometry values not covered). Item 5 BLOCKED: no PostgreSQL fixture without PostGIS exists locally. 6-8 UI.
