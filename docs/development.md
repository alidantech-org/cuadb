# CuaDB developer workflow

CuaDB is a **single-binary, read-only database explorer**. Bun builds local
Tailwind/HTMX assets and drives developer commands; Axum serves the HTML and
assets. No PostgreSQL, Node server, separate frontend process, Docker Compose,
or SQL role is needed to run the **current foundation shell**.

## Stable commands

Run from the repository root:

```bash
bun run setup
bun run format
bun run format:check
bun run check
bun run test
bun run build
bun run smoke
bun run dev
# In a different terminal: curl -i http://127.0.0.1:4848/health
```

`setup` installs Bun dependencies with `bun install --frozen-lockfile`,
builds CSS and vendors HTMX, then fetches Rust dependencies using
`cargo fetch --locked`. `build` creates minified CSS and a locked Rust
release binary. `start` rebuilds the release binary to prevent stale-code
execution, then runs it. `dev` runs the Rust server and watches templates,
Rust sources and CSS. `format` applies rustfmt and Prettier; `check` is
read-only and checks both formatters, Cargo compilation, strict Clippy and
TypeScript/checked JavaScript. `test` generates required static assets, runs
all Rust tests and the Bun roadmap tests, then checks JS/TS types.

`smoke` builds a release executable and launches it with an ephemeral
**127.0.0.1** port, disconnected from any database. It tests `/health`, the
home page, the status fragment and three local assets. On Unix it also starts
an independent process and verifies that SIGTERM exits cleanly. Tests never
mutate a database and never use credentials from developer environment
variables.

Production-style Docker:

```bash
bun run docker:build
bun run docker:run
# Separate terminal:
curl -i http://127.0.0.1:4848/health
```

Docker's multi-stage build uses `bun install --frozen-lockfile` and
`cargo build --locked --release`. The runtime image contains only the
compiled Rust server and generated assets. Only the UI port is published
on host loopback by default. To stop the container use Ctrl+C or
`docker stop cuadb` from another terminal. The server handles SIGINT and
SIGTERM, closes its listener and drains active HTTP handlers for up to
**8 seconds**, then exits even if a handler never finishes. This is shorter
than Docker's typical 10-second stop grace period.

## Error responses and request tracing

Each application error maps to one stable HTML message and HTTP status:

| Category | HTTP status |
| --- | --- |
| Invalid input | 400 |
| Permission denied | 403 |
| Database unavailable / server overloaded | 503 |
| Request timeout | 504 |
| Payload too large | 413 |
| Template error / internal error | 500 |

A completed request returns an internally generated `x-request-id` header.
The server writes a compact structured **stderr** record with generated ID,
method, **matched static route pattern**, status, safe error code and elapsed
milliseconds. Do **not** log full URLs, query parameters, headers, SQL,
database rows, full connection URLs or credentials. Use the safe error
classification to aid operations without leaking internal causes to users.

A template render failure returns an `AppError::template()` response, not
the underlying Askama error. Future database errors should likewise be
mapped to the closest safe error category at the adapter boundary. Direct
visitor-facing messages must only use the static templates/constants. The
HTTP `/health` endpoint means **server liveness**; database reachability
will be exposed separately when adapters exist.

## HTTP resource and shutdown policy

Validated settings in `.env.example` include:

- `CUADB_REQUEST_TIMEOUT_MS` (default **30000**, range 500–180000).
- `CUADB_MAX_IN_FLIGHT_REQUESTS` (default **64**, range 1–512).
- `CUADB_MAX_REQUEST_BODY_BYTES` (default **65536**, range 1024–1048576).
- Future connection/query deadlines and database-pool limits.

Requests exceeding a declared Content-Length limit receive HTTP 413. Full
request-body extractor limits are also applied through Axum. Overload returns
HTTP 503 **without allocating an unbounded queue**. A handler that exceeds
its deadline receives HTTP 504; dropping its future cancels cooperative
in-flight work. **A detached background task and response-body streaming
need their own cancellation, timeout and size bounds** when implemented.
PostgreSQL connection/query deadlines are now active for pool acquisition,
read-only connection initialization and connection probes. Future row streams
must enforce their own bounded SQL execution and response-size limits. Ordinary GET-only routes don't consume request bodies,
so the extractor limit primarily protects future routes.

