# Saved connections (Administration → Connections)

Administrators can add, edit, test and remove connections from the browser. Connections defined in the server configuration (`CUADB_DATABASE_URL` or `CUADB_PROFILES_FILE`) stay read-only in the UI and are shown as "server configuration".

## Encryption key

Passwords are sealed with **AES-256-GCM** before they are stored in CuaDB's own state database (`CUADB_DATA_DIR/cuadb.db`). The key comes only from the environment:

```bash
openssl rand -hex 32          # 64 hex characters
export CUADB_SECRET_KEY=<the value>   # or keep it in an untracked .env file
```

- The key is never stored, logged, rendered or sent to the browser. `.env` is git-ignored; `.env.example` only documents the variable.
- Each value is bound to its connection id: copying a stored value to another record makes it undecryptable.
- Without the key the Connections tab explains how to set one and refuses to save. If saved connections exist and the key is missing or different, **startup fails** with a message that names the variable but no secret. Losing the key means the saved connections must be entered again.
- Rotating the key: set the new key, re-enter each connection's password (a blank password on edit keeps the stored one, which cannot be read with a different key).

## What the browser can do

| Action | Notes |
|---|---|
| New / edit | Name, engine (PostgreSQL or SQLite), host, port, user, password, database, TLS mode (verify certificate, encrypt only, off for loopback) or a file name inside `CUADB_SQLITE_ROOT`; "Allow changes" adds the writer pool used by the SQL workspace and row editor. A blank password on edit keeps the saved one. |
| Test | Opens a throw-away connection with a 10 s limit and reports only "Connected in N ms" or a category such as "authentication failed". Nothing is saved. |
| Remove | Closes the pools, rolls back open transactions on it and deletes its permissions. File-defined connections and the last remaining connection cannot be removed. |

Every change writes a `connection_changed` audit event (created, updated, removed, test ok/failed) with the connection id and no URL, host or password. All routes are administrator-only and CSRF-protected.

## Limits

- At most 16 connections in total (file-defined plus saved). Saved connections use at most a quarter of the pool budget each.
- MySQL / MariaDB are not offered in the dialog until their frontend phase.
- Hosts are chosen by administrators, who are trusted; the form still validates host characters, ports and TLS values, and SQLite files must resolve inside `CUADB_SQLITE_ROOT`.
