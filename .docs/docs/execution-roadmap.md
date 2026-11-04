---
{id: DOC-9B73548B4ECD,name: CuaDB execution stages and completion policy,type: documentation,tags: [cuadb,architecture]}
---

---
{id: DOC-9B73548B4ECD,name: CuaDB execution stages and completion policy,type: documentation,tags: [cuadb,architecture]}
---

# CuaDB execution stages and completion policy

## Source of truth
The Task workspace at .docs/ is authoritative for group progress, individual task status, checklist ticks, comments and dependency relationships. This document provides strategic sequencing only; it is not a second TODO list. All task and group records should be created or updated through the Task CLI and validated with task validate. Group goal.md and plan.md files describe their outcomes and risks. The roadmap bootstrap script is used once to seed the initial backlog, after which work must update the generated Task records, not the bootstrap definitions.

## Working stages
Start with Rust foundation and stable database contracts. Next make PostgreSQL catalog inspection, bounded row retrieval and strict read-only behavior operational. Build the server routes and browser explorer in parallel only after contracts are stable, then complete paginated grids, metadata panels and PostGIS-aware value details. This first milestone is the usable PostgreSQL CuaDB release and should be tested against a disposable fixture as well as the operator’s existing postgis container using read-only credentials.

## Broader compatibility
After PostgreSQL and the browser experience are reliable, build MySQL/MariaDB and SQLite adapters against the same capability contract. Then introduce multiple trusted server-side connection profiles and complete security, performance, Docker and documentation acceptance. Keep the engine scope limited to PostgreSQL, MySQL, MariaDB and SQLite until the current product features are complete. Avoid claiming feature parity for statistics, namespaces and special data types across engines.

## Execution discipline
For each work session, inspect task list and the exact task, check its dependencies, move it to in-progress, implement only its scope, and run relevant verification. Record findings and decisions in the Task workspace, tick checklist items through task check, and move the task to completed only when its acceptance criteria and checks pass. Keep one small piece of work active at a time and prefer a complete vertical slice to broad untested scaffolding.