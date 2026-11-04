---
{id: TSK-242B6B658703,name: Fetch and paginate MySQL/MariaDB rows with typed values,type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:07.718Z,updated_at: 2026-11-04T12:46:13.178Z,tags: [cuadb,mysql],depends_on: [TSK-A7B0DFC23314,TSK-8B8935A1684A],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T12:46:13.178Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-242B6B658703/1793796373180-start,started_at: 2026-11-04T12:46:13.202Z}]}}}
---

# Fetch and paginate MySQL/MariaDB rows with typed values

## Goal
Provide the same bounded grid and sort interaction used for PostgreSQL, with MySQL-specific SQL generation, type decoding and deterministic-order rules. Avoid front-end forks for the two database families.

## Context
MySQL date, decimal, JSON, binary and enum representations can differ from PostgreSQL. Limit retrievals, use safe identifiers and bind ordinary values. Cursor paging should be used only when a valid stable unique key and compatible ordering exist; otherwise disclose paging limitations.

This task belongs to **09 — MySQL and MariaDB compatibility**. The group outcome is: Bring MySQL and MariaDB into CuaDB through a separate adapter while preserving the same read-only navigation and data-grid contract. Reuse common models and browser components, but implement engine-specific discovery, identifier quoting, data decoding and statistics rather than assuming PostgreSQL SQL syntax works everywhere.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Implement allowlisted relation and column quoting for MySQL SQL generation.
- [x] Provide bounded pages, safe sort controls and deterministic ties when available.
- [ ] Normalize JSON, binary, decimal, time and NULL values without losing meaning.
- [x] Reuse shared page responses and HTMX templates without MySQL-specific HTML branches.
- [ ] Test pagination and sorting against MySQL and MariaDB fixtures.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:20:01.478Z · PETER IRUNGU

Implemented bounded typed grid retrieval with catalog-allowlisted quoted identifiers, primary-key ordering/tie breaks when available, validated sorts, literal bound filter, bytea-like binary hex decoding, bounded text and shared RowPage model. MariaDB 11.8 disposable fixture verifies one-row page, has_more and integer decoding. Keyset cursors are not supported and fall back to bounded offset paging; mark cursor-specific checklist items as limitations.
