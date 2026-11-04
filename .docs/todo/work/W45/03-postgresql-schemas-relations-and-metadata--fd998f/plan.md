---
{id: GRP-51AE7EFD998F,name: "03 — PostgreSQL schemas, relations and metadata",type: documentation,extra: {created_at: 2026-11-04T07:32:04.794Z}}
---

# 03 — PostgreSQL schemas, relations and metadata — Plan

## Implementation approach
Use PostgreSQL pg_catalog rather than building the interface around unportable information_schema-only assumptions. Filter system objects unless explicitly requested, honor table and schema permissions, and represent relations and partition parents accurately. Structure and statistics queries should be small, parameterized where possible and bounded. Add fixtures covering mixed-case names, Unicode, unusual types, foreign keys and partitions.

## Risks and safeguards
Catalog APIs differ by server version and enabled extensions. Approximate row counts are not exact, partitioned parents may have little physical storage, and statistics can be stale. Never issue a full-table COUNT automatically. An unavailable or permission-denied metric must be labeled honestly instead of fabricated.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.
