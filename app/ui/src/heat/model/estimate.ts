// Time: the estimate chain, measured time, and the sums built from them
// (docs/SPEC.md 3.1, 3.5, 3.6).
//
// A task's estimate is its own estMin if it has one, else the average for its
// type in its space, else difficulty × 20 minutes. A parent's estimate is the
// sum of its open children. actualMin = Σ focus minutes + adjustMin.

import { DAY_MS } from './heat';
import * as copy from './copy';
import type { FocusSession, Id, Space, Task } from './records';

export interface Average {
  minutes: number;
  count: number;
}

export interface EstimateContext {
  /** Average measured minutes by space and type, from done tasks with time. */
  averages: Map<string, Average>;
  children: Map<Id, Task[]>;
}

/** "45m", "1h 15m", "3h 10m", "2h". */
export function formatMinutes(minutes: number): string {
  const m = Math.round(minutes);
  const h = Math.floor(m / 60);
  if (h === 0) return `${m}m`;
  return m % 60 === 0 ? `${h}h` : `${h}h ${m % 60}m`;
}

function focusMinutesByTask(sessions: readonly FocusSession[]): Map<Id, number> {
  const out = new Map<Id, number>();
  for (const s of sessions) {
    if (s.taskId) out.set(s.taskId, (out.get(s.taskId) ?? 0) + s.focusMin);
  }
  return out;
}

export function actualMin(task: Task, sessions: readonly FocusSession[]): number {
  return (focusMinutesByTask(sessions).get(task.id) ?? 0) + task.adjustMin;
}

const typeKey = (spaceId: Id, type: string) => `${spaceId}\u0000${type}`;

export function estimateContext(tasks: readonly Task[], sessions: readonly FocusSession[]): EstimateContext {
  const focus = focusMinutesByTask(sessions);
  const sums = new Map<string, { total: number; count: number }>();
  const children = new Map<Id, Task[]>();
  for (const t of tasks) {
    if (t.parentTaskId) children.set(t.parentTaskId, [...(children.get(t.parentTaskId) ?? []), t]);
    const actual = (focus.get(t.id) ?? 0) + t.adjustMin;
    if (!t.done || actual <= 0) continue;
    const key = typeKey(t.spaceId, t.type);
    const sum = sums.get(key) ?? { total: 0, count: 0 };
    sums.set(key, { total: sum.total + actual, count: sum.count + 1 });
  }
  const averages = new Map<string, Average>();
  for (const [key, { total, count }] of sums) averages.set(key, { minutes: Math.round(total / count), count });
  return { averages, children };
}

/**
 * A task's estimate. Parent links that loop (a task its own parent, or
 * A → B → A) are walked once: a task already on the way down counts as no
 * child of the one below it.
 */
export function estimateMin(task: Task, ctx: EstimateContext): number {
  return estimateBelow(task, ctx, new Set());
}

function estimateBelow(task: Task, ctx: EstimateContext, above: ReadonlySet<Id>): number {
  const path = new Set(above).add(task.id);
  const open = (ctx.children.get(task.id) ?? []).filter((c) => !c.done && !path.has(c.id));
  if (open.length > 0) return open.reduce((sum, c) => sum + estimateBelow(c, ctx, path), 0);
  if (task.estMin !== null && task.estMin > 0) return task.estMin;
  const average = ctx.averages.get(typeKey(task.spaceId, task.type));
  if (average) return average.minutes;
  return Math.min(5, Math.max(1, Math.round(task.difficulty))) * 20;
}

/** The sidebar's "Your average time": "Homework 1h 15m (6)", in the space's type order. */
export function averageLines(space: Space, ctx: EstimateContext): string[] {
  return space.types.flatMap((type) => {
    const a = ctx.averages.get(typeKey(space.id, type));
    return a ? [copy.averageTime(type, formatMinutes(a.minutes), a.count)] : [];
  });
}

export interface Load {
  minutes: number;
  count: number;
}

/**
 * The sum of estimates for open tasks due within 7 days, overdue ones
 * included. A subtask whose parent is also counted is counted once, inside
 * the parent's estimate.
 */
export function weeklyLoad(tasks: readonly Task[], ctx: EstimateContext, now: number): Load {
  const inWeek = tasks.filter((t) => !t.done && t.due !== null && t.due - now <= 7 * DAY_MS);
  const counted = new Set(inWeek.map((t) => t.id));
  const byId = new Map(tasks.map((t) => [t.id, t]));
  // Stops where parent links loop back, so a malformed chain can't hang the re-render.
  const hasCountedAncestor = (t: Task): boolean => {
    const seen = new Set([t.id]);
    for (let p = t.parentTaskId; p && !seen.has(p); p = byId.get(p)?.parentTaskId) {
      if (counted.has(p)) return true;
      seen.add(p);
    }
    return false;
  };
  const top = inWeek.filter((t) => !hasCountedAncestor(t));
  return { minutes: top.reduce((sum, t) => sum + estimateMin(t, ctx), 0), count: top.length };
}

/** "This week: 3h 20m across 5 tasks" */
export function weeklyLoadLine(load: Load): string {
  return copy.weeklyLoad(formatMinutes(load.minutes), load.count);
}
