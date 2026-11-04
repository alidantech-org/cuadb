---
{id: GOAL-GRP-C1E9A83A4145,name: 11 — Connection profiles and everyday usability goal,type: documentation,extra: {created_at: 2026-11-04T07:32:08.365Z,group: GRP-C1E9A83A4145}}
---

---
{id: GOAL-GRP-C1E9A83A4145,name: 11 — Connection profiles and everyday usability goal,type: documentation,extra: {created_at: 2026-11-04T07:32:08.365Z,group: GRP-C1E9A83A4145}}
---

# 11 — Connection profiles and everyday usability — Goal

## Outcome
Allow safe switching among multiple known database connections while keeping credentials out of the browser and eliminating the need to repeatedly reconfigure the viewer. Improve the everyday explorer experience with durable, non-sensitive preferences and sensible keyboard shortcuts.

## Definition of done
Acceptance of **11 — Connection profiles and everyday usability** requires evidence for these results:
1. **Support multiple trusted server-side database profiles:** Let a single CuaDB instance choose among more than one configured connection, including different PostgreSQL databases and supported other engines. Keep profile labels safe and never expose connection URLs or passwords through the HTML interface. Verification includes: Define a validated server-side profile configuration format with stable identifiers.
2. **Build secure connection switching and database selection UX:** Offer an understandable connection picker, accurately update the schema tree and selected relation, and avoid retaining stale rows from a previously selected database. Keep navigation responsive without storing secrets on the client. Verification includes: Render available trusted profile labels with clear connected and offline states.
3. **Polish non-sensitive preferences, navigation and keyboard shortcuts:** Remember convenient presentation preferences such as theme, page size and sidebar width without persisting database rows, raw SQL or credentials in the browser. Make common explorer actions easy to discover and use. Verification includes: Persist theme and selected grid presentation settings without saving database values.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.

## Mandatory visual acceptance

The canonical frontend specification for this group is `assets/cuadb-ui-demo.html`, reinforced by `.docs/docs/frontend-interactions.md`. It is a binding acceptance baseline for all related tasks, not an alternative design sketch. Completion requires the precise sidebar-bottom connection picker, account label/select/checkmark patterns, settings gear/popover, Simple/Advanced switch, theme/palette/accent/corner/font/density preferences, per-table remembered state and reset controls shown in the demonstration. Real profiles must be operator configured; the demo's fake connection names and states must not ship as live data. Persist only non-sensitive preferences and keep all controls accessible. All relevant tasks must pass a side-by-side comparison against the same browser viewport, mode and selected object state, and must not be marked complete based on a different layout.
