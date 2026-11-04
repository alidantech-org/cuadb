# Docker runtime verification

Verified on 2026-11-04 with `bun run docker:build` (Docker, `Dockerfile`) and the commands below. Builds are reproducible from the committed `Cargo.lock` (`cargo build --locked`) and `bun.lock` (`bun install --frozen-lockfile`); Bun and the Rust toolchain are absent from the runtime image (`debian:bookworm-slim`, ~131 MB).

| Check | Result |
|---|---|
| User | image `USER 10001:10001`; `docker exec ... id` -> `uid=10001 gid=10001` |
| Exposed ports | only `4848/tcp`; published as `127.0.0.1:<port>:4848` |
| Hardening flags | ran with `--read-only --cap-drop ALL --security-opt no-new-privileges --pids-limit 128 -m 256m`: starts, serves, no writes to `/app`, `/data` or `/tmp` ("Read-only file system") |
| `/health` | `200 ok` with security headers |
| Bundled assets | `/`, `/static/app.css`, `/static/app.js`, `/static/htmx.min.js` all `200` from the Rust process (no CDN) |
| SQLite on a `:ro` bind mount | `-v <dir>:/data:ro`, `CUADB_DATABASE_URL=sqlite:////data/fixture.db`: status Connected, tree and rows render; file SHA-256 unchanged; directory still contains only the database (no `-journal`/`-wal`/`-shm`) |
| Shutdown | `docker stop` took 0.5 s, exit code 0, log `server_shutdown phase=drain`, not OOM-killed |
| PostgreSQL over a user-defined network | temporary network `cuadb-verify-net` with the existing `postgis` container attached (detached and removed afterwards): Connected, 120-row table paged. Needs `?sslmode=disable` because that container has no TLS, and a non-empty password because CuaDB refuses passwordless non-loopback URLs (the fixture uses trust auth, so this verified the network path, **not** password authentication) |

```bash
docker run --rm --read-only --cap-drop ALL --security-opt no-new-privileges \
  -v "$PWD/data:/data:ro" -e CUADB_DATABASE_URL=sqlite:////data/app.db \
  -p 127.0.0.1:4848:4848 cuadb:local
```

Not verified: Docker Compose files (none in the repo), image vulnerability scan, multi-arch builds, TLS to a remote database, a WAL database with a live writer on a read-only mount.
