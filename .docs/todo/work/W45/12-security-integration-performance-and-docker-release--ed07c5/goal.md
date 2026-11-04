---
{id: GOAL-GRP-C11E16ED07C5,name: "12 — Security, integration, performance and Docker release goal",type: documentation,extra: {created_at: 2026-11-04T07:32:08.777Z,group: GRP-C11E16ED07C5}}
---

## Final release gate

This group is the final acceptance phase. Do not start it as a substitute for completing the core backend, frontend, deferred engine work, trusted profiles, authentication or controlled SQL workspace. Release evidence must cover the current product scope and the dedicated security boundaries for any enabled SQL/CRUD capability. Keep engine verification focused on PostgreSQL and SQLite plus only the remaining MySQL/MariaDB gaps actually being completed.
# 12 — Security, integration, performance and Docker release — Goal

## Outcome
Prove CuaDB is safe and dependable as a local-first read-only explorer with separately gated authenticated SQL/CRUD capabilities. Ensure reproducible builds, meaningful automated tests, bounded query and memory behavior, documentation for real connections and a single production Docker container with one published UI port.

## Definition of done
Acceptance of **12 — Security, integration, performance and Docker release** requires evidence for these results:
1. **Complete a threat-model review and negative security test suite:** Verify the browser cannot cause writes, inject SQL identifiers, render malicious HTML, access unintended local files or reveal secrets through logs and history. Make each security control observable through a repeatable negative test. Verification includes: Document entry points, trust boundaries, sensitive assets and threat assumptions.
2. **Harden and verify the single-container production image:** Ship a minimal runtime image containing only the Rust executable and bundled static assets, with one host-published web port and access to external database servers through Docker networking. Docker must not bundle PostgreSQL or a second web service. Verification includes: Verify reproducible Docker builds with committed Cargo and Bun lockfiles.
3. **Establish disposable database fixtures and end-to-end acceptance tests:** Verify that the actual browser routes, adapter queries and templates work together for PostgreSQL, MySQL, MariaDB and SQLite. Tests must not modify an existing user database and should be explicit about which fixtures are required. Verification includes: Add isolated PostgreSQL/PostGIS, MySQL/MariaDB and SQLite integration fixtures.
4. **Measure large-table navigation, rendering and memory behavior:** Keep the viewer responsive on databases with many schemas, thousands of tables and large relations. Benchmark actual query plans and response sizes before introducing cache or concurrency complexity. Verification includes: Create representative large-table and wide-table fixtures with safe synthetic data.
5. **Publish verified setup, supported engines and release acceptance guide:** Make CuaDB easy to clone, configure, test and operate on Fedora or another supported host. Documentation must distinguish implemented features from planned work and provide concrete examples for existing PostgreSQL-in-Docker, remote databases and SQLite mounts. Verification includes: Update README with verified native and Docker workflows for each supported engine.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
