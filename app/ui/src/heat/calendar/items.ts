// What is due on each day of the window (docs/SPEC.md 3.7): a task on its due
// day, a recurring task on each of its occurrences. The heat level and the
// open tasks' order are the snapshot's; this only files them by day.

import { dayKey, type DayKey, minuteOfDay } from '../../shared/time/zone';
import type { HeatLevel, Id, Snapshot } from '../client';

export interface DueItem {
  taskId: Id;
  title: string;
  day: DayKey;
  /** Minutes after midnight the deadline falls at. */
  minute: number;
  /** Null for an occurrence that isn't the next one: it has no heat of its own to show. */
  level: HeatLevel | null;
  done: boolean;
  repeats: boolean;
}

/**
 * Every task due on a day in `days`, with a recurring task once per
 * occurrence, open tasks first in the snapshot's heat order, done ones after.
 */
export function dueByDay(snap: Snapshot, days: readonly DayKey[]): Map<DayKey, DueItem[]> {
  const out = new Map<DayKey, DueItem[]>();
  const wanted = new Set(days);
  const order = new Map(snap.derived.lists.allOpen.map((id, i) => [id, i]));
  const put = (item: DueItem) => out.set(item.day, [...(out.get(item.day) ?? []), item]);
  const ticked = new Set(snap.records.taskOccurrence.map((o) => `${o.taskId}\u0000${o.date}`));
  const occurrences = snap.derived.occurrences;

  for (const t of snap.records.task) {
    const d = snap.derived.tasks[t.id];
    const minute = t.due === null ? 0 : minuteOfDay(t.due, snap.zone);
    if (t.rrule) {
      // The snapshot gives each series' occurrences in its window; without them, only the next one.
      const own = occurrences
        ? occurrences.filter((o) => o.taskId === t.id)
        : d?.next
          ? [{ date: d.next, done: false }]
          : [];
      for (const o of own) {
        if (!wanted.has(o.date)) continue;
        const done = o.done || ticked.has(`${t.id}\u0000${o.date}`);
        put({
          taskId: t.id,
          title: t.title,
          day: o.date,
          minute,
          level: done ? 'Done' : o.date === d?.next ? (d?.heat.level ?? null) : null,
          done,
          repeats: true,
        });
      }
      continue;
    }
    if (t.due === null) continue;
    const day = dayKey(t.due, snap.zone);
    if (!wanted.has(day)) continue;
    put({
      taskId: t.id,
      title: t.title,
      day,
      minute,
      level: t.done ? 'Done' : (d?.heat.level ?? null),
      done: t.done,
      repeats: false,
    });
  }

  for (const [day, items] of out) {
    const rank = (i: DueItem) => (i.done ? 1e6 : (order.get(i.taskId) ?? 1e5));
    out.set(
      day,
      [...items].sort((a, b) => rank(a) - rank(b)),
    );
  }
  return out;
}
