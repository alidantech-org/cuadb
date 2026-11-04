---
{id: TSK-EC8CC62AD5CD,name: Build validated identifier and query construction helpers,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:05.266Z,updated_at: 2026-11-04T11:13:52.096Z,tags: [cuadb,postgres-rows],depends_on: [TSK-7AA22F3D0843],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:13:51.650Z},{from: in-progress,to: completed,at: 2026-11-04T11:13:52.096Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-EC8CC62AD5CD/1793790831652-start,started_at: 2026-11-04T11:13:51.665Z,end_ref: refs/tracking/TSK-EC8CC62AD5CD/1793790832100-end,ended_at: 2026-11-04T11:13:52.096Z,summary: {files: 0,insertions: 0,deletions: 0}}]}}}
---

# Build validated identifier and query construction helpers

## Goal
Ensure every dynamic SQL identifier comes from a catalog-validated allowlist, with correct PostgreSQL quoting and no execution of user-submitted statements. This provides the trusted boundary for relation selection, sorting and row reads.

## Context
SQLx parameter binding protects values but not identifiers such as schema, table and column names. Catalog-discovered relation IDs or validated exact names should be used, with rules for reserved words, Unicode, quotes and case sensitivity. The helper should be small, independently testable and used by all PostgreSQL row routes.

This task belongs to **04 — Safe data retrieval, sorting and pagination**. The group outcome is: Implement the core read-only viewing capability with bounded, predictable SQL and correct typed values. A user must be able to open any permitted PostgreSQL table, read a page, sort an allowed column and navigate safely without requesting the entire relation. This group is the performance and query-security center of the project.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Implement safe schema/table/column resolution against trusted catalog objects.
- [x] Quote identifiers correctly and bind pagination values instead of concatenating them.
- [x] Reject unknown columns, unsupported relation types and malformed sort instructions.
- [x] Test injection payloads, quoted names, unusual Unicode and reserved keywords.
- [x] Require all row-fetch code paths to use the shared validated helper.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:13:51.921Z · PETER IRUNGU

Verified catalog allowlisting, PostgreSQL identifier quoting, bound pagination parameters, malformed sort/column rejection, reserved names, quoted Unicode, and injection-shaped inputs via unit/HTTP tests and live PostgreSQL 18.6 fixtures.
