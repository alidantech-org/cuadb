---
{id: GRP-C1E9A83A4145,name: 11 — Connection profiles and everyday usability,type: documentation,extra: {created_at: 2026-11-04T07:32:08.365Z}}
---

## Phase gate

Complete this group after the PostgreSQL + SQLite backend, the canonical frontend and the remaining required MySQL/MariaDB compatibility work. Profiles are operator-configured and server-owned; never accept arbitrary browser-supplied connection URLs or expose credentials. Switching must reset all database-specific selection and cursor state.
# 11 — Connection profiles and everyday usability — Plan

## Implementation approach
Start with server-owned named profiles configured through environment or mounted configuration. Present labels, engine and reachability, not raw URLs. Only after a security review consider allowing browser creation or modification of profiles. Keep non-secret preferences such as selected theme or page size local, while ensuring no row contents or credentials are persisted.

## Risks and safeguards
Browser-managed connections can become an SSRF or secret-exposure mechanism if implemented casually. Profiles should be controlled by the operator in the first release, and persistent credentials require an explicit storage and encryption design. Local browser history or settings must not cache raw database content.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.

## Demo fidelity implementation plan

**Required reference:** `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. Start with the demo's sidebar-bottom connection popover, then implement settings controls using exact labels, groups and visual treatments from the reference. Treat local preferences as a tightly allowlisted, non-sensitive set; keep URLs and credentials server-side. Verify fast switching and settings interactions after HTMX swaps and compare side-by-side snapshots. At least 1440×900 and 390×844 comparisons must be recorded with light/dark states where applicable. The demo's example values are fixture data only; deliver server-backed, permission-aware read-only functionality. Any material visual exception must be documented and approved before group completion.
