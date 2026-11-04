---
{id: DOC-9B73548B4ECD,name: CuaDB execution stages and completion policy,type: documentation,tags: [cuadb,architecture]}
---

# CuaDB execution stages and completion policy

## Source of truth

The Task workspace at .docs/ is authoritative for group progress, individual task status, checklist ticks, comments and dependencies. This document defines sequencing, not a second TODO list. Create and update task records through the Task CLI, never hand-edit managed IDs or status history, and run task validate after workspace changes. The roadmap bootstrap is idempotent and may add missing groups/tasks without replacing existing work.

## Phase 1 — Backend first

Complete Rust foundation, engine-neutral contracts, PostgreSQL URL/pool handling, PostgreSQL catalog discovery, bounded row retrieval and typed HTTP routes/HTML-fragment contracts. Then complete the SQLite adapter: restricted file opening, catalog, rows, dynamic value handling and capability reporting. PostgreSQL is the primary engine; SQLite is the second required engine. Both must work end to end before visual frontend implementation. Use focused disposable fixtures only.

## Backend acceptance gate

The backend gate passes when the normal server-side features are usable for PostgreSQL and SQLite: safe connections, namespace/relation discovery, structure metadata, bounded row pages, sorting/pagination, typed values, connection/error states and stable HTML fragment contracts. The gate is not a demand for every possible database type or exhaustive version matrix. Do not spend time on MySQL/MariaDB compatibility here.

## Phase 2 — Canonical frontend

Only after the backend gate passes, complete explorer navigation, the records grid, inspector, Structure and Indexes/Info surfaces. The committed assets/cuadb-ui-demo.html and .docs/docs/frontend-interactions.md are binding. Implement a complete vertical slice with real backend data, then perform focused visual and keyboard acceptance. Do not introduce a second frontend runtime.

## Phase 3 — Additional database engines

Resume unfinished MySQL/MariaDB work only after the frontend milestone is accepted. Preserve already-completed connection groundwork; do not redo it or run broad cross-version matrices. Finish only the catalog, rows, metadata and integration gaps that the product actually needs. No additional engines are in scope.

## Phase 4 — Multi-connection profiles

Complete trusted server-side connection profiles and switching after the core engines and frontend are stable. Keep credentials server-side, scope pool budgets and reset selected relation/page/cursor state when switching profiles.

## Phase 5 — Authentication and resource permissions

Add one-time administrator bootstrap, user accounts, secure sessions, CSRF protection, deny-by-default roles and profile/schema/relation/action permissions. Enforce authorization in every route and fragment before enabling protected capabilities.

## Phase 6 — SQL workspace and controlled editing

Add a separate opt-in SQL workspace and explicit CRUD/transaction operations only after authentication and resource permissions. Use parser-backed statement policy, per-resource grants, bounded execution, cancellation and audit events. The existing explorer remains read-only by default.

## Phase 7 — Release acceptance

Finish security review, focused PostgreSQL/SQLite plus later MySQL/MariaDB integration tests, performance checks, single-container Docker verification and verified documentation. Group numbers are stable identifiers, not execution order.

## Vertical-slice discipline

For each work session, inspect the exact task and blockers, start one task, implement a coherent feature slice, run the smallest relevant tests and record evidence. Run full format/check/test gates at meaningful group boundaries rather than repeatedly running every engine and browser scenario. Never test against a personal or production database.
