/**
 * Temporary Task CLI editor for repairing the first generated group goals.
 *
 * The CLI passes a BODY-ONLY temporary Markdown file to EDITOR and then writes
 * it back with its own managed metadata, ID and content-hash validation.
 * Never execute this script directly against a managed workspace file.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { groups } from './spec';
import { groupDefinitionOfDone, LEGACY_GOAL_PARAGRAPH } from './goals';

const file = process.argv[2];
const groupKey = process.env.CUADB_REPAIR_GROUP;
const group = groups.find((item) => item.key === groupKey);

if (!file || !group) {
  throw new Error('This editor must be launched by taskcode with a known CuaDB group.');
}

const original = readFileSync(file, 'utf8');
const occurrences = original.split(LEGACY_GOAL_PARAGRAPH).length - 1;
if (occurrences !== 1) {
  throw new Error(
    'Refusing to change goal content: the original generated paragraph was modified or missing.',
  );
}

writeFileSync(file, original.replace(LEGACY_GOAL_PARAGRAPH, groupDefinitionOfDone(group)), 'utf8');
