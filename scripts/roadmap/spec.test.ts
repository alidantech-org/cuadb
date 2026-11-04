import { describe, expect, test } from 'bun:test';
import { documents, groups } from './spec';
import { groupGoal, groupDefinitionOfDone, LEGACY_GOAL_PARAGRAPH } from './goals';

describe('CuaDB Task roadmap definition', () => {
  test('defines ordered groups and complete documentation', () => {
    expect(groups).toHaveLength(14);
    expect(documents).toHaveLength(5);
    expect(groups.map((group) => group.key)).toEqual([
      'foundation',
      'contracts',
      'postgres-catalog',
      'postgres-rows',
      'http',
      'sqlite',
      'explorer-ui',
      'data-grid-ui',
      'inspectors',
      'mysql',
      'profiles',
      'auth',
      'sql-workspace',
      'release',
    ]);
  });

  test('has unique keys and resolves every dependency earlier in the plan', () => {
    const known = new Set<string>();
    const titles = new Set<string>();
    let count = 0;
    for (const group of groups) {
      expect(group.goal.length).toBeGreaterThan(100);
      expect(group.approach.length).toBeGreaterThan(100);
      for (const task of group.tasks) {
        expect(known.has(task.key)).toBe(false);
        expect(titles.has(task.title)).toBe(false);
        expect(task.checks.length).toBeGreaterThanOrEqual(3);
        expect(task.goal.length).toBeGreaterThan(100);
        expect(task.context.length).toBeGreaterThan(100);
        for (const dependency of task.dependsOn ?? []) {
          expect(known.has(dependency)).toBe(true);
        }
        known.add(task.key);
        titles.add(task.title);
        count++;
      }
    }
    expect(count).toBe(62);
  });

  test('renders meaningful group-specific goals that satisfy Task quality markers', () => {
    const placeholderMarker =
      /\b(?:TODO|TBD|FIXME)\b|\b(?:lorem ipsum|to be (?:written|defined|determined)|describe here|fill (?:this|me) in)\b/i;
    for (const group of groups) {
      const goal = groupGoal(group);
      expect(goal).toContain(group.goal);
      expect(goal).toContain(group.tasks[0]!.title);
      expect(goal).toContain(groupDefinitionOfDone(group));
      expect(goal).not.toMatch(placeholderMarker);
      expect(goal.split(/\s+/).length).toBeGreaterThan(100);
      expect(goal).not.toContain(LEGACY_GOAL_PARAGRAPH);
    }
  });

  test('has unique document paths and meaningful documentation bodies', () => {
    const paths = documents.map((doc) => doc.path);
    expect(new Set(paths).size).toBe(paths.length);
    for (const document of documents) {
      expect(document.path.startsWith('docs/')).toBe(true);
      expect(document.body.length).toBeGreaterThan(900);
      expect(document.body.startsWith('# ')).toBe(true);
    }
  });
});
