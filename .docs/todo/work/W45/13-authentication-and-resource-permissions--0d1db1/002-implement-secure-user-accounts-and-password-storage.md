---
{id: TSK-F75882D3ABE8,name: Implement secure user accounts and password storage,type: task,status: planned,priority: medium,created_at: 2026-11-04T16:14:14.682Z,updated_at: 2026-11-04T16:15:02.506Z,tags: [cuadb,auth],depends_on: [TSK-F7474DCC9B66],order: 2048,links: [{type: blocks,task: TSK-D91E3CD3D4C6}]}
---

# Implement secure user accounts and password storage

## Goal
Create the minimal user model and account lifecycle required for multiple people to use CuaDB safely. Store only strong password hashes, normalize unique login identifiers and make account state explicit. Keep account management separate from database credentials and connection profiles.

## Context
Application users are not database users. CuaDB must never reuse a database password as an application password or expose database credentials to a logged-in user. Use a well-maintained password hashing implementation with per-password salts and parameters suitable for interactive logins.

This task belongs to **13 — Authentication and resource permissions**. The group outcome is: Add a secure local administrator bootstrap, user accounts, sessions and deny-by-default permissions before CuaDB exposes protected multi-user or write-capable features. Authorization must be scoped to trusted connection profiles and database resources.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Define user identity, status, role assignment and unique login identifier models.
- [ ] Hash passwords with a modern memory-hard password hashing algorithm.
- [ ] Never store or return plaintext passwords or database credentials.
- [ ] Add safe create, disable and password-change flows for authorized administrators.
- [ ] Test duplicate identifiers, disabled users and password verification failures.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.
