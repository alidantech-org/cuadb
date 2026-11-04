---
{id: GRP-4E0980620848,name: 09 — MySQL and MariaDB compatibility,type: documentation,extra: {created_at: 2026-11-04T07:32:07.411Z}}
---

## Deferred execution gate

This group is intentionally deferred until the PostgreSQL + SQLite backend and the complete canonical frontend (groups 06–08) are accepted. Preserve completed connection groundwork. If a MySQL/MariaDB task is already in progress, stop at a safe boundary, record concise findings and mark it blocked through the Task CLI. Do not continue catalog/type/statistics/export verification or build a broad MariaDB/MySQL version matrix during the core milestone. Resume only for remaining product gaps after frontend acceptance.
# 09 — MySQL and MariaDB compatibility — Plan

## Implementation approach
Implement URL recognition and bounded SQLx MySQL pools, then map information_schema and vendor-specific metadata into the capability contract. Reuse the shared pagination and rendering models where correct, adding MySQL-specific sorting and type handling. Validate on independently provisioned disposable MySQL and MariaDB fixtures. Avoid broad admin features such as server variables, permissions editing or SQL console support.

## Risks and safeguards
MySQL and MariaDB differ in metadata details, collations, server modes and some data types; even identical URL schemes do not guarantee identical behavior. INFORMATION_SCHEMA statistics can be approximate, some storage engines report different physical sizes, and row ordering without a key remains unstable. Make absent metrics explicit.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.
