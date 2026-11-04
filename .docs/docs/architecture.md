---
{id: DOC-A146C291CE62,name: CuaDB architecture and adapter boundaries,type: documentation,tags: [cuadb,architecture]}
---

# CuaDB architecture and adapter boundaries

## Product boundary

CuaDB is a local-first database explorer and workspace served by one Rust/Axum process, Askama HTML, HTMX, compiled Tailwind CSS and small JavaScript assets. The current explorer is read-only. Later data editing and SQL execution are separate capabilities gated by authenticated users, explicit resource permissions and audit events. Bun is a development task runner, not a production runtime. A single Docker container publishes one HTTP port on host loopback by default; databases remain external.

## Layer ownership

The HTTP layer parses browser selection requests, validates input and returns full HTML pages or scoped HTMX fragments. Templates present typed view models and never construct database queries. The application layer owns trusted connection profiles, bounded pools, error conversion, request cancellation, authorization and redaction. The adapter interface describes discovery, relation metadata, paged data, statistics and capabilities without assuming a common catalog implementation. Each adapter owns its driver, identifier rules, SQL generation, type decoding and metadata support.

## Engine support order

PostgreSQL is the primary engine and SQLite is the second required engine for the initial backend milestone. Complete both before frontend implementation. MySQL/MariaDB are deliberately deferred until the canonical frontend is complete; preserve existing connection groundwork and do not run broad compatibility matrices during core work. No other engine is in scope. PostgreSQL connections target one database; SQLite files are opened only under configured read-only roots; engine-specific namespaces are represented honestly.

## Delivery stages

Complete the backend first: Rust foundations, PostgreSQL contracts/catalog/rows, typed HTTP routes and HTML-fragment contracts, then SQLite connection/catalog/rows/capabilities. PostgreSQL and SQLite are the only required engines in this stage. Complete the canonical frontend only after this backend gate. Resume MySQL/MariaDB compatibility after the frontend milestone, then complete multi-connection profiles, authentication/resource permissions, the controlled SQL workspace and final release acceptance. Group numbers are stable identifiers, not execution order.

## Query flow

A browser selects a trusted profile, namespace, relation, page and sort. Typed routes validate every selection against catalog metadata. The core explorer uses bounded read-only adapter queries and returns typed cells and capability-aware metadata for Askama/HTMX. Later SQL and editing operations use a separate authenticated execution boundary with per-profile and per-resource authorization; they must never be added as an unrestricted generic endpoint. Sensitive rows and credentials are not persisted in browser storage.

## Evolution rules

Keep modules small and testable: config, errors, state, web handlers, templates and engine adapters. Avoid complex service hierarchies, plugin frameworks and multiple runtime services until measurable needs justify them. The Task workspace in .docs/ remains the single source of truth for execution status; this architecture document records boundaries and decisions, not a parallel task checklist.
