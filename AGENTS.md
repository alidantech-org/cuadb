# CuaDB implementation rules

CuaDB is a local-first database explorer and database workspace built as one Rust/Axum server, Askama/HTMX frontend, Tailwind CSS, small vanilla JavaScript and one Docker runtime container with one published UI port. The currently delivered explorer is read-only. Any future data editing or SQL execution must be implemented only through the dedicated authenticated and permission-controlled roadmap groups; never weaken the current browsing safety boundary to shortcut those features.

**Workspace path:** `task.yaml` selects `.docs/`. Treat it as authoritative. Read `.docs/README.md`, `.docs/rules/task-writing.md`, the execution roadmap and the selected group's goal/plan before work.

**Branch policy:** All work is committed directly to the existing `main` branch. Do not create branches, worktrees or pull requests. Do not force-push. Keep commits focused and preserve existing user work.

**Database priority and verification budget:**
- PostgreSQL is the primary engine. SQLite is the second required engine and is already implemented in the current core roadmap.
- During core backend work, PostgreSQL and SQLite are the only engines that must be exercised. Use disposable fixtures and focused tests for the feature being implemented.
- Do not spend time on broad MySQL/MariaDB catalog, data-type, statistics, export or compatibility matrices during the core milestone. Preserve already-completed MySQL/MariaDB groundwork; do not revert it. Pause remaining MySQL/MariaDB work until the complete frontend milestone has been accepted.
- After the frontend milestone, finish only the MySQL/MariaDB gaps that are actually needed. Do not add any other database engines.
- A focused PostgreSQL + SQLite success is sufficient to proceed. Do not repeat the same checks across several engines or run exhaustive database permutations for every task.

**Execution order — follow the phase gate, not the historical group numbers:**
1. **Backend:** finish Rust foundation, engine-neutral contracts, PostgreSQL connections/catalog/rows, typed HTTP routes and HTML-fragment contracts, plus SQLite adapter and backend acceptance. No visual frontend implementation in this phase.
2. **Frontend:** only after the PostgreSQL + SQLite backend gate passes, complete the canonical explorer, grid, inspector, structure and index interfaces.
3. **Additional engines:** resume MySQL/MariaDB compatibility only after the frontend milestone.
4. **Multi-connection:** complete trusted connection profiles and switching after the core engines and frontend are stable.
5. **Authentication:** implement administrator bootstrap, users, sessions and resource permissions before exposing protected capabilities.
6. **SQL workspace and editing:** implement only after authentication and permission controls. Keep it separate from the read-only explorer and disabled unless explicitly authorized.
7. **Release:** complete security, integration, performance, Docker and release documentation last.

Group numbers are stable identifiers, not execution order. The authoritative phase order and dependencies are in `.docs/docs/execution-roadmap.md`. If an older in-progress task conflicts with this phase policy, stop at a safe boundary, preserve its work and update its status/comments through the Task CLI before moving to the current phase.

**Canonical UI constraint:** `assets/cuadb-ui-demo.html` is the required exact frontend layout and interaction baseline, not inspiration. Read it and `.docs/docs/frontend-interactions.md` before Phase 2. Preserve its sidebar, responsive shell, Simple/Advanced explorer, grid, Data/Structure/Indexes selector, inspector, settings and export surfaces. Use real backend data, not fake rows or unbounded in-memory exports. Perform focused visual comparison during the frontend milestone; do not spend backend time on visual polish.

**Vertical-slice execution:** Use the Task workspace as the single source of truth. Inspect the selected task and its blockers, move only that task to in-progress, implement the smallest complete vertical slice, add targeted tests, record findings and tick checklists through the Task CLI. Prefer finishing one normal feature end to end over broad scaffolding. Run the root quality commands at meaningful group boundaries; do not repeatedly run every database or browser scenario after each small change. Run `task validate` after Task workspace changes.

**Quality and security:** Keep database-specific SQL and driver behavior in `src/db/` adapters, route/input validation in Rust handlers and display-only behavior in templates/assets. The core explorer uses least-privilege read-only database access, bounded queries, safe identifier quoting, escaped output, redacted logs and explicit capability flags. Future write/SQL capabilities require authenticated users, explicit per-resource permissions, audit events and dedicated acceptance tests. Never mutate a user's existing database during tests.

---

<!-- BEGIN TASK MANAGED TASK-TOOL -->
## Task workspace

This project manages its work with Task (`taskcode`; `task` and `tc` are aliases). Task records, documents, decisions and handoffs live in `tasks/` as Markdown files with YAML frontmatter. Read `tasks/README.md` for how the tool works.

- **Write detailed tasks.** Task rejects scaffold-only or thin files: one task file per piece of work, with a Goal, Context and an Acceptance checklist of at least a few `- [ ]` steps ("task create --body-file"). Groups need detailed `goal.md` and `plan.md`. Read `tasks/rules/task-writing.md` before creating work, and update the same file in follow-ups.
- Discuss and organise work on the task itself: `task comment <id> --text "…"` (authors come from `TASK_AUTHOR`, `identity.name` in `task.yaml` or git), `task move <id> --before|--after <id>` to reorder, `task update <id> --parent … --link blocks:TSK-… --add-watcher name --estimate 3 --repeat week`, `task attach <id> <file|url>` (files up to 1 MB, links for anything larger), `task template create|list` and `task create --template <id>`.
- Tick checkboxes with `task check <id> --item N | --section "Heading" | --all` instead of hand-editing.
- Create and change tasks through the CLI (`task create`, `task update <id> --status in-progress`) rather than keeping a separate TODO list. Do not hand-edit `id`, `status_history` or other managed frontmatter.
- Use `--json` for machine-readable output; commands exit non-zero on failure, and outside an interactive terminal they require explicit arguments instead of prompting.
- Moving a task to `in-progress` records the project's file changes automatically (with or without Git); `task changes <id>` shows what the task changed.
- Deleting is recoverable only briefly: `task delete <id> --yes` moves items to the OS temp folder; restore with `task undelete <trash-id>`. Prefer `task archive` for work that should be kept.
- Run `task validate` after editing anything inside `tasks/`. Every Markdown file there needs valid frontmatter.
- Record meaningful decisions in `tasks/decisions/` and agent handoffs in `tasks/handoffs/`.
<!-- END TASK MANAGED TASK-TOOL -->