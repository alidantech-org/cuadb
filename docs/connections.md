# Saved connections

Visitors can add, edit, test, reconnect and remove saved connections in the browser. The form accepts a PostgreSQL URL or individual fields, and supports SQLite files under `CUADB_SQLITE_ROOT`. The built-in Demo database is always present and cannot be removed. Connections defined in the server configuration (`CUADB_DATABASE_URL` or `CUADB_PROFILES_FILE`) stay read-only in the UI and are shown as "server configuration".

## Encryption key

Passwords are sealed with **AES-256-GCM** before they are stored in CuaDB's own state database (`CUADB_DATA_DIR/cuadb.db`). Set `CUADB_SECRET_KEY` to manage the encryption key yourself; otherwise CuaDB creates a persistent owner-only `connection.key` in `CUADB_DATA_DIR`.

```bash
openssl rand -hex 32          # 64 hex characters
export CUADB_SECRET_KEY=<the value>   # or keep it in an untracked .env file
```

- The key is never logged, rendered or sent to the browser. The generated key is stored separately from the database with owner-only filesystem permissions.
- Each value is bound to its connection id: copying a stored value to another record makes it undecryptable.
- If saved connections exist and the configured or generated key is missing or different, **startup fails** without exposing a secret. Losing the key means the saved connections must be entered again.
- Rotating the key: set the new key, re-enter each connection's password (a blank password on edit keeps the stored one, which cannot be read with a different key).

## What the browser can do

| Action | Notes |
|---|---|
| New / edit | Name, engine (PostgreSQL or SQLite), host, port, user, password, database, TLS mode or a SQLite file; "Allow changes" controls database writes. A blank password on edit keeps the saved one. |
| Test | Opens a throw-away connection with a 10 s limit. **Test only** leaves it unsaved; **Test and save** saves it after a successful connection. |
| Reconnect | Rebuilds the connection pool and refreshes the selected catalog. Switching connections reconnects the new selection and closes the previous one. |
| Remove | Closes the pools, rolls back open transactions on it and deletes its permissions. File-defined connections and the built-in demo cannot be removed. |
| PostgreSQL lifecycle | SQL-capable visitors can create/drop databases and drop schemas/tables. The UI asks for the exact target name; schema and table drops cascade. Database deletion requires PostgreSQL support for `DROP DATABASE ... WITH (FORCE)`. |

Every change writes a `connection_changed` audit event with the connection id and no URL, host or password. Connection management and full access to managed profiles are public on any bind, including after admin setup. These connections are shared by visitors; the write toggle controls database writes. Unsafe browser requests must be same-origin.

## Limits

- At most 16 connections in total (file-defined plus saved). Saved connections use at most a quarter of the pool budget each.
- MySQL / MariaDB are not offered in the dialog until their frontend phase.
- Connection details are shared by all visitors; the form validates host characters, ports and TLS values, and SQLite files must resolve inside `CUADB_SQLITE_ROOT`.
