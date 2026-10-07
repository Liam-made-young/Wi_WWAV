// How Tasks lays out the ids the snapshot gives it (docs/SPEC.md 3.6). The
// order is the core's; this only indents a subtask under its parent and
// leaves out what a space, a group or a filter doesn't want.

import type { Id, Task } from '../client';

export interface RowSpec {
  id: Id;
  depth: number;
  /** How many subtasks sit under it in this list. */
  children: number;
}

/**
 * Subtasks indent under their parent, which keeps the place its own heat
 * gives it; a subtask whose parent isn't in the list stands by itself. A
 * collapsed parent hides its children. A parent chain that loops is walked
 * once.
 */
export function nest(ids: readonly Id[], parentOf: (id: Id) => Id | undefined, collapsed: ReadonlySet<Id>): RowSpec[] {
  const inList = new Set(ids);
  const kids = new Map<Id, Id[]>();
  const top: Id[] = [];
  for (const id of ids) {
    const parent = parentOf(id);
    if (parent !== undefined && parent !== id && inList.has(parent))
      kids.set(parent, [...(kids.get(parent) ?? []), id]);
    else top.push(id);
  }
  const out: RowSpec[] = [];
  const seen = new Set<Id>();
  // What a collapsed parent hides is neither shown nor taken for a stray.
  const hide = (id: Id) => {
    for (const c of kids.get(id) ?? []) {
      if (seen.has(c)) continue;
      seen.add(c);
      hide(c);
    }
  };
  const visit = (id: Id, depth: number) => {
    if (seen.has(id)) return;
    seen.add(id);
    const children = kids.get(id) ?? [];
    out.push({ id, depth, children: children.length });
    if (collapsed.has(id)) hide(id);
    else for (const c of children) visit(c, depth + 1);
  };
  for (const id of top) visit(id, 0);
  // A loop of parents leaves its members without a top: show them rather than lose them.
  for (const id of ids) visit(id, 0);
  return out;
}

/** What the filter box keeps: a row whose title, group or type holds every word typed. */
export function matchesFilter(words: string, ...fields: (string | undefined)[]): boolean {
  const hay = fields.join(' ').toLowerCase();
  return words
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => hay.includes(w));
}

/** A task's group as a row names it: the course code, the milestone, the area, or the project. */
export function groupOf(
  t: Task,
  by: {
    course(id: Id): { code: string } | undefined;
    milestone(id: Id): { title: string } | undefined;
    project(id: Id): { title: string } | undefined;
  },
): string {
  if (t.courseId) return by.course(t.courseId)?.code ?? '';
  if (t.milestoneId) return by.milestone(t.milestoneId)?.title ?? '';
  if (t.group) return t.group;
  if (t.projectId) return by.project(t.projectId)?.title ?? '';
  return '';
}
