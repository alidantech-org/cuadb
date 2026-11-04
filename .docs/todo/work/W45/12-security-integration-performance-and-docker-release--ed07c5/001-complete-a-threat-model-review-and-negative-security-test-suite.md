---
{id: TSK-103E94FA3053,name: Complete a threat-model review and negative security test suite,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:08.931Z,updated_at: 2026-11-04T14:37:44.709Z,tags: [cuadb,release],depends_on: [TSK-E09AA19D0352,TSK-0DB3969BA9F6,TSK-5689F3668ED7],order: 1024,extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T14:37:44.709Z}]}}
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
- [x] Document entry points, trust boundaries, sensitive assets and threat assumptions.
- [x] Test identifier injection, XSS strings, enormous values and invalid cursors.
- [x] Verify read-only credentials and timeout behavior for supported engines.
- [x] Inspect browser caching, response headers, logs and clipboard behavior for leaks.
- [x] Record residual risks and required controls before exposing the port beyond localhost.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:34:18.498Z · PETER IRUNGU

VERIFIED (docs/threat-model.md; tests/security_negative.rs on SQLite, PostgreSQL 18, MariaDB 11.8, MySQL 8.4; tests/performance_live.rs). 12 hostile payload classes x 13 routes: never executed, never reflected raw, data intact afterwards; 30 KB values, 5000 column params, 20-digit pages, malformed/forged/oversized cursors rejected in <10 s total with <4 KB bodies; no-store/nosniff/DENY/CSP on every response, no server banner, no connection details or host paths in bodies, non-GET -> 405. Read-only roles: manual proof that cuadb_fixture_reader / cuadb_reader get permission-denied for INSERT/UPDATE/DELETE/CREATE/DROP on PostgreSQL, MariaDB and MySQL (rows unchanged); SQLite read_only+query_only tested. Timeouts: 100 ms query timeout returns 504 in ~105 ms on all four engines and the pool recovers. Defect fixed here: MySQL/MariaDB only had a client-side timeout; now max_execution_time/max_statement_time and error codes 3024/1969 -> 504. LIMITATIONS: logs were inspected by reading emitted lines (route/status/id only), not by an automated log-capture test; clipboard behavior is browser-side (Phase B).

### 2026-11-04T14:36:21.724Z · PETER IRUNGU

Additional defect found by the timeout test and fixed: MySQL/MariaDB error classification compared numeric error codes (1142, 1969, ...) against the SQLSTATE string, so permission-denied and timeout errors were all reported as 'database unavailable' (503). classify() now reads the driver's numeric code (sqlx MySqlDatabaseError::number) with SQLSTATE fallbacks; timeouts are a stable 504 on both engines (4/4 repeated runs).
