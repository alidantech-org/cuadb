---
{id: TSK-23F3D2D0DDE1,name: Introduce typed application errors and structured request tracing,type: task,status: completed,priority: high,created_at: 2026-11-04T07:32:04.240Z,updated_at: 2026-11-04T09:00:55.994Z,tags: [cuadb,foundation],depends_on: [TSK-BDEAE8F3E238],order: 2048,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T08:59:42.533Z},{from: in-progress,to: completed,at: 2026-11-04T09:00:55.994Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-23F3D2D0DDE1/1793782782535-start,started_at: 2026-11-04T08:59:42.545Z,end_ref: refs/tracking/TSK-23F3D2D0DDE1/1793782855996-end,ended_at: 2026-11-04T09:00:55.994Z,summary: {files: 0,insertions: 0,deletions: 0}}]}}}
---

# Introduce typed application errors and structured request tracing

## Goal
Create a consistent error boundary for database, validation, rendering and transient infrastructure failures. Operators should diagnose problems from safe logs while browsers receive stable status codes and understandable HTML responses without stack traces.

## Context
The current web handler prints template errors and returns a bare 500 response. Later HTTP routes will have user-supplied identifiers, database timeouts and catalog permission errors. Centralizing error conversion early prevents each handler from inventing incompatible behavior. Tracing must avoid query parameters containing secrets, full connection URLs and cell content.

This task belongs to **01 — Rust foundation and observability**. The group outcome is: Turn the verified application shell into a maintainable Rust service with controlled configuration, predictable errors, observable request behavior and reproducible quality gates. Preserve the current single binary, single Docker runtime and local-only deployment assumptions. This group does not connect to live databases; its deliverables make later adapters safe to introduce without leaking details into HTTP handlers.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Define a focused application error type and HTTP status mapping with no sensitive debug text.
- [x] Integrate structured request IDs and minimal redacted tracing for route-level diagnostics.
- [x] Add tests for bad request, unavailable database, timeout and template failure mappings.
- [x] Keep health and initial page routes working with existing success behavior.
- [x] Document when to log an internal cause and when to expose a user-safe message.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T09:00:55.853Z · PETER IRUNGU

Verified typed HTTP errors, redacted request tracing, error mapping tests, shell compatibility and developer documentation. Bun checks, tests, build and smoke tests passed.
