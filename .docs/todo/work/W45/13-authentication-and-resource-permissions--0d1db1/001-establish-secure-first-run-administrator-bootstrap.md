---
{id: TSK-F7474DCC9B66,name: Establish secure first-run administrator bootstrap,type: task,status: planned,priority: medium,created_at: 2026-11-04T16:14:14.512Z,updated_at: 2026-11-04T16:15:02.323Z,tags: [cuadb,auth],depends_on: [TSK-4C7273B9E46A,TSK-4258A9C13469],order: 1024,links: [{type: blocks,task: TSK-F75882D3ABE8}]}
---

# Establish secure first-run administrator bootstrap

## Goal
Provide an explicit and safe first-run process for creating the initial CuaDB administrator. Do not ship default credentials, expose an unauthenticated registration endpoint or store bootstrap secrets in the browser. The operator must be able to complete setup locally and then disable the bootstrap path.

## Context
CuaDB is currently local-first and does not yet have application users. The first administrator is the root of trust for later profile access and SQL permissions. Bootstrap should use a one-time operator-supplied secret or an interactive local setup command, expire after successful use and fail closed if setup state is ambiguous.

This task belongs to **13 — Authentication and resource permissions**. The group outcome is: Add a secure local administrator bootstrap, user accounts, sessions and deny-by-default permissions before CuaDB exposes protected multi-user or write-capable features. Authorization must be scoped to trusted connection profiles and database resources.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Implement a one-time administrator bootstrap with no built-in username/password pair.
- [ ] Keep bootstrap secrets out of logs, URLs, HTML, browser storage and error messages.
- [ ] Disable or expire bootstrap after the first administrator is created.
- [ ] Restrict initial setup to the local operator and document recovery behavior.
- [ ] Test repeated setup, invalid secrets and interrupted bootstrap safely.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.
