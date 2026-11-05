<h1 align="center">CuaDB</h1>
<p align="center"><b>A local-first database explorer and SQL workspace for PostgreSQL and SQLite.</b><br>
One small Rust binary or one container. A fast browser UI. Your data stays on your machine.</p>

<p align="center">
<a href="https://github.com/alidantech-org/cuadb/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/alidantech-org/cuadb/actions/workflows/ci.yml/badge.svg"></a>
<a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
<a href="https://github.com/alidantech-org/cuadb/pkgs/container/cuadb"><img alt="Container" src="https://img.shields.io/badge/ghcr.io-alidantech--org%2Fcuadb-blue"></a>
</p>

---

## Why CuaDB

- **Instant start.** One command and a browser tab. A built-in demo database is ready on first launch.
- **Daily-driver explorer.** Schemas, tables, columns, keys, indexes, row browsing with sorting, search, filters and exports, with a rich row sheet and foreign-key navigation.
- **A real SQL workspace.** Multi-statement scripts, transactions you can commit or roll back, per-statement result tabs, charts, column stats, history, a query builder, and a schema-aware AI agent.
- **Safe by design.** Trusted local mode on your own computer, or a permissions mode with sign-in, roles, grants, audit log, CSRF protection and encrypted saved credentials for anything you expose.
- **Private.** No telemetry, no CDN, no account. The UI is served from the binary. AI features are optional and use your own key.

## Install and run

### Docker (recommended)

```bash
docker run --rm -p 127.0.0.1:4848:4848 \
  -e CUADB_LOCAL_MODE=true \
  -v cuadb-data:/data \
  ghcr.io/alidantech-org/cuadb:latest
```

Open <http://localhost:4848>. Images are published for `linux/amd64` and `linux/arm64`.

Reaching databases from a container (your computer, your router/LAN, VPNs) is covered in **[docs/docker.md](docs/docker.md)**. The short version:

| You want to connect to | Use |
|---|---|
| A database on your own computer | host `host.docker.internal` (Docker Desktop) or add `--add-host=host.docker.internal:host-gateway` (Linux) |
| A database on your router/LAN or the internet | its normal address, nothing special needed |
| Everything the host can see, with `localhost` meaning your computer | `--network host` (Linux; see docs) |

Or use the ready-made Compose files: [`docker-compose.yml`](docker-compose.yml) and [`docker-compose.host.yml`](docker-compose.host.yml).

### Command line (Rust)

```bash
cargo install cuadb          # needs Rust 1.88+
cuadb --open                 # http://127.0.0.1:4848, opens your browser
```

