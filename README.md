# CuaDB

**CuaDB** is a planned, lightweight, read-only database explorer for the browser. Its purpose is simple: connect to a database, navigate its structure, and view records quickly without the complexity of a full database administration tool.

> **Project status: planning / README only.** This repository does not yet contain an application, a Docker image, or a working release. The setup and file tree below describe the intended implementation, not commands or files that currently work.

## Principles

- **Browsing first.** Navigate connections → schemas/namespaces → tables and views → rows.
- **Minimal interface.** Fast, readable tables and straightforward controls; no dashboard clutter.
- **One container, one web port.** The Rust server serves HTML and static assets directly.
- **No frontend framework.** Server-rendered HTML, HTMX for partial updates, and small CSS.
- **Database-agnostic design.** Database-specific operations stay behind adapters.
- **Read-only by design.** No SQL console, row editing, migrations, or administration features.
- **Local-first.** Intended to be published on localhost, not exposed publicly by default.

## Planned database support

A database connection URL is the primary configuration format. **PostgreSQL is the default and first implementation target.** Support for other engines will be added through adapters; *planned* does not mean already implemented.

| Engine | Planned priority | Connection URL example | Notes |
| --- | --- | --- | --- |
| PostgreSQL | First / MVP | `postgresql://viewer:password@db:5432/app` | Also accept the `postgres://` scheme. |
| MySQL | Next | `mysql://viewer:password@db:3306/app` | Share common driver code with MariaDB when practical. |
| MariaDB | Next | `mysql://viewer:password@db:3306/app` | Handle engine/version differences in metadata. |
| SQLite | Next | `sqlite:///data/app.sqlite` | Mount a database file into the container; prefer read-only access. |
| Microsoft SQL Server | Later, after core engines | To be specified | Requires a separate driver/adapter and metadata implementation. |

Other compatible engines (such as PostgreSQL-wire-compatible servers) may be evaluated individually. Compatibility is not assumed merely because a URL resembles a supported engine.

The application should detect the engine from the URL scheme, with PostgreSQL as the default engine in the connection form. It must **not** assume all databases share PostgreSQL's catalog tables or SQL syntax.

A PostgreSQL URL identifies one database. Schemas and tables are discovered inside that database; switching to another PostgreSQL database requires another connection. MySQL/MariaDB databases, and SQLite namespaces, will be presented through the same navigation UI where meaningful.

## User experience

The intended layout is a collapsible sidebar alongside a wide data grid.

```text
CuaDB                                Connected: local-postgres
┌─────────────────────────┬──────────────────────────────────────────┐
│ Connections             │ public / users                           │
│  └─ local-postgres      │ [Data] [Structure] [Info]                │
│     ├─ public           ├──────────────────────────────────────────┤
│     │  ├─ users         │ id  ↑ │ email         │ created_at       │
│     │  ├─ orders        │ 1      │ a@example.com │ …                │
│     │  └─ products      │ 2      │ b@example.com │ …                │
│     └─ analytics        │ …      │ …             │ …                │
│        └─ events        ├──────────────────────────────────────────┤
│                         │ 50 rows/page       [Previous] [Next]    │
└─────────────────────────┴──────────────────────────────────────────┘
```

### Data tab

- Paginated, **server-side** row retrieval (default 50; selectable 25/50/100).
- Click a column header to sort ascending or descending.
- Preserve sorting and the current table when moving between pages.
- Horizontal scrolling for wide tables, with clear headers and NULL indicators.
- Human-readable dates, booleans, numbers, bytes, and JSON where possible.
- Expand long text, JSON, and binary-value metadata without overwhelming cells.
- Refresh the current result set on demand.
- Show an **estimated** row count when available; do not run expensive exact counts automatically.

Use stable ordering with a unique key where available. Prefer keyset/cursor pagination for very large tables when a suitable index/order exists; use bounded offset pagination as a practical fallback. Explain when pagination cannot guarantee a stable ordering.

### Structure tab

Show each column's name, type, nullable status, default, key/constraint status, and index relationships. Display primary keys, foreign keys, unique constraints, and indexes where supported by the engine.

### Info tab

Show the table or view type, estimated row count, and available size statistics. Examples include table data size, index size, total size, and approximate column widths. **Statistics differ by database engine** and might be estimates or unavailable. Unsupported metrics should be labeled accordingly, not fabricated.

### Navigation

