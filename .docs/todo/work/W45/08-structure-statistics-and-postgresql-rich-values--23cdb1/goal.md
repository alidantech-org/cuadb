---
{id: GOAL-GRP-230A7623CDB1,name: "08 — Structure, statistics and PostgreSQL rich values goal",type: documentation,extra: {created_at: 2026-11-04T07:32:06.961Z,group: GRP-230A7623CDB1}}
---

## Phase gate

This is Phase 2 frontend work. Do not start or continue implementation until the PostgreSQL + SQLite backend groups (01–05 and 10) are accepted. The backend must provide real, stable data and HTML-fragment contracts first. Do not add MySQL/MariaDB checks to frontend tasks. Keep the committed `assets/cuadb-ui-demo.html` as the binding reference and perform focused visual acceptance at the group boundary, not repeated full screenshot matrices for every small change.
# 08 — Structure, statistics and PostgreSQL rich values — Goal

## Outcome
Complete the table inspection experience through Structure and Info panels and careful handling of PostgreSQL-specific value types. Users should inspect data definitions and storage without complex administration screens, and PostgreSQL/PostGIS objects should remain readable even when they do not map to common scalar types.

## Definition of done
Acceptance of **08 — Structure, statistics and PostgreSQL rich values** requires evidence for these results:
1. **Implement table Structure tab from real metadata:** Show ordered column definitions, nullability, defaults, identity flags, primary and foreign keys and relevant indexes in a readable layout. The tab must work for supported views as well as ordinary tables and explain when metadata does not apply. Verification includes: Render ordered columns and native data type names with correct key indicators.
2. **Implement Table Info tab with honest statistics and storage:** Display useful measurements including estimated rows, table data bytes, index bytes, combined size and relation type. Every value must be clearly identified as exact, estimated or unavailable where applicable. Verification includes: Render adapter-provided sizes, estimated rows and relation classification clearly.
3. **Improve PostgreSQL JSON, arrays, binary and extension value display:** Make complex PostgreSQL values readable through structured previews and safe detail formatting. Types such as jsonb, UUID, arrays, enum labels, timestamp with time zone and bytea should retain important semantics without bloating row pages. Verification includes: Add bounded formatting for JSON/JSONB, arrays, enums and bytea.
4. **Add optional PostGIS geometry and geography inspection:** Detect supported PostGIS types and show geometry subtype, SRID and bounded spatial representations on demand. Keep spatial data optional, read-only and safe even when geometry columns contain large or complex objects. Verification includes: Detect PostGIS extension availability and geometry/geography columns per database.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.

## Mandatory visual acceptance

The canonical frontend specification for this group is `assets/cuadb-ui-demo.html`, reinforced by `.docs/docs/frontend-interactions.md`. It is a binding acceptance baseline for all related tasks, not an alternative design sketch. Completion requires the demonstration's inline Data / Structure / Indexes mode switcher, with structure column details, indexes and storage together, and a separate Advanced Overview. The visual style, compact cards, size bars, object details and selected-row inspector must follow the demo. When an engine lacks specific metadata, display an honest unsupported state in the same design instead of inventing values or using a different page layout. All relevant tasks must pass a side-by-side comparison against the same browser viewport, mode and selected object state, and must not be marked complete based on a different layout.
