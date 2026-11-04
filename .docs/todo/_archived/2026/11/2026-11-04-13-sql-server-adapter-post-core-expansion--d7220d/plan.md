---
{id: GRP-5F8CABD7220D,name: 13 — SQL Server adapter (post-core expansion),type: documentation,extra: {created_at: 2026-11-04T07:32:09.542Z}}
---

# 13 — SQL Server adapter (post-core expansion) — Plan

## Implementation approach
First verify a well-maintained Rust SQL Server client and safe TLS/authentication options. Integrate driver-specific connection management and catalog discovery behind the shared adapter interface. Then implement bounded data pages, sorting, type handling and storage statistics where feasible. Test against a disposable supported server version and document differences transparently.

## Risks and safeguards
SQL Server driver maturity, TLS defaults and corporate authentication mechanisms may differ from SQLx-supported databases. Avoid claiming compatibility before end-to-end verification. Some index, storage and statistics operations may require privileges beyond simple SELECT; unsupported metadata must remain visibly unavailable rather than cause application failure.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.
