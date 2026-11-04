---
{id: GOAL-GRP-4E0980620848,name: 09 — MySQL and MariaDB compatibility goal,type: documentation,extra: {created_at: 2026-11-04T07:32:07.411Z,group: GRP-4E0980620848}}
---

---
{id: GOAL-GRP-4E0980620848,name: 09 — MySQL and MariaDB compatibility goal,type: documentation,extra: {created_at: 2026-11-04T07:32:07.411Z,group: GRP-4E0980620848}}
---

# 09 — MySQL and MariaDB compatibility — Goal

## Outcome
Bring MySQL and MariaDB into CuaDB through a separate adapter while preserving the same read-only navigation and data-grid contract. Reuse common models and browser components, but implement engine-specific discovery, identifier quoting, data decoding and statistics rather than assuming PostgreSQL SQL syntax works everywhere.

## Definition of done
Acceptance of **09 — MySQL and MariaDB compatibility** requires evidence for these results:
1. **Implement MySQL/MariaDB URL parsing, pools and capabilities:** Support mysql:// connections through a bounded, read-only adapter that identifies the server flavor and reports available catalog, data and statistics features. Keep network failure and authentication errors consistent with PostgreSQL behavior. Verification includes: Recognize supported MySQL URLs and reject unsupported option or host forms.
2. **Discover MySQL/MariaDB namespaces, relations and columns:** Map engine catalog metadata into the common database explorer tree. Users should browse available databases, tables and views, then inspect column types, defaults, indexes, primary keys and foreign-key references without a SQL editor. Verification includes: List accessible databases, tables and views with engine-correct object types.
3. **Fetch and paginate MySQL/MariaDB rows with typed values:** Provide the same bounded grid and sort interaction used for PostgreSQL, with MySQL-specific SQL generation, type decoding and deterministic-order rules. Avoid front-end forks for the two database families. Verification includes: Implement allowlisted relation and column quoting for MySQL SQL generation.
4. **Map MySQL/MariaDB sizes and metadata limitations to Info tab:** Display accurate available metadata such as estimated rows, storage and index bytes while making engine-specific limitations visible. The UI should not pretend exact PostgreSQL-equivalent statistics are always available. Verification includes: Read supported table and index size metrics without costly full-table scans.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
