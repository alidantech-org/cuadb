---
{id: TSK-B18207BD473C,name: Build the authenticated SQL workspace and bounded result viewer,type: task,status: planned,priority: medium,created_at: 2026-11-04T16:14:15.596Z,updated_at: 2026-11-04T16:14:15.596Z,tags: [cuadb,sql-workspace],depends_on: [TSK-68384A11A749,TSK-470D81309D89],order: 2048}
---

# Build the authenticated SQL workspace and bounded result viewer

## Goal
Provide a separate SQL workspace for authorized users with a clear editor, execution controls, status feedback and a bounded result grid. Keep the existing database explorer intact and make it obvious which connection profile is selected and which permissions apply.

## Context
This is a later frontend feature, not part of the initial canonical explorer milestone. Reuse the approved visual system and keep SQL execution server-side. Do not store credentials or sensitive result sets in browser history, local storage or long-lived client caches.

This task belongs to **14 — SQL workspace and controlled database editing**. The group outcome is: Add a separate, explicitly authorized workspace for SQL execution and normal table data editing. Preserve the safe read-only explorer as the default experience. Every mutating action must be authenticated, permission-checked, bounded, attributable and auditable.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [ ] Add a distinct SQL workspace entry visible only to users with the relevant permission.
- [ ] Show the selected trusted profile and effective read/write permission before execution.
- [ ] Render bounded typed results, affected-row counts and safe execution errors.
- [ ] Support cancellation and prevent stale results from replacing a newer profile selection.
- [ ] Verify keyboard accessibility and that unauthorized users cannot invoke the routes directly.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.
