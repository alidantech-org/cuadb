# Supported engines, compatibility and release checklist

## Compatibility matrix (what was actually run)

| Capability | PostgreSQL 18 (+PostGIS 3.6) | MariaDB 11.8.9 | MySQL 8.4.13 | SQLite (bundled via SQLx) |
|---|---|---|---|---|
| Schemas / tables / views | yes | yes | yes | `main` only |
| Materialized views, partitions | yes | partitions | partitions | no (virtual tables listed) |
| Columns, PK, FK, indexes | yes | yes | yes | yes |
| Identity / generated columns | `ALWAYS`/`BY DEFAULT`, `STORED` | via `extra` | via `extra` | `VIRTUAL`/`STORED` generated |
| Row estimates / sizes | `reltuples`, sizes | `information_schema` | `information_schema` | unavailable |
| Paging | keyset (single int key) or offset | offset | offset | offset |
| Sort NULL order | last | last | last | last |
| Rich values | JSON/arrays/enum/bytea/PostGIS GeoJSON (opt-in) | JSON as text, BLOB, BIT | native JSON, BLOB, BIT | dynamic types labelled |
| Server-side query abort | `statement_timeout` | `max_statement_time` | `max_execution_time` | client deadline |
| Live evidence | `tests/*_live_http`, `rich_live_http`, `export_live`, `adapter_contract`, `performance_live` | `mysql_live_http` + shared | `mysql_live_http` + shared | `sqlite_live` + shared |
| Not verified | PostgreSQL without PostGIS, TLS to a remote server, password auth via the test role | TLS, old versions | TLS, 5.7 | WAL with a live writer on a `:ro` mount |

The supported engine scope is PostgreSQL, MySQL, MariaDB and SQLite. MariaDB and MySQL are tested separately; neither result is claimed for the other. Do not add another engine until the current feature roadmap is complete.

## Connecting
- Single server: `CUADB_DATABASE_URL`; several: `CUADB_PROFILES_FILE` (see README, *Connection profiles*). Examples: `postgresql://viewer:***@host:5432/db`, `mysql://...`, `mariadb://...`, `sqlite:////data/app.db`.
- Create a dedicated SELECT-only role. PostgreSQL: `GRANT CONNECT`, `USAGE` on schemas, `SELECT` on tables (+ `USAGE` on sequences/types if you want them shown). MySQL/MariaDB: `GRANT SELECT, SHOW VIEW ON db.* TO ...`. The app also sets read-only sessions, but that is only defense in depth.
- TLS: loopback defaults to no TLS; remote URLs verify certificates by default; use `sslmode=disable` (PostgreSQL) or `ssl-mode` (MySQL) only deliberately. Remote URLs require a password.
- SQLite: mount read-only (`-v dir:/data:ro`), see `docs/sqlite.md`.

## Native and Docker workflows
```bash
bun install --frozen-lockfile && bun run assets:build && cargo run --locked          # native
bun run docker:build && docker run --rm --read-only --cap-drop ALL --security-opt no-new-privileges \
  -p 127.0.0.1:4848:4848 -e CUADB_DATABASE_URL=... cuadb:local                         # container (docs/docker.md)
```

## Release checklist
1. `bun run format:check && bun run check && bun run test` (offline, no databases).
2. With fixtures configured (`docs/testing.md`): `cargo test --locked --no-fail-fast`; confirm no live test was skipped for an engine you claim.
3. `task validate && bun run task:verify`; review open Task criteria — UI items must have user browser sign-off.
4. `bun run build && bun run smoke`.
5. `bun run docker:build`, then the container checks in `docs/docker.md` (non-root, read-only root fs, `:ro` mount, health, assets, shutdown).
6. Re-read `docs/threat-model.md` residual risks for the intended exposure.
7. Compare the UI against `assets/cuadb-ui-demo.html` at 1440×900, 1280×800, 390×844, light and dark (user-run; not automated in this repository).

## How the roadmap was executed
Each Task group was implemented backend-first with evidence recorded as Task comments (tests, live engines, limitations) and only verified acceptance items checked through the Task CLI. Items that need the browser are intentionally left unchecked and marked "FRONTEND ACCEPTANCE PENDING". Details live in the Task records, not here.

## Approved deviations from the demo
None recorded. Demo fixtures (fake connections, rows and exports) must never appear as live data.
