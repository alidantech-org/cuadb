---
{id: GOAL-GRP-E86C8856AAA4,name: 10 — SQLite file-based database adapter goal,type: documentation,extra: {created_at: 2026-11-04T07:32:07.879Z,group: GRP-E86C8856AAA4}}
---

---
{id: GOAL-GRP-E86C8856AAA4,name: 10 — SQLite file-based database adapter goal,type: documentation,extra: {created_at: 2026-11-04T07:32:07.879Z,group: GRP-E86C8856AAA4}}
---

# 10 — SQLite file-based database adapter — Goal

## Outcome
Enable browsing local SQLite database files securely from the same CuaDB interface. A database file must be mounted into the container or available through a configured host path; the viewer may inspect it but must never edit the database or produce write side-effects as part of ordinary browsing.

## Definition of done
Acceptance of **10 — SQLite file-based database adapter** requires evidence for these results:
1. **Open mounted SQLite databases through restricted read-only paths:** Accept a configured SQLite URL or file path only within an allowed mounted directory and open the database in read-only mode. Reject arbitrary filesystem traversal, unsupported URI options and paths outside configured roots. Verification includes: Validate configured SQLite URLs against an explicit allowed data root.
2. **Discover SQLite tables, views, columns and key constraints:** Map SQLite schema objects into the common explorer without inventing server databases or PostgreSQL namespaces. Show columns, declared types, primary and foreign keys and indexes where SQLite metadata supports them. Verification includes: List ordinary tables, supported views and virtual tables with accurate type distinctions.
3. **Provide safe SQLite row pagination and dynamic type rendering:** Read bounded pages from SQLite files with stable sorting where possible, using engine-correct identifier quoting and row identity rules. Preserve the common grid interface while accurately representing SQLite’s dynamic per-value types. Verification includes: Implement validated table and column resolution with safe SQLite identifier quoting.
4. **Expose SQLite capabilities and integration tests honestly:** Complete SQLite support with verified Info and Structure behavior that clearly describes unavailable per-table size estimates. Ensure that read-only operation works with real mounted files in Docker as well as native development. Verification includes: Render usable SQLite database information with explicit unavailable metrics.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
