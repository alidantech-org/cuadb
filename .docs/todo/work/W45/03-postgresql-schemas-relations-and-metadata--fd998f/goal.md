---
{id: GOAL-GRP-51AE7EFD998F,name: "03 — PostgreSQL schemas, relations and metadata goal",type: documentation,extra: {created_at: 2026-11-04T07:32:04.794Z,group: GRP-51AE7EFD998F}}
---

---
{id: GOAL-GRP-51AE7EFD998F,name: "03 — PostgreSQL schemas, relations and metadata goal",type: documentation,extra: {created_at: 2026-11-04T07:32:04.794Z,group: GRP-51AE7EFD998F}}
---

# 03 — PostgreSQL schemas, relations and metadata — Goal

## Outcome
Deliver complete, permission-aware catalog browsing for PostgreSQL and PostGIS installations. Users must find schemas, tables and views; inspect column definitions, indexes, keys, constraints and storage sizes; and understand when a statistic is approximate or unavailable. All catalog queries must be scoped, efficient and read-only.

## Definition of done
Acceptance of **03 — PostgreSQL schemas, relations and metadata** requires evidence for these results:
1. **Discover accessible PostgreSQL schemas, tables and views:** Return a stable tree of visible schemas and supported relation types for the sidebar, including ordinary tables, views, materialized views and partitions. Hide noise by default while retaining a clear path to inspect relevant non-default schemas. Verification includes: List accessible user schemas without assuming public is the only namespace.
2. **Inspect PostgreSQL columns, types, defaults and constraints:** Provide a complete column structure model including native type, nullability, default expression, generated/identity status and primary or unique key indicators. The user should understand table shape without opening a SQL console. Verification includes: Return ordered column names, data types, nullable flags and default expressions.
3. **Expose keys, foreign keys, indexes and partition hierarchy:** Model PostgreSQL relationships and indexes so users can understand table references, uniqueness and sorting opportunities. Keep this metadata read-only and bounded, and support partitioned tables without double-counting data or misrepresenting inherited structure. Verification includes: Discover foreign key source and target columns with schema-qualified relation names.
4. **Implement efficient PostgreSQL table size and statistics inspection:** Expose total relation storage, table bytes, index bytes and estimated rows without running expensive full scans. Make estimates clearly distinguishable from exact measurements and avoid misleading totals for partitioned or foreign tables. Verification includes: Report available table, index and total size metrics with correct units and nullability.
5. **Validate catalog coverage against PostgreSQL 18 and PostGIS:** Prove the catalog adapter is usable with the existing Fedora postgis/postgis:18-3.6 image and conventional PostgreSQL deployments. Record supported object behavior and failure modes without depending on a developer machine for ordinary unit tests. Verification includes: Create repeatable integration fixtures for tables, views, indexes, constraints and partitions.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
