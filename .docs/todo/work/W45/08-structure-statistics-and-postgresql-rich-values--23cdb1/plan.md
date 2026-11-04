---
{id: GRP-230A7623CDB1,name: "08 — Structure, statistics and PostgreSQL rich values",type: documentation,extra: {created_at: 2026-11-04T07:32:06.961Z}}
---

# 08 — Structure, statistics and PostgreSQL rich values — Plan

## Implementation approach
Connect the catalog metadata to reusable table inspector templates. Show columns, defaults, keys and indexes under Structure, while Info holds estimated row counts and storage measurements. Add JSON, arrays and bytea formatting with bounded content. Detect PostGIS availability and offer metadata and safe GeoJSON representations, without adding a map dependency in the first release.

## Risks and safeguards
Catalog statistics can be absent or stale and spatial objects can contain enormous coordinate arrays. Avoid automatic expensive geometry transformations and do not present estimated widths as exact storage usage. Keep errors understandable when a schema has custom types or when the connected role cannot inspect all metadata.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.
