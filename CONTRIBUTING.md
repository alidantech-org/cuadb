# Contributing

Thanks for helping! CuaDB is a Rust (Axum, Askama) server with a small vanilla JavaScript front end built with Tailwind and Bun.

## Setup

```bash
bun install
bun run dev          # builds assets, runs the server with auto-reload of assets
```

You need Rust 1.88+ and Bun. PostgreSQL is optional; most tests run without a database and live tests skip themselves when their fixture URL is not set (see [docs/testing.md](docs/testing.md)).

## Before you open a pull request

```bash
bun run check        # prettier, rustfmt, clippy, TypeScript check
bun run test         # cargo test
bun run smoke        # end-to-end smoke tests
```

- Keep database-specific SQL and driver behaviour in `src/db/`, validation in Rust handlers, and display-only behaviour in templates and assets.
- Add or update tests for behaviour you change, and a line to [CHANGELOG.md](CHANGELOG.md) under *Unreleased*.
- Never run tests against a database you care about; use a disposable one.
- Security issues: see [SECURITY.md](SECURITY.md).

## Releases

Maintainers: see [docs/release.md](docs/release.md).

By contributing you agree that your contribution is licensed under the [MIT License](LICENSE).
