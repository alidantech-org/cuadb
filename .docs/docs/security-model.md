---
{id: DOC-72FE83C59465,name: Read-only database and browser security model,type: documentation,tags: [cuadb,architecture]}
---

# Read-only database and browser security model

## Threat assumptions
CuaDB is local-first but views potentially sensitive records. The default deployment trusts the operator who configures the database URL, not arbitrary browser input. Network exposure beyond host loopback is a separate security decision requiring authentication, TLS, origin controls and access review. Attack surfaces include connection strings, schema/table/column identifiers, sort parameters, cursor tokens, large cell contents, HTML responses, filesystem paths for SQLite and database privileges.

## Database boundaries
Use dedicated least-privilege reader credentials for every configured engine. Where supported, additionally enforce read-only sessions or transactions; this supplements, rather than replaces, database grants. CuaDB exposes no arbitrary SQL endpoint, mutation API or administrative actions. Values use bind parameters. Dynamic identifiers must be resolved against trusted catalog entries and correctly quoted for the current engine. Query deadlines, pool limits, page caps, cell limits and response-size limits prevent one request from monopolizing a database or the viewer.

## Credentials and network
Read URLs only from trusted server-side settings or operator-controlled profiles in the initial releases. Do not render connection secrets, append them to URLs, persist them in browser storage, or include them in logs. In Docker, connect to the existing postgis container through a shared user-defined network and internal hostname instead of publishing additional database ports. The Rust server may listen on 0.0.0.0 inside Docker, while Docker publishes 127.0.0.1:4848 on the host by default. For SQLite, only open explicitly configured read-only mounts under approved filesystem roots.

## Browser controls
Escape all database-provided text in Askama. Use local bundled JavaScript and CSS, a compatible restrictive content security policy, no-store cache headers for database-bearing responses, frame protection and secure content-type settings. Disable HTMX history snapshot caching of row contents and prevent stale asynchronous responses from replacing the active table. No credentials or raw database rows should enter persistent browser preferences. Error fragments must not leak SQL text, driver stack traces or full URLs.

## Verification
Use negative tests for SQL identifier injection, XSS strings, special names, invalid cursors, unauthorized reads, huge binary/JSON values, connection failures and filesystem traversal. Integration fixtures must be disposable and must never perform setup writes against the developer’s actual data. Document residual risks, especially RLS behavior, views executing privileged functions, metadata visibility and external network deployment.