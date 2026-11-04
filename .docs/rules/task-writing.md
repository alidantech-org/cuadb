---
{id: RULE-TASK-WRITING,name: Task writing standard,type: rule,tags: [task,agent-guidance],extra: {generated_by: task init}}
---

# Task writing standard

Task enforces this standard. A task or group document that does not meet it is flagged by `task validate`, and a planned task cannot be moved to `in-progress`, `blocked` or `completed` until it does. The policy is set under `quality` in `task.yaml` (default `mode: reject`).

## One detailed file per piece of work

- Put one piece of work in **one** task file. Do not create many tiny task files for steps; write the steps as `- [ ]` checkboxes inside the file.
- When the work has several parts, group the checkboxes under headings (for example `## Design`, `## Build`, `## Release`). The dashboard shows each heading as a titled sub-task group.
- Write for someone who has never seen the work: explain the real purpose, the context and constraints, and what "done" means. Use full sentences, not fragments.

## A task must have

1. A title heading, then a **Goal** section (or Context, Purpose, Background) saying why the work matters and the outcome.
2. A **Context** section with background, constraints, links to related documents and decisions.
3. An **Acceptance checklist** section (or Checklist, Done when, Steps) with at least 3 concrete checkboxes.
4. At least 150 words of real content, with no empty headings and no placeholder text such as TODO or TBD.

Create it in one command so it is detailed from the start:

```
task create --title "Short, specific title" --group <group-id> --body-file task.md
```

`task create` without a body writes only a scaffold; the scaffold cannot be started until it is filled in (`task edit <id>`).

## Follow-ups edit the same file

- Update the existing task instead of adding a new one: refine the text, add checkboxes, record findings under a `## Progress` or `## Notes` heading.
- Tick checkboxes as work completes, without editing the Markdown by hand: `task check <id> --item 2`, `task check <id> --section "Build"`, `task check <id> --all` (and `task uncheck`). `task checklist <id>` lists them grouped by heading.
- Move the task through `planned`, `in-progress`, `blocked` and `completed` with `task start <id>` and `task update <id> --status ...`.

## Groups need a goal and a plan

Every group has `goal.md` and `plan.md` (each at least 100 words, no empty headings); `README.md` is optional. The goal says what the group achieves and how success is measured; the plan says the approach, milestones and risks. Create them in one command:

```
task group create --title "Group name" --goal-file goal.md --plan-file plan.md
```

Tasks in a group cannot start until its goal and plan are detailed.

## If a command is rejected

The error lists every finding and how to fix it. Fix the files with `task edit <id>` (or edit the Markdown), run `task validate`, and retry. Do not weaken the policy in `task.yaml` to get around a rejection; ask the user instead.

## Change tracking

While a task is `in-progress`, Task records the project's file changes automatically in its own storage outside the project (works with or without Git). `task changes <id>` shows the files and diffs; `todo/` changes are ignored.
