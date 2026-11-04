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