Shutdown is signaled by Ctrl+C (SIGINT) or SIGTERM on Unix, including
Docker stops. Axum stops accepting new connections and drains active requests
with an 8-second maximum wait. Database pool draining and other resources
will be introduced in the relevant adapter lifecycle tasks.

## Tests and review discipline

`bun run test` covers config parsing, error mapping, request IDs,
body-size/rejection policy, overload behavior, cancellation of a pending
handler, shell routes and Task roadmap validity. `bun run smoke` checks a
real HTTP listener and bundled assets. These tests do not depend on your
personal PostgreSQL/PostGIS container.

PostgreSQL connection integration tests are **opt-in and disposable**, and
must use an isolated database fixture plus a dedicated least-privilege SELECT
role. See [PostgreSQL connections](postgresql-connections.md) for fixture
variables, TLS options, grants and Docker networking. Never run fixtures against an existing user or production database.
Do not claim completion before running `bun run format:check`,
`bun run check`, `bun run test`, `bun run smoke` and
`task validate` on the relevant developer platform.

Managed task IDs, statuses, checklist ticks, and history are owned by the
Task CLI in `.docs/`. Use `task start`, `task check`, `task comment`,
`task update`, and `task validate` rather than hand-editing Task metadata.

## PostgreSQL connection foundation

The default shell needs no credentials and never waits for the database to
answer `/health`. When `CUADB_DATABASE_URL` is present, CuaDB validates a
restricted PostgreSQL URL at startup, constructs a bounded SQLx pool with no
minimum connections, and begins a single cached read-only probe. A failed
connection changes only the status fragment; it does not bring down the
HTTP server. Credentials and database hostnames are never included in
browser fragments or diagnostic logs.

If `Cargo.toml` changes, run `bun run deps:sync` intentionally **once** to
refresh the dependency lockfile, inspect `Cargo.lock`, commit it, and then
use locked commands as normal. Never hide lockfile drift by silently updating
it during production builds. The existing `bun run smoke` test intentionally
starts a disconnected server to remain fast and deterministic.

Full role setup, PostgreSQL URL/TLS requirements, opt-in fixture tests and
PostGIS container networking are documented in
[PostgreSQL connections](postgresql-connections.md).

## PostgreSQL catalog inspection

Group 03 adds only read-only PostgreSQL metadata endpoints; it does not
implement row scanning, table export, query editing, or a second web process.
The explorer's catalog panel loads accessible schemas via HTMX and shows
relation structure, indexes and exact storage bytes with approximate row
estimates. Advanced objects (routine/sequence/type/trigger metadata) and a
bounded overview are also exposed; data grids remain a later milestone.

See [PostgreSQL catalog discovery](postgresql-catalog.md) for the API surface,
privilege policies, visibility limits, and explicit **disposable PostgreSQL 18
and PostGIS** fixtures. Do not run those fixture scripts against any existing
database. Ordinary tests remain DB-independent; live catalog tests are opt-in
and never execute writes.

## Safe read-only row paging (Group 04)

The `/catalog/data` fragment displays a bounded server-side page, never a
whole table. PostgreSQL identifies a usable unique key when possible,
validates all requested columns and sorts through its catalog, quotes names
correctly and binds limit, offset and cursor values. Cursor tokens are
short-lived and scoped to the selected relation. Large and unknown values
are shown as explicit bounded previews. Read-only SELECT is enforced by
the existing pool and viewer-role recommendations.

See [PostgreSQL rows](postgresql-rows.md) for supported parameters,
deterministic-order limitations, export policy and disposable fixture tests.
The final demo-matching grid, inspector and whole-table exports remain
separate milestones. Do not mark the Group 04 Task checklists complete until
local compilation and opt-in fixtures pass.