- Connection → schema/namespace → table or view.
- Search/filter table names in the sidebar.
- Distinguish tables from views and other supported object types.
- Keep navigation and paging responsive by returning only the necessary HTML fragment.

## Deliberately out of scope

CuaDB is **not** intended to replace a database administrator's tool:

- No SQL editor or arbitrary query runner.
- No create/insert/update/delete operations on data.
- No schema migrations, triggers/functions editor, or user/role administration.
- No terminal/CLI inside the web UI.
- No mandatory account system or separate settings database.
- No SSR JavaScript framework, Node.js server, or additional web proxy.

Internally generated, parameterized read queries are still required to display data and metadata. “No SQL runner” refers to the user-facing product, not to the implementation.

## Technology and deployment design

| Concern | Planned choice |
| --- | --- |
| Server | Rust + Axum |
| Database connections | SQLx for PostgreSQL, MySQL/MariaDB, SQLite |
| Additional engines | Separate driver behind the same adapter interface |
| HTML templates | Askama |
| Partial page updates | HTMX, bundled locally |
| Styling | Small, accessible custom CSS |
| Application state | In-process state; no CuaDB database required |
| Packaging | Multi-stage Docker build → one runtime container |
| Browser address | `http://localhost:4848` |
| Published ports | One HTTP port: `4848` |

Rust will produce the full pages and the HTML fragments used for table rows, pagination, sidebar expansion, and metadata tabs. CSS and HTMX should be served by the same binary, without a CDN dependency or second frontend process.

**The database itself does not have to run inside the CuaDB container.** It may run on the host, in another container, or on a remote server. Only the CuaDB web port needs to be *published* to the host; the container still needs an outbound network connection to the selected database.

## Planned local Docker usage

> These commands document the **target workflow after implementation**. No `cuadb:local` image exists yet.

Prepare a local environment file that is not committed to Git:

```dotenv
CUADB_DATABASE_URL=postgresql://viewer:YOUR_PASSWORD@host.docker.internal:5432/appdb
CUADB_PORT=4848
```

Then start the eventual Docker image:

```bash
docker run --rm --name cuadb \
  --publish 127.0.0.1:4848:4848 \
  --add-host=host.docker.internal:host-gateway \
  --env-file .env \
  cuadb:local
```

Open **http://localhost:4848**. Inside the container, the server listens on `0.0.0.0:4848` so Docker can forward requests; publishing to host `127.0.0.1` keeps the web interface local.

For PostgreSQL on the **Linux host**, `host.docker.internal` requires the host-gateway mapping shown above. PostgreSQL must accept connections from the Docker network and permit the intended user; a server listening only on host `127.0.0.1` will usually not be reachable over Docker's bridge. For databases in another container, use a shared Docker network and the database container/service hostname instead.

For SQLite, mount the file or directory into the container, ideally read-only. For remote PostgreSQL, use its real host name and appropriate TLS settings.

**Connection configuration:** Initially accept one `CUADB_DATABASE_URL` and infer the database engine from it. A later lightweight connection-picker may support multiple profiles without adding a persistence service. Do not store passwords in browser local storage or commit real URLs with credentials. URL-encode special characters in passwords.

## Security requirements

1. Bind the published Docker port to localhost by default; do not treat CuaDB as an internet-facing service.
2. Prefer a **database user with SELECT and only necessary metadata permissions**. The app should also enforce read-only behavior where the driver/engine allows it.
3. Use parameter binding for values, strict allowlists and correct engine-specific identifier quoting for schema/table/column names. Never concatenate user-provided identifiers unchecked.
4. Set sensible query timeouts, connection-pool limits, row limits, and maximum rendered cell sizes.
5. Escape rendered HTML and prevent database cell contents from becoming executable markup.
6. Avoid logging credentials or complete database URLs; keep secrets out of repository files and browser state.
7. Make remote access an explicit deployment choice, with authentication, TLS, and network protections required before exposing the viewer beyond localhost.

## Proposed repository structure

**This is a plan, not an existing directory tree.** Add files incrementally as their features are implemented.

