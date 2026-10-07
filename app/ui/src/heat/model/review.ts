// The weekly review's step 2, "Last week" (docs/SPEC.md 3.13): facts only.
// Tasks done and focus time per space, milestones reached, and estimate
// accuracy. Claude's note restates these and may not add a number of its
// own (3.12); its draft is built elsewhere from what this returns.

import { addDays, type DayKey, dayKey, startOfDay } from '../../shared/time/zone';
import * as copy from './copy';
import { actualMin, estimateContext, estimateMin, formatMinutes } from './estimate';
import type { FocusSession, Id, Milestone, Space, Task, TaskOccurrence } from './records';

export interface ReviewData {
  spaces: Space[];
  tasks: Task[];
  occurrences: TaskOccurrence[];
  sessions: FocusSession[];
  milestones: Milestone[];
}

export interface WeekFacts {
  /** The 7 days before today, both ends inclusive. */
  from: DayKey;
  to: DayKey;
  spaces: { spaceId: Id; name: string; tasksDone: number; focusMin: number }[];
  habitFocusMin: number;
  milestonesReached: Milestone[];
  accuracy: { label: string; estimatedMin: number; tookMin: number; count: number }[];
}

export function lastWeekFacts(data: ReviewData, now: number, tz: string): WeekFacts {
  const today = dayKey(now, tz);
  const from = addDays(today, -7);
  const start = startOfDay(from, tz);
  const end = startOfDay(today, tz);
  const inWeek = (ms: number | null) => ms !== null && ms >= start && ms < end;
  const spaceOf = new Map(data.tasks.map((t) => [t.id, t.spaceId]));

  const doneTasks = data.tasks.filter((t) => t.done && inWeek(t.doneAt));
  const doneOccurrences = data.occurrences.filter((o) => inWeek(o.doneAt));
  const weekSessions = data.sessions.filter((s) => inWeek(s.endedAt));
  const spaces = data.spaces.map((space) => ({
    spaceId: space.id,
    name: space.name,
    tasksDone:
      doneTasks.filter((t) => t.spaceId === space.id).length +
      doneOccurrences.filter((o) => spaceOf.get(o.taskId) === space.id).length,
    focusMin: weekSessions
      .filter((s) => s.taskId !== undefined && spaceOf.get(s.taskId) === space.id)
      .reduce((sum, s) => sum + s.focusMin, 0),
  }));

  // Estimates as Heat made them when the week began: the week's own tasks
  // don't yet count toward the averages they are measured against.
  const asAtStart = data.tasks.map((t) => (t.done && inWeek(t.doneAt) ? { ...t, done: false } : t));
  const ctx = estimateContext(asAtStart, data.sessions);
  const groups = new Map<string, { spaceId: Id; type: string; estimated: number; took: number; count: number }>();
  for (const t of doneTasks) {
    const took = actualMin(t, data.sessions);
    if (took <= 0) continue;
    const key = `${t.spaceId}\u0000${t.type}`;
    const g = groups.get(key) ?? { spaceId: t.spaceId, type: t.type, estimated: 0, took: 0, count: 0 };
    groups.set(key, {
      ...g,
      estimated: g.estimated + estimateMin({ ...t, done: false }, ctx),
      took: g.took + took,
      count: g.count + 1,
    });
  }
  const typeCount = new Map<string, number>();
  for (const g of groups.values()) typeCount.set(g.type, (typeCount.get(g.type) ?? 0) + 1);
  const spaceName = (id: Id) => data.spaces.find((s) => s.id === id)?.name ?? id;

  return {
    from,
    to: addDays(today, -1),
    spaces,
    habitFocusMin: weekSessions.filter((s) => s.habitId !== undefined).reduce((sum, s) => sum + s.focusMin, 0),
    milestonesReached: data.milestones.filter((m) => m.done && m.date >= from && m.date < today),
    accuracy: [...groups.values()].map((g) => ({
      label: typeCount.get(g.type)! > 1 ? `${g.type} (${spaceName(g.spaceId)})` : g.type,
      estimatedMin: Math.round(g.estimated / g.count),
      tookMin: Math.round(g.took / g.count),
      count: g.count,
    })),
  };
}

/** The facts as the review shows them, and as Claude receives them. */
export function factLines(f: WeekFacts): string[] {
  return [
    ...f.spaces.map((s) => copy.review.space(s.name, s.tasksDone, formatMinutes(s.focusMin))),
    ...(f.habitFocusMin > 0 ? [copy.review.habits(formatMinutes(f.habitFocusMin))] : []),
    ...f.milestonesReached.map((m) => copy.review.milestone(m.title)),
    ...f.accuracy.map((a) => copy.review.accuracy(a.label, formatMinutes(a.estimatedMin), formatMinutes(a.tookMin), a.count)),
  ];
}
