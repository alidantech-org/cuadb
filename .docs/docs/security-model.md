---
{id: DOC-72FE83C59465,name: Read-only database and browser security model,type: documentation,tags: [cuadb,architecture]}
---

# CuaDB security model

## Threat assumptions

CuaDB is local-first but views potentially sensitive records. The default deployment trusts the operator who configures database connections, not arbitrary browser input. Network exposure beyond host loopback requires authentication, TLS, origin controls and access review. Attack surfaces include connection strings, identifiers, sort parameters, cursors, large cell contents, HTML responses, SQLite filesystem paths, sessions and future SQL execution.

## Core database browsing

The current explorer is read-only. Use dedicated least-privilege reader credentials for PostgreSQL and SQLite. Where supported, enforce read-only sessions or transactions in addition to database grants. CuaDB exposes no mutation endpoint in the core explorer. Values use bind parameters. Dynamic identifiers must be resolved against trusted catalog entries and correctly quoted for the engine. Query deadlines, pool limits, page caps, cell limits and response-size limits prevent one request from monopolizing a database or the viewer.

## Credentials and network

Read database URLs only from trusted server-side settings or operator-controlled profiles. Never render connection secrets, append them to URLs, persist them in browser storage or include them in logs. In Docker, connect to external databases through a shared user-defined network instead of publishing extra database ports. SQLite may open only explicitly configured read-only mounts under approved filesystem roots.

## Future authenticated operations

Do not expose CuaDB beyond loopback until authentication, CSRF protection, TLS/origin controls and authorization have been reviewed. SQL execution and editing are later opt-in capabilities, not part of the read-only explorer. Require one-time administrator bootstrap, secure password hashing, opaque server-side sessions, deny-by-default profile/resource permissions and auditable changes. Keep arbitrary connection targets out of browser requests and do not log SQL secrets or row contents.

## SQL and editing boundary

Keep future SQL/CRUD capabilities disabled by default. Require an authenticated user, explicit per-profile and per-resource permission, a parser-backed statement policy, bounded execution time/results, cancellation and audit events. Generated CRUD must validate catalog identifiers and bind values. Reject disallowed administrative, file, network and privilege-changing operations. Database grants remain an independent security boundary; CuaDB authorization never replaces them.

## Browser controls

Escape all database-provided text in Askama. Use bundled JavaScript and CSS, restrictive content security policy, no-store cache headers for database-bearing responses, frame protection and secure content-type settings. Disable HTMX history snapshots of row contents and prevent stale asynchronous responses from replacing the active table. No credentials or raw database rows should enter persistent browser preferences. Error fragments must not leak SQL text, driver stack traces or full URLs.

## Verification

Use negative tests for identifier injection, XSS strings, special names, invalid cursors, unauthorized reads, huge binary/JSON values, connection failures and filesystem traversal. Core live fixtures are disposable PostgreSQL and SQLite only. Tests for future authentication and write operations must cover denied access, CSRF, session expiry, privilege escalation, transaction rollback and audit coverage. Never mutate a developer or production database during setup.
