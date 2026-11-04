---
{id: TSK-103E94FA3053,name: Complete a threat-model review and negative security test suite,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:08.931Z,updated_at: 2026-11-04T07:32:08.931Z,tags: [cuadb,release],depends_on: [TSK-E09AA19D0352,TSK-0DB3969BA9F6,TSK-5689F3668ED7],order: 1024}
---

# Complete a threat-model review and negative security test suite

## Goal
Verify the browser cannot cause writes, inject SQL identifiers, render malicious HTML, access unintended local files or reveal secrets through logs and history. Make each security control observable through a repeatable negative test.

## Context
CuaDB opens databases that may hold sensitive data, even though its goal is only browsing. Threats include user-controlled query parameters, malformed database values, unsafe browser caching, SQLite path traversal and connection URL disclosure. A read-only database role remains the most important final defense.

This task belongs to **12 — Security, integration, performance and Docker release**. The group outcome is: Prove CuaDB is safe and dependable as a local-first read-only database viewer for the supported engines. Ensure reproducible builds, meaningful automated tests, bounded query and memory behavior, documentation for real connections and a single production Docker container with one published UI port.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Document entry points, trust boundaries, sensitive assets and threat assumptions.
- [ ] Test identifier injection, XSS strings, enormous values and invalid cursors.
- [ ] Verify read-only credentials and timeout behavior for supported engines.
- [ ] Inspect browser caching, response headers, logs and clipboard behavior for leaks.
- [ ] Record residual risks and required controls before exposing the port beyond localhost.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
