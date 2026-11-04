/**
 * Retire the mistaken SQL Server roadmap group without destroying its history.
 *
 * Run once from the CuaDB repository root after pulling this branch:
 *   bun scripts/roadmap/retire-sqlserver.ts
 *
 * Task CLI remains the owner of group/task metadata and archive operations.
 * The three old task records and Group 13 are archived, not deleted.
 */
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const groupId = 'GRP-5F8CABD7220D';
const groupName = '13 — SQL Server adapter (post-core expansion)';
const taskIds = ['TSK-E25A9F7241A3', 'TSK-C00E0A305361', 'TSK-EF78B53DDF3C'];
const documents = ['DOC-A146C291CE62', 'DOC-9B73548B4ECD', 'DOC-373E23773C3D'];

type RecordWithId = { id: string; name?: string };

function taskCommand(): string {
  for (const command of ['taskcode', 'task']) {
    const probe = spawnSync(command, ['--version'], { cwd: root, encoding: 'utf8' });
    if (!probe.error && probe.status === 0) return command;
  }
  throw new Error('Task CLI not found. Install taskcode and confirm task --version.');
}

function run(command: string, args: string[], env: NodeJS.ProcessEnv = process.env): string {
  const result = spawnSync(command, args, {
    cwd: root,
    env,
    encoding: 'utf8',
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.error || result.status !== 0) {
    throw new Error(
      'Command failed: ' + command + ' ' + args.join(' ') + '\n' +
      (result.stderr || result.stdout || result.error),
    );
  }
  return result.stdout.trim();
}

function json<T>(command: string, args: string[]): T {
  return JSON.parse(run(command, [...args, '--json'])) as T;
}

function updateDocument(command: string, docId: string): void {
  const editor = resolve(root, 'scripts/roadmap/retire-sqlserver-editor.ts');
  const editorCommand = 'bun "' + editor.replaceAll('"', '\\"') + '"';
  run(command, ['docs', 'open', docId], {
    ...process.env,
    EDITOR: editorCommand,
    CUADB_RETIRE_SQLSERVER_DOC: docId,
  });
  console.log('Updated managed architecture document: ' + docId);
}

function main(): void {
  if (!existsSync(resolve(root, 'task.yaml')) || !existsSync(resolve(root, '.docs'))) {
    throw new Error('Run this script from the CuaDB repository root.');
  }
  const config = readFileSync(resolve(root, 'task.yaml'), 'utf8');
  if (!/^workspace:\s*["']?\.docs["']?\s*$/m.test(config)) {
    throw new Error('Expected task.yaml to continue using workspace: ".docs".');
  }

  const command = taskCommand();
  const activeTasks = json<RecordWithId[]>(command, ['list']);
  const archivedTasks = json<RecordWithId[]>(command, ['list', '--archived']);
  const activeIds = new Set(activeTasks.map((item) => item.id));
  const archivedIds = new Set(archivedTasks.map((item) => item.id));

  for (const id of taskIds) {
    if (activeIds.has(id)) {
      run(command, ['archive', id]);
      console.log('Archived SQL Server task: ' + id);
    } else if (archivedIds.has(id)) {
      console.log('Already archived: ' + id);
    } else {
      throw new Error(
        'Expected SQL Server task ' + id + ' was not found; refusing to continue.',
      );
    }
  }

  const activeGroups = json<RecordWithId[]>(command, ['group', 'list']);
  if (activeGroups.some((item) => item.id === groupId || item.name === groupName)) {
    run(command, ['archive', groupId]);
    console.log('Archived mistaken roadmap group: ' + groupId);
  } else {
    console.log('Group already archived or not active: ' + groupId);
  }

  for (const docId of documents) updateDocument(command, docId);

  const remainingActive = json<RecordWithId[]>(command, ['list']);
  const stillActive = taskIds.filter((id) => remainingActive.some((item) => item.id === id));
  if (stillActive.length) {
    throw new Error('SQL Server tasks remain active: ' + stillActive.join(', '));
  }

  console.log(run(command, ['validate']));
  console.log(
    'SQL Server work is archived, not deleted. Supported engines are PostgreSQL, MySQL, MariaDB and SQLite.',
  );
  console.log(
    'Review with: task group list && task list && task list --archived && git status --short',
  );
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
