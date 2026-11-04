---
{id: GOAL-GRP-033BDB49C4F9,name: 07 — Read-only records grid and interactions goal,type: documentation,extra: {created_at: 2026-11-04T07:32:06.325Z,group: GRP-033BDB49C4F9}}
---

# 07 — Read-only records grid and interactions — Goal

## Outcome
Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.

## Definition of done
This group is complete when its implemented behavior is verifiable through the documented checks, not merely when files exist. Functionality must remain read-only and obey the shared database capability contract. All error states need truthful, usable messages and no credential disclosures. Work should be integrated with the existing single-process Rust application and browser shell, without introducing a separate runtime service. The group tasks describe individual deliverables and must be completed through Task checklists rather than a parallel root TODO file.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
