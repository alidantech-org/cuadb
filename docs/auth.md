# Authentication, sessions and permissions

CuaDB keeps its own state (users, sessions, grants, audit log) in a SQLite file `cuadb.db` inside `CUADB_DATA_DIR` (default `./data`; Docker: mount a volume, e.g. `-v cuadb-data:/var/lib/cuadb -e CUADB_DATA_DIR=/var/lib/cuadb`). It is never a target database. The directory is created `0700` and the file `0600`.

## Modes
- **No administrator yet:** the local read-only explorer is open to anyone who can reach the port (startup logs a warning). Admin pages answer `403`; there is **no HTTP registration endpoint**.
- **Administrator exists:** authentication is enforced everywhere (pages, HTMX fragments, exports, `/connection/*`); only `/health`, `/static/*` and `/login` are public. Enforcement is never silently relaxed. `CUADB_AUTH=required` additionally refuses to start without an active administrator or an openable store (fail closed).

## First-run bootstrap (local operator only)
```bash
printf '%s' 'a long passphrase' | cuadb admin bootstrap --login admin --password-stdin
```
No default username/password exists. The password comes from standard input (never a flag, URL or environment variable) and is never printed or logged. Rules: login 3-64 chars of `a-z 0-9 . _ @ -` (case-insensitive); password 12-256 characters, no control characters, not containing the login. Setup is refused if any user exists or the bootstrap marker is already set (ambiguous state fails closed); it runs in a single transaction, so an interrupted run leaves nothing behind.

**Recovery** (needs filesystem access): `printf '%s' 'new passphrase' | cuadb admin reset-password --login NAME --password-stdin` resets the password, re-enables the account and revokes its sessions. `cuadb admin list-users` lists logins, roles and status.

## Users and passwords
Passwords are stored as Argon2id (19 MiB, 2 iterations, per-password random salt). Unknown logins cost the same time as wrong passwords, and unknown logins, wrong passwords and disabled accounts give the same answer. Admins create users, disable/enable them (disabling revokes all sessions; the last active administrator cannot be disabled) and reset passwords (`/admin/users`, `/admin/users/status`, `/admin/users/password`); users change their own password with `/account/password` (needs the current password; ends their other sessions).

## Sessions and CSRF
`POST /login` issues a random 256-bit opaque token in a cookie `cuadb_session` (`HttpOnly; SameSite=Strict; Path=/`, plus `Secure` unless bound to loopback or `CUADB_COOKIE_SECURE=false`); only its SHA-256 is stored. A fresh token is issued at every login (fixation-proof); `POST /logout` revokes it. Sessions expire after `CUADB_SESSION_IDLE_MINUTES` (30) idle or `CUADB_SESSION_MAX_HOURS` (12) in total. Every non-GET request must carry the session's CSRF token (form field `csrf` or header `x-csrf-token`) and a same-origin `Origin` (or no cross-site `Sec-Fetch-Site`). After 5 failed logins for a login name within 15 minutes it is locked for 15 minutes (same behaviour whether or not the login exists). Unauthenticated page loads redirect to `/login`; HTMX requests get `401` with `HX-Redirect: /login`.

## Roles and grants (deny by default)
Roles cap what grants can do: `viewer` ≤ view+export, `editor` ≤ view+export+edit+sql, `admin` unrestricted. A grant is `(user, profile, schema?, relation?, actions)`; empty schema/relation mean "any". With no matching grant everything is denied. Enforcement is on the server in every handler: lists are filtered (schemas, relations, profiles), exports and the related-table ZIP skip and do not reveal relations the user may not export, and metadata spanning relations (`/catalog/overview`, `/catalog/advanced`) needs a profile-wide / schema-wide grant. Manage grants at `/admin/grants` (a relation grant needs its schema; profile ids must exist). Database grants remain an independent boundary.

## Audit
Append-only `audit_events` (a trigger blocks UPDATE and deleting rows younger than 90 days): logins (ok/failed/locked), logout, session revocation, user and grant changes, password changes, profile access, exports, denied access (including CSRF and cross-origin refusals). Values are length-bounded with control characters removed; passwords, tokens, URLs, queries and row contents are never recorded. Administrators read it at `/admin/audit?limit=<=200&before=<id>`.

## Limitations
No UI beyond minimal unstyled forms (a UI needs an approved demo first); no password rules beyond those above; lockout is per login name, not per client address; sessions and the audit log are not replicated or exported; grants are re-read on every request, so changes apply immediately, but a session keeps working until it expires, is revoked or its user is disabled.


## Administration screen (Phase 2)

`/admin` is the browser front end for the routes above. Members: add, enable/disable, reset password, **change role** (`POST /admin/users/role`) and **delete** (`POST /admin/users/delete`). Nobody changes or deletes their own account, and the last active administrator can be neither demoted, disabled nor deleted; deleting a member removes their sessions and grants and ends their open transactions, while the audit history stays. Policy refusals (for example "password is too simple") are shown to the administrator as their static reason.

The audit log (`GET /admin/audit`) filters by `actor`, `kind`, `outcome`, `since` and `until` (`YYYY-MM-DD`, UTC). Unknown kinds or malformed dates are ignored, never echoed. Saved connections are described in `docs/connections.md`. The Security tab is read-only operator configuration.
