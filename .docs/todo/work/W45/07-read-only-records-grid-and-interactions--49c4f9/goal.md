---
{id: GOAL-GRP-033BDB49C4F9,name: 07 — Read-only records grid and interactions goal,type: documentation,extra: {created_at: 2026-11-04T07:32:06.325Z,group: GRP-033BDB49C4F9}}
---

---
{id: GOAL-GRP-033BDB49C4F9,name: 07 — Read-only records grid and interactions goal,type: documentation,extra: {created_at: 2026-11-04T07:32:06.325Z,group: GRP-033BDB49C4F9}}
---

# 07 — Read-only records grid and interactions — Goal

## Outcome
Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Definition of done
Acceptance of **07 — Read-only records grid and interactions** requires evidence for these results:
1. **Render typed records in an efficient semantic HTML grid:** Turn adapter row pages into a clean table with stable column ordering, readable type formatting and clear NULL values. Keep the layout usable with dozens of columns and avoid loading unnecessary data on the client. Verification includes: Render row headers, column labels, typed values and explicit NULL markers safely.
2. **Wire paging controls and page size to the Rust server:** Let users move between pages and choose common bounded sizes such as 25, 50 or 100 rows. Paging must preserve the selected relation and sort while clearly indicating whether next and previous pages are available. Verification includes: Render previous/next buttons using actual backend continuation state.
3. **Connect sortable headers with clear direction and stable state:** Allow one-column ascending or descending sorting with a clear indicator, keyboard support and bounded server-side queries. The UI must never sort just the current page locally while implying that the whole table is sorted. Verification includes: Generate sort links only for backend-approved column identifiers.
4. **Add safe expanded cell detail and copy actions:** Let users inspect JSON, long text, arrays and other complex values without cluttering the grid. A detail drawer should show the complete supported value when within limits and clearly mark truncation or unavailable content. Verification includes: Build an accessible reusable detail panel with formatted text and JSON when supported.
5. **Polish grid resizing, keyboard navigation and accessibility:** Make dense database browsing comfortable with the keyboard and usable for assistive technology. Keep client-side behavior lightweight and resilient across HTMX fragment replacement. Verification includes: Support focused table navigation, copy shortcuts and escape-to-close behavior.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.

## Mandatory visual acceptance

The canonical frontend specification for this group is `assets/cuadb-ui-demo.html`, reinforced by `.docs/docs/frontend-interactions.md`. It is a binding acceptance baseline for all related tasks, not an alternative design sketch. Completion means a data-backed grid visually and behaviorally matching the demonstration: type-annotated sticky headers, row rail, status pills, dense rows, sorting cycle, 25/50/100 pagination, column visibility, row inspector, foreign-key navigation, copy actions, menus, toasts and permitted exports. Include Advanced linked-table ZIP selection using a secure streaming or bounded export strategy; never copy the prototype's unbounded in-memory full-table reads into production. All relevant tasks must pass a side-by-side comparison against the same browser viewport, mode and selected object state, and must not be marked complete based on a different layout.
