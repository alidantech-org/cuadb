---
{id: TSK-B92248F7876E,name: Support multiple trusted server-side database profiles,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:08.458Z,updated_at: 2026-11-04T07:32:08.458Z,tags: [cuadb,profiles],depends_on: [TSK-E68FB08CB73D,TSK-0D56383DBA25,TSK-F71C30D38C2C],order: 1024}
---

# Support multiple trusted server-side database profiles

## Goal
Let a single CuaDB instance choose among more than one configured connection, including different PostgreSQL databases and supported other engines. Keep profile labels safe and never expose connection URLs or passwords through the HTML interface.

## Context
A PostgreSQL URL selects one database, so browsing other databases on the same server requires a separate connection. Multi-profile configuration should remain optional and backwards-compatible with CUADB_DATABASE_URL. Avoid implementing a browser-submitted arbitrary URL form in this task.

This task belongs to **11 — Connection profiles and everyday usability**. The group outcome is: Allow safe switching among multiple known database connections while keeping credentials out of the browser and eliminating the need to repeatedly reconfigure the viewer. Improve the everyday explorer experience with durable, non-sensitive preferences and sensible keyboard shortcuts.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Define a validated server-side profile configuration format with stable identifiers.
- [x] Maintain compatibility with the single PostgreSQL URL default.
- [x] Bound total active connection pools and close unused resources safely.
- [x] Expose safe profile labels, engine types and connection state to templates.
- [x] Test duplicate names, invalid URLs, profile removal and secret redaction.

## Canonical UI fidelity

The committed `assets/cuadb-ui-demo.html` is the binding reference. Implement the exact applicable control positions, popup states, theme/size options and behavior as required by `.docs/docs/frontend-interactions.md`. Demo profiles and records are illustrative only; real credentials stay server-side and real data is fetched by safe, read-only Rust endpoints. Do not treat visual similarity alone as completion if an existing demo control does not work.

### Reference-specific acceptance
- [ ] Use the demo's bottom-left compact connection/profile button and dropdown rather than creating a new top navbar or full-screen profile picker for normal switching.
- [ ] Show only permitted labels, database/host descriptions and connection state, with the same compact selected checkmark treatment; do not expose passwords or full credential-bearing URLs.
- [ ] Keep prototype connection names and statuses as fake design fixtures and render real configured profiles in the final UI.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T14:16:26.648Z · PETER IRUNGU

BACKEND VERIFIED - FRONTEND ACCEPTANCE PENDING (items 1-5; 6-8 are the picker UI). src/profiles.rs + AppState registry: JSON file CUADB_PROFILES_FILE (<=8 profiles, stable ids, unique labels, unknown fields rejected), single CUADB_DATABASE_URL = profile 'default' (mutually exclusive with the file), redacted Debug and secret-free errors, pool budget divided across profiles with lazy pools and idle close. Evidence: 4 unit tests + tests/profiles_http.rs (7 pass; real SQLite profiles + an unreachable PostgreSQL profile). LIMITATIONS: file read once at startup (no live reload); 'bounded active pools' is a divided budget with lazy/idle pools, not an LRU eviction.
