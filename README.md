# CuaDB

**CuaDB** is a fast, local-first, **read-only database explorer** for the browser. The goal is to connect to a database, browse schemas and tables, and inspect paginated records, column types, indexes and storage details—without a SQL runner or a bulky administration UI.

> **Current status: runnable application shell (milestone 0).** The Rust HTTP server, Askama page, Tailwind styling, HTMX fragment and Bun commands are scaffolded. **No database connection, table discovery or row queries are implemented yet.** The UI's preview is intentionally a placeholder.

## Canonical frontend UI reference

The committed [CuaDB UI demo](assets/cuadb-ui-demo.html) is the **mandatory visual and interaction reference** for implementation, not a loose example. The final Rust/Askama/HTMX frontend must faithfully reproduce the demo's compact sidebar, resizer, Simple/Advanced explorer, Data/Structure/Indexes switcher, dense grid, row inspector, bottom connection/settings controls, themes and export menus. Compare against the standalone HTML at matching desktop/mobile viewports before completing frontend tasks.

The HTML demo uses simulated connection names, data, statistics and client-side exports. The real application must keep the same UI but source values from safe read-only database adapters and implement heavy data operations through bounded or streamed Rust routes. Security and accessibility adaptations must be documented and reviewed, not silently used to redesign the interface. The shared frontend specification and screenshot acceptance matrix live in `.docs/docs/frontend-interactions.md` and are linked from the existing Task records.

## Stack

- **Rust + Axum**: HTTP server (one port).
- **Askama**: compiled, escaped server-rendered HTML templates.
- **HTMX 2**: HTML-fragment updates without React or Next.js.
- **Tailwind CSS 4 + small custom CSS**: minimal interface and future data grid.
- **Vanilla JavaScript**: browser-only interactions such as sidebar toggling.
- **Bun**: root task runner and frontend build tooling. Bun is **not** the production server.
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

**No PostgreSQL credentials are needed to start or test the app shell.** A working browser page is the acceptance criterion for this milestone.

## Quick start (from the repository root)

```bash
git clone https://github.com/alidantech-org/cuadb.git
cd cuadb

bun run dev
```

Open **http://localhost:4848**. On the first invocation the Bun dispatcher installs missing JS tooling automatically, builds CSS, copies HTMX and the local JS into `public/`, and launches Rust. Subsequent code changes under `src/` or `templates/` restart the Rust process; Tailwind watches styles. Reload the browser after changes. Restart `bun run dev` if you create a *new nested directory*, change environment variables, or add dependencies.

On a fresh clone, the first Rust compilation downloads crates from crates.io and can take longer than later incremental builds. `bun.lock` and `Cargo.lock` are committed for reproducible dependency resolution.

## Root commands

| Command | Behavior |
| --- | --- |
| `bun run setup` | Install frontend tooling, generate web assets and fetch Rust crates. |
| `bun run dev` | Build assets and launch Rust + Tailwind watches; restart Rust on source/template changes. |
| `bun run check` | Rust formatting check, type/compile check, strict Clippy, and TypeScript/JavaScript type check. |
| `bun run build` | Build minified assets and a release Rust binary. |
| `bun run start` | Start `target/release/cuadb`; build it first if missing. |
| `bun run format` | Apply rustfmt and Prettier to Rust, scripts and CSS/JS. |
| `bun run format:check` | Verify formatting without writing changes. |
| `bun run test` | Run Rust unit/integration tests and frontend script type checks. |
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

The `/health` endpoint tests the HTTP server, **not a database connection**.

## Validated server configuration

CuaDB validates typed settings before binding its HTTP listener. Errors identify the invalid **variable and accepted bounds**, but never echo a supplied value. Native development defaults to `127.0.0.1:4848`; Docker sets `CUADB_HOST=0.0.0.0` internally and publishes its web port to host loopback.

| Environment variable | Default | Accepted range | Current usage |
| --- | --- | --- | --- |
| `CUADB_HOST` | `127.0.0.1` | IPv4 or IPv6 literal | HTTP listener |
| `CUADB_PORT` | `4848` | 1–65535 | HTTP listener |
| `CUADB_DATABASE_URL` | Unset | Optional nonempty string up to 8192 bytes | Safely stored but not yet connected |
| `CUADB_CONNECTION_TIMEOUT_MS` | `5000` | 100–60000 | Future database connection |
| `CUADB_QUERY_TIMEOUT_MS` | `15000` | 100–120000 | Future database queries |
| `CUADB_REQUEST_TIMEOUT_MS` | `30000` | 500–180000 | Future HTTP request bounds |
| `CUADB_DEFAULT_PAGE_SIZE` | `25` | 1–500; cannot exceed maximum | Future grid |
| `CUADB_MAX_PAGE_SIZE` | `100` | 1–500 | Future grid |
| `CUADB_MAX_POOL_CONNECTIONS` | `4` | 1–32 | Future database pool |

The default page size reduces automatically when only the maximum is lowered; an explicitly configured default above the maximum is rejected. PostgreSQL URL schemes, hosts, TLS and authentication will be validated by the dedicated connection task—not this configuration milestone. **Setting a database URL does not yet connect the shell.**

To change the local port:

```bash
CUADB_PORT=5050 bun run dev
# Open http://127.0.0.1:5050
```

The Rust server reads **process environment variables**. The example file is documentation, not automatically loaded by the application; export variables or use your local environment loader. Never commit a real credential-bearing `.env`.

## Docker: one container and one frontend port

```bash
bun run docker:build
bun run docker:run
```

Open **http://localhost:4848**. The Dockerfile uses Bun only for asset compilation and Rust only for compilation, then copies the executable and assets into a minimal Debian runtime container. Neither Bun, Node.js, Nginx, Redis nor PostgreSQL is required **inside the CuaDB runtime container**.

The browser port is published on the host loopback interface by default. The native server binds to `127.0.0.1:4848` by default, while the Docker image sets `CUADB_HOST=0.0.0.0` inside the container so port forwarding works.

For a future PostgreSQL connection, your existing `postgis` container can share a user-defined Docker network with CuaDB and use a URL like `postgresql://viewer:password@postgis:5432/mydb`. **The app does not consume `CUADB_DATABASE_URL` yet**. Do not supply real credentials until the read-only adapter exists.

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
├── .env.example              # Host and port defaults
├── src/
│   ├── lib.rs                 # Testable library entry points
│   ├── main.rs                # HTTP listener
│   ├── app.rs                 # Router and static assets
│   ├── config.rs              # Server configuration + unit tests
│   ├── db/
│   │   ├── mod.rs             # Database integration boundary (empty)
│   │   └── postgres/mod.rs    # PostgreSQL adapter placeholder (empty)
│   └── web/mod.rs             # Askama pages and HTML-fragment handlers
├── templates/
│   ├── base.html
│   ├── pages/explorer.html
│   └── components/status.html
├── assets/
│   ├── css/app.css            # Tailwind source + specialized CSS
│   └── js/app.js              # Local browser interactions
├── public/                    # Generated, not committed
├── scripts/tasks.ts           # Single typed Bun command dispatcher
└── tests/shell.rs             # HTTP smoke tests without database
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

**Next milestone:** Follow the managed Task roadmap, beginning with Rust foundation and secure connection contracts. The shell has been verified on Fedora; actual database browsing is not implemented yet.

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
