/**
 * Static, one-time bootstrap specification for the Task CLI.
 *
 * Once created, .docs/ is the source of truth for ongoing work. Never edit a
 * task's managed frontmatter or status by changing these seed definitions.
 */
export interface TaskSpec {
  /** Stable seed key used for dependency resolution, not a Task-managed ID. */
  key: string;
  title: string;
  goal: string;
  context: string;
  checks: string[];
  dependsOn?: string[];
}

export interface GroupSpec {
  key: string;
  title: string;
  goal: string;
  approach: string;
  risks: string;
  tasks: TaskSpec[];
}

export interface DocumentSpec {
  key: string;
  title: string;
  path: string;
  body: string;
}