```text
cuadb/
├── README.md                      # Overview, requirements, roadmap (exists now)
├── Cargo.toml                     # Rust dependencies and build configuration
├── Cargo.lock                     # Locked application dependencies
├── Dockerfile                     # Multi-stage build; one runtime container
├── .dockerignore                  # Keep unnecessary files out of Docker context
├── .gitignore                     # Ignore secrets, build products, local DB files
├── .env.example                   # Safe, credential-free configuration example
├── src/
│   ├── main.rs                    # Startup, config, server binding
│   ├── config.rs                  # Environment settings and connection URL parsing
│   ├── state.rs                   # Shared app state and connection pools
│   ├── http/
│   │   ├── mod.rs
│   │   ├── routes.rs              # Page/HTMX routes
│   │   └── handlers/
│   │       ├── navigation.rs      # Connections, schemas, tables
│   │       ├── data.rs            # Data grid, sorting, pagination
│   │       └── metadata.rs        # Structure and size/statistics views
│   ├── db/
│   │   ├── mod.rs
│   │   ├── adapter.rs             # Shared database capabilities/interface
│   │   ├── models.rs              # Common schema/table/column/row structures
│   │   └── adapters/
│   │       ├── postgres.rs        # First engine implementation
│   │       ├── mysql.rs           # MySQL and MariaDB
│   │       └── sqlite.rs          # SQLite
│   └── web.rs                     # Template rendering/static asset delivery
├── templates/
│   ├── layout.html                # Shared page layout
│   ├── index.html                 # Main explorer page
│   └── partials/
│       ├── sidebar.html           # Navigable object tree
│       ├── table_data.html        # Grid + paging controls
│       ├── table_structure.html   # Column/key/index information
│       └── table_info.html        # Size and statistics
├── assets/
│   ├── css/app.css                # Minimal styles
│   └── js/htmx.min.js             # Locally bundled HTMX
└── tests/
    ├── pagination.rs              # Sorting and pagination behavior
    ├── adapters.rs                # Capability/metadata contracts
    └── security.rs                # Escaping and safe query construction
```

### Responsibility boundaries

- `http/` handles requests and responses, not database-specific SQL.
- `db/adapter.rs` defines a small common contract: list namespaces, list objects, describe an object, fetch a page, and retrieve available statistics.
- `db/adapters/` contains each engine's catalog queries, identifier quoting, pagination strategy, and feature detection.
- `db/models.rs` exposes neutral data types to the UI, including capability/availability flags instead of fake cross-engine uniformity.
- `templates/` and `assets/` contain presentation only; HTMX requests reuse the same server-side templates.
- `tests/` verifies correctness and read-only behavior, especially across differing database engines.

Keep this structure pragmatic: avoid creating abstractions or directories before they are needed.

## Implementation plan

### Phase 1 — PostgreSQL MVP

- [ ] Bootstrap the Rust/Axum project and a single-container, multi-stage Docker build.
- [ ] Connect with a PostgreSQL URL via `CUADB_DATABASE_URL`.
- [ ] Render an HTML/HTMX explorer with schema and table navigation.
- [ ] Implement server-side data pagination, column sorting, and refresh.
- [ ] Add Structure and Info tabs with PostgreSQL metadata/size functions.
- [ ] Bundle local static assets and provide accessible loading/error states.
- [ ] Enforce read-only querying, safe identifier handling, and bounded requests.
- [ ] Add tests and validate Docker-to-host / Docker-network connectivity.

### Phase 2 — Common relational databases

- [ ] Introduce or finalize the shared adapter/capability interface.
- [ ] Add MySQL and MariaDB browsing, sorting, and metadata handling.
- [ ] Add SQLite browsing using mounted database files.
- [ ] Clearly identify unavailable engine-specific statistics.
- [ ] Add engine-specific integration tests and fixtures.

### Phase 3 — Usability and compatibility

- [ ] Add optional local connection profiles and a simple connection picker.
- [ ] Improve large-table pagination, wide-grid rendering, and JSON viewing.
- [ ] Evaluate Microsoft SQL Server and other requested engines individually.
- [ ] Document supported database versions and connection/TLS configurations.
- [ ] Optimize image size and release reproducible Docker builds.

No phase adds a user-facing SQL console or write operations unless the product requirements explicitly change.

## Acceptance criteria for the first usable release

A user should be able to start **one Docker container**, open `http://localhost:4848`, connect to an existing PostgreSQL database via its URL, expand a schema, click a table, and immediately see a page of rows. The user can sort a column, navigate pages, inspect columns and table sizes, and do all of this **without a SQL editor, a second container, or a JavaScript application framework**.

---

CuaDB intentionally optimizes for one job: **open a database and understand its contents quickly.**
