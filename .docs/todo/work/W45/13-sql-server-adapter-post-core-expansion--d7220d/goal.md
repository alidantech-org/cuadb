---
{id: GOAL-GRP-5F8CABD7220D,name: 13 — SQL Server adapter (post-core expansion) goal,type: documentation,extra: {created_at: 2026-11-04T07:32:09.542Z,group: GRP-5F8CABD7220D}}
---

# 13 — SQL Server adapter (post-core expansion) — Goal

## Outcome
Evaluate and implement Microsoft SQL Server support after the PostgreSQL, MySQL/MariaDB and SQLite viewer is proven. Preserve the same read-only explorer, data grid and metadata contract while respecting SQL Server-specific drivers, namespaces, catalog views and pagination requirements.

## Definition of done
This group is complete when its implemented behavior is verifiable through the documented checks, not merely when files exist. Functionality must remain read-only and obey the shared database capability contract. All error states need truthful, usable messages and no credential disclosures. Work should be integrated with the existing single-process Rust application and browser shell, without introducing a separate runtime service. The group tasks describe individual deliverables and must be completed through Task checklists rather than a parallel root TODO file.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
