---
{id: TSK-11615BE5E97B,name: "Improve PostgreSQL JSON, arrays, binary and extension value display",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:07.257Z,updated_at: 2026-11-04T07:32:07.257Z,tags: [cuadb,inspectors],depends_on: [TSK-8DB2E738839F,TSK-ABE2D51C83F3],order: 3072}
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
- [ ] Add bounded formatting for JSON/JSONB, arrays, enums and bytea.
- [ ] Preserve date/time zone, exact numeric and NULL semantics in displayed values.
- [ ] Prevent database strings from rendering as unsanitized HTML.
- [ ] Test deeply nested JSON, large arrays, invalid UTF-8 representations and binary values.
- [ ] Document type-specific rendering behavior and size limits.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
