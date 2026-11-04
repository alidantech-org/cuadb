---
{id: TSK-91DF6AB91FA3,name: Add real connection profile management with encrypted secrets and a test endpoint,type: task,status: planned,priority: high,created_at: 2026-11-04T23:18:10.763Z,updated_at: 2026-11-04T23:18:10.763Z,tags: [cuadb,phase2-ui],order: 3072}
---

# Add real connection profile management with encrypted secrets and a test endpoint

## Goal
Let administrators create, edit, test and delete connection profiles at runtime without ever exposing stored secrets to the browser.

## Context
Profiles are currently static, loaded from `CUADB_PROFILES_FILE` or `CUADB_DATABASE_URL` (`src/profiles.rs`, `src/config.rs`), and config rejects setting both. The demo expects a modal with name, engine (PostgreSQL and SQLite), host or file path, user, password and default database, a Test connection button and a Remove action. The backend needs a persisted profile store that merges with file-defined profiles (file-defined ones are read-only in the UI), AES-GCM or equivalent encryption for passwords using an operator-supplied key whose source must be agreed with the owner before it is wired, a test endpoint with a short timeout that reports only a safe status, and audit events for every change. Blank password on edit keeps the stored secret, secrets are never logged, and SQLite paths must be confined to the configured data directory.

This task belongs to **15 — Authenticated application UI, administration and connection management**. The look, feel and density come from `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. No inline scripts or styles are allowed by the CSP, and acceptance is a manual side-by-side browser comparison plus focused PostgreSQL and SQLite Rust tests; MySQL and MariaDB are out of scope until the frontend milestone.

## Acceptance checklist
- [ ] Agree the encryption key source with the owner, document it in `.env.example` and fail closed when profiles with secrets exist but the key is missing.
- [ ] Implement the profile store with create, update and delete, merged with file-defined profiles, admin-only, CSRF-protected and audited.
- [ ] Add a bounded test-connection endpoint that returns only success, failure category and latency, with tests for PostgreSQL and SQLite including path confinement.
- [ ] Prove secrets never appear in responses, logs or the audit log with a negative test, and update `docs/` accordingly.

## Verification and handoff
Run the focused tests for this slice, run `bun run format:check` and `bun run check` at the group boundary, add a Task comment with the evidence and any deliberate departure from the demo, tick accepted items with `task check`, and only then mark the task completed.
