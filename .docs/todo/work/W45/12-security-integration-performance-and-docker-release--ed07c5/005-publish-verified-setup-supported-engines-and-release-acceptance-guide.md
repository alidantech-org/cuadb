---
{id: TSK-0A7991D875A8,name: "Publish verified setup, supported engines and release acceptance guide",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:09.471Z,updated_at: 2026-11-04T07:32:09.471Z,tags: [cuadb,release],depends_on: [TSK-103E94FA3053,TSK-B51DF4691392,TSK-445BE17209D5],order: 5120}
---

# Publish verified setup, supported engines and release acceptance guide

## Goal
Make CuaDB easy to clone, configure, test and operate on Fedora or another supported host. Documentation must distinguish implemented features from planned work and provide concrete examples for existing PostgreSQL-in-Docker, remote databases and SQLite mounts.

## Context
The README began as an app-shell plan and includes commands validated by the user. As adapters become operational, replace planned-only language with verified support claims, tested environment variables and real connection troubleshooting. Users should understand Docker network setup, least-privilege role creation, limitations and expected error states.

This task belongs to **12 — Security, integration, performance and Docker release**. The group outcome is: Prove CuaDB is safe and dependable as a local-first read-only database viewer for the supported engines. Ensure reproducible builds, meaningful automated tests, bounded query and memory behavior, documentation for real connections and a single production Docker container with one published UI port.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Update README with verified native and Docker workflows for each supported engine.
- [x] Document connection URLs, database permissions, TLS and read-only mounts safely.
- [x] Add a compatibility matrix and explicit known limitations for metadata and pagination.
- [x] Include a release checklist that exercises check, test, build and container smoke flows.
- [x] Record how Task groups were completed, validated and handed off without duplicating task records.

## UI-demo backend support

The real frontend must match the canonical committed `assets/cuadb-ui-demo.html`. This backend/release task must expose the safe metadata or data behavior required by the reference rather than forcing the frontend to fake its controls. The complete cross-layer fidelity contract is `.docs/docs/frontend-interactions.md`.

### Demo-related acceptance
- [ ] Document assets/cuadb-ui-demo.html as the mandatory visual/interaction baseline; show how release reviewers perform same-viewport screenshot and functional comparisons.
- [ ] State any approved deviations required by data permissions, limits or accessibility and confirm no fake demo content is presented as live database results.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:34:19.141Z · PETER IRUNGU

Items 2-5 verified as documentation (docs/release.md, docs/threat-model.md, docs/testing.md, docs/docker.md, README links): permissions, TLS, :ro mounts, compatibility matrix with explicit not-verified list, release checklist, execution record. Item 1 left open: native run verified for PostgreSQL/MariaDB/SQLite and Docker for SQLite/PostgreSQL, but no MySQL/MariaDB container run. Items 6-7: UI baseline statement and deviations - nothing approved yet; PENDING Phase B.
