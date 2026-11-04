---
{id: TSK-BDEAE8F3E238,name: Harden server configuration and startup lifecycle,type: task,status: planned,priority: high,created_at: 2026-11-04T07:32:04.183Z,updated_at: 2026-11-04T07:32:04.183Z,tags: [cuadb,foundation],order: 1024}
---

# Harden server configuration and startup lifecycle

## Goal
Define typed, validated settings for host, port, deadlines, maximum page size and optional connection configuration while preserving the existing zero-credential shell startup. Invalid values should fail fast with actionable, redacted errors instead of panicking or silently selecting dangerous defaults.

## Context
Today config only parses a listening host and port. Database credentials, pool size and statement limits will arrive later, so this task should define a minimal configuration boundary without implementing adapters. Environment settings must be documented and secrets never printed in Debug output. Do not force a database connection just to render the shell.

This task belongs to **01 — Rust foundation and observability**. The group outcome is: Turn the verified application shell into a maintainable Rust service with controlled configuration, predictable errors, observable request behavior and reproducible quality gates. Preserve the current single binary, single Docker runtime and local-only deployment assumptions. This group does not connect to live databases; its deliverables make later adapters safe to introduce without leaking details into HTTP handlers.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Extract typed configuration validation with explicit defaults, bounds and tests for malformed values.
- [ ] Verify startup works with no database URL and rejects invalid host, port and timeout values.
- [ ] Ensure logs and error output cannot print passwords or full credential-bearing URLs.
- [ ] Document environment variables in .env.example and relevant README sections.
- [ ] Run native shell and HTTP health smoke checks after the configuration change.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
