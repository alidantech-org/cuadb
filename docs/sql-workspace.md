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


## Browser screens

`GET /sql` renders the editor (Ctrl/Cmd+Enter runs, Cancel stops a running read) for users who hold a profile-wide `sql` grant while `CUADB_SQL_WORKSPACE=enabled`; the sidebar entry appears only for them. The pending-change bar mirrors `/tx/status` and commits with the reviewed `expected_changes`. In the data grid, **Edit / Delete / + Add row** appear only when the profile is writable, the user holds `edit` on that relation and the relation has a primary key visible in the grid; edits are single statements (not part of a transaction). The browser never builds SQL: it posts typed values to `/data/*`, with `old.*` values so a concurrent change is reported as a conflict.

## Local mode and object actions

`CUADB_LOCAL_MODE` (`auto` by default, or `true` / `false`) marks the instance as a trusted local one. With `auto` that is any server bound to a loopback address (the default `127.0.0.1`); Docker and other non-loopback binds stay strict. In local mode:

- The caller is a built-in administrator without sign-in, with full access to every connection that allows changes. Connections added in the UI default to *Allow changes*; the bundled demo stays read-only.
- `CUADB_SQL_WORKSPACE` defaults to enabled (set it to `disabled` to turn it off).
- The ⋯ menu on tables and schemas opens the object-action dialog (rename, add/rename/drop column, comment, empty, drop, create table) with a live preview of the statement; destructive actions need the object's name typed. A **Run SQL** tab accepts arbitrary SQL (including DDL) and exists only in local mode.
- The SQL editor adds syntax highlighting, autocomplete from the catalog (Ctrl+Space), snippets, a formatter and a query builder.

Outside local mode the strict rules above apply unchanged: sign-in, per-profile grants, parser-checked statements, and no free-form DDL. The dialog's structured actions still require a profile-wide `sql` grant on a writable connection.

### Scripts, transactions and database actions (local mode)

- The workspace accepts any number of statements separated by `;` (quotes, comments and `$$` bodies are respected) and any statement type, including DDL. **Run as** chooses `Auto` (several statements run as one transaction), `One transaction` (all or nothing) or `One by one` (each commits on its own; stops at the first error and keeps what already ran).
- A script that contains its own `BEGIN`/`COMMIT`/`ROLLBACK` runs exactly as written; statements outside an explicit transaction commit immediately, and a script that ends inside an open transaction is rolled back. `SAVEPOINT`, `RELEASE` and `ROLLBACK TO` pass through.
- Every statement's result or the database's own error message is shown in order.
- The ⋯ menu on a database (database selector), plus **＋ Create database** and **＋ New schema**, open the same dialog with *New schema*, *New database* and *Drop database* tabs (PostgreSQL). Create/drop database run on the server's `postgres` maintenance database outside a transaction.

## SQL workspace layout, tools and AI

The workspace is a header (Query builder, Examples, History, AI, Format), the code box, an output header (summary, Table/Chart/Stats views, filter, *Run as*, Run) and the output area, which stays empty until something runs.

- **Side sheet:** *Builder* writes the query into the editor as you change options (table, columns or an aggregate with group by, match all/any conditions, order, limit); *Examples* lists suggestions drawn from the editor and the schema (latest rows, counts, duplicates, joins on `*_id` columns) plus engine examples; *History* keeps your recent queries and ones you star, in this browser only.
- **Output:** results can be sorted by clicking a header and filtered by text; *Chart* draws bar, line, area or pie from any numeric columns; *Stats* shows filled/null/distinct counts, min/max/avg and the most common value per column.
- **AI assistant (optional):** add a Google Gemini API key under Settings → AI assistant (or in the sheet). The key is stored in this browser and sent only to `generativelanguage.googleapis.com` — the Content-Security-Policy `connect-src` allows that single extra host. Row data is never sent; table and column names are, unless you turn that off. It writes, explains and optimises queries, offers *Fix with AI* on errors, and *✦ Ask AI* on a table opens the workspace with that table.

### One Run button (local mode)

There is a single **Run**. Reads, however many statements, run directly. As soon as a script contains a statement that changes data or schema (INSERT, UPDATE, DELETE, DDL …), the whole script runs inside one transaction that **stays open**: the transaction bar shows the staged changes with **Commit** and **Roll back**, later runs join the same transaction (and see its uncommitted changes), and the transaction rolls back by itself after 15 minutes. A script with its own `BEGIN`/`COMMIT`/`ROLLBACK` runs exactly as written. In permissions mode the *Begin transaction* button remains for grouping single statements.

The AI assistant supports Google Gemini, OpenAI, Anthropic and any OpenAI-compatible endpoint (for example Ollama on `localhost`); provider and model can be changed at any time from the AI tab, and any model id can be typed. The Content-Security-Policy allows those provider hosts and `localhost` for `connect-src` only.

## Output tabs and AI agent

Each statement of a run gets its own tab (between the Run header and a per-output summary strip) and its result is shown in the same table as the explorer: typed headers and icons, resizable columns, sticky header, overlay scrollbar, boolean/date/code/link cells, click-to-sort, a text filter, row details and Table/Chart/Stats views. The active output fills all the remaining height.

The ✦ AI tab is a chat agent with tools: `list_schemas`, `list_tables`, `describe_table` (columns, keys, foreign keys), `search_schema`, `validate_sql` (dry run with `EXPLAIN`), `sample_rows` (off unless enabled in AI settings), `get_last_run` and `get_editor_sql`. It validates a query before showing it, is told whether a query you run passed or failed, offers suggested replies, and keeps chat history per connection in this browser. Models with tool calling use it natively; if a model rejects tools the agent switches to a text protocol automatically (badge *Tools* / *Text mode*, overridable under Settings → AI assistant → Agent mode).

### Agent v2: relationships, analytics dashboards, voice

- **Explore relationships:** `find_relationships` (what a table points to and what points to it), `find_join_path` (a chain of foreign-key joins between any two tables, across schemas and through intermediate tables, with the ready `FROM … JOIN … ON …` hint) and `suggest_joins` (inferred joins from matching column names and types when no foreign key links two tables; flagged as inferred). `analytics_ideas` returns ready SQL for counts, trends and breakdowns of a table.
- **Dashboards in the chat:** the agent can answer with an `analytics` block. The app draws it as a dashboard: stat tiles, bar/line/area/pie charts and tables, each card runnable on its own or with *Run all*, with *Open in editor*, *Copy SQL*, *Interpret* and *Fix*. The cards run in your browser, so the agent does not need to read your data. **Save** keeps a dashboard under History → Analyses so you can reopen and re-run it.
- **Data-aware mode (off by default):** Settings → AI assistant → *Let the agent read up to 5 sample rows* also enables `profile_column` and `run_query` (a single read-only SELECT, up to 20 rows) so the agent can check values and interpret results. With it off the agent works only from schema and the queries you run.
- **Voice and composer:** dictate with the microphone button (browser speech recognition, shown only where supported); quick chips add your editor query or last error to the message.
- **Layout:** the tool sheet (Builder, Examples, History, AI) can be widened by dragging its left edge; double-click resets it.