Prebuilt binaries for Linux, macOS and Windows are attached to every [GitHub release](https://github.com/alidantech-org/cuadb/releases). See **[docs/cli.md](docs/cli.md)**.

### From source

```bash
git clone https://github.com/alidantech-org/cuadb && cd cuadb
bun install && bun run assets:build   # builds the CSS and JavaScript
cargo run --release
```

## Connect a database

Open **Connections** (bottom-left) and choose **New connection**: pick PostgreSQL or SQLite, then paste a URL or fill in the details. The connection is tested before it is saved, passwords are encrypted at rest, and the form tells you exactly what went wrong when it cannot connect.

```text
postgresql://user:password@host:5432/database
sqlite file in the SQLite folder, for example shop.db
```

You can also define connections from the environment (`CUADB_DATABASE_URL`) or a JSON file (`CUADB_PROFILES_FILE`). See [docs/connections.md](docs/connections.md).

## Features

| Area | What you get |
|---|---|
| **Explorer** | Sidebar tree with schemas, tables and views; hover actions; search; counts and sizes; advanced mode for functions, sequences, types and triggers |
| **Tables** | Typed headers and icons, resizable columns, overlay scrollbar, sort by clicking a header, text search, page-level filters, column picker, compact mode, themes for booleans, enums, dates, ids, codes and links |
| **Row sheet** | Row details, foreign-key navigation, add/edit/delete with search-and-select for foreign keys and value suggestions |
| **Schema tools** | Create, rename, comment, empty and drop tables, schemas and databases from one dialog that previews the SQL and asks for confirmation on destructive actions |
| **Export** | CSV and JSON, whole relation or page, related-table ZIP with a manifest |
| **SQL workspace** | Syntax highlighting, autocomplete, snippets, formatter, query builder, examples and smart suggestions, recent and saved queries |
| **Execution** | Reads run directly; changes run in a transaction that stays open until you commit or roll back; several statements per run, one result tab each; your own BEGIN/COMMIT/ROLLBACK honoured |
| **Results** | Explorer-grade tables, sorting and filtering, bar/line/area/pie charts, column statistics, CSV/JSON copy |
| **AI agent** | Chat with schema tools (list, describe, search, relationships, join paths, dry-run validation), analytics dashboards you can run and save, suggested replies, voice input; Gemini, OpenAI, Anthropic or any OpenAI-compatible server including local models |
| **Access control** | Trusted local mode, or administrators, members, per-connection grants, sessions, CSRF protection, rate-limited sign-in, append-only audit log |
| **Appearance** | Light, dark, paper, dim and midnight modes; twenty palettes; accents; fonts; icon styles; surfaces; density; custom selects and calendar |
| **Delivery** | PWA-ready, self-contained binary, multi-arch container, health check, graceful shutdown |

The complete list with details is in **[docs/features.md](docs/features.md)**.

## Two ways to run

| | Trusted local mode | Permissions mode |
|---|---|---|
| For | Your own computer | Anything other people can reach |
| Turned on by | Binding to `127.0.0.1` (the default), or `CUADB_LOCAL_MODE=true` | Binding to another address, or `CUADB_LOCAL_MODE=false` |
| Sign-in | None | Administrator bootstrap, users, roles, grants |
| SQL workspace | On, full access, DDL allowed | Off unless enabled; one parsed statement at a time; per-connection grants |

> **Never publish a local-mode instance on a network.** In a container, publish the port as `127.0.0.1:4848:4848`. To share CuaDB, run it in permissions mode behind HTTPS. See [docs/auth.md](docs/auth.md) and [SECURITY.md](SECURITY.md).

## Configuration

Settings come from flags and environment variables. The most useful:

| Variable | Default | Meaning |
|---|---|---|
| `CUADB_HOST` / `CUADB_PORT` | `127.0.0.1` / `4848` | Address to listen on |
| `CUADB_LOCAL_MODE` | `auto` | `true`, `false`, or `auto` (on when bound to loopback) |
| `CUADB_DATA_DIR` | OS data folder (`./data` in development) | Users, sessions, saved connections, audit log |
| `CUADB_SQLITE_ROOT` | `<data>/sqlite` | Folder SQLite files are opened from |
| `CUADB_SECRET_KEY` | generated | 64 hex characters that encrypt saved passwords |
| `CUADB_SQL_WORKSPACE` | follows local mode | `enabled` / `disabled` |
| `CUADB_DATABASE_URL`, `CUADB_PROFILES_FILE` | none | Connections defined by the server |

Every variable, limit and default is in **[docs/configuration.md](docs/configuration.md)**.

## Documentation

[Features](docs/features.md) · [CLI](docs/cli.md) · [Docker and networking](docs/docker.md) · [Configuration](docs/configuration.md) · [Connections](docs/connections.md) · [Authentication and permissions](docs/auth.md) · [SQL workspace and AI agent](docs/sql-workspace.md) · [SQLite](docs/sqlite.md) · [Threat model](docs/threat-model.md) · [Performance](docs/performance.md) · [Testing](docs/testing.md) · [Development](docs/development.md) · [Releasing](docs/release.md) · [Changelog](CHANGELOG.md)

## Development

```bash
bun install
bun run dev        # rebuilds assets and runs the server
bun run check      # format, lint, type-check
bun run test       # unit and integration tests
bun run smoke      # end-to-end smoke tests
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/development.md](docs/development.md).

## License

[MIT](LICENSE) © 2026 Alidantech.
