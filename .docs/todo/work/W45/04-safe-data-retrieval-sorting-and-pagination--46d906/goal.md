---
{id: GOAL-GRP-54CDE646D906,name: "04 — Safe data retrieval, sorting and pagination goal",type: documentation,extra: {created_at: 2026-11-04T07:32:05.197Z,group: GRP-54CDE646D906}}
---

---
{id: GOAL-GRP-54CDE646D906,name: "04 — Safe data retrieval, sorting and pagination goal",type: documentation,extra: {created_at: 2026-11-04T07:32:05.197Z,group: GRP-54CDE646D906}}
---

# 04 — Safe data retrieval, sorting and pagination — Goal

## Outcome
Implement the core read-only viewing capability with bounded, predictable SQL and correct typed values. A user must be able to open any permitted PostgreSQL table, read a page, sort an allowed column and navigate safely without requesting the entire relation. This group is the performance and query-security center of the project.

## Definition of done
Acceptance of **04 — Safe data retrieval, sorting and pagination** requires evidence for these results:
1. **Build validated identifier and query construction helpers:** Ensure every dynamic SQL identifier comes from a catalog-validated allowlist, with correct PostgreSQL quoting and no execution of user-submitted statements. This provides the trusted boundary for relation selection, sorting and row reads. Verification includes: Implement safe schema/table/column resolution against trusted catalog objects.
2. **Fetch bounded PostgreSQL row pages with deterministic ordering:** Return a small set of records for a permitted relation without loading its full contents or computing exact totals. Preserve a stable column order and make explicit when the relation lacks a reliable key for deterministic navigation. Verification includes: Fetch only requested columns and a bounded number of rows using stable ordering where possible.
3. **Support single-column sorting with safe null and tie behavior:** Allow ascending and descending sorting on valid columns while preserving deterministic ties where feasible. Make sorting parameters explicit and ensure the UI cannot request arbitrary expressions or issue unbounded database work. Verification includes: Accept only catalog-listed columns and an explicit ascending or descending direction.
4. **Implement previous/next pages and indexed cursor optimization:** Provide predictable page navigation using bounded offsets for ordinary tables and keyset paging when a suitable index and sort order exist. Keep page tokens opaque, validated and scoped to the chosen relation and sort. Verification includes: Implement previous/next states and safe bounds for requested page numbers or cursors.
5. **Normalize PostgreSQL cell types and bound large values:** Convert supported SQLx row values into a consistent typed data model for Askama templates while retaining nullability and avoiding unlimited memory or HTML response sizes. Support graceful representation of extension and unknown types. Verification includes: Decode common PostgreSQL scalar types and NULL distinctly from empty strings.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
