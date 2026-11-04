---
{id: TSK-445BE17209D5,name: Harden and verify the single-container production image,type: task,status: completed,priority: medium,created_at: 2026-11-04T07:32:09.092Z,updated_at: 2026-11-04T16:15:06.481Z,tags: [cuadb,release],depends_on: [TSK-4258A9C13469,TSK-4C7273B9E46A],order: 2048,extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T14:37:44.862Z}]},links: [{type: blocks,task: TSK-0A7991D875A8}]}
---

# Harden and verify the single-container production image

## Goal
Ship a minimal runtime image containing only the Rust executable and bundled static assets, with one host-published web port and access to external database servers through Docker networking. Docker must not bundle PostgreSQL or a second web service.

## Context
The existing Dockerfile already has separate Bun and Rust build stages. Production deployment should lock dependency versions, run as a non-root user, avoid writable app directories and provide reasonable resource limits. The server should bind internally for Docker forwarding while the default host published port remains loopback-only.

This task belongs to **12 — Security, integration, performance and Docker release**. The group outcome is: Prove CuaDB is safe and dependable as a local-first read-only database viewer for the supported engines. Ensure reproducible builds, meaningful automated tests, bounded query and memory behavior, documentation for real connections and a single production Docker container with one published UI port.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Verify reproducible Docker builds with committed Cargo and Bun lockfiles.
- [x] Run image as non-root with minimal writable filesystem and dropped privileges where practical.
- [x] Publish only the web port on 127.0.0.1 by default and document remote access risks.
- [x] Test connection from CuaDB to postgis on a shared user-defined Docker network.
- [x] Verify clean startup, shutdown, health and bundled CSS/HTMX behavior in Docker.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:34:18.677Z · PETER IRUNGU

VERIFIED in a real container (docs/docker.md): locked builds (cargo --locked, bun --frozen-lockfile), uid 10001, only 4848/tcp exposed and published on 127.0.0.1, runs with --read-only --cap-drop ALL --no-new-privileges, SQLite on a :ro bind mount browsed with unchanged SHA-256 and no side files, assets served from the Rust process, docker stop 0.5 s exit 0, PostgreSQL reached over a temporary user-defined network (removed afterwards; existing containers untouched). CAVEATS: the postgis container uses trust auth so password authentication was not exercised, and it needed sslmode=disable; two independent builds were not digest-compared; no MySQL/MariaDB container run yet.
