---
{id: DOC-D166315C8869,name: "Owner decision: PostgreSQL + SQLite backend gate",type: documentation,tags: [cuadb,decision]}
---

# Owner decision: PostgreSQL + SQLite backend gate

**Decision:** the backend acceptance gate defined in the execution roadmap is accepted for PostgreSQL and SQLite. Frontend implementation may begin when the owner starts Phase 2.

## Evidence (commits on main)
- Groups 01-04 (foundation, contracts, PostgreSQL catalog and rows) and Group 10 (SQLite) are completed in Task.
- Group 05 backend: typed read routes, stale-response sync, bounded CSV/JSON/ZIP export, failure contracts (9ef5bde, c00fa7d, 1c9d8e1).
- Groups 06-08 backend: relation filter, sort/paging, structure, rich values, PostGIS, live PostgreSQL tests (8de9ab5, 6c783a3, ec25e6e).
- SQLite adapter and four-engine contract test (61a8013); connection profiles backend (67fd267); security, performance and Docker verification (e6c8a35).
- Full detail is in the Task comments on each task.

## Explicitly deferred to Phase 2 (not ticked, not faked)
Acceptance items that need the browser: demo-region template mapping, Simple/Advanced rendering, popovers/toasts, in-region error styling, local preference persistence, visual regression, keyboard and accessibility checks, and stale-response behavior in a real browser. Their tasks remain open and are paused by the seed until the owner starts the frontend phase.

## Dependencies
Group 13/14 backend tasks are not blocked by UI-only criteria on profiles-switch (TSK-4C7273B9E46A) or grid-keyboard. The SQL workspace UI task stays blocked until a UI demo is approved.