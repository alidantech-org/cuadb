/** Group-specific Task documents and compatibility with the first bootstrap. */
import type { GroupSpec } from './types';

/**
 * Exact paragraph from the first roadmap seed. A capitalized Task marker in
 * this prose caused `task validate` to reject all generated group goals.
 * Keep this constant unchanged so the existing records can be repaired safely.
 */
export const LEGACY_GOAL_PARAGRAPH =
  'This group is complete when its implemented behavior is verifiable through the documented checks, not merely when files exist. Functionality must remain read-only and obey the shared database capability contract. All error states need truthful, usable messages and no credential disclosures. Work should be integrated with the existing single-process Rust application and browser shell, without introducing a separate runtime service. The group tasks describe individual deliverables and must be completed through Task checklists rather than a parallel root TODO file.';

/** Specific, measurable acceptance outcomes, not generic bootstrap wording. */
export function groupDefinitionOfDone(group: GroupSpec): string {
  return [
    `Acceptance of **${group.title}** requires evidence for these results:`,
    ...group.tasks.map(
      (task, index) =>
        `${index + 1}. **${task.title}:** ${task.goal} Verification includes: ${task.checks[0]}`,
    ),
    'Every listed result must be supported by tests, safe failure handling, and updated documentation where applicable. Verify the complete group against its own plan and linked task checklists. Keep the current single-process Rust deployment and read-only database scope intact. Record unavoidable engine limitations and integration test requirements explicitly.',
  ].join('\n');
}

/** Goal Markdown created by Task CLI from a temporary body file. */
export function groupGoal(group: GroupSpec): string {
  return [
    `# ${group.title} — Goal`,
    '',
    '## Outcome',
    group.goal,
    '',
    '## Definition of done',
    groupDefinitionOfDone(group),
    '',
    '## Boundaries',
    'Do not implement adjacent groups speculatively. Keep changes modular, add tests that run without live database access when possible, and document fixture-only validation clearly. Preserve the existing Bun commands and local-only Docker defaults. Record architectural decisions and compatibility limits in the Task workspace; never create a competing task tracker.',
  ].join('\n');
}
