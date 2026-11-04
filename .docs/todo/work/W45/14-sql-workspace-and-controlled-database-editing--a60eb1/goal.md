---
{id: GOAL-GRP-CA8149A60EB1,name: 14 — SQL workspace and controlled database editing goal,type: documentation,extra: {created_at: 2026-11-04T16:14:15.288Z,group: GRP-CA8149A60EB1}}
---

# 14 — SQL workspace and controlled database editing — Goal

## Outcome
Add a separate, explicitly authorized workspace for SQL execution and normal table data editing. Preserve the safe read-only explorer as the default experience. Every mutating action must be authenticated, permission-checked, bounded, attributable and auditable.

## Definition of done
Acceptance of **14 — SQL workspace and controlled database editing** requires evidence for these results:
1. **Define SQL statement policy and authorized execution boundary:** Create a server-side execution boundary that classifies submitted SQL and checks the authenticated user’s permissions for the selected trusted profile. The default must remain disabled. Reject unsupported or dangerous statement classes, multi-statement payloads and resource-exhausting requests before contacting the database. Verification includes: Add parser-backed statement classification for supported PostgreSQL, SQLite and MySQL-family dialects.
2. **Build the authenticated SQL workspace and bounded result viewer:** Provide a separate SQL workspace for authorized users with a clear editor, execution controls, status feedback and a bounded result grid. Keep the existing database explorer intact and make it obvious which connection profile is selected and which permissions apply. Verification includes: Add a distinct SQL workspace entry visible only to users with the relevant permission.
3. **Implement permission-checked table create, update and delete operations:** Add normal table data editing through explicit typed operations rather than silently making the existing explorer grid editable. Validate relation and column identities against catalog metadata, bind values, enforce per-resource edit permissions and keep every operation within a transaction. Verification includes: Implement create, update and delete operations using validated catalog identifiers and bound values.
4. **Add explicit transaction review, commit and rollback controls:** Make write behavior understandable and recoverable by exposing explicit transaction boundaries for supported editing workflows. Users must see the target profile, pending changes and affected-row estimates before committing, with a clear rollback path for uncommitted work. Verification includes: Provide explicit transaction begin, commit and rollback handling for supported edit flows.
5. **Audit SQL execution and database mutations:** Make every SQL execution and CRUD mutation attributable and reviewable. Store the authenticated actor, profile, operation category, timestamp, outcome and bounded metadata without copying sensitive row values or credentials into the audit log. Verification includes: Record authorized and denied SQL execution attempts with actor, profile and outcome.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that run without live database access when possible, and document fixture-only validation clearly. Preserve the existing Bun commands and local-only Docker defaults. Record architectural decisions and compatibility limits in the Task workspace; never create a competing task tracker.
