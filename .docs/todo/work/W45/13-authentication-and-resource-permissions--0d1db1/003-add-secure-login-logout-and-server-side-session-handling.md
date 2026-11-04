---
{id: TSK-D91E3CD3D4C6,name: "Add secure login, logout and server-side session handling",type: task,status: completed,priority: medium,created_at: 2026-11-04T16:14:14.859Z,updated_at: 2026-11-04T16:33:01.861Z,tags: [cuadb,auth],depends_on: [TSK-F75882D3ABE8],order: 3072,links: [{type: blocks,task: TSK-FF6C70D26908}],extra: {status_history: [{from: planned,to: completed,at: 2026-11-04T16:33:01.861Z}]}}
---

# Add secure login, logout and server-side session handling

## Goal
Implement authenticated browser sessions with predictable expiry, rotation and logout behavior. Protect state-changing requests against CSRF and prevent session fixation, cookie leakage and brute-force login attempts. Keep the existing local disconnected shell usable when authentication is not yet configured.

## Context
The application will eventually support profile switching and controlled SQL operations. Authentication must be enforced consistently for full-page routes, HTMX fragments, downloads and future APIs. Prefer opaque server-side sessions over long-lived browser-readable tokens.

This task belongs to **13 — Authentication and resource permissions**. The group outcome is: Add a secure local administrator bootstrap, user accounts, sessions and deny-by-default permissions before CuaDB exposes protected multi-user or write-capable features. Authorization must be scoped to trusted connection profiles and database resources.

## Implementation boundaries
CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Implement login and logout with opaque server-side session identifiers.
- [x] Set secure, HttpOnly, SameSite cookies with configurable expiry and rotation.
- [x] Add CSRF protection to state-changing requests and rate-limit failed logins.
- [x] Ensure protected routes and downloads reject missing, expired or revoked sessions.
- [x] Test session fixation, logout, expiry, cookie flags and CSRF failures.

## Verification and handoff
Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.

## Comments

### 2026-11-04T16:33:00.880Z · PETER IRUNGU

VERIFIED. Opaque 256-bit tokens (SHA-256 stored), cookie HttpOnly/SameSite=Strict/Path=/ (+Secure by policy), idle 30 min and absolute 12 h expiry (configurable), rotation on every login, logout and disable revoke, CSRF token (form or header) plus Origin/Sec-Fetch-Site on all non-GET, login lockout (5 failures/15 min, same for unknown logins), middleware protects pages, fragments (401 + HX-Redirect), exports. tests/auth_http.rs. Limitations: lockout is per login name not client address; plain HTTP so run behind TLS before non-loopback use.
