---
{id: GOAL-GRP-FE4B6079185A,name: 01 — Rust foundation and observability goal,type: documentation,extra: {created_at: 2026-11-04T07:32:04.103Z,group: GRP-FE4B6079185A}}
---

---
{id: GOAL-GRP-FE4B6079185A,name: 01 — Rust foundation and observability goal,type: documentation,extra: {created_at: 2026-11-04T07:32:04.103Z,group: GRP-FE4B6079185A}}
---

# 01 — Rust foundation and observability — Goal

## Outcome
Turn the verified application shell into a maintainable Rust service with controlled configuration, predictable errors, observable request behavior and reproducible quality gates. Preserve the current single binary, single Docker runtime and local-only deployment assumptions. This group does not connect to live databases; its deliverables make later adapters safe to introduce without leaking details into HTTP handlers.

## Definition of done
Acceptance of **01 — Rust foundation and observability** requires evidence for these results:
1. **Harden server configuration and startup lifecycle:** Define typed, validated settings for host, port, deadlines, maximum page size and optional connection configuration while preserving the existing zero-credential shell startup. Invalid values should fail fast with actionable, redacted errors instead of panicking or silently selecting dangerous defaults. Verification includes: Extract typed configuration validation with explicit defaults, bounds and tests for malformed values.
2. **Introduce typed application errors and structured request tracing:** Create a consistent error boundary for database, validation, rendering and transient infrastructure failures. Operators should diagnose problems from safe logs while browsers receive stable status codes and understandable HTML responses without stack traces. Verification includes: Define a focused application error type and HTTP status mapping with no sensitive debug text.
3. **Implement graceful shutdown and bounded request behavior:** Ensure CuaDB exits cleanly, closes outstanding work and does not hang indefinitely during HTTP or database operations. Keep the service resource footprint predictable for laptop use and single-container deployment. Verification includes: Handle process termination signals and let the Axum server shut down gracefully.
4. **Make all root quality commands reproducible and testable:** Make formatting, checking, unit tests, asset compilation and Docker image verification consistent across Fedora development and fresh clones. Keep Bun as a task runner rather than introducing a second application server. Verification includes: Verify root dev, check, build, start, format and test scripts with documented prerequisites.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.
