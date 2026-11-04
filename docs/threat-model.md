# CuaDB threat model

Scope: CuaDB is a **local-first, read-only** database explorer: one Rust process, one HTTP port, operator-configured connections. It is designed for `127.0.0.1`; exposing it beyond localhost needs the controls listed under *Residual risks*.

## Assets
- Database contents readable by the configured role (the main asset).
- Connection credentials (`CUADB_DATABASE_URL`, `CUADB_PROFILES_FILE`).
- Host filesystem (SQLite files, the profiles file).
- Availability of the databases (no query should be able to exhaust them) and of the host.

## Entry points and trust boundaries
| Entry point | Trust | Controls (evidence) |
|---|---|---|
| HTTP `GET /catalog/*`, `/connection/*`, `/health`, `/static/*` | **Untrusted** browser or any local process | Typed query structs reject unknown/duplicate parameters; identifiers validated then quoted/allow-listed against the live catalog; no SQL, URL, host or path accepted (`tests/security_negative.rs`, `tests/catalog_http.rs`, `tests/router_fake_adapter.rs`) |
| Non-GET methods | Untrusted | Only GET routes exist (405 test) |
| Environment / profiles file | **Trusted operator** | Validated at startup, secrets redacted from errors and `Debug` (`src/profiles.rs`, `src/config.rs`) |
| Database server | Semi-trusted (data is untrusted) | Read-only sessions, server-side timeouts, bounded results; values are escaped and control/bidi characters made visible (`tests/rich_live_http.rs`) |
| SQLite files | Trusted mount, untrusted content | Root confinement, symlink/traversal rejection, `read_only` + `query_only` (`tests/sqlite_live.rs`) |

## Threats and mitigations
- **SQL injection / identifier injection:** values are bound parameters; identifiers come from the catalog and are quoted per engine. Verified with 12 payload classes × 13 routes on SQLite, PostgreSQL, MariaDB, MySQL.
- **XSS / template injection:** Askama auto-escaping; error pages never echo input; CSP `default-src 'self'` with no inline script; `nosniff`, `X-Frame-Options: DENY`.
- **Write access:** the explorer has no write path and uses read-only sessions (`SET ... READ ONLY`, SQLite `query_only`). Writes exist only in the opt-in SQL workspace (`CUADB_SQL_WORKSPACE=enabled`, `docs/sql-workspace.md`): authenticated, CSRF-protected, per-grant authorized, read-write profiles only (separate writer pool), parser-checked single statements, row-limited, transactional and audited. The real protection is still a least-privilege database role.
- **Credential disclosure:** URLs are never rendered; the browser selects a profile only by id; logs record route, status and request id, never URLs or query strings.
- **Denial of service:** page size ≤ 100, offset ≤ 10,000, ≤ 24 visible columns, 128-byte filters, 64 KiB request bodies, in-flight request cap, request and **server-side query timeouts** on every engine (100 ms setting aborts a 1M-row scan in ~105 ms; see `docs/performance.md`), export caps (10k rows, 16 MiB, 8 relations).
- **Cache/Clipboard leaks:** database-bearing responses are `Cache-Control: no-store`; HTMX history snapshots disabled on panels. Clipboard copy is user-initiated, in the page.
- **Filesystem escape (SQLite):** canonical path must stay under `CUADB_SQLITE_ROOT`; symlink targets re-checked.
- **Unauthorized access (when authentication is enabled):** server-side sessions (hashed tokens, idle/absolute expiry, rotation on login), CSRF token plus Origin checks on all state changes, Argon2id passwords, login lockout, deny-by-default grants enforced in every handler and applied to list filtering and exports (`tests/auth_http.rs`, `tests/auth_service.rs`).
- **Cross-profile leakage:** per-profile adapters and cursors; cursors from one profile are rejected on another.

## SQL workspace (when enabled)
Additional threats: parser bypass, multi-statement payloads, dangerous functions, resource exhaustion, accidental commits and privilege escalation. Mitigations: `sqlparser`-based single-statement policy with a function denylist, read-only transactions for reads (even side-effecting functions fail), server-side timeouts/cancel, row/byte caps, one-row typed edits with stale-row checks, explicit transactions that must be reviewed before commit, per-profile/per-resource grants capped by role, profile-switch refusal during a pending transaction, and an audit trail without values. Residual: the denylist is not exhaustive; SQLite cannot interrupt a running statement; an administrator with a read-write profile can run any allowed DML.

## Residual risks and required controls before non-localhost use
1. **Authentication is opt-in and there is no TLS in CuaDB.** Until an administrator is bootstrapped (`docs/auth.md`) anyone who can reach the port can read everything the role can. Once one exists, sessions, CSRF protection, deny-by-default grants and an audit log apply, but traffic is still plain HTTP: put it behind a TLS reverse proxy, or keep the default `127.0.0.1` publish.
2. **Read-only is defense in depth only.** Use a dedicated SELECT-only database role; session flags can be undone by a role with write rights.
3. **Data sensitivity:** column-level secrets (password hashes, tokens) are displayed if the role can SELECT them; use views/grants to hide them.
4. **Passwordless trust authentication** (as in local fixtures) is accepted only for loopback hosts; remote hosts require passwords and verified TLS.
5. **Resource exhaustion by many clients** is bounded per request, not per client IP; rate limiting belongs in the proxy.
6. **Export buffering:** exports are assembled in memory within the 16 MiB cap, not streamed.
7. **Supply chain:** builds use `Cargo.lock` and `bun.lock` (`--locked` / `--frozen-lockfile`); no automated dependency audit is run in this repository.
8. Not assessed: browser-side behavior (HTMX abort, clipboard) pending Phase B validation.
