# PostgreSQL connection and read-only security

CuaDB is a **read-only browser explorer**, not a SQL console. The connection
is configured by the operator on the **Rust server**, not by an arbitrary
HTTP request. In this milestone the application probes PostgreSQL and displays
a safe connection state. Catalog/table navigation is a separate group.

## Prerequisites

- A PostgreSQL database (tested target: PostgreSQL 18 / PostGIS 3.6).
- An operator-created **least-privilege login role** that can only inspect the
  intended schemas and SELECT permitted tables/views.
- Rust stable, Bun and a fresh generated Cargo.lock after adding SQLx. Run
  `bun run deps:sync` once after pulling this milestone, then commit the
  resulting lockfile. Normal builds run with `--locked`.

Do **not** run database role-creation instructions against an existing
production instance without reviewing grants with its administrator.
The SQL below is **setup guidance**, not executed by CuaDB.

## Example dedicated reader role

In an intentionally chosen database, connect with a separate administrative
account and adjust names and passwords. Do not reuse the application's
write-capable credentials.

```sql
-- Run as an administrator; replace database and role identifiers.
CREATE ROLE cuadb_viewer LOGIN PASSWORD 'USE_A_UNIQUE_SECRET';
GRANT CONNECT ON DATABASE exampledb TO cuadb_viewer;

-- Connect to exampledb before the following grants.
GRANT USAGE ON SCHEMA public TO cuadb_viewer;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO cuadb_viewer;

-- If you want future objects readable, run as/grant for each owner separately:
-- ALTER DEFAULT PRIVILEGES FOR ROLE app_owner IN SCHEMA public
--   GRANT SELECT ON TABLES TO cuadb_viewer;

-- Optional extra session-level defense; the application also enforces it.
ALTER ROLE cuadb_viewer SET default_transaction_read_only = on;
```

Grant USAGE and SELECT only for the schemas you intend to expose, not all
application data. For other schemas repeat with explicit, reviewed grants.
View definitions and security-definer functions can expose information outside
normal grants; review them. PostgreSQL RLS may hide rows or behave differently
with owner/security-definer views. Metadata visibility depends on privileges.
CuaDB never bypasses access controls and never creates database objects.

PostgreSQL's read-only transaction default is a **secondary defense**:
it can be overridden by privileged session code, and special temporary
objects or functions require separate attention. A restricted reader account
is the real security boundary. The current HTTP application exposes no
arbitrary SQL, editing, migration or function-execution endpoint.

## Configure a connection

Set `CUADB_DATABASE_URL` as a **process environment variable**. The parser
accepts only URLs of the form:

```text
postgresql://username:percent-encoded-password@host:5432/database?sslmode=require
postgres://username:percent-encoded-password@host:5432/database?sslmode=verify-full
```

Host, username, password and database are **required**. `sslmode` supports:
`require`, `verify-ca`, `verify-full`, and **explicit local-network-only**
`disable`. **`verify-full` is the default** when omitted; it verifies the
server hostname and certificate chain. `require` only encrypts traffic and
does not verify hostname identity; avoid it on untrusted networks.
`sslrootcert` is an optional operator-selected file path. The validator
rejects ambiguous/duplicate parameters and any URL `options` field that could
change session settings. Use URL percent encoding for reserved password
characters such as `@`, `#`, `?`, `%`, `/` and `:`.

On your Fedora host, if the PostgreSQL container publishes host port 5432
to loopback:

```bash
# Enter URL interactively without writing it into shell history:
read -rsp 'CUADB_DATABASE_URL: ' CUADB_DATABASE_URL; echo
export CUADB_DATABASE_URL
bun run dev
curl -i http://127.0.0.1:4848/health
curl -i http://127.0.0.1:4848/connection/status
```

Use `@127.0.0.1:5432` in the URL for host-to-container connections.
Inside a separate Docker container **127.0.0.1 means CuaDB itself**.
For container-to-container networking, use a shared user-defined network and
the database container's name, such as `@postgis:5432`:

```bash
docker network create cuadb-local
docker network connect cuadb-local postgis

# Export CUADB_DATABASE_URL beforehand, with host 'postgis' and
# an explicit local-network sslmode=disable if PostGIS lacks TLS.
docker run --rm --name cuadb \
  --network cuadb-local \
  --env CUADB_DATABASE_URL \
  -p 127.0.0.1:4848:4848 cuadb:local
```

The named network and existing `postgis` container do not become part of
the CuaDB image. The host exposes **only the CuaDB UI port**; PostgreSQL
continues running independently. Avoid putting passwords directly in
command-line arguments; Docker environment variables are still visible to
authorized Docker administrators. For custom root CAs, mount a read-only
certificate path into the container and use the corresponding URL setting.

## Runtime behavior and limits

- Missing URL: **Not configured**; the shell and `/health` still work.
- Valid URL: one **lazy** SQLx PostgreSQL pool, default maximum four
  connections, zero minimum, 120-second idle timeout, 30-minute lifetime.
- Every new pooled connection executes session read-only initialization and
  sets PostgreSQL statement and lock timeouts from the bounded configuration.
- On startup a single asynchronous probe checks read-only state and whether
  at least one non-system schema is usable; subsequent status requests reuse
  that result for eight seconds before retrying. A ready status is **not** a
  promise of SELECT access to every table.
- Invalid URLs fail startup with static, redacted validation errors; wrong
  credentials, insufficient permissions and unavailable hosts do not prevent
  the HTTP listener from running.
- The `/connection/status` endpoint sends a safe HTML fragment, never a URL,
  password, host or database identifier. The header indicator refreshes using
  HTMX. The endpoint returns `Cache-Control: no-store`.
- `/health` checks **only the Rust server** and never waits for PostgreSQL.
- On shutdown the application closes the database pool after HTTP shutdown,
  within a bounded deadline.

`CUADB_CONNECTION_TIMEOUT_MS` (default 5000), `CUADB_QUERY_TIMEOUT_MS`
(default 15000), and `CUADB_MAX_POOL_CONNECTIONS` (default 4) control the
connection layer. A failed probe logs a **static error code**, not the driver
error. To simulate an unavailable database, stop a **disposable** fixture or
provide a nonexistent local port; status will recover after the retry interval.

## Offline and opt-in tests

All standard `bun run test` tests work without a database; they check URL
validation, redaction, fake adapters, no-URL state, network failures and HTTP
status fragments. Real engine tests run **only when explicitly configured**:

```bash
# Use a disposable fixture database, NEVER a developer's real dataset.
export CUADB_TEST_POSTGRES_URL='postgresql://viewer:...@127.0.0.1:5432/fixture?sslmode=disable'
cargo test --locked --test postgres_connection
```

That fixture must use password authentication (prefer SCRAM-SHA-256), a
dedicated SELECT-only reader, and at least one schema with USAGE. The
wrong-password test expects PostgreSQL SQLSTATE 28P01. For the separate
insufficient-permissions test, set `CUADB_TEST_DENIED_POSTGRES_URL` to a
dedicated fixture login with no USAGE on user schemas. Restrict `public`
grants in the **disposable** fixture if needed. The tests do not execute any
INSERT, UPDATE, DELETE, DDL, or schema creation.

Do not point these tests at the existing personal `postgis` database.
Run live fixtures separately from unit tests; the database catalog and
paged row tests are upcoming tasks.
