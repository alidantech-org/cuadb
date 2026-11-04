---
{id: TSK-0D56383DBA25,name: "Implement MySQL/MariaDB URL parsing, pools and capabilities",type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:07.499Z,updated_at: 2026-11-04T14:05:35.314Z,tags: [cuadb,mysql],depends_on: [TSK-A3CFF0B5E267,TSK-55D20E7D39E9],order: 1024,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T12:10:59.177Z},{from: in-progress,to: completed,at: 2026-11-04T14:05:35.314Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-0D56383DBA25/1793794259179-start,started_at: 2026-11-04T12:10:59.202Z,end_ref: refs/tracking/TSK-0D56383DBA25/1793801135317-end,ended_at: 2026-11-04T14:05:35.315Z,summary: {files: 48,insertions: 3791,deletions: 113}}]}}}
---

# Implement MySQL/MariaDB URL parsing, pools and capabilities

## Goal
Support mysql:// connections through a bounded, read-only adapter that identifies the server flavor and reports available catalog, data and statistics features. Keep network failure and authentication errors consistent with PostgreSQL behavior.

## Context
MySQL and MariaDB are common application databases, but sharing a driver does not make them equivalent in every metadata capability. A user should supply an operator-controlled URL or a saved trusted profile, without browser-accessible credentials. Testing should run against disposable fixtures rather than a developer’s personal database.

This task belongs to **09 — MySQL and MariaDB compatibility**. The group outcome is: Bring MySQL and MariaDB into CuaDB through a separate adapter while preserving the same read-only navigation and data-grid contract. Reuse common models and browser components, but implement engine-specific discovery, identifier quoting, data decoding and statistics rather than assuming PostgreSQL SQL syntax works everywhere.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Recognize supported MySQL URLs and reject unsupported option or host forms.
- [x] Create bounded connection pools with redacted errors and configurable timeouts.
- [x] Expose engine/flavor capabilities to the common UI contract.
- [x] Enforce least-privilege read-only access as far as the engine supports it.
- [x] Add independent MySQL and MariaDB connection failure integration tests.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T12:20:01.226Z · PETER IRUNGU

Implemented strict redacted mysql:// and mariadb:// parsing, loopback/remote TLS policy, bounded SQLx pool, read-only session mode, cached server flavor probe, common adapter routing, and fixture-gated catalog integration. Evidence: cargo check; cargo test mysql::url::tests; cargo test --test mysql_catalog; CUADB_TEST_MARIADB_URL=... cargo test --test mysql_catalog disposable_mariadb_catalog_and_rows; bun run format:check; task validate. MariaDB 11.8 fixture verified using a restricted reader. MySQL 8 container pull unavailable; invalid-auth live test still needs fixture evidence. Keep corresponding acceptance items unchecked for unsupported proof.

### 2026-11-04T14:04:41.934Z · PETER IRUNGU

Item 5 verified live: tests/mysql_live_http.rs each_engine_reports_authentication_and_network_failures_without_secrets runs for MariaDB 11.8.9 and MySQL 8.4.13 independently: wrong password and unknown user -> AuthenticationFailed, closed port -> Unavailable, no secret in status text.
