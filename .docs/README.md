---
{id: TASK-WORKSPACE-README,name: Task workspace guide,type: workspace-guide,tags: [task,agent-guidance],extra: {generated_by: task init}}
---

# Task workspace

This directory is the Task-managed workspace for cuadb. Task (`taskcode`, with `task` and `tc` aliases) is the project's tool for planning and tracking work. Humans and agents share these Markdown files; the CLI, terminal UI and local dashboard all read and write the same records.

## Layout

```
tasks/
  README.md          this guide
  todo/              task groups and tasks (one Markdown file per task)
    _completed/      groups whose tasks are all completed (YYYY/MM/YYYY-MM-DD-<slug>/)
    _archived/       archived groups and tasks (YYYY/MM/YYYY-MM-DD-<slug>/)
  docs/              project documentation
  decisions/         meaningful decisions
  handoffs/          agent and human handoff notes
```

Folders are created when first needed. The workspace location is set in `task.yaml` at the project root.

## Rules

- Write detailed, purposeful tasks and group documents: see `rules/task-writing.md`. Scaffold-only files are rejected when you try to start them.
- Every group has `goal.md` and `plan.md` (README.md optional).
- Every Markdown file in this workspace starts with YAML frontmatter that includes `id`, `name` and `type`. Unknown top-level fields are invalid; put supplemental data under `extra`.
- Ids (`TSK-…`, `GRP-…`, `DOC-…`) are stable. Never change or reuse them.
- Task status is `planned`, `in-progress`, `blocked` or `completed`. Status changes are recorded in `extra.status_history` by the tool; do not edit it by hand.
- Keep one source of truth: do not mirror tasks in a root `TODO.md` or another tracker.

## Everyday commands

| Command | Purpose |
| --- | --- |
| `task` | Workspace overview |
| `task list` | List active tasks (`--archived` for archived) |
| `task show <id>` | Show a task or document |
| `task create --title "…" --group <id> --body-file task.md` | Create a detailed task in a group (a bare create writes only a scaffold that cannot start) |
| `task group create --title "…" --goal-file goal.md --plan-file plan.md` | Create a group with its goal and plan |
| `task update <id> --status in-progress` | Change task fields |
| `task edit <id>` | Edit a task's Markdown body |
| `task start <id>` | Move to in-progress; project file changes are recorded automatically (stored outside the project, `todo/` ignored) |
| `task checklist <id>` | Show a task's checkboxes grouped by heading |
| `task check <id> --item N \| --section "Heading" \| --all` | Tick checkboxes (`task uncheck` reverses); no manual editing needed |
| `task changes <id>` / `--diff <path>` | Show files, line counts and diffs made while the task was in progress |
| `task archive <id>` / `task restore <id>` | Archive or restore a task or group |
| `task docs list` / `task docs create` | Work with documents |
| `task delete <id> --yes` | Move a task, group (`--cascade`) or document to the OS temp trash; `--force` overrides dependents |
| `task undelete <trash-id>` / `task trash list` | Restore or list deleted items (temporary: the OS may clear the trash) |
| `task search <query>` | Search metadata and text |
| `task comment <id> --text "…"` / `task comments <id>` | Add or read comments (stored under `## Comments` in the task) |
| `task move <id> --before <id> \| --after <id> \| --group <id>` | Reorder a task within its group or move it to another group |
| `task update <id> --parent <id> --link blocks:<id> --estimate N --add-watcher name --repeat week:2 --start YYYY-MM-DD` | Set hierarchy, relations, estimate, watchers, recurrence and start date (`none` clears) |
| `task attach <id> <file\|url>` / `task detach <id> <name>` | Attach small files (up to 1 MB) or links |
| `task template create --title … --body-file …` / `task create --template <id>` | Reusable task templates |
| `task bulk <id…> --status … --priority … --group …` | Change many tasks; each result is reported |
| `task activity <id>` | Timeline of status changes, comments, tracked changes and files |
| `task list --mine --watching --parent <id> --sort order` | Filter and order tasks |
| `task validate` | Validate frontmatter, links and references |
| `task run` | Start the local dashboard |
| `task tui` | Open the terminal interface |

Add `--json` for machine-readable output. Commands exit non-zero on failure. Outside an interactive terminal they require explicit arguments instead of prompting, so scripts and agents can call them safely.

## Working as an agent

1. Run `task list` and `task show <id>` before starting work; pick or create a task.
2. Move it to `in-progress` with `task update <id> --status in-progress`, and to `completed` or `blocked` when that is true.
3. Record decisions and handoffs as Markdown here rather than in chat only.
4. Run `task validate` before finishing.
