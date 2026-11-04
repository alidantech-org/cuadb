# CuaDB

**CuaDB** is a fast, local-first, **read-only database explorer** for the browser. The goal is to connect to a database, browse schemas and tables, and inspect paginated records, column types, indexes and storage details—without a SQL runner or a bulky administration UI.

> **Current status: safe PostgreSQL row paging (Group 04).** The Axum/Askama/HTMX server includes bounded row pages, optional catalog-validated column selection, sorting and indexed keyset navigation for eligible integer keys. **Whole-table exports and full canonical grid UI are not enabled yet.** Real row pages require the operator-configured PostgreSQL connection and a SELECT-only role. The catalog and row paths are compiled and verified against disposable PostgreSQL 18.6 and PostGIS fixtures.

## Catalog inspection (Group 03)

With a configured read-only PostgreSQL URL, open the explorer and inspect the
live **Catalog** panel. It lists accessible schemas and relations; selecting a
relation displays structure, constraints, partitions, indexes and storage
details. Direct read-only endpoints also provide an advanced object tree and
bounded overview:

```text
/catalog/schemas
/catalog/relations?schema=public
/catalog/structure?schema=public&relation=example
/catalog/indexes?schema=public&relation=example
/catalog/stats?schema=public&relation=example
/catalog/advanced?schema=public
/catalog/overview
```

No endpoint accepts arbitrary SQL or a connection URL. System schemas are
not navigable, source identifiers are parameter-bound, catalog sizes are
bounded and truncation is visible. Row retrieval is provided separately by Group 04. Full SQL/permissions/fixture details: [PostgreSQL catalog
discovery](docs/postgresql-catalog.md).

## Safe row retrieval (Group 04)

Use `/catalog/data?schema=public&relation=example` to read bounded pages.
Request `size=25|50|100`, `page=N`, `sort=column&direction=asc|desc`,
or repeated `column=name` for column selection. Rows are permission-scoped,
never writable, and value previews are capped. Appropriate integer unique
keys receive opaque next/previous cursor tokens; otherwise bounded offset
pagination is used. See [the row-query contract](docs/postgresql-rows.md)
for caps, limitations, fixture tests and export policy.

## Bounded export (Group 05)

`GET /catalog/export?schema=&relation=&format=csv|json[&scope=page|relation][&size=&page=&sort=&direction=&filter=&column=]`

- Rows come from the same validated, SELECT-scoped adapter paging as the grid; no SQL or connection target is accepted.
- `scope=page` (default) exports the requested page (size ≤ 100). `scope=relation` walks pages until the end or a cap: 10,000 rows and 16 MiB. Hitting a cap, a truncated cell or truncated columns sets `x-cuadb-export-truncated: true` (JSON also has a `truncated` field).
- CSV neutralizes formula-leading text cells (`= + - @`, tab, CR) with a leading `'`. Responses are `attachment`, `no-store`, with an ASCII-safe filename.
- The request timeout cancels a running export. Output is buffered within the byte cap, not streamed.
- `format=zip&scope=related[&depth=0..2]` (default depth 1) returns a stored (uncompressed) ZIP: the relation plus relations it references through foreign keys (outgoing keys only, at most 8 relations), one bounded CSV each, and `manifest.json` (relations, row counts, relationships, truncation). Linked relations the role cannot read are listed as omitted; the root must be readable. The archive obeys a 16 MiB total cap and refuses sort/filter/column/page parameters. It is assembled in memory within that cap, not streamed.
- Limits: incoming (child) relations are not followed; cells over the preview limit export their bounded preview; a SQLite/MySQL walk has not been verified for export (Postgres live only).

## Stale responses, failures and retry (Group 05)

- Each data/structure/indexes panel root carries `hx-sync="this:replace"`, so a newer sort, page, size or filter request aborts the in-flight one; the tree and main body use the same `replace` strategy. Fragments set `hx-history="false"` and `Cache-Control: no-store`.
- Errors (validation, permission, timeout, disconnected) are the same small `role="alert"` fragment (`x-cuadb-error` header) for full-page and HTMX requests; `assets/js/app.js` lets 4xx/5xx fragments swap into their region.
- Retry is manual: re-click the control or reload the relation. Nothing polls; the connection badge is refreshed only when requested. Sorting and filtering are validated server-side per request, so a rejected request never changes server state.
- Not covered by automated tests: actual browser abort behavior (needs Phase B browser validation).

## Rich values, structure and spatial data (Group 08)

- **Cell limits:** the grid shows a bounded preview per cell (truncated cells carry a `truncated` marker); the inspector detail is bounded too, so 20,000-element JSON/array/bytea and 100 KB text values keep the page small. Raw bytes render as `\x…` hex, arrays as PostgreSQL array text, JSON pretty-printed when small.
- **Exactness:** numerics are shown as the database's exact text (no float rounding); NULL is distinct from empty text; `timestamptz` keeps microseconds and is rendered in the connection session's time zone (UTC), not the zone it was typed in.
- **Untrusted text:** values are HTML-escaped by Askama; control characters and bidirectional overrides (U+202A–202E, U+2066–2069, U+200E/F, U+061C) are shown as visible `\u{XXXX}` escapes. Newline and tab are preserved.
- **Structure:** columns (position, type, nullability, key, identity `ALWAYS`/`BY DEFAULT`, generated `STORED`/`VIRTUAL`, defaults, enum/domain info), constraints with foreign-key targets, partitions. Views show no constraints. Relations the role cannot read return an error, never partial metadata.
- **Storage numbers** are engine estimates (`reltuples`, relation sizes), never `COUNT(*)`; partitioned parents report `Unavailable`.
- **PostGIS:** geometry/geography columns show WKT/type; `spatial=geojson` is opt-in and returns bounded GeoJSON per cell (over ~1 KB is refused with a message), labels projected CRS (e.g. `EPSG:3857`), and keeps `POINT EMPTY` empty. No maps and no bulk conversion.
- **Not verified:** a PostgreSQL instance *without* PostGIS (no such fixture available), invalid-geometry values, and deeply nested JSON beyond the preview bound.

## Canonical frontend UI reference

The committed [CuaDB UI demo](assets/cuadb-ui-demo.html) is the **mandatory visual and interaction reference** for implementation, not a loose example. The final Rust/Askama/HTMX frontend must faithfully reproduce the demo's compact sidebar, resizer, Simple/Advanced explorer, Data/Structure/Indexes switcher, dense grid, row inspector, bottom connection/settings controls, themes and export menus. Compare against the standalone HTML at matching desktop/mobile viewports before completing frontend tasks.

The HTML demo uses simulated connection names, data, statistics and client-side exports. The real application must keep the same UI but source values from safe read-only database adapters and implement heavy data operations through bounded or streamed Rust routes. Security and accessibility adaptations must be documented and reviewed, not silently used to redesign the interface. The shared frontend specification and screenshot acceptance matrix live in `.docs/docs/frontend-interactions.md` and are linked from the existing Task records.

## Stack

- **Rust + Axum**: HTTP server (one port), bounded HTTP requests and safe diagnostics.
- **Askama**: compiled, escaped server-rendered HTML templates.
- **HTMX 2**: HTML-fragment updates without React or Next.js.
- **Tailwind CSS 4 + small custom CSS**: compact explorer and read-only data grid.
- **Vanilla JavaScript**: browser-only interactions such as sidebar toggling.
- **Bun**: root task runner and frontend build tooling. Bun is **not** the production server.
- **SQLx PostgreSQL, MySQL and MariaDB**: bounded lazy pools, read-only sessions and protected URL validation (no SQL editor).
- **Docker**: a single multi-stage image and **one** runtime container.

## Prerequisites

For native development, install **Bun 1.x** and **Rust stable** with Cargo, rustfmt and Clippy. The included `rust-toolchain.toml` requests stable Rust with formatting and lint components. Docker is optional for native development, but needed for Docker commands.

