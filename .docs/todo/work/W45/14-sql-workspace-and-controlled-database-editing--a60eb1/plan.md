---
{id: GRP-CA8149A60EB1,name: 14 — SQL workspace and controlled database editing,type: documentation,extra: {created_at: 2026-11-04T16:14:15.288Z}}
---

# 14 — SQL workspace and controlled database editing — Plan

## Implementation approach
Implement a server-side SQL policy and capability boundary before building the editor. Use a real SQL parser for statement classification rather than regular expressions, enforce per-profile permissions and query limits, and keep result rendering bounded. Add explicit CRUD operations and transaction controls as separate typed actions. Never accept credentials or arbitrary connection targets from the browser.

## Risks and safeguards
A SQL workspace can turn a local viewer into a destructive database client. Parser bypasses, multi-statements, dangerous functions, privilege escalation, unbounded results and accidental commits are key risks. Default the feature off, require explicit operator enablement and per-user grants, rely on least-privilege database accounts and audit every mutation.

## Execution and verification
Work through the dependency order and documented delivery phases. Before editing code, inspect the task and blockers, then move only the selected task to in-progress. Complete one vertical slice with focused tests. During core backend and frontend phases, verify only PostgreSQL and SQLite; do not run broad MySQL/MariaDB compatibility matrices. Resume MySQL/MariaDB only after the frontend gate. Run full root quality commands at group boundaries, not repeatedly for every small change. Never edit Task-managed IDs or status history manually; use task comment, task check and task validate.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.
