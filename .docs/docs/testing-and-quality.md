---
{id: DOC-373E23773C3D,name: "Quality gates, fixtures and integration verification",type: documentation,tags: [cuadb,architecture]}
---

---
{id: DOC-373E23773C3D,name: "Quality gates, fixtures and integration verification",type: documentation,tags: [cuadb,architecture]}
---

# Quality gates, fixtures and integration verification

## Always-on developer checks
Run bun run format:check for Rust formatting and Prettier, bun run check for Cargo compile/type checking, strict Clippy and TypeScript validation, bun run test for unit and HTTP integration tests, and bun run build for a release build. These baseline checks must not require a database. Preserve Cargo.lock and bun.lock for reproducible dependency resolution. A format command may edit files, while check/test must not hide errors or silently rewrite source.

## Test isolation
Unit tests should cover config parsing, identifier resolution, type conversion, pagination planning, cursor validation and redacted error mapping without opening sockets or databases. Router tests should use a fake adapter to exercise HTML status codes and escaped output. Browser interaction tests should cover HTMX fragments, loading states, keyboard behavior, page-size changes, sorting and response synchronization. Build and static asset tests verify that Rust serves local CSS, JS and HTMX without a CDN.

## Live engine fixtures
Live PostgreSQL, PostGIS, MySQL, MariaDB and SQLite tests are opt-in and must use disposable fixtures. Seed tables with composite keys, unusual quoted identifiers, NULL values, JSON, large text, timestamps and related relations. Use both a setup role and a restricted viewer role; never mutate a developer’s existing user database. Confirm that catalog visibility, read-only permissions, query timeouts and expected metadata are correct. Do not add another engine to this matrix until the current product features are complete.

## Performance and release checks
Measure server response size, rendering time, database query time, pool utilization, deep-page behavior and wide-grid usability on synthetic datasets. Verify that no implicit COUNT(*) full scan or unrestricted SELECT occurs during initial table browsing. Docker acceptance includes one published loopback UI port, non-root runtime, bundled assets, startup/shutdown behavior and a shared-network connection to an external PostgreSQL container. Record exceptions and unsupported engine features explicitly rather than weakening tests.