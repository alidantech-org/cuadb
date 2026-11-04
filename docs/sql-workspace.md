# SQL workspace and typed editing (backend)

Everything here is **off by default**. The read-only explorer is unchanged; writes exist only on separate authenticated, authorized, audited endpoints. There is no UI yet (a UI needs an approved demo first); the endpoints return minimal HTML fragments.

## Enabling
1. Authentication must be enforced (`docs/auth.md`): the open local principal can never run SQL or edit.
2. `CUADB_SQL_WORKSPACE=enabled` (default `disabled`; endpoints answer `404 feature_disabled` otherwise). Limits: `CUADB_SQL_MAX_ROWS` (default 500, max 5000), `CUADB_QUERY_TIMEOUT_MS` (statement deadline), `CUADB_TX_TIMEOUT_SECONDS` (60, transaction lifetime).
3. Writes additionally need a **read-write profile**: `"access": "read-write"` in the profiles file (or `CUADB_DATABASE_ACCESS=read-write` for the single URL). Such a profile gets a *separate writer pool*; the explorer keeps its read-only pool. Prefer a dedicated database role with only the write privileges you intend; database grants stay an independent boundary.
4. Per-user grants (`/admin/grants`): `sql` (run statements; profile-wide grant), `edit` (typed edits on a relation, or profile-wide for SQL writes and transactions). Roles cap grants: viewers can never `edit`/`sql`.

Supported engines: PostgreSQL and SQLite. MySQL/MariaDB parse for classification only; execution returns "not enabled" until the engine phase.

## Statement policy (`src/sql/policy.rs`)
A real SQL parser (`sqlparser`), run before the database is contacted. Exactly one statement; allowed: `SELECT` (incl. CTEs, set operations, VALUES), `EXPLAIN` of a SELECT, and `INSERT`/`UPDATE`/`DELETE` (only when the profile is read-write **and** the user has `edit`). Refused: DDL, admin/privilege statements, `COPY`, `MERGE`, `TRUNCATE`, `PRAGMA`, `ATTACH`, transaction/session control, `EXPLAIN ANALYZE`, `SELECT INTO`, locking reads, data-modifying CTEs/subqueries, statements over 20 KB, and a denylist of functions wherever they appear (files, network, extensions, sleep, advisory locks, sequence advance, `set_config`, session control). The audit summary replaces every literal with `?`.

## Execution
- `POST /sql/execute` (`sql`, optional `profile`, `tx`, `statement_id`): reads run in a **READ ONLY transaction** on the read pool with a server-side `statement_timeout` (PostgreSQL), row cap and byte cap; a side-effecting function the policy cannot see into still cannot write (tested). Writes run in their own transaction and must affect at most `CUADB_SQL_MAX_ROWS` rows, otherwise they are rolled back. Database errors are reduced to a category (`statement_rejected`, `conflict`, timeout...); statement text and driver messages are never echoed.
- `POST /sql/cancel` (`statement_id`, 32 hex chars chosen by the caller when it starts the statement): only the owner can cancel; PostgreSQL gets `pg_cancel_backend`.
- SQLite limitation: a statement cannot be interrupted, so after a timeout/cancel the caller gets the answer immediately but the worker keeps running until it finishes (the workspace uses its own single-connection pool so the explorer is not blocked).

## Typed edits
`POST /data/insert|update|delete` with `schema`, `relation`, optional `profile`/`tx`:
- `col.<column>=<value>` / `null.<column>=1` (values to write), `key.<column>=<value>` (full primary key), `old.<column>=<value>` / `oldnull.<column>=1` (previous values of every changed column), `confirm=<schema>.<relation>` (deletes).
- Identifiers must exist in the catalog; values are bound text parameters cast to the catalog type (PostgreSQL) or bound with SQLite's affinity rules; generated and `GENERATED ALWAYS` identity columns are refused, binary columns are unsupported, tables without a primary key cannot be updated/deleted. Updates and deletes affect **exactly one row** (else rollback and `409`); a changed previous value is detected as a stale row (`409`).

## Transactions
`POST /tx/begin` (profile-wide `edit`), edits and writes with `tx=<id>`, `GET /tx/status?tx=<id>` (profile, remaining time, each pending change with affected-row count, no values), `POST /tx/commit` (`tx`, `expected_changes=<n>` must equal what is pending, so unseen changes cannot be committed), `POST /tx/rollback`. At most 2 per user, one profile at a time (other-profile operations get `409` until committed or rolled back), private to their owner, expire after the TTL, and are rolled back on any failed step, logout, user disable, session expiry (a sweeper runs every 5 s) or process exit. SQLite has a single writer connection, so a second `begin` waits (bounded by the connection timeout) and then fails.

## Audit
`sql_execute`, `data_change`, `tx_begin`, `tx_commit`, `tx_rollback` events with actor, profile, outcome (`ok`, `denied`, `failed`) and a value-free summary; denied attempts by users without grants are recorded as `access_denied`. See `docs/auth.md` for retention and access.

## Limitations
No UI; MySQL/MariaDB execution not implemented; SQLite statements cannot be interrupted; PostgreSQL results are rendered via `to_json`, so column types appear as JSON (bytea as hex text, timestamps as ISO text); only whole-row-key edits (no bulk edits); the function denylist is not exhaustive (read-only transactions and database privileges are the real boundaries).
