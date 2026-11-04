---
{id: GOAL-GRP-C1E9A83A4145,name: 11 — Connection profiles and everyday usability goal,type: documentation,extra: {created_at: 2026-11-04T07:32:08.365Z,group: GRP-C1E9A83A4145}}
---

# 11 — Connection profiles and everyday usability — Goal

## Outcome
Allow safe switching among multiple known database connections while keeping credentials out of the browser and eliminating the need to repeatedly reconfigure the viewer. Improve the everyday explorer experience with durable, non-sensitive preferences and sensible keyboard shortcuts.

## Definition of done
This group is complete when its implemented behavior is verifiable through the documented checks, not merely when files exist. Functionality must remain read-only and obey the shared database capability contract. All error states need truthful, usable messages and no credential disclosures. Work should be integrated with the existing single-process Rust application and browser shell, without introducing a separate runtime service. The group tasks describe individual deliverables and must be completed through Task checklists rather than a parallel root TODO file.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
