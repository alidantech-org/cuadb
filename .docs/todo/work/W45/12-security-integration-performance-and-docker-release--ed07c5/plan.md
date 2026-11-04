---
{id: GRP-C11E16ED07C5,name: "12 — Security, integration, performance and Docker release",type: documentation,extra: {created_at: 2026-11-04T07:32:08.777Z}}
---

## Final release gate

This group is the final acceptance phase. Do not start it as a substitute for completing the core backend, frontend, deferred engine work, trusted profiles, authentication or controlled SQL workspace. Release evidence must cover the current product scope and the dedicated security boundaries for any enabled SQL/CRUD capability. Keep engine verification focused on PostgreSQL and SQLite plus only the remaining MySQL/MariaDB gaps actually being completed.
# 12 — Security, integration, performance and Docker release — Plan

## Implementation approach
Create disposable database fixtures and automated adapter contract tests, then exercise HTTP/HTML navigation through smoke and end-to-end checks. Review attacker-controlled inputs, secrets, read-only browsing guarantees, authentication and the explicit SQL/CRUD authorization boundary. Verify multi-stage Docker builds, runtime resource limits and static assets. Benchmark representative large datasets to find pagination and rendering bottlenecks; publish clear compatibility and operational documentation.

## Risks and safeguards
A passing unit test does not prove the container can reach PostgreSQL or that database privileges prevent writes. Releasing with outdated lockfiles, missing build assets, expensive default SQL or unsafe Docker port binding would defeat the project goals. Use explicit acceptance gates and distinguish native-only checks from optional integration tests requiring disposable databases.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.
