---
{id: GOAL-GRP-51AE7EFD998F,name: "03 — PostgreSQL schemas, relations and metadata goal",type: documentation,extra: {created_at: 2026-11-04T07:32:04.794Z,group: GRP-51AE7EFD998F}}
---

# 03 — PostgreSQL schemas, relations and metadata — Goal

## Outcome
Deliver complete, permission-aware catalog browsing for PostgreSQL and PostGIS installations. Users must find schemas, tables and views; inspect column definitions, indexes, keys, constraints and storage sizes; and understand when a statistic is approximate or unavailable. All catalog queries must be scoped, efficient and read-only.

## Definition of done
This group is complete when its implemented behavior is verifiable through the documented checks, not merely when files exist. Functionality must remain read-only and obey the shared database capability contract. All error states need truthful, usable messages and no credential disclosures. Work should be integrated with the existing single-process Rust application and browser shell, without introducing a separate runtime service. The group tasks describe individual deliverables and must be completed through Task checklists rather than a parallel root TODO file.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
