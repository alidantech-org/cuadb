---
{id: DOC-373E23773C3D,name: "Quality gates, fixtures and integration verification",type: documentation,tags: [cuadb,architecture]}
---

# Quality gates, fixtures and integration verification

## Always-on developer checks

Run bun run format:check for Rust formatting and Prettier, bun run check for Cargo compile/type checking, strict Clippy and TypeScript validation, bun run test for unit and HTTP integration tests, and bun run build for a release build. Baseline checks must not require a database. Preserve Cargo.lock and bun.lock for reproducible dependency resolution.

## Test isolation

Unit tests cover configuration, identifier resolution, type conversion, pagination planning, cursor validation and redacted error mapping without opening database sockets. Router tests use a fake adapter for HTML status codes and escaped output. Browser interaction tests cover HTMX fragments, loading states, keyboard behavior, page-size changes, sorting and response synchronization. Build tests verify local CSS, JavaScript and HTMX assets without a CDN.

## Core live engine fixtures

Core backend acceptance uses disposable PostgreSQL/PostGIS and SQLite fixtures only. Seed representative tables with composite keys, quoted identifiers, NULL values, JSON, large text, timestamps and foreign keys. Never mutate a developer or production database. Use focused adapter tests for the changed feature; do not build a large cross-engine matrix or repeatedly verify unrelated engines. MySQL/MariaDB tests begin only after the frontend milestone and only for the compatibility work actually being completed.

## Phase-specific verification

Backend phase: verify normal connection, catalog, bounded rows, sorting, metadata, error handling and read-only behavior against PostgreSQL and SQLite. Frontend phase: verify the canonical demo interactions with real backend data and a small set of matched desktop/mobile states. Later engine phase: run focused MySQL/MariaDB fixtures for remaining compatibility gaps. Authentication/SQL phase: test authorization, CSRF, session expiry, parser policy, row limits, rollback and audit events.

## Performance and release checks

Measure response size, rendering time, query time, pool utilization, deep-page behavior and wide-grid usability on synthetic datasets at group boundaries. Verify that initial browsing does not trigger implicit full-table scans or unrestricted SELECTs. Docker acceptance includes one published loopback UI port, non-root runtime, bundled assets and shared-network access to external PostgreSQL. Record exceptions and unsupported engine features explicitly.
