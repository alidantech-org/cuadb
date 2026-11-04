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
 *   bun run task:verify   # validate existing Task workspace
 */
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { documents, groups } from './spec';
import type { GroupSpec, TaskSpec } from './types';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');

type TaskRecord = { id: string; name: string; path?: string };
type GroupRecord = { id: string; name: string; path?: string };
type DocRecord = { id: string; name: string; path: string };

function words(value: string): number {
  return value.match(/[\p{L}\p{N}]+(?:[-'][\p{L}\p{N}]+)*/gu)?.length ?? 0;
}

/** Task quality policy: detailed goal/plan and at least three real acceptance steps. */
function groupGoal(group: GroupSpec): string {
  return [
    `# ${group.title} — Goal`,
    '',
    '## Outcome',
    group.goal,
    '',
    '## Definition of done',
    'This group is complete when its implemented behavior is verifiable through the documented checks, not merely when files exist. Functionality must remain read-only and obey the shared database capability contract. All error states need truthful, usable messages and no credential disclosures. Work should be integrated with the existing single-process Rust application and browser shell, without introducing a separate runtime service. The group tasks describe individual deliverables and must be completed through Task checklists rather than a parallel root TODO file.',
    '',
    '## Boundaries',
    'Do not implement adjacent groups speculatively. Keep changes modular, add tests that can run without a live database when possible, and document any fixture-only validation explicitly. Preserve the existing Bun root commands and localhost deployment assumptions. Record decisions and unexpected compatibility limits in the Task workspace.'
  ].join('\n');
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
    'Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.',
    '',
    '## Deliverables',
    'The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.'
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
    'CuaDB is a local-first, read-only database viewer served by one Rust/Axum process. Keep database-specific behavior inside adapter modules, use typed HTTP request and view models, and avoid exposing arbitrary SQL or credentials to browser state. Before adding a dependency or abstraction, check whether the existing shell and adapter contract provide a simpler solution. Never introduce unsafe write operations as a shortcut for inspection, and keep every query bounded by reasonable row, time and memory limits.',
    '',
    '## Acceptance checklist',
    checklist,
    '',
    '## Verification and handoff',
    'Verify both the expected result and realistic failure cases. Run `bun run format:check`, `bun run check` and `bun run test`, plus targeted integration checks when this task touches live database behavior. Tests using real databases must run against disposable fixtures, not a personal or production database. If there are unsupported data types, catalog permissions, engine versions or pagination limits, document them instead of masking the limitation. Add a Task comment summarizing implementation evidence, test commands and unresolved risks; tick accepted checklist items with `task check` and only then mark the task completed.'
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
  const command = ['task', 'taskcode'].find(commandAvailable);
  if (!command) {
    throw new Error(
      'Task CLI not found. Install the taskcode executable, confirm "task --version", then retry. Do not run task init: this repository already has task.yaml.'
    );
  }
  return command;
}

function taskRun(executable: string, args: string[]): string {
  const processResult = spawnSync(executable, args, {
    cwd: root,
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (processResult.error || processResult.status !== 0) {
    throw new Error(
      `Task command failed: ${executable} ${args.filter((a) => !a.includes('\n')).slice(0, 9).join(' ')}\n${processResult.stderr || processResult.error || processResult.stdout}`
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
    throw new Error('Expected task.yaml to select workspace: ".docs". Review config before seeding.');
  }
}

/** Save only temporary body files; Task CLI creates the actual managed Markdown. */
function withTextFile<T>(directory: string, filename: string, body: string, callback: (path: string) => T): T {
  const filepath = join(directory, filename);
  writeFileSync(filepath, body, 'utf8');
  return callback(filepath);
}

function preview(): void {
  console.log('CuaDB Task roadmap preview (read-only)');
  console.log(`Workspace: .docs/ | ${groups.length} groups | ${groups.reduce((count, group) => count + group.tasks.length, 0)} tasks | ${documents.length} documents`);
  for (const group of groups) {
    console.log(`\n${group.title} [${group.key}]`);
    for (const task of group.tasks) {
      console.log(`  - ${task.title}${task.dependsOn?.length ? ` (needs: ${task.dependsOn.join(', ')})` : ''}`);
    }
  }
  console.log('\nNo Task workspace records were created or changed.');
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
        'docs', 'create', '--title', doc.title,
        '--path', doc.path, '--body', doc.body,
        '--tag', 'cuadb,architecture',
      ]);
      newDocuments++;
      console.log(`Created document: ${doc.path}`);
    }

    const existingGroups = asArray<GroupRecord>(taskJson(executable, ['group', 'list']), 'group list');
    const groupsByName = new Map(existingGroups.map((group) => [group.name, group.id]));

    // Read both active and archived names so rerunning never creates duplicates.
    const active = asArray<TaskRecord>(taskJson(executable, ['list']), 'list');
    const archived = asArray<TaskRecord>(taskJson(executable, ['list', '--archived']), 'list --archived');
    const tasksByName = new Map([...archived, ...active].map((task) => [task.name, task.id]));
    const taskIds = new Map<string, string>();

    for (const group of groups) {
      let groupId = groupsByName.get(group.title);
      if (!groupId) {
        const slug = group.key.replace(/[^a-z0-9-]/g, '-');
        const created = withTextFile(tempDirectory, `${slug}-goal.md`, groupGoal(group), (goalFile) =>
          withTextFile(tempDirectory, `${slug}-plan.md`, groupPlan(group), (planFile) =>
            taskJson<GroupRecord>(executable, [
              'group', 'create', '--title', group.title,
              '--goal-file', goalFile, '--plan-file', planFile,
            ])
          )
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
          'create', '--title', task.title,
          '--group', groupId,
          '--priority', group.key === 'foundation' || group.key === 'contracts' ? 'high' : 'medium',
          '--tag', `cuadb,${group.key}`,
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

    taskRun(executable, ['validate']);
    console.log(
      `\nTask workspace validated. Added ${newGroups} groups, ${newTasks} tasks and ${newDocuments} documents.\nReview: task group list; task list; task docs list; git status`
    );
  } finally {
    rmSync(tempDirectory, { force: true, recursive: true });
  }
}

function verify(): void {
  validateWorkspace();
  const executable = taskCommand();
  console.log(taskRun(executable, ['validate']));
  console.log('Task workspace verification complete.');
}

try {
  validateSpecs();
  const action = process.argv[2] ?? '--preview';
  if (action === '--preview') preview();
  else if (action === '--verify') verify();
  else if (action === '--seed') seed();
  else throw new Error('Usage: bun scripts/roadmap/seed.ts --preview|--seed|--verify');
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
