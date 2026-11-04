/**
 * One-time CuaDB roadmap bootstrap.
 *
 * IMPORTANT: The Task CLI owns IDs, status history and YAML frontmatter.
 * This script never hand-writes managed records in .docs/. It passes detailed
 * Markdown bodies to `task group create`, `task create` and `task docs create`.
 *
 * Usage:
 *   bun run task:preview  # inspect only, no Task installation required
 *   bun run task:seed     # create records in .docs/, then task validate
 *   bun run task:repair   # repair first bootstrap's group goals via Task CLI
 *   bun run task:verify   # validate existing Task workspace
 */
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { documents, groups } from './spec';
import type { GroupSpec, TaskSpec } from './types';
import { groupGoal, LEGACY_GOAL_PARAGRAPH } from './goals';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

type TaskRecord = { id: string; name: string; path?: string; status?: string };
type GroupRecord = { id: string; name: string; path?: string };
type DocRecord = { id: string; name: string; path: string };

function words(value: string): number {
  return value.match(/[\p{L}\p{N}]+(?:[-'][\p{L}\p{N}]+)*/gu)?.length ?? 0;
}

function groupPlan(group: GroupSpec): string {
  return [
    `# ${group.title} — Plan`,
    '',
    '## Implementation approach',
    group.approach,
    '',
    '## Risks and safeguards',
    group.risks,
    '',
    '## Execution and verification',
    'Work through the dependency order and documented delivery phases. Before editing code, inspect the task and blockers, then move only the selected task to in-progress. Complete one vertical slice with focused tests. During core backend and frontend phases, verify only PostgreSQL and SQLite; do not run broad MySQL/MariaDB compatibility matrices. Resume MySQL/MariaDB only after the frontend gate. Run full root quality commands at group boundaries, not repeatedly for every small change. Never edit Task-managed IDs or status history manually; use task comment, task check and task validate.',
    '',
    '## Deliverables',
    'The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.',
  ].join('\n');
}

function taskBody(group: GroupSpec, task: TaskSpec): string {
  const checklist = task.checks.map((step) => `- [ ] ${step}`).join('\n');
  return [
    `# ${task.title}`,
    '',
    '## Goal',
    task.goal,
    '',
    '## Context',
    task.context,
    '',
    `This task belongs to **${group.title}**. The group outcome is: ${group.goal}`,
    '',
    '## Implementation boundaries',
    'CuaDB is a local-first database workspace served by one Rust/Axum process. The core explorer remains read-only. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and never expose credentials or arbitrary connection targets to browser state. Do not add data writes or SQL execution outside the later dedicated SQL workspace tasks, which require the authentication and resource-permission group first. Keep every query bounded by reasonable row, time and memory limits.',
    '',
    '## Acceptance checklist',
    checklist,
    '',
    '## Verification and handoff',
    'Verify the expected result and realistic failure cases with the smallest relevant test set. Use disposable PostgreSQL and SQLite fixtures for the core milestone; do not add MySQL/MariaDB or other engines to routine verification. Run `bun run format:check`, `bun run check` and `bun run test` at meaningful group boundaries. Do not test against a personal or production database. Document unsupported capabilities honestly, record concise evidence in a Task comment, tick accepted items with `task check` and only then mark the task completed.',
  ].join('\n');
}

function validateSpecs(): void {
  const groupKeys = new Set<string>();
  const taskKeys = new Set<string>();
  const titles = new Set<string>();

  for (const group of groups) {
    if (groupKeys.has(group.key)) throw new Error(`Duplicate group seed key: ${group.key}`);
    groupKeys.add(group.key);

    const goalText = groupGoal(group);
    const planText = groupPlan(group);
    if (words(goalText) < 100 || words(planText) < 100) {
      throw new Error(`Group quality threshold not met: ${group.key}`);
    }

    for (const task of group.tasks) {
      if (taskKeys.has(task.key)) throw new Error(`Duplicate task seed key: ${task.key}`);
      if (titles.has(task.title)) throw new Error(`Duplicate task title: ${task.title}`);
      if (task.checks.length < 3 || words(taskBody(group, task)) < 150) {
        throw new Error(`Task quality threshold not met: ${task.key}`);
      }
      for (const dependency of task.dependsOn ?? []) {
        if (!taskKeys.has(dependency)) {
          throw new Error(`Unknown/forward dependency ${dependency} in ${task.key}`);
        }
      }
      taskKeys.add(task.key);
      titles.add(task.title);
    }
  }
  const docPaths = new Set<string>();
  for (const doc of documents) {
    if (docPaths.has(doc.path)) throw new Error(`Duplicate Task document: ${doc.path}`);
    docPaths.add(doc.path);
    if (words(doc.body) < 150) throw new Error(`Thin Task document: ${doc.path}`);
  }
}

function commandAvailable(executable: string): boolean {
  const probe = spawnSync(executable, ['--version'], { cwd: root, encoding: 'utf8' });
  return !probe.error && probe.status === 0;
}

function taskCommand(): string {
  const command = ['taskcode', 'task'].find(commandAvailable);
  if (!command) {
    throw new Error(
      'Task CLI not found. Install the taskcode executable, confirm "task --version", then retry. Do not run task init: this repository already has task.yaml.',
    );
  }
  return command;
}

function taskRun(executable: string, args: string[], env: NodeJS.ProcessEnv = process.env): string {
  const processResult = spawnSync(executable, args, {
    cwd: root,
    env,
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (processResult.error || processResult.status !== 0) {
    throw new Error(
      `Task command failed: ${executable} ${args
        .filter((a) => !a.includes('\n'))
        .slice(0, 9)
        .join(' ')}\n${processResult.stderr || processResult.error || processResult.stdout}`,
    );
  }
  return processResult.stdout;
}

function asArray<T>(value: unknown, action: string): T[] {
  if (!Array.isArray(value)) throw new Error(`Unexpected Task JSON for ${action}`);
  return value as T[];
}

function taskJson<T>(executable: string, args: string[]): T {
  const output = taskRun(executable, [...args, '--json']);
  try {
    return JSON.parse(output) as T;
  } catch {
    throw new Error(`Could not parse Task JSON from: ${args.join(' ')}`);
  }
}

function validateWorkspace(): void {
  const config = readFileSync(join(root, 'task.yaml'), 'utf8');
  if (!/^workspace:\s*["']?\.docs["']?\s*$/m.test(config)) {
    throw new Error(
      'Expected task.yaml to select workspace: ".docs". Review config before seeding.',
    );
  }
}

/** Save only temporary body files; Task CLI creates the actual managed Markdown. */
function withTextFile<T>(
  directory: string,
  filename: string,
  body: string,
  callback: (path: string) => T,
): T {
  const filepath = join(directory, filename);
  writeFileSync(filepath, body, 'utf8');
  return callback(filepath);
}

function preview(): void {
  console.log('CuaDB Task roadmap preview (read-only)');
  console.log(
    `Workspace: .docs/ | ${groups.length} groups | ${groups.reduce((count, group) => count + group.tasks.length, 0)} tasks | ${documents.length} documents`,
  );
  for (const group of groups) {
    console.log(`\n${group.title} [${group.key}]`);
    for (const task of group.tasks) {
      console.log(
        `  - ${task.title}${task.dependsOn?.length ? ` (needs: ${task.dependsOn.join(', ')})` : ''}`,
      );
    }
  }
  console.log('\nNo Task workspace records were created or changed.');
}

/**
 * Repair ONLY the known, legacy boilerplate in group goal bodies.
 *
 * Task CLI's `docs open` passes a temporary body-only file to the editor and
 * writes it back through its managed-document API. IDs and YAML metadata are
 * neither read into our editor nor hand-written by this code.
 */
function repairExistingGroupGoals(executable: string): number {
  const savedGroups = asArray<GroupRecord>(taskJson(executable, ['group', 'list']), 'group list');
  const byName = new Map(savedGroups.map((group) => [group.name, group]));
  const workspaceRoot = resolve(root, '.docs');
  const approvedRoot = resolve(workspaceRoot, 'todo') + sep;
  let repaired = 0;

  for (const group of groups) {
    const existing = byName.get(group.title);
    if (!existing) continue;
    if (!existing.path) throw new Error(`Task group ${existing.id} has no filesystem path`);

    const goalPath = resolve(workspaceRoot, existing.path, 'goal.md');
    if (!goalPath.startsWith(approvedRoot)) {
      throw new Error(`Unexpected Task group path for ${existing.id}; refusing repair`);
    }
    if (!existsSync(goalPath)) throw new Error(`Missing goal document for ${existing.id}`);

    const current = readFileSync(goalPath, 'utf8');
    if (!current.includes(LEGACY_GOAL_PARAGRAPH)) continue;

    // No in-place frontmatter edits: use the Task document editor operation.
    const editorPath = resolve(root, 'scripts/roadmap/repair-goal-editor.ts');
    const editorCommand = `bun "${editorPath.replaceAll('"', '\\"')}"`;
    taskRun(executable, ['docs', 'open', `GOAL-${existing.id}`], {
      ...process.env,
      EDITOR: editorCommand,
      CUADB_REPAIR_GROUP: group.key,
    });

    if (readFileSync(goalPath, 'utf8').includes(LEGACY_GOAL_PARAGRAPH)) {
      throw new Error(`Task CLI did not persist repaired goal for ${existing.id}`);
    }
    console.log(`Repaired group goal: ${existing.name}`);
    repaired++;
  }
  return repaired;
}

function reconcilePhasePolicy(executable: string, taskIds: Map<string, string>): void {
  const active = asArray<TaskRecord>(taskJson(executable, ['list']), 'list');
  const archived = asArray<TaskRecord>(
    taskJson(executable, ['list', '--archived']),
    'list --archived',
  );
  const records = [...active, ...archived];
  const byId = new Map(records.map((task) => [task.id, task]));
  const workspaceRoot = resolve(root, '.docs');

  const pauseGroup = (groupKey: string, reason: string): void => {
    const group = groups.find((item) => item.key === groupKey);
    for (const task of group?.tasks ?? []) {
      const id = taskIds.get(task.key);
      const record = id ? byId.get(id) : undefined;
      if (record?.status === 'in-progress') {
        taskRun(executable, ['update', id!, '--status', 'blocked']);
        taskRun(executable, ['comment', id!, '--text', reason]);
        record.status = 'blocked';
        console.log(`Paused phase-conflicting task: ${record.name} (${id})`);
      }
    }
  };

  const allCompleted = (keys: string[]): boolean =>
    keys.every((key) =>
      (groups.find((group) => group.key === key)?.tasks ?? []).every((task) => {
        const id = taskIds.get(task.key);
        return Boolean(id && byId.get(id)?.status === 'completed');
      }),
    );

  // Group 05 carries UI-only acceptance items, so 'all tasks completed' can never be true
  // before the frontend phase. The owner records the gate decision as a document instead
  // (docs/backend-gate-decision.md); the gate is open when that document exists.
  const gateDecisionRecorded = asArray<DocRecord>(
    taskJson(executable, ['docs', 'list']),
    'docs list',
  ).some(
    (doc) =>
      doc.path === 'docs/backend-gate-decision.md' ||
      doc.path === '.docs/docs/backend-gate-decision.md',
  );
  const backendReady =
    gateDecisionRecorded ||
    allCompleted([
      'foundation',
      'contracts',
      'postgres-catalog',
      'postgres-rows',
      'http',
      'sqlite',
    ]);
  if (!backendReady) {
    pauseGroup(
      'explorer-ui',
      'Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.',
    );
    pauseGroup(
      'data-grid-ui',
      'Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.',
    );
    pauseGroup(
      'inspectors',
      'Paused by the CuaDB phase gate: complete PostgreSQL + SQLite backend work and typed HTTP/fragment contracts before continuing visual frontend implementation. Preserve this task work and resume after the backend gate is accepted.',
    );
  }

  const frontendReady = allCompleted(['explorer-ui', 'data-grid-ui', 'inspectors']);
  if (!frontendReady) {
    pauseGroup(
      'mysql',
      'Paused by the CuaDB phase gate: preserve completed MySQL/MariaDB groundwork and resume remaining engine-specific work only after the PostgreSQL + SQLite backend and canonical frontend milestones are accepted. Do not spend core-milestone time on cross-engine verification.',
    );
  }

  // Task CLI owns dependency metadata. Add missing links without editing
  // managed frontmatter; skip completed and currently active tasks.
  let linksAdded = 0;
  for (const group of groups) {
    for (const task of group.tasks) {
      if (!task.dependsOn?.length) continue;
      const taskId = taskIds.get(task.key);
      const record = taskId ? byId.get(taskId) : undefined;
      if (!taskId || !record || record.status === 'completed' || record.status === 'in-progress')
        continue;
      if (!record.path) throw new Error(`Task ${taskId} has no path for dependency reconciliation`);
      const relative = record.path.startsWith('.docs/') ? record.path.slice(6) : record.path;
      const filepath = resolve(workspaceRoot, relative);
      const current = readFileSync(filepath, 'utf8');
      const match = current.match(/depends_on:\s*\[([^\]]*)\]/);
      const existing = new Set(
        [...(match?.[1] ?? '').matchAll(/TSK-[A-F0-9]+/g)].map((item) => item[0]),
      );
      for (const dependencyKey of task.dependsOn) {
        const dependencyId = taskIds.get(dependencyKey);
        if (!dependencyId) throw new Error(`Missing dependency task id for ${dependencyKey}`);
        if (existing.has(dependencyId)) continue;
        taskRun(executable, ['update', dependencyId, '--link', `blocks:${taskId}`]);
        existing.add(dependencyId);
        linksAdded++;
      }
    }
  }
  console.log(`Reconciled ${linksAdded} missing Task dependency links.`);
}

function seed(): void {
  validateWorkspace();
  const executable = taskCommand();
  const tempDirectory = mkdtempSync(join(tmpdir(), 'cuadb-task-seed-'));
  let newDocuments = 0;
  let newGroups = 0;
  let newTasks = 0;

  try {
    const existingDocs = asArray<DocRecord>(taskJson(executable, ['docs', 'list']), 'docs list');
    const docPaths = new Set(existingDocs.map((doc) => doc.path));
    for (const doc of documents) {
      if (docPaths.has(doc.path) || docPaths.has(`.docs/${doc.path}`)) {
        console.log(`Already exists: ${doc.path}`);
        continue;
      }
      taskJson<DocRecord>(executable, [
        'docs',
        'create',
        '--title',
        doc.title,
        '--path',
        doc.path,
        '--body',
        doc.body,
        '--tag',
        'cuadb,architecture',
      ]);
      newDocuments++;
      console.log(`Created document: ${doc.path}`);
    }

    const existingGroups = asArray<GroupRecord>(
      taskJson(executable, ['group', 'list']),
      'group list',
    );
    const groupsByName = new Map(existingGroups.map((group) => [group.name, group.id]));

    // Read both active and archived names so rerunning never creates duplicates.
    const active = asArray<TaskRecord>(taskJson(executable, ['list']), 'list');
    const archived = asArray<TaskRecord>(
      taskJson(executable, ['list', '--archived']),
      'list --archived',
    );
    const tasksByName = new Map([...archived, ...active].map((task) => [task.name, task.id]));
    const taskIds = new Map<string, string>();

    for (const group of groups) {
      let groupId = groupsByName.get(group.title);
      if (!groupId) {
        const slug = group.key.replace(/[^a-z0-9-]/g, '-');
        const created = withTextFile(
          tempDirectory,
          `${slug}-goal.md`,
          groupGoal(group),
          (goalFile) =>
            withTextFile(tempDirectory, `${slug}-plan.md`, groupPlan(group), (planFile) =>
              taskJson<GroupRecord>(executable, [
                'group',
                'create',
                '--title',
                group.title,
                '--goal-file',
                goalFile,
                '--plan-file',
                planFile,
              ]),
            ),
        );
        groupId = created.id;
        groupsByName.set(group.title, groupId);
        newGroups++;
        console.log(`Created group: ${group.title} (${groupId})`);
      } else {
        console.log(`Existing group: ${group.title} (${groupId})`);
      }

      for (const task of group.tasks) {
        const existingId = tasksByName.get(task.title);
        if (existingId) {
          taskIds.set(task.key, existingId);
          console.log(`  Existing: ${task.title}`);
          continue;
        }

        const dependencies = (task.dependsOn ?? []).map((key) => {
          const id = taskIds.get(key);
          if (!id) throw new Error(`Dependency ${key} not created before ${task.key}`);
          return id;
        });
        const args = [
          'create',
          '--title',
          task.title,
          '--group',
          groupId,
          '--priority',
          group.key === 'foundation' || group.key === 'contracts' ? 'high' : 'medium',
          '--tag',
          `cuadb,${group.key}`,
        ];
        if (dependencies.length > 0) args.push('--depends-on', dependencies.join(','));
        const created = withTextFile(
          tempDirectory,
          `${task.key}.md`,
          taskBody(group, task),
          (bodyFile) => taskJson<TaskRecord>(executable, [...args, '--body-file', bodyFile]),
        );
        taskIds.set(task.key, created.id);
        tasksByName.set(task.title, created.id);
        newTasks++;
        console.log(`  Created: ${created.id} ${task.title}`);
      }
    }

    reconcilePhasePolicy(executable, taskIds);
    const repaired = repairExistingGroupGoals(executable);
    taskRun(executable, ['validate']);
    console.log(
      `\nTask workspace validated. Added ${newGroups} groups, ${newTasks} tasks and ${newDocuments} documents; repaired ${repaired} existing group goals.\nReview: task group list; task list; task docs list; git status`,
    );
  } finally {
    rmSync(tempDirectory, { force: true, recursive: true });
  }
}

function repair(): void {
  validateWorkspace();
  const executable = taskCommand();
  const repaired = repairExistingGroupGoals(executable);
  console.log(taskRun(executable, ['validate']));
  console.log(`Task workspace validated; repaired ${repaired} generated group goals.`);
}

function verify(): void {
  validateWorkspace();
  const executable = taskCommand();
  const savedGroups = asArray<GroupRecord>(taskJson(executable, ['group', 'list']), 'group list');
  const savedTasks = [
    ...asArray<TaskRecord>(taskJson(executable, ['list']), 'list'),
    ...asArray<TaskRecord>(taskJson(executable, ['list', '--archived']), 'list --archived'),
  ];
  const savedDocs = asArray<DocRecord>(taskJson(executable, ['docs', 'list']), 'docs list');
  const groupNames = new Set(savedGroups.map((group) => group.name));
  const taskNames = new Set(savedTasks.map((task) => task.name));
  const docPaths = new Set(
    savedDocs.map((doc) => (doc.path.startsWith('.docs/') ? doc.path.slice(6) : doc.path)),
  );
  const missingGroups = groups.filter((group) => !groupNames.has(group.title));
  const missingTasks = groups
    .flatMap((group) => group.tasks)
    .filter((task) => !taskNames.has(task.title));
  const missingDocs = documents.filter((doc) => !docPaths.has(doc.path));

  console.log(taskRun(executable, ['validate']));
  if (missingGroups.length || missingTasks.length || missingDocs.length) {
    throw new Error(
      `Roadmap is incomplete: missing ${missingGroups.length} groups, ${missingTasks.length} tasks, ${missingDocs.length} documents. Run bun run task:seed.`,
    );
  }
  console.log(
    `Task roadmap verified: ${groups.length} groups, ${groups.reduce((count, group) => count + group.tasks.length, 0)} tasks, ${documents.length} documents.`,
  );
}

try {
  validateSpecs();
  const action = process.argv[2] ?? '--preview';
  if (action === '--preview') preview();
  else if (action === '--repair') repair();
  else if (action === '--verify') verify();
  else if (action === '--seed') seed();
  else throw new Error('Usage: bun scripts/roadmap/seed.ts --preview|--seed|--repair|--verify');
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
