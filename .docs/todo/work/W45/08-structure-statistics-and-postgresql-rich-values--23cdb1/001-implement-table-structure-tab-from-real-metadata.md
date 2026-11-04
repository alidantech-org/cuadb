---
{id: TSK-8EC2E48964BC,name: Implement table Structure tab from real metadata,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:07.044Z,updated_at: 2026-11-04T11:29:48.511Z,tags: [cuadb,inspectors],depends_on: [TSK-047C7016EEAA,TSK-FFD6A5B94574],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:48.511Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-8EC2E48964BC/1793791788513-start,started_at: 2026-11-04T11:29:48.525Z}]}}}
---

# Implement table Structure tab from real metadata

## Goal
Show ordered column definitions, nullability, defaults, identity flags, primary and foreign keys and relevant indexes in a readable layout. The tab must work for supported views as well as ordinary tables and explain when metadata does not apply.

## Context
Catalog information is supplied by the PostgreSQL adapter, not by direct queries inside Askama templates. Composite keys and cross-schema foreign keys need clear representation. Users should be able to understand a table at a glance without opening a secondary database management application.

This task belongs to **08 — Structure, statistics and PostgreSQL rich values**. The group outcome is: Complete the table inspection experience through Structure and Info panels and careful handling of PostgreSQL-specific value types. Users should inspect data definitions and storage without complex administration screens, and PostgreSQL/PostGIS objects should remain readable even when they do not map to common scalar types.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Render ordered columns and native data type names with correct key indicators.
- [ ] Expose indexes, constraints and foreign-key targets with safe relation links.
- [x] Handle views, partition parents and inaccessible related objects gracefully.
- [x] Test Structure tab with a complex fixture containing composite keys and defaults.
- [ ] Keep the tab responsive through HTMX fragment updates without page reloads.

## Canonical UI fidelity

The canonical frontend is `assets/cuadb-ui-demo.html`, described in `.docs/docs/frontend-interactions.md`. Match its actual visible control labels, compact placement and selected-object behavior. In particular, the source uses **Data / Structure / Indexes** in its table toolbar and a separate Advanced **Overview**, so the older plan's independent `Info` tab terminology is superseded. The production UI uses safe real metadata rather than hard-coded statistics. User approval is required for material visual changes.

### Reference-specific acceptance
- [ ] Match the demo's inline Data / Structure / Indexes segmented toolbar, where 'Structure' is a mode within the existing main panel rather than a separate page or redesigned tab.
- [ ] Render Structure fields in the reference order and styling: #, Column, Type, Nullable, Key, References, with accurate schema metadata.
- [ ] Keep the schema-qualified header and compact table styling identical while distinguishing views and unavailable attributes honestly.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:22:43.140Z · PETER IRUNGU

Implemented initial structure and indexes/statistics table fragments backed by shared adapter metadata. A live MariaDB fixture verified columns, keys, generated values and rough row statistics. Canonical UI comparison and engine-specific feature details remain to validate.

### 2026-11-04T14:00:06.046Z · PETER IRUNGU

BACKEND VERIFIED - FRONTEND ACCEPTANCE PENDING. Live PG18 (tests/rich_live_http.rs): composite PK, identity, generated, defaults, enum/domain, FK targets, views (no constraints), partition parents, inaccessible catalog relation -> 4xx. Defect fixed: identity/generated showed raw catalog codes ('a','s'); now ALWAYS/BY DEFAULT and STORED/VIRTUAL. Item 2 left open: FK targets render as text, not safe relation links. Items 5-8 are UI/HTMX visual (Phase B).
