---
{id: DOC-A146C291CE62,name: CuaDB architecture and adapter boundaries,type: documentation,tags: [cuadb,architecture]}
---

---
{id: DOC-A146C291CE62,name: CuaDB architecture and adapter boundaries,type: documentation,tags: [cuadb,architecture]}
---

# CuaDB architecture and adapter boundaries

## Product boundary
CuaDB is a lightweight, read-only database explorer. It is deliberately not a SQL editor, migration system, write-capable client, user administration panel or database server. A single compiled Rust/Axum HTTP service serves Askama HTML, local HTMX, compiled Tailwind CSS and small JavaScript assets. Bun is a development task runner, not a production runtime. A single Docker container publishes one HTTP port on host loopback by default. Databases remain external to the CuaDB runtime.

## Layer ownership
The HTTP layer parses browser selection requests, validates basic input and returns full HTML pages or scoped HTMX fragments. Templates present typed view models; they never construct database queries. The application layer owns configured connection profiles, bounded pools, error conversion, request cancellation and redaction. The adapter interface describes schema discovery, relation metadata, paged data, statistics and capabilities without assuming a common catalog implementation. Each database adapter owns its driver, identifier rules, SQL generation, type decoding and metadata support.

## Engine support order
PostgreSQL is first and the default URL scheme. It must work with PostgreSQL 18 and optionally PostGIS 3.6. MySQL/MariaDB and SQLite follow using the same browser-level contracts but independent metadata semantics. The supported engines for this roadmap are PostgreSQL, MySQL, MariaDB and SQLite. No additional engine adapter is planned until the current product features are complete. PostgreSQL connections target one database; another database is a separate trusted connection profile. MySQL database namespaces and SQLite file locations are represented honestly rather than renamed into fictitious PostgreSQL schemas.

## Query flow
A browser selects a configured connection, schema, relation, page and sort. The typed route resolves the trusted connection profile, consults the adapter and validates relation identifiers against catalog metadata. The adapter executes only bounded, read-only queries. It returns typed cells and availability-aware metadata. Rust maps this into an Askama view model; HTMX swaps a small HTML fragment. Sensitive rows are never stored in the browser as a durable client cache or exposed through a generic JSON SQL API.

## Evolution rules
Keep modules small and testable: config, errors, state, web handlers, templates and engine adapters. Avoid complex domain service hierarchies, plugin frameworks and multiple runtime services until measurable needs justify them. The Task workspace in .docs/ remains the single source of truth for execution status; this architecture document records boundaries and decisions, not a parallel task checklist.