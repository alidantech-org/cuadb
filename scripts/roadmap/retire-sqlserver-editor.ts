/**
 * Task CLI body-only editor used by retire-sqlserver.ts.
 * Never point this at a managed workspace file directly.
 */
import { readFileSync, writeFileSync } from 'node:fs';

const file = process.argv[2];
const docId = process.env.CUADB_RETIRE_SQLSERVER_DOC;
if (!file || !docId) {
  throw new Error('This editor must be launched by retire-sqlserver.ts through task docs open.');
}

const replacements: Record<string, [string, string]> = {
  'DOC-A146C291CE62': [
    'Microsoft SQL Server is a post-core expansion only after a driver compatibility decision.',
    'The supported engines for this roadmap are PostgreSQL, MySQL, MariaDB and SQLite. No additional engine adapter is planned until the current product features are complete.',
  ],
  'DOC-9B73548B4ECD': [
    'SQL Server is deliberately a post-core extension with a driver feasibility gate.',
    'Keep the engine scope limited to PostgreSQL, MySQL, MariaDB and SQLite until the current product features are complete.',
  ],
  'DOC-373E23773C3D': [
    'SQL Server may be added to this matrix after the driver feasibility task.',
    'Do not add another engine to this matrix until the current product features are complete.',
  ],
};

const replacement = replacements[docId];
if (!replacement) throw new Error(`Unexpected Task document ID: ${docId}`);

const body = readFileSync(file, 'utf8');
const [before, after] = replacement;
const occurrences = body.split(before).length - 1;
if (occurrences !== 1) {
  throw new Error(
    `Refusing to edit ${docId}: expected one exact SQL Server sentence, found ${occurrences}.`,
  );
}
writeFileSync(file, body.replace(before, after), 'utf8');
