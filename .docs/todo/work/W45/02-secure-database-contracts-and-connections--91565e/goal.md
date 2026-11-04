---
{id: GOAL-GRP-92882891565E,name: 02 — Secure database contracts and connections goal,type: documentation,extra: {created_at: 2026-11-04T07:32:04.404Z,group: GRP-92882891565E}}
---

---
{id: GOAL-GRP-92882891565E,name: 02 — Secure database contracts and connections goal,type: documentation,extra: {created_at: 2026-11-04T07:32:04.404Z,group: GRP-92882891565E}}
---

# 02 — Secure database contracts and connections — Goal

## Outcome
Provide one small engine-independent interface for database discovery, metadata and row retrieval. Make a PostgreSQL URL the default connection input and define capabilities before expanding to MySQL, SQLite or SQL Server. This group owns lifecycle, timeout and read-only guarantees; it must not leak SQL or vendor-specific catalog assumptions into web templates.

## Definition of done
Acceptance of **02 — Secure database contracts and connections** requires evidence for these results:
1. **Define engine-neutral objects, capabilities and error types:** Establish minimal data contracts for connections, namespaces, relations, columns, keys, statistics, pages and typed cell values. The interface must expose unavailable capabilities explicitly instead of pretending that all SQL engines implement the same metadata. Verification includes: Define typed relation identifiers and feature flags without engine-specific catalog leakage.
2. **Parse and validate PostgreSQL connection URLs safely:** Accept postgresql:// and postgres:// URLs through server-side configuration and validate malformed input, unsupported schemes, network locations and TLS options. A user should be able to point CuaDB at its existing postgis container without disclosing credentials to the browser. Verification includes: Validate supported schemes, database names, URL encoding and disallowed unsupported options.
3. **Implement bounded PostgreSQL pool and connection lifecycle:** Add asynchronous PostgreSQL connections using SQLx, with bounded pool size, acquisition and connection timeouts, predictable retry behavior and explicit startup status. Avoid reconnecting on every HTMX request or creating unbounded background tasks. Verification includes: Create one lazily initialized or bounded connection pool for the configured database.
4. **Enforce read-only database access and least-privilege assumptions:** Make it materially difficult for a bug or browser request to mutate a connected database. Every supported query path must be constrained to introspection and SELECT operations, using least-privilege credentials and transaction/session limits as additional safeguards. Verification includes: Document a least-privilege PostgreSQL role with schema and table read permissions.
5. **Expose safe connection status and operator diagnostics:** Provide browser-safe connection health and meaningful failure categories without embedding secrets, stack traces or SQL statements in rendered templates. Separate server liveness from actual database reachability to keep the app usable when its database is offline. Verification includes: Return safe status categories such as disconnected, ready, unavailable and insufficient permissions.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
