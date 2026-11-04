---
{id: GRP-63970A3E9BC9,name: 05 — HTTP read endpoints and HTML fragments,type: documentation,extra: {created_at: 2026-11-04T07:32:05.597Z}}
---

# 05 — HTTP read endpoints and HTML fragments — Plan

## Implementation approach
Define an explicit routing contract for home, connection status, schema children, table data, structure and statistics. Parse identifiers and pagination controls through typed request models and translate adapter errors into safe HTTP statuses. Render full navigation pages and narrow fragments from the same view models. Add request cancellation, caching boundaries and secure defaults before wiring the polished UI.

## Risks and safeguards
An HTML-fragment application can accidentally render private data into browser history or shared caches. Cross-origin requests, raw diagnostic strings and unbounded HTMX updates are other risks. Disable caching for sensitive data, escape values and make browser state transitions predictable. The server must not turn query parameters into unvalidated SQL identifiers.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.

## Demo fidelity implementation plan

**Required reference:** `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. Define server routes and view models that correspond exactly to the HTML demo's distinct sections before implementing interactions. Treat each HTMX fragment as part of the existing shell, preserve stable swap targets and keep context navigation when server-side data changes. Document safe exports as server-driven data transfer rather than browser-side full-table memory snapshots. At least 1440×900 and 390×844 comparisons must be recorded with light/dark states where applicable. The demo's example values are fixture data only; deliver server-backed, permission-aware read-only functionality. Any material visual exception must be documented and approved before group completion.
