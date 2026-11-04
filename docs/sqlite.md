# SQLite support

Set `CUADB_SQLITE_ROOT` to the directory containing files available for read-only browsing (default `/data` in Docker; point it to an existing local directory during native development) and configure `CUADB_DATABASE_URL=sqlite:////data/example.db`. CuaDB resolves both the root and target canonically, rejects missing/non-file targets, options, traversal and symlinks that escape the root, then opens SQLx with `read_only(true)` and `create_if_missing(false)`. Docker declares `/data` as the mount point.

The adapter presents SQLite's `main` schema, visible tables and views, column declarations, primary and foreign keys, indexes, and bounded offset pages. SQLite uses dynamic value typing; values are rendered by actual SQLite result type where supported. Keyset cursors, virtual-table-specific metadata and per-table row/storage estimates are not currently available and are shown honestly. Queries are read-only and bounded by the normal request/query limits.

The local fixture is `tests/fixtures/sqlite-data/cuadb_fixture_sqlite.db`; `cargo test --test sqlite_catalog` verifies confinement, metadata, paging and read-only connection behavior.
