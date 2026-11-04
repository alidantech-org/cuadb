---
{id: TSK-4258A9C13469,name: Make all root quality commands reproducible and testable,type: task,status: completed,priority: high,created_at: 2026-11-04T07:32:04.355Z,updated_at: 2026-11-04T09:01:22.361Z,tags: [cuadb,foundation],depends_on: [TSK-08D8BC3178A3],order: 4096,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T09:01:22.004Z},{from: in-progress,to: completed,at: 2026-11-04T09:01:22.361Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-4258A9C13469/1793782882006-start,started_at: 2026-11-04T09:01:22.018Z,end_ref: refs/tracking/TSK-4258A9C13469/1793782882363-end,ended_at: 2026-11-04T09:01:22.361Z,summary: {files: 0,insertions: 0,deletions: 0}}]}}}
---

# Make all root quality commands reproducible and testable

## Goal
Make formatting, checking, unit tests, asset compilation and Docker image verification consistent across Fedora development and fresh clones. Keep Bun as a task runner rather than introducing a second application server.

## Context
The shell has passed local development tests. Cargo.lock and bun.lock are now present in the repository, and the toolchain should honor them. Add focused test helpers and static checks only when they improve confidence. Distinguish tasks requiring a live PostgreSQL fixture from those that can run offline in every developer environment.

This task belongs to **01 — Rust foundation and observability**. The group outcome is: Turn the verified application shell into a maintainable Rust service with controlled configuration, predictable errors, observable request behavior and reproducible quality gates. Preserve the current single binary, single Docker runtime and local-only deployment assumptions. This group does not connect to live databases; its deliverables make later adapters safe to introduce without leaking details into HTTP handlers.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Verify root dev, check, build, start, format and test scripts with documented prerequisites.
- [x] Ensure Rust, Bun and Docker builds prefer committed lockfiles and fail visibly on mismatch.
- [x] Add a dedicated smoke-test path for HTTP routes and generated static assets.
- [x] Document fixture requirements without requiring a database to run ordinary unit tests.
- [x] Capture a clean reproducible command sequence in README and developer documentation.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
