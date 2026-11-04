---
{id: GRP-51BFCC8B1369,name: 06 — Database navigation and workspace UI,type: documentation,extra: {created_at: 2026-11-04T07:32:05.939Z}}
---

## Phase gate

This is Phase 2 frontend work. Do not start or continue implementation until the PostgreSQL + SQLite backend groups (01–05 and 10) are accepted. The backend must provide real, stable data and HTML-fragment contracts first. Do not add MySQL/MariaDB checks to frontend tasks. Keep the committed `assets/cuadb-ui-demo.html` as the binding reference and perform focused visual acceptance at the group boundary, not repeated full screenshot matrices for every small change.
# 06 — Database navigation and workspace UI — Plan

## Implementation approach
Use Askama components and Tailwind CSS tokens to create a coherent visual system, then connect sidebar actions using HTMX. Keep the most common navigation one click away, support filtering long table lists, and make loading/disabled states obvious. Use vanilla JavaScript only for local toggles such as resizing and keyboard handling, with no frontend data fetching duplicated in JavaScript.

## Risks and safeguards
A crowded sidebar or expensive full-tree render makes database browsing unpleasant when a schema has thousands of tables. The UI must remain usable on small laptop screens, maintain accessible focus and not infer connection success from a stale indicator. Avoid rendering fake metadata when a relation has not yet loaded.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.

## Demo fidelity implementation plan

**Required reference:** `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. Use the demo's CSS tokens, compact measurements and actual sidebar hierarchy as implementation fixtures. Implement Simple explorer, then Advanced group navigation and Overview, then mobile/resizer behaviors. Review desktop and mobile screenshots before marking the group complete; do not introduce unrelated navigation or design abstractions. At least 1440×900 and 390×844 comparisons must be recorded with light/dark states where applicable. The demo's example values are fixture data only; deliver server-backed, permission-aware read-only functionality. Any material visual exception must be documented and approved before group completion.
