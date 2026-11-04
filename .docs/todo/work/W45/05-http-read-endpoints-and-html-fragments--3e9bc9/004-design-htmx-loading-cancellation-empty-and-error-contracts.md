---
{id: TSK-9890D0090922,name: "Design HTMX loading, cancellation, empty and error contracts",type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:05.887Z,updated_at: 2026-11-04T07:32:05.887Z,tags: [cuadb,http],depends_on: [TSK-E09AA19D0352],order: 4096}
---

# Design HTMX loading, cancellation, empty and error contracts

## Goal
Make asynchronous data navigation predictable during slow queries, network interruptions and permission failures. Stale responses should not overwrite the currently selected table, and errors should render in a consistent, accessible way.

## Context
Switching between tables, sorting and clicking next rapidly may produce overlapping requests. The UI must avoid showing mixed results for different relations. Distinguish empty dataset, connection offline, timeout, denied permission and invalid request. Browser-only interactions should remain available if the database is temporarily unreachable.

This task belongs to **05 — HTTP read endpoints and HTML fragments**. The group outcome is: Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Define swap targets, request synchronization and stable loading indicators per fragment.
- [ ] Provide safe fragments for timeout, validation, permission and disconnected states.
- [ ] Ensure stale responses cannot replace a newer selected relation or sort.
- [ ] Test full-page and HTMX failure behavior with simulated slow fake adapters.
- [ ] Document retry and refresh actions without automatic unbounded polling.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.
