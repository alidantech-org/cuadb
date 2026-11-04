---
{id: TSK-9DEA186D9800,name: Parse and validate PostgreSQL connection URLs safely,type: task,status: planned,priority: high,created_at: 2026-11-04T07:32:04.539Z,updated_at: 2026-11-04T07:32:04.539Z,tags: [cuadb,contracts],depends_on: [TSK-A3CFF0B5E267,TSK-BDEAE8F3E238],order: 2048}
---

# Parse and validate PostgreSQL connection URLs safely

## Goal
Accept postgresql:// and postgres:// URLs through server-side configuration and validate malformed input, unsupported schemes, network locations and TLS options. A user should be able to point CuaDB at its existing postgis container without disclosing credentials to the browser.

## Context
The Fedora setup currently publishes postgis on host loopback; Docker-to-Docker access should instead use a shared network and postgis:5432. URL parsing must respect percent-encoding, database names and connection parameters. For the initial milestone, the URL is trusted operator configuration, not a freely editable browser-submitted URL.

This task belongs to **02 — Secure database contracts and connections**. The group outcome is: Provide one small engine-independent interface for database discovery, metadata and row retrieval. Make a PostgreSQL URL the default connection input and define capabilities before expanding to MySQL, SQLite or SQL Server. This group owns lifecycle, timeout and read-only guarantees; it must not leak SQL or vendor-specific catalog assumptions into web templates.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Validate supported schemes, database names, URL encoding and disallowed unsupported options.
- [ ] Load a configured connection without leaking password or user information to HTML/log output.
- [ ] Document host, Docker network, TLS and credentials handling for PostgreSQL 18.
- [ ] Add tests for malformed URLs, redaction and absent optional configuration.
- [ ] Keep any future browser connection form behind explicit permission and security design.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
