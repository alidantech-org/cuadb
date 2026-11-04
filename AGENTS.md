# CuaDB implementation rules

CuaDB is a local-first, read-only database explorer: one Rust/Axum server, Askama/HTMX frontend, Tailwind CSS, small vanilla JavaScript, and one Docker runtime container with one published UI port. PostgreSQL is the first engine; MySQL/MariaDB and SQLite follow through typed adapters. Do not add SQL editing, write operations, a second web runtime or unrequested administrative features.

**Workspace path:** `task.yaml` configures Task at `.docs/`. Treat that file as authoritative even when the Task-generated guidance below uses the generic `tasks/` example. Read `.docs/README.md`, `.docs/rules/task-writing.md` and the selected group's goal/plan before work.

**Canonical UI constraint:** The committed `assets/cuadb-ui-demo.html` is the **required exact frontend layout and interaction baseline**, not inspiration. Before implementing any web UI, read it and `.docs/docs/frontend-interactions.md`. Preserve its sidebar, responsive shell, Simple/Advanced explorer, grid, Data/Structure/Indexes selector, inspector, settings and export surfaces. Use real, safe read-only backend data rather than the demo's fake rows or unbounded in-memory exports. Compare reference and implementation at matching browser viewports; material deviations require user approval.

**Execution order:** A one-time seed command (`bun run task:seed`) prepares the backlog through the Task CLI. Once created, `.docs/` is the single source of truth; never regenerate or hand-edit managed task IDs, status history or checkboxes. Before implementation, inspect the selected task and its blocking dependencies, move only that task to in-progress, implement the smallest complete vertical slice, and record tests and findings on that task. Run `task validate` for any Task workspace edits.

**Quality and security:** Keep database-specific SQL and driver behavior in `src/db/` adapters, route/input validation in Rust web handlers and display-only behavior in templates/assets. Use least-privilege, bounded read-only database access, safe identifier quoting, escaped output, redacted logs and explicit feature capability flags. Run `bun run format:check`, `bun run check`, and `bun run test` after implementation. Live database integration tests require disposable fixtures; never mutate the user's existing PostgreSQL data.

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