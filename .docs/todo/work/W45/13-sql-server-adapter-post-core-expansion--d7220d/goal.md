---
{id: GOAL-GRP-5F8CABD7220D,name: 13 — SQL Server adapter (post-core expansion) goal,type: documentation,extra: {created_at: 2026-11-04T07:32:09.542Z,group: GRP-5F8CABD7220D}}
---

---
{id: GOAL-GRP-5F8CABD7220D,name: 13 — SQL Server adapter (post-core expansion) goal,type: documentation,extra: {created_at: 2026-11-04T07:32:09.542Z,group: GRP-5F8CABD7220D}}
---

# 13 — SQL Server adapter (post-core expansion) — Goal

## Outcome
Evaluate and implement Microsoft SQL Server support after the PostgreSQL, MySQL/MariaDB and SQLite viewer is proven. Preserve the same read-only explorer, data grid and metadata contract while respecting SQL Server-specific drivers, namespaces, catalog views and pagination requirements.

## Definition of done
Acceptance of **13 — SQL Server adapter (post-core expansion)** requires evidence for these results:
1. **Evaluate SQL Server Rust driver, TLS and connection contract:** Validate the feasibility and security of a SQL Server adapter using an actively maintained Rust driver before committing to the implementation. Produce a clear go/no-go decision and compatible URL, timeout and authentication strategy. Verification includes: Evaluate supported Rust drivers, maintenance status, TLS and licensing constraints.
2. **Implement SQL Server schema, relation and column discovery:** Expose SQL Server databases and schemas, tables and views, columns, keys and indexes through CuaDB’s adapter interface. Do not force SQL Server catalog behavior into PostgreSQL-only models. Verification includes: Discover accessible schemas, tables and views with correct identifiers.
3. **Add SQL Server bounded rows, sorting and honest Info metadata:** Complete basic SQL Server browsing with paginated data, safe column sorting and available table-level metadata, using the same grid and inspector templates already supported by other engines. Verification includes: Fetch bounded pages and enforce relation/column allowlists for sorting.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
