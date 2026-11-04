---
{id: TSK-0AA246B508ED,name: Implement Table Info tab with honest statistics and storage,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:07.130Z,updated_at: 2026-11-04T11:29:48.662Z,tags: [cuadb,inspectors],depends_on: [TSK-CFEA5F712D4B,TSK-FFD6A5B94574],order: 2048,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:48.662Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-0AA246B508ED/1793791788664-start,started_at: 2026-11-04T11:29:48.680Z}]}}}
---

# Implement Table Info tab with honest statistics and storage

## Goal
Display useful measurements including estimated rows, table data bytes, index bytes, combined size and relation type. Every value must be clearly identified as exact, estimated or unavailable where applicable.

## Context
PostgreSQL pg_stat and size functions expose useful metrics but have important semantics for partitioned and foreign tables. Expensive exact counts are excluded from default navigation. Cached values may be stale; the interface should offer a lightweight explicit refresh action.

This task belongs to **08 — Structure, statistics and PostgreSQL rich values**. The group outcome is: Complete the table inspection experience through Structure and Info panels and careful handling of PostgreSQL-specific value types. Users should inspect data definitions and storage without complex administration screens, and PostgreSQL/PostGIS objects should remain readable even when they do not map to common scalar types.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Render adapter-provided sizes, estimated rows and relation classification clearly.
- [ ] Explain estimates, unknown values and partition-related size caveats in the UI.
- [ ] Provide a bounded refresh action without forcing SELECT COUNT(*) over large tables.
- [ ] Test empty, partitioned, foreign and permission-restricted fixture scenarios.
- [ ] Use the same capability flags that other database adapters will expose.

## Canonical UI fidelity

The canonical frontend is `assets/cuadb-ui-demo.html`, described in `.docs/docs/frontend-interactions.md`. Match its actual visible control labels, compact placement and selected-object behavior. In particular, the source uses **Data / Structure / Indexes** in its table toolbar and a separate Advanced **Overview**, so the older plan's independent `Info` tab terminology is superseded. The production UI uses safe real metadata rather than hard-coded statistics. User approval is required for material visual changes.

### Reference-specific acceptance
- [ ] Expose indexes and storage in the demo's 'Indexes' segment, NOT a standalone 'Info' tab; show index name/definition/unique and storage rows for total, table data, indexes, referenced-by, estimated rows and schema.
- [ ] Match the Advanced-mode database Overview cards, schema size bars, largest tables and extension list, using available and permission-safe real metadata.
- [ ] Label estimates, empty sizes and unsupported metrics truthfully; do not perform unbounded full scans merely to reproduce example demo numbers.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T13:56:03.605Z · PETER IRUNGU

Group 07 note: the inspector reads bounded per-cell detail embedded in the data page (RowPreview + detail_value), so there is no separate row-detail route and no row-identity lookup; item 2 (detail fetch tied to row identity) is satisfied by design only if that is accepted - leaving items open for Phase B. Truncation markers (aria-label truncated) are rendered and escaping verified.
