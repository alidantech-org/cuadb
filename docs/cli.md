# Command line

```text
cuadb [OPTIONS]
cuadb admin <bootstrap|reset-password|list-users> [--login NAME] [--password-stdin]
cuadb healthcheck
```

## Install

```bash
cargo install cuadb              # from crates.io, Rust 1.88+
cargo install --path .           # from a checkout (run `bun run assets:build` first)
```

Or download a prebuilt archive for Linux, macOS or Windows from the [GitHub releases](https://github.com/alidantech-org/cuadb/releases). The web UI is embedded in the binary: nothing else needs to be installed.

## Options

| Flag | Variable | Meaning |
|---|---|---|
| `-p, --port <PORT>` | `CUADB_PORT` | Port (default 4848) |
| `--host <IP>` | `CUADB_HOST` | Bind address (default 127.0.0.1) |
| `--data-dir <DIR>` | `CUADB_DATA_DIR` | State folder |
| `--sqlite-root <DIR>` | `CUADB_SQLITE_ROOT` | Folder SQLite files are opened from |
| `--database-url <URL>` | `CUADB_DATABASE_URL` | Add a connection at startup |
| `--profiles-file <FILE>` | `CUADB_PROFILES_FILE` | Several connections from JSON |
| `--local` / `--no-local` | `CUADB_LOCAL_MODE` | Force trusted local mode or permissions mode |
| `--open` | | Open the browser when ready |
| `-V, --version`, `-h, --help` | | |

## Examples

```bash
cuadb --open                                   # local, with the demo database
cuadb --database-url postgresql://me:pw@localhost/app --open
cuadb --sqlite-root ~/databases                # browse SQLite files in a folder
```

## Sharing safely

```bash
cuadb --host 0.0.0.0 --no-local                # permissions mode
printf 'a-long-password' | cuadb admin bootstrap --login admin --password-stdin
```

Put it behind HTTPS and read [auth.md](auth.md) first.

## Health check

`cuadb healthcheck` requests `/health` on the configured port and exits 0 when the server answers. The container image uses it for its `HEALTHCHECK`.
