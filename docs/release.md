# Releasing

## One-time setup
1. Repository secrets: `CARGO_REGISTRY_TOKEN` (crates.io API token with publish scope). Without it the crates.io step is skipped.
2. After the first image is pushed, open the package at `github.com/orgs/alidantech-org/packages` → *Package settings* → set visibility to **Public** so anyone can `docker pull` without logging in.
3. Check that the crate name `cuadb` is free on crates.io (`cargo search cuadb`), otherwise rename the package in `Cargo.toml`.

## Cutting a release
1. Move the *Unreleased* notes in `CHANGELOG.md` under `## [X.Y.Z] - YYYY-MM-DD`.
2. Set `version = "X.Y.Z"` in `Cargo.toml`, run `cargo check` so `Cargo.lock` updates, commit.
3. Tag and push: `git tag vX.Y.Z && git push origin main vX.Y.Z`.
4. The **Release** workflow then: verifies the tag matches `Cargo.toml`, runs the tests, builds `linux/amd64` and `linux/arm64` images and pushes `ghcr.io/alidantech-org/cuadb:{X.Y.Z,X.Y,X,latest}` with provenance and SBOM, builds binaries for Linux, macOS and Windows, creates the GitHub release with the CHANGELOG section and the archives, and publishes the crate to crates.io.
5. Verify: `docker run --rm ghcr.io/alidantech-org/cuadb:X.Y.Z --version`, `cargo install cuadb --version X.Y.Z`.

Pre-release tags such as `v0.2.0-rc.1` publish images and a GitHub pre-release but never move `latest` and skip crates.io.

Every push and pull request runs the **CI** workflow (format, clippy, tests, asset build, smoke test, container build).

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
