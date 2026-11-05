# Features

## Explorer
- Connection switcher with live status; saved connections with encrypted passwords; PostgreSQL databases of a server can be switched without a new connection.
- Sidebar tree: schemas, tables, views (and in advanced mode functions, sequences, types, triggers) with search, counts, sizes and per-object action menus.
- Table view: typed headers with icons, resizable and reorderable columns, sticky header, overlay scrollbar, sorting by header click, text search, filter builder, column visibility, density, compact mode, page sizes up to 1000, row estimates for large tables.
- Value rendering: booleans and enums as coloured text, dates and times, ids, code and JSON, clickable URLs, long text truncation with full view.
- Row sheet: resizable and draggable, previous/next navigation, foreign-key peek and jump, related rows.
- Structure and indexes tabs for every relation.

## Editing and management (local mode or granted permission)
- Insert, edit and delete rows; foreign keys use search-and-select; free-text columns suggest existing values.
- Create, rename, comment, empty and drop tables, schemas and databases through dialogs that preview the SQL and confirm destructive actions.
- Table, schema and database actions are also available from the sidebar menus.

## Export
- CSV and JSON for a whole relation or the current page, related-table ZIP with manifest.

## SQL workspace
- Editor with highlighting, autocomplete from the live schema, snippets, formatting, keyboard shortcuts.
- Query builder (pick tables and columns, joins, filters), examples, smart suggestions, history and saved queries in a resizable side sheet.
- One Run button: reads run directly; changes run in a transaction kept open until you **Commit** or **Roll back**; your own `BEGIN`/`COMMIT`/`ROLLBACK` is honoured; multiple statements produce one result tab each with its own summary.
- Result views: table (same engine as the explorer), chart (bar, line, area, pie), column statistics, copy as CSV/JSON.

## AI agent
- Optional chat agent beside the editor with tools to list schemas, describe tables, search columns, follow foreign keys, discover join paths, sample data (opt-in) and validate SQL with a dry run.
- Builds analytics dashboards (several queries with charts) that you can run, edit and save.
- Suggested replies, run feedback so the agent sees errors and fixes them, voice input.
- Providers: Gemini, OpenAI, Anthropic, OpenRouter and any OpenAI-compatible endpoint (including local servers). Keys stay in your browser.

## Access control
- Trusted local mode (no sign-in) or permissions mode: administrator bootstrap, users, roles, per-connection and per-relation grants, sessions with idle and absolute timeouts, rate-limited sign-in, CSRF tokens, append-only audit log, read-only database roles by default.

## Appearance and platform
- Light, dark, paper, dim and midnight; palettes, accent colours, fonts, icon styles, surfaces, density; custom select and calendar controls.
- Installable PWA, responsive shell, keyboard friendly.
- Single binary with embedded assets; multi-arch container; `cuadb healthcheck`; graceful shutdown; strict Content-Security-Policy; no telemetry.

## Engines
PostgreSQL (primary) and SQLite are fully supported. MySQL/MariaDB groundwork exists in the codebase and is not yet part of the supported feature set.
