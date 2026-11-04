---
{id: TSK-08D8BC3178A3,name: Implement graceful shutdown and bounded request behavior,type: task,status: completed,priority: high,created_at: 2026-11-04T07:32:04.296Z,updated_at: 2026-11-04T09:01:21.841Z,tags: [cuadb,foundation],depends_on: [TSK-23F3D2D0DDE1],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T09:01:21.512Z},{from: in-progress,to: completed,at: 2026-11-04T09:01:21.841Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-08D8BC3178A3/1793782881514-start,started_at: 2026-11-04T09:01:21.523Z,end_ref: refs/tracking/TSK-08D8BC3178A3/1793782881843-end,ended_at: 2026-11-04T09:01:21.841Z,summary: {files: 0,insertions: 0,deletions: 0}}]}}}
---

# Implement graceful shutdown and bounded request behavior

## Goal
Ensure CuaDB exits cleanly, closes outstanding work and does not hang indefinitely during HTTP or database operations. Keep the service resource footprint predictable for laptop use and single-container deployment.

## Context
A local-only tool can still encounter stalled network connections, abandoned browser requests and repeated reloads. Rust cancellation, timeout defaults and an orderly shutdown path should be designed before adding real SQL operations. The health endpoint must remain a shallow server signal and should not issue expensive queries or expose secrets.

This task belongs to **01 — Rust foundation and observability**. The group outcome is: Turn the verified application shell into a maintainable Rust service with controlled configuration, predictable errors, observable request behavior and reproducible quality gates. Preserve the current single binary, single Docker runtime and local-only deployment assumptions. This group does not connect to live databases; its deliverables make later adapters safe to introduce without leaking details into HTTP handlers.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Handle process termination signals and let the Axum server shut down gracefully.
- [x] Define request concurrency, body-size and timeout policies with realistic defaults.
- [x] Add cancellation behavior tests that do not require a database or network dependency.
- [x] Confirm /health remains fast and reports server liveness rather than query success.
- [x] Document shutdown expectations for both bun run dev and Docker.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
