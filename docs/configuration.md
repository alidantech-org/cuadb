# Configuration

CuaDB is configured with command-line flags (see [cli.md](cli.md)) and environment variables. Flags win over variables. Invalid values stop the server at startup with a message naming the variable.

## Server

| Variable | Default | Notes |
|---|---|---|
| `CUADB_HOST` | `127.0.0.1` | Bind address. The container image sets `0.0.0.0`. |
| `CUADB_PORT` | `4848` | |
| `CUADB_DATA_DIR` | OS data folder (`~/.local/share/cuadb`, `~/Library/Application Support/cuadb`, `%APPDATA%\cuadb`); `./data` in development | Users, sessions, saved connections, audit log. The image uses `/data/app-state`. |
| `CUADB_SQLITE_ROOT` | `<data dir>/sqlite` | SQLite files are only opened from this folder. The image uses `/data/sqlite`. |
| `CUADB_SECRET_KEY` | generated and stored in the data dir | 64 hex characters; encrypts saved connection passwords. Back it up if you set it. |
| `CUADB_SHUTDOWN_GRACE_MS` | `10000` | Time allowed for in-flight requests on shutdown. |

## Modes and access

| Variable | Default | Notes |
|---|---|---|
| `CUADB_LOCAL_MODE` | `auto` | `true`: trusted local mode, full access, no sign-in. `false`: permissions mode. `auto`: local mode only when bound to a loopback address. |
| `CUADB_AUTH` | follows mode | Authentication behaviour override for advanced setups. |
| `CUADB_SQL_WORKSPACE` | `enabled` in local mode, `disabled` otherwise | Turns the SQL workspace and typed editing on or off. |
| `CUADB_COOKIE_SECURE` | `auto` | Set `true` when serving over HTTPS. |
| `CUADB_SESSION_IDLE_MINUTES` | `30` (1-1440) | |
| `CUADB_SESSION_MAX_HOURS` | `12` (1-168) | |

## Connections

| Variable | Default | Notes |
|---|---|---|
| `CUADB_DATABASE_URL` | none | One connection defined by the server (`postgresql://…` or a SQLite path). |
| `CUADB_PROFILES_FILE` | none | JSON file with several connections ([example](profiles.example.json)). Cannot be combined with `CUADB_DATABASE_URL`. |
| `CUADB_DATABASE_ACCESS` | `read-only` | `read-write` adds a writer pool for the single `CUADB_DATABASE_URL` connection. |

Connections added in the UI are stored in the data directory with encrypted passwords and need none of these.

## Limits

| Variable | Default | Range |
|---|---|---|
| `CUADB_CONNECTION_TIMEOUT_MS` | `5000` | 100-60000 |
| `CUADB_QUERY_TIMEOUT_MS` | `15000` | 100-120000 |
| `CUADB_REQUEST_TIMEOUT_MS` | `30000` | 500-180000 |
| `CUADB_DEFAULT_PAGE_SIZE` | `100` | 1-1000 |
| `CUADB_MAX_PAGE_SIZE` | `1000` | 1-1000 |
| `CUADB_MAX_POOL_CONNECTIONS` | per engine | 1-512 |
| `CUADB_MAX_IN_FLIGHT_REQUESTS` | `512` | 1-512 |
| `CUADB_MAX_REQUEST_BODY_BYTES` | `65536` | 1024-1048576 |
| `CUADB_SQL_MAX_ROWS` | `500` | 1-5000 rows kept per result |
| `CUADB_TX_TIMEOUT_SECONDS` | `60` | 5-600, idle open transactions are rolled back |

## Browser-side settings

Theme, palette, fonts, density and the AI provider, model and API key are stored in your browser only (`localStorage`). They are never sent to the CuaDB server. The AI key is sent only to the provider you configured.
