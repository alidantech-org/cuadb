---
{id: TSK-E09AA19D0352,name: "Harden browser responses, headers and fragment cache behavior",type: task,status: in-progress,priority: medium,created_at: 2026-11-04T07:32:05.812Z,updated_at: 2026-11-04T11:29:07.459Z,tags: [cuadb,http],depends_on: [TSK-FFD6A5B94574],order: 3072,extra: {status_history: [{from: planned,to: in-progress,at: 2026-11-04T11:29:07.459Z}],tracking: {segments: [{base_ref: refs/tracking/TSK-E09AA19D0352/1793791747476-start,started_at: 2026-11-04T11:29:07.497Z}]}}}
---

# Harden browser responses, headers and fragment cache behavior

## Goal
Protect local database content from accidental exposure through HTTP caches, browser history snapshots and cross-origin interactions. Keep web access on localhost by default and document the boundary for any later remote deployment.

## Context
CuaDB is a browser-based interface to sensitive database values. Even read-only responses may contain personal or proprietary information. HTMX must not persist data snapshots unexpectedly, while CSS and JS should be bundled locally. Security headers should permit intended HTMX behavior without opening permissive script or embedding policies.

This task belongs to **05 — HTTP read endpoints and HTML fragments**. The group outcome is: Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Add no-store cache behavior to all authenticated or database-bearing HTML responses.
- [x] Define CSP, clickjacking and MIME-sniffing protections compatible with bundled assets.
- [x] Disable HTMX history caching of sensitive database fragments and test navigation behavior.
- [x] Verify that unsafe origins or methods cannot trigger unauthorized future state changes.
- [x] Add response security regression tests and document local-only deployment assumptions.

## Canonical UI fidelity

**Mandatory reference:** `assets/cuadb-ui-demo.html`, with the complete interaction and visual contract in `.docs/docs/frontend-interactions.md`. Reproduce the exact applicable layout and behaviors rather than designing a new interface. The HTML file uses fake records; the actual implementation must render verified database-backed content with safe Rust endpoints and HTMX swaps. This requirement is part of task completion, not a future polish option.

### Reference-specific acceptance
- [ ] Keep database-bearing HTML no-store and disable sensitive HTMX history snapshots without breaking the demo's interaction pattern, visual states, popovers or row inspector.
- [ ] Allow local persistence only for non-sensitive demo settings and per-table sort/hidden-column names; never save cell values, raw SQL, connection credentials or URLs.
- [x] Serve all CSS, HTMX and browser JavaScript from the Rust app with content security headers that do not break supported popup, copy, search or preference behavior.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T11:29:33.423Z · PETER IRUNGU

Verified global no-store for HTML/database routes, CSP, frame denial, nosniff and referrer policy via regression tests. Disabled HTMX history snapshots for database fragments and set historyCacheSize to zero. CSS, JS and HTMX remain same-origin bundled assets.
