---
{id: GOAL-GRP-F7DADB0D1DB1,name: 13 — Authentication and resource permissions goal,type: documentation,extra: {created_at: 2026-11-04T16:14:14.389Z,group: GRP-F7DADB0D1DB1}}
---

# 13 — Authentication and resource permissions — Goal

## Outcome
Add a secure local administrator bootstrap, user accounts, sessions and deny-by-default permissions before CuaDB exposes protected multi-user or write-capable features. Authorization must be scoped to trusted connection profiles and database resources.

## Definition of done
Acceptance of **13 — Authentication and resource permissions** requires evidence for these results:
1. **Establish secure first-run administrator bootstrap:** Provide an explicit and safe first-run process for creating the initial CuaDB administrator. Do not ship default credentials, expose an unauthenticated registration endpoint or store bootstrap secrets in the browser. The operator must be able to complete setup locally and then disable the bootstrap path. Verification includes: Implement a one-time administrator bootstrap with no built-in username/password pair.
2. **Implement secure user accounts and password storage:** Create the minimal user model and account lifecycle required for multiple people to use CuaDB safely. Store only strong password hashes, normalize unique login identifiers and make account state explicit. Keep account management separate from database credentials and connection profiles. Verification includes: Define user identity, status, role assignment and unique login identifier models.
3. **Add secure login, logout and server-side session handling:** Implement authenticated browser sessions with predictable expiry, rotation and logout behavior. Protect state-changing requests against CSRF and prevent session fixation, cookie leakage and brute-force login attempts. Keep the existing local disconnected shell usable when authentication is not yet configured. Verification includes: Implement login and logout with opaque server-side session identifiers.
4. **Enforce role and resource permissions across profiles and data:** Add deny-by-default authorization for administrators, read-only users and explicitly permitted editors. Permissions must scope users to trusted connection profiles, schemas, relations and allowed actions so one user cannot browse or modify another user’s resources. Verification includes: Define roles and explicit view, export, edit and SQL-execution permissions.
5. **Record security events and administrator access changes:** Provide a minimal audit trail for security-sensitive actions so operators can understand who accessed CuaDB and who changed permissions. Record login outcomes, session revocation, profile access and permission changes without storing passwords, connection URLs, SQL secrets or database row contents. Verification includes: Record authentication outcomes, session revocations and permission changes.
Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.

## Boundaries
Do not implement adjacent groups speculatively. Keep changes modular, add tests that run without live database access when possible, and document fixture-only validation clearly. Preserve the existing Bun commands and local-only Docker defaults. Record architectural decisions and compatibility limits in the Task workspace; never create a competing task tracker.
