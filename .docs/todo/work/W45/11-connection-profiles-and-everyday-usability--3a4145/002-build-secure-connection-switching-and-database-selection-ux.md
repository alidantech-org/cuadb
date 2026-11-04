---
{id: TSK-4C7273B9E46A,name: Build secure connection switching and database selection UX,type: task,status: planned,priority: medium,created_at: 2026-11-04T07:32:08.583Z,updated_at: 2026-11-04T16:15:04.087Z,tags: [cuadb,profiles],depends_on: [TSK-B92248F7876E,TSK-61DA50E7C274],order: 2048,links: [{type: blocks,task: TSK-E5355EF46139}]}
---

# Build secure connection switching and database selection UX

## Goal
Offer an understandable connection picker, accurately update the schema tree and selected relation, and avoid retaining stale rows from a previously selected database. Keep navigation responsive without storing secrets on the client.

## Context
Switching connections changes the database trust boundary, catalog and permissions. HTMX requests must be tied to a stable configured profile ID and not accept arbitrary database hosts. Browser navigation should not display records from another profile after rapid switching or a failed connection.

This task belongs to **11 — Connection profiles and everyday usability**. The group outcome is: Allow safe switching among multiple known database connections while keeping credentials out of the browser and eliminating the need to repeatedly reconfigure the viewer. Improve the everyday explorer experience with durable, non-sensitive preferences and sensible keyboard shortcuts.

## Implementation boundaries
CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.

## Acceptance checklist
- [x] Render available trusted profile labels with clear connected and offline states.
- [ ] Reset or scope table selection, sorting and cursors when a profile changes.
- [x] Reject unknown or removed profile identifiers without exposing connection details.
- [x] Test rapid switching and inaccessible databases for stale response leakage.
- [x] Document the connection selection behavior in the README.

## Canonical UI fidelity

The committed `assets/cuadb-ui-demo.html` is the binding reference. Implement the exact applicable control positions, popup states, theme/size options and behavior as required by `.docs/docs/frontend-interactions.md`. Demo profiles and records are illustrative only; real credentials stay server-side and real data is fetched by safe, read-only Rust endpoints. Do not treat visual similarity alone as completion if an existing demo control does not work.

### Reference-specific acceptance
- [ ] Match the demo's bottom-of-sidebar connection dropdown position, selectable rows, active checkmark, disabled 'New connection' placeholder (until securely supported), and transient feedback toast.
- [ ] When switching trusted profiles, update sidebar, schema-qualified heading, selected relation, pagination and inspector without leaking stale rows from another connection.
- [ ] Preserve the demo's responsive dropdown and click-away behavior, rejecting unknown profiles or arbitrary browser connection URLs.

## Verification and handoff
Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.

## Updated demo amendment (2026-11-05)
The owner updated `assets/cuadb-ui-demo.html` (login and roles, database switcher, connection create/edit/test modal, Administration view, typed row editor and SQL workspace) and decided that Phase 2 is open (`.docs/docs/backend-gate-decision.md`). The demo remains the binding look, feel and density baseline, but it may be **improved** where that makes the product better; every departure is recorded in a Task comment. Production rules that override the demo: the Content-Security-Policy allows no inline `<script>`, `style=` or `on*=` handlers (use classes, `data-*` attributes and CSS custom properties), every state-changing request carries the CSRF token, and every screen is built from real, permission-scoped backend data. Acceptance is checked manually in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, side by side with the demo, and boxes stay unticked until that comparison passes.

### Amendment acceptance
- [ ] The connection picker lists only profiles the principal may see and the database picker shows per-database access tags; both are server-driven, switching clears selection and transaction state, and no raw URLs or passwords reach the browser.
- [ ] Admins can open `Manage connections...` from the picker (implemented by the connection-management task in Group 15).

## Comments

### 2026-11-04T14:16:26.776Z · PETER IRUNGU

BACKEND PARTIAL - FRONTEND PENDING. Items 1,3,4,5 verified: GET /connection/profiles (id,label,engine,state, no URLs/paths), unknown/removed/malformed ids -> 404/400 without details, 40 concurrent mixed-profile requests never mix rows and an offline profile fails alone, README section. Pagination cursors are rejected across profiles (live PG, profiles_http.rs). Item 2 left open: server links keep the profile, but resetting selected relation/sort in the page on switch is client behavior (Phase B). Reference items 6-8 are UI.
