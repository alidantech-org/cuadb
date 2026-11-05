# Changelog

All notable changes are listed here. The project follows [Semantic Versioning](https://semver.org/) and [Keep a Changelog](https://keepachangelog.com/).

## [0.1.0] - 2026-11-05

First public release.

### Added
- Browser database explorer for PostgreSQL and SQLite with a built-in demo database.
- Table browsing with typed headers, sorting, search, filters, exports (CSV, JSON, related-table ZIP), row sheet and foreign-key navigation.
- Row editing with foreign-key lookup and value suggestions; table, schema and database management dialogs with SQL preview and confirmation.
- SQL workspace: multi-statement scripts, transactions with commit/rollback, per-statement result tabs, charts and column statistics, query builder, history and saved queries.
- AI agent with schema tools, join discovery, analytics dashboards and voice input (Gemini, OpenAI, Anthropic, OpenAI-compatible).
- Trusted local mode and a permissions mode with sign-in, roles, grants, CSRF protection and an audit log.
- Saved connections with encrypted passwords, a two-step connection wizard and live connection status.
- Themes, palettes, accents, fonts, icon styles and custom controls.
- Self-contained binary (`cargo install cuadb`), command-line options, multi-arch container image on GHCR, and automated CI and release workflows.