On Fedora, install Rust via [rustup](https://rustup.rs/) and Bun via the [official installer](https://bun.com/docs/installation). Then verify:

```bash
bun --version
cargo --version
cargo fmt --version
cargo clippy --version
```

**Database credentials are optional**: without them the shell and tests work in disconnected mode. A configured PostgreSQL, MySQL, MariaDB or SQLite URL is validated at startup; the status indicator probes the database without delaying the health endpoint. SQLite files must be mounted under the configured allowed root.

## Quick start (from the repository root)

```bash
git clone https://github.com/alidantech-org/cuadb.git
cd cuadb

bun run dev
```

Open **http://localhost:4848**. On the first invocation the Bun dispatcher installs missing JS tooling automatically, builds CSS, copies HTMX and the local JS into `public/`, and launches Rust. Subsequent code changes under `src/` or `templates/` restart the Rust process; Tailwind watches styles. Reload the browser after changes. Restart `bun run dev` if you create a *new nested directory*, change environment variables, or add dependencies.

On a fresh clone, the first Rust compilation downloads crates from crates.io. Bun uses its committed lockfile. The committed Cargo.lock includes SQLx. Subsequent commands enforce locked builds. Use `bun run deps:sync` only after intentionally editing Cargo.toml.

## Root commands

| Command | Behavior |
| --- | --- |
| `bun run setup` | Install frontend tooling, generate web assets and fetch locked Rust crates. |
| `bun run deps:sync` | Explicitly resolve new Rust dependencies and refresh Cargo.lock after Cargo.toml changes. |
| `bun run dev` | Build assets and launch Rust + Tailwind watches; restart Rust on source/template changes. |
| `bun run check` | Rust formatting check, type/compile check, strict Clippy, and TypeScript/JavaScript type check. |
| `bun run build` | Build minified assets and a release Rust binary. |
| `bun run start` | Start `target/release/cuadb`; build it first if missing. |
| `bun run format` | Apply rustfmt and Prettier to Rust, scripts and CSS/JS. |
| `bun run format:check` | Verify formatting without writing changes. |
| `bun run test` | Build assets; run locked Rust tests, roadmap tests, and JS/TS checks. |
| `bun run smoke` | Build and launch a release server on a temporary localhost port; test routes, assets and shutdown. |
| `bun run docker:build` | Build the single-image Docker deployment (`cuadb:local`). |
| `bun run docker:run` | Start the Docker container publishing only `127.0.0.1:4848`. |

The task runner gives actionable errors if Rust is missing, and installs missing Bun dependencies on first use. It **does not silently install system-wide compilers**.

### Verify the shell

With `bun run dev` or `bun run start` active in another terminal:

```bash
curl -i http://localhost:4848/health
# HTTP/1.1 200 OK, body: ok

curl -I http://localhost:4848/
# HTTP/1.1 200 OK (HEAD is supported for GET routes by Axum)

curl -i http://localhost:4848/fragments/status
# HTML fragment containing "App shell ready"

bun run check
bun run test
```

The browser home page should show a **CuaDB** sidebar, a **Not connected** label, a disabled table preview, and a **Check status** button. That button exercises an HTMX request and replaces only the status fragment. The **Sidebar** button is handled locally in vanilla JS.

The `/health` endpoint tests the HTTP server, **not a database connection**. `GET /connection/status` renders a separate, redacted HTML connection status and can report **Not configured**, **Connected**, **Unavailable**, **Authentication failed**, or **Insufficient permissions**.

## Validated server configuration

CuaDB validates typed settings before binding its HTTP listener. Errors identify the invalid **variable and accepted bounds**, but never echo a supplied value. Native development defaults to `127.0.0.1:4848`; Docker sets `CUADB_HOST=0.0.0.0` internally and publishes its web port to host loopback.

| Environment variable | Default | Accepted range | Current usage |
| --- | --- | --- | --- |
| `CUADB_HOST` | `127.0.0.1` | IPv4 or IPv6 literal | HTTP listener |
| `CUADB_PORT` | `4848` | 1–65535 | HTTP listener |
| `CUADB_DATABASE_URL` | Unset | PostgreSQL, MySQL, MariaDB, or SQLite URL; remote server URLs require credentials | Validated and used by a bounded read-only adapter |
| `CUADB_SQLITE_ROOT` | `/data` | Absolute filesystem directory | Allowed root for SQLite database files |
| `CUADB_CONNECTION_TIMEOUT_MS` | `5000` | 100–60000 | SQLx pool acquisition/connection deadline |
| `CUADB_QUERY_TIMEOUT_MS` | `15000` | 100–120000 | Database query deadlines and status probe |
| `CUADB_REQUEST_TIMEOUT_MS` | `30000` | 500–180000 | Maximum HTTP request duration |
| `CUADB_DEFAULT_PAGE_SIZE` | `25` | 1–500; cannot exceed maximum | Default data grid page size |
| `CUADB_MAX_PAGE_SIZE` | `100` | 1–500 | Maximum data grid page size |
| `CUADB_MAX_POOL_CONNECTIONS` | `4` | 1–32 | Maximum SQLx pooled connections |

The default page size reduces automatically when only the maximum is lowered; an explicitly configured default above the maximum is rejected. PostgreSQL, MySQL and MariaDB URL parsers reject unsupported driver parameters and require passwords for non-loopback hosts. Loopback development may omit passwords and defaults to TLS disabled. Remote URLs use verified TLS by default. MySQL/MariaDB allow only the explicit `ssl-mode` option. SQLite URLs must use a file under `CUADB_SQLITE_ROOT`, default `/data` in Docker; details and fixture instructions are in [docs/sqlite.md](docs/sqlite.md), [docs/mysql-mariadb.md](docs/mysql-mariadb.md) and the [PostgreSQL connection and security guide](docs/postgresql-connections.md).

To change the local port:

```bash
CUADB_PORT=5050 bun run dev
# Open http://127.0.0.1:5050
```

The Rust server reads **process environment variables**. The example file is documentation, not automatically loaded by the application; export variables or use your local environment loader. Never commit a real credential-bearing `.env`.

## Request safety, shutdown and reproducible checks

The foundation implements a typed browser-safe error contract and a compact
server request log. Every request gets an internal `x-request-id`; logs contain
only the static route pattern, safe error code, method, status and elapsed time,
**never the full URI, query text, connection URL or database row content**.
The error mappings are documented in [developer workflow](docs/development.md).

The HTTP shell enforces a bounded number of concurrent requests (default 64),
a 64 KiB declared request-size/extractor limit, and a handler deadline
(default 30 seconds). These are independent of future SQL deadlines and
per-cell limits. The server handles SIGINT and Docker SIGTERM; shutdown drains
handlers for at most eight seconds.

All Bun installs use `--frozen-lockfile`, and Cargo commands use `--locked`.
Run `bun run smoke` to build and exercise a real release HTTP listener and
all bundled assets without a database. Use `bun run format` before
`bun run check` when modifying Rust or frontend sources.

| Variable | Default | Allowed values |
| --- | --- | --- |
| `CUADB_MAX_IN_FLIGHT_REQUESTS` | `64` | 1–512 |
| `CUADB_MAX_REQUEST_BODY_BYTES` | `65536` | 1024–1048576 |

For more details, see [docs/development.md](docs/development.md).

## Docker: one container and one frontend port

```bash
bun run docker:build
bun run docker:run
```

Open **http://localhost:4848**. The Dockerfile uses Bun only for asset compilation and Rust only for compilation, then copies the executable and assets into a minimal Debian runtime container. Neither Bun, Node.js, Nginx, Redis nor PostgreSQL is required **inside the CuaDB runtime container**.

The browser port is published on the host loopback interface by default. The native server binds to `127.0.0.1:4848` by default, while the Docker image sets `CUADB_HOST=0.0.0.0` inside the container so port forwarding works.

For a PostgreSQL connection, join the CuaDB and existing `postgis` containers to a user-defined Docker network and configure `CUADB_DATABASE_URL` using the hostname `postgis` rather than 127.0.0.1. **Use a dedicated least-privilege reader role, not a privileged database account.** Full steps are in [docs/postgresql-connections.md](docs/postgresql-connections.md).

For an alternate host port, use `docker run --rm -p 127.0.0.1:PORT:4848 cuadb:local`, keeping `CUADB_PORT=4848` inside the container.

## Current repository structure

```text
cuadb/
├── Cargo.toml                 # Rust package and dependencies
├── rust-toolchain.toml        # stable + rustfmt + clippy
├── rustfmt.toml
├── package.json               # Bun scripts and frontend dev dependencies
├── tsconfig.json              # Strict TS + browser JS type checking
├── Dockerfile
├── .env.example               # Host, port and resource defaults
├── src/
│   ├── lib.rs                 # Testable library entry points
│   ├── main.rs                # HTTP listener
│   ├── app.rs                 # Router, tracing and request limits
│   ├── config.rs              # Typed server configuration
│   ├── error.rs               # Browser-safe error contract
│   ├── state.rs               # Shared connection lifecycle
│   ├── db/
│   │   ├── mod.rs             # Engine-neutral database boundary
│   │   ├── types.rs           # Common data/metadata/capability models
│   │   ├── adapter.rs         # Testable cross-engine adapter trait
│   │   └── postgres/
│   │       ├── mod.rs
│   │       ├── url.rs         # Restricted, redacted PostgreSQL URL validation
│   │       └── manager.rs     # SQLx pool, read-only initialization, probes
│   └── web/mod.rs             # Askama pages and connection-status fragments
├── templates/
│   ├── base.html
│   ├── pages/explorer.html
│   └── components/status.html
├── assets/
│   ├── css/app.css            # Tailwind source + specialized CSS
│   └── js/app.js              # Local browser interactions
├── public/                    # Generated, not committed
├── scripts/
│   ├── tasks.ts                # Single typed Bun command dispatcher
│   └── smoke.test.ts           # Live release HTTP smoke test
├── docs/development.md        # Quality gates and security notes
└── tests/                     # Offline Rust tests
```

**Ownership rules:** HTTP and templates must not contain engine-specific SQL. Future `src/db/<engine>/` adapters will implement metadata discovery and paginated data retrieval. Askama templates contain presentation, and `assets/js/` is limited to browser-only actions. Add files when needed instead of creating empty framework layers.

## Product plan (after shell verification)

1. **PostgreSQL MVP:** parse a PostgreSQL URL (default driver), connect read-only, discover schemas/tables/views, browse paginated and sortable rows, show columns, indexes and table sizes.
2. **PostgreSQL value rendering:** JSON/JSONB, arrays, nullable values, and PostGIS geometry inspection.
3. **Database adapters:** MySQL/MariaDB and SQLite with engine-specific metadata/capability handling; evaluate SQL Server later.
4. **Usability:** multiple saved connection profiles (no password storage in browser), polished grids, safe large-cell inspection, and optimized keyset pagination where possible.

CuaDB is intentionally **not** a SQL console, migration system, editor or full database administrator. Every database adapter should use constrained, read-only access and safe SQL identifier handling; metadata not supported by an engine must be indicated as unavailable.

## Security and project conventions

- Keep secrets out of Git and browser storage; `.env.example` is illustrative only.
- Default to local-only Docker port publishing; add authentication/TLS before any external deployment.
- Use strict `cargo clippy -- -D warnings` and TypeScript `strict` validation.
- Run `bun run format`, `bun run check`, and `bun run test` before merging code.
- There is no external CSS/JS CDN dependency: generated CSS and HTMX are served by Rust.
- UI placeholders must never pretend a real database query succeeded.

**Next milestone:** PostgreSQL catalog discovery and safe row pagination. The connection lifecycle is implemented; schema/table browsing is deliberately planned in later Task groups. Follow the managed Task workspace in `.docs/`.

## Task-managed implementation roadmap

The project uses [Task / taskcode](https://github.com/alidantech-org/task) to manage implementation. **The active Task workspace is `.docs/`**, as configured in the root `task.yaml`; the generic Task-managed instructions below may refer to `tasks/`, but `task.yaml` is authoritative. The Task workspace has already been initialized; **do not run `task init` again**.

A one-time, idempotent bootstrap in `scripts/roadmap/` prepares a full dependency-ordered implementation roadmap: **13 groups, 55 detailed tasks, and 5 architecture/design/test documents**. The bootstrap is source input only; after creation, the generated Markdown under `.docs/` is the **single source of truth**. Never change task IDs, status history or status manually, and never maintain a parallel TODO list.

### Preview and create the real Task records

From the CuaDB repository root, with the Task CLI installed:

```bash
task --version
bun run task:preview        # Read-only preview; works even without Task installed
bun run task:seed           # Creates .docs/docs and .docs/todo through Task CLI
bun run task:verify         # Runs task validate on the generated workspace
task group list
task list
task docs list
```

### Repair the initial generated group goals

The first bootstrap contained an uppercase task marker in a shared goal paragraph, which the Task quality checker rejects. The Task CLI already created the group and task records, so **do not delete or reinitialize the workspace**. A targeted repair updates only the affected goal bodies through `taskcode docs open` and preserves IDs, frontmatter, status history and dependency links.

```bash
git pull --ff-only origin main
bun run task:repair
bun run task:verify
```

The repair command checks for the exact original paragraph before editing, replaces it with group-specific acceptance criteria, and validates the entire workspace. It is safe to rerun; goals without the original paragraph are left untouched. If the bootstrap was interrupted before creating every task, run `bun run task:seed` afterward to fill only missing records. Check `git diff -- .docs` before committing the repaired files.

If `task` is not installed, install the `taskcode` CLI using its upstream installation instructions; the bootstrap also recognizes an executable named `taskcode`. **Do not install or run Task as part of the CuaDB runtime container.** The tasks are developer workflow metadata only. The seeder safely skips existing documents, named groups and named tasks on reruns, making it possible to resume after a partial failure. It will not overwrite existing task bodies or status history.

After successful validation, commit the CLI-generated Markdown on the existing `main` branch:

```bash
git add .docs
git commit -m "plan: initialize CuaDB Task roadmap and architecture documents"
git push origin main
```

To work one task at a time:

```bash
task list
task show <TSK-ID>
task start <TSK-ID>
# Implement, format, check and test the task's scoped change.
task checklist <TSK-ID>
task check <TSK-ID> --item 1
task comment <TSK-ID> --text "Implementation and verification evidence"
task update <TSK-ID> --status completed
task validate
```

Group plans describe purpose, order, risks and completion criteria; blocking task links prevent starting dependent work without checking prerequisites. The seeded groups cover Rust foundations, secure database connections, PostgreSQL catalogs and paging, HTTP/HTMX, explorer navigation, data grid, inspector/PostGIS support, MySQL/MariaDB, SQLite, trusted connection profiles, release quality, and later SQL Server compatibility.

The root `bun run test` includes Task roadmap definition tests; `bun run format` and `bun run format:check` include the roadmap scripts. Task-managed Markdown is **not** automatically reformatted because its tool owns the frontmatter and status fields.

<!-- BEGIN TASK MANAGED TASK-TOOL -->
## Task workspace

This project manages its work with Task (`taskcode`; `task` and `tc` are aliases). Task records, documents, decisions and handoffs live in `tasks/` as Markdown files with YAML frontmatter. Read `tasks/README.md` for how the tool works.

- **Write detailed tasks.** Task rejects scaffold-only or thin files: one task file per piece of work, with a Goal, Context and an Acceptance checklist of at least a few `- [ ]` steps ("task create --body-file"). Groups need detailed `goal.md` and `plan.md`. Read `tasks/rules/task-writing.md` before creating work, and update the same file in follow-ups.
- Discuss and organise work on the task itself: `task comment <id> --text "…"` (authors come from `TASK_AUTHOR`, `identity.name` in `task.yaml` or git), `task move <id> --before|--after <id>` to reorder, `task update <id> --parent … --link blocks:TSK-… --add-watcher name --estimate 3 --repeat week`, `task attach <id> <file|url>` (files up to 1 MB, links for anything larger), `task template create|list` and `task create --template <id>`.
- Tick checkboxes with `task check <id> --item N | --section "Heading" | --all` instead of hand-editing.
- Create and change tasks through the CLI (`task create`, `task update <id> --status in-progress`) rather than keeping a separate TODO list. Do not hand-edit `id`, `status_history` or other managed frontmatter.
- Use `--json` for machine-readable output; commands exit non-zero on failure, and outside an interactive terminal they require explicit arguments instead of prompting.
- Moving a task to `in-progress` records the project's file changes automatically (with or without Git); `task changes <id>` shows what the task changed.
- Deleting is recoverable only briefly: `task delete <id> --yes` moves items to the OS temp folder; restore with `task undelete <trash-id>`. Prefer `task archive` for work that should be kept.
- Run `task validate` after editing anything inside `tasks/`. Every Markdown file there needs valid frontmatter.
- Record meaningful decisions in `tasks/decisions/` and agent handoffs in `tasks/handoffs/`.
<!-- END TASK MANAGED TASK-TOOL -->
