---
{id: TSK-8DB2E738839F,name: Normalize PostgreSQL cell types and bound large values,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:05.546Z,updated_at: 2026-11-04T07:32:05.546Z,tags: [cuadb,postgres-rows],depends_on: [TSK-23B4836BB3AE],order: 5120}
---

# Normalize PostgreSQL cell types and bound large values

## Goal
Convert supported SQLx row values into a consistent typed data model for Askama templates while retaining nullability and avoiding unlimited memory or HTML response sizes. Support graceful representation of extension and unknown types.

## Context
PostgreSQL tables contain primitives, UUIDs, timestamps, JSON, arrays, bytea and custom enums. The grid does not need to materialize every value in full; long content should be truncated with a safe marker and inspected on demand through a separate bounded endpoint. Output escaping is mandatory.

This task belongs to **04 — Safe data retrieval, sorting and pagination**. The group outcome is: Implement the core read-only viewing capability with bounded, predictable SQL and correct typed values. A user must be able to open any permitted PostgreSQL table, read a page, sort an allowed column and navigate safely without requesting the entire relation. This group is the performance and query-security center of the project.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Decode common PostgreSQL scalar types and NULL distinctly from empty strings.
- [ ] Cap rendered bytes per cell, total response size and expanded-value retrieval.
- [ ] Preserve precise raw representations for timestamps, numbers and UUID values.
- [ ] Handle unsupported or extension types with a safe textual fallback and type indicator.
- [ ] Test UTF-8 escaping, very large values, binary data and malformed conversions.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
