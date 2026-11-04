# SQLite support

Set `CUADB_SQLITE_ROOT` to the directory containing files available for read-only browsing (default `/data` in Docker; point it to an existing local directory during native development) and configure `CUADB_DATABASE_URL=sqlite:////data/example.db`.

## Path rules

CuaDB canonicalizes the root and the target, then rejects: missing files, directories, URL options (`?mode=rwc`), fragments, NUL bytes, `sqlite::memory:`/`file:` forms, `..` traversal, and symlinks whose final target is outside the root (a symlink to another file inside the root is allowed and resolves to its real path). Error messages and the connection status never include filesystem paths. The file is opened with `read_only(true)`, `create_if_missing(false)` and `PRAGMA query_only=ON`; there is no ATTACH, SQL or schema-modification endpoint, and only the `main` schema is exposed.

## What is listed

Ordinary tables, views and virtual tables (`CREATE VIRTUAL TABLE`, shown as `VirtualTable`); virtual-table shadow tables and `sqlite_*` internals are hidden. Columns include declared type (or `dynamic` when undeclared), primary key, defaults and generated columns (`VIRTUAL` / `STORED`). Foreign keys and indexes are listed. There is no partitioning (the list is empty) and no per-table row or storage statistics (shown as unavailable).

## Rows and dynamic typing

SQLite stores any value in any column. Cells use the *actual* storage class; when it differs from what the declared affinity predicts, the type label says so (for example `INTEGER · text`, `dynamic · integer`). Text and blob cells are truncated inside SQLite (257 characters/bytes fetched) so a 1 MiB blob never reaches the server process; the true length is shown (`1048576 bytes (first 257 shown)`). Paging is offset-based (100 pages × 100 rows). Order is deterministic: primary key, else `rowid`; `WITHOUT ROWID` tables use their primary key; views and virtual tables have no stable order and report that. Sorting puts NULLs last in both directions and ties break on the same key. The row filter is a literal substring of every column's text.

## Docker, read-only mounts and WAL

Mount database files read-only: `-v "$PWD/data:/data:ro"`. Tests emulate a `:ro` bind mount (read-only directory and file) and verify that a rollback-journal database browses cleanly, that the file bytes are unchanged and that no `-journal`/`-wal`/`-shm` files are created.

- A WAL-mode database that was closed cleanly (no `-wal` file, checkpointed) opened from a read-only mount works in tests (SQLite 3.4x via SQLx).
- A WAL database with a live writer needs its `-wal`/`-shm` files readable **and** the directory writable for shared memory; on a strictly read-only mount that may fail closed (status "Unavailable"). Checkpoint the database (`PRAGMA wal_checkpoint(TRUNCATE)`) or copy a snapshot before mounting it. This case is not covered by automated tests.
- Do not mount the live database of a running application into CuaDB; browse a copy.

## Tests

`cargo test --test sqlite_live` builds throw-away databases under `target/tmp` and always runs (no external service). `cargo test --test sqlite_catalog` uses the committed fixture `tests/fixtures/sqlite-data/cuadb_fixture_sqlite.db`. A running-container check of the `:ro` mount is part of the Group 12 Docker verification.
