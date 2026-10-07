// Habits (docs/SPEC.md 3.9). Up to 6, each with an orb for today, a 14-day
// grid, and "Show the year". The log is a set of day keys and is never
// pruned. By default a habit shows a record that only grows ("Done 41 days
// since August 26"); the streak counter is a per-habit setting, off by
// default (Open #9), and a streak survives until midnight.

import { monthDay } from '../../shared/time/format';
import { addDays, type DayKey, dayKey, keyParts, weekdayOf } from '../../shared/time/zone';
import * as copy from './copy';
import { formatMinutes } from './estimate';
import type { Habit, Id } from './records';

export const HABIT_LIMIT = 6;

export function addHabit(
  habits: readonly Habit[],
  fields: { title: string; minutes?: number },
  newId: () => Id,
): { habit: Habit } | { error: string } {
  if (habits.length >= HABIT_LIMIT) return { error: copy.habits.limit };
  const title = fields.title.trim();
  if (!title) return { error: copy.habits.nameFirst };
  const habit: Habit = { id: newId(), title, log: {}, showCounter: false };
  return { habit: fields.minutes === undefined ? habit : { ...habit, minutes: fields.minutes } };
}

/** "Practise kanji, 20m" */
export function habitLabel(h: Habit): string {
  return h.minutes === undefined ? h.title : `${h.title}, ${formatMinutes(h.minutes)}`;
}

export function toggleHabit(h: Habit, day: DayKey): Habit {
  const log = { ...h.log };
  if (log[day]) delete log[day];
  else log[day] = true;
  return { ...h, log };
}

/** Today's orbs and "3 of 5 done". */
export function todayOrbs(
  habits: readonly Habit[],
  now: number,
  tz: string,
): { orbs: { habit: Habit; done: boolean }[]; line: string | null } {
  const today = dayKey(now, tz);
  const orbs = habits.map((habit) => ({ habit, done: habit.log[today] === true }));
  const done = orbs.filter((o) => o.done).length;
  return { orbs, line: habits.length > 0 ? copy.habits.ofDone(done, habits.length) : null };
}

/** The 14-day grid, oldest first, ending today. */
export function lastFourteen(h: Habit, today: DayKey): { date: DayKey; done: boolean }[] {
  return Array.from({ length: 14 }, (_, i) => {
    const date = addDays(today, i - 13);
    return { date, done: h.log[date] === true };
  });
}

/** "Show the year": 53 weeks of 7 days from Sunday, ending with this week; days still to come are null. */
export function yearGrid(h: Habit, today: DayKey): ({ date: DayKey; done: boolean } | null)[][] {
  const first = addDays(today, -weekdayOf(today) - 52 * 7);
  return Array.from({ length: 53 }, (_, w) =>
    Array.from({ length: 7 }, (_, d) => {
      const date = addDays(first, w * 7 + d);
      return date > today ? null : { date, done: h.log[date] === true };
    }),
  );
}

/** The record that only grows: "Done 41 days since August 26". */
export function doneRecord(h: Habit, today: DayKey): string {
  const days = Object.keys(h.log).filter((d) => h.log[d]).sort();
  if (days.length === 0) return copy.habits.notYet;
  const first = days[0];
  const year = keyParts(first).year;
  const since = year === keyParts(today).year ? monthDay(first) : `${monthDay(first)}, ${year}`;
  return copy.habits.record(days.length, since);
}

/** Days in a row, ending today or, until midnight, yesterday. */
export function streak(h: Habit, today: DayKey): number {
  let day = h.log[today] ? today : addDays(today, -1);
  let n = 0;
  while (h.log[day]) {
    n += 1;
    day = addDays(day, -1);
  }
  return n;
}

/** The line under a habit: its counter when switched on (and above 0), else its record. */
export function habitLine(h: Habit, today: DayKey): string | null {
  if (!h.showCounter) return doneRecord(h, today);
  const n = streak(h, today);
  return n > 0 ? copy.habits.streak(n) : null;
}
