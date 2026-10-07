// Calendar (docs/SPEC.md 3.1 and 3.7). The month grid stays as Heat has it:
// from Sunday, 3 pills a cell with a heat border, "N more", today circled.
// Week and Day views sit on the PKM's grid (44 px an hour, the whole day):
// Google events grey behind blocks, and each deadline a small heat-coloured
// flag at its time. Brightspace items are tasks, so they never draw as events.

import { clock, longDay, MONTHS, shortMonthDay } from '../../shared/time/format';
import { addDays, atMinute, type DayKey, dayKey, keyOf, keyParts, minuteOfDay, startOfDay, weekdayOf } from '../../shared/time/zone';
import * as copy from './copy';
import { byHeat, heatOf, type HeatLevel, LEVEL_COLOUR } from './heat';
import { HOUR_PX } from './plan';
import type { CalendarEvent, Habit, Id, Milestone, Task, TaskOccurrence, TimeBlock } from './records';
import { openTasks, taskOccurrences, withEffectiveDue } from './recurrence';

export const GRID_HOUR_PX = HOUR_PX;
const MIN_ITEM_PX = 16;
const PILLS_PER_CELL = 3;

export interface CalendarData {
  tasks: Task[];
  occurrences: TaskOccurrence[];
  blocks: TimeBlock[];
  events: CalendarEvent[];
  milestones: Milestone[];
  habits?: Habit[];
}

export type CalendarMode = 'month' | 'week' | 'day';

export interface Pill {
  taskId: Id;
  title: string;
  /** Null for a past occurrence nobody ticked: it is left behind, not overdue. */
  level: HeatLevel | null;
  colour: string | null;
}

const colourOf = (level: HeatLevel | null) => (level === null || level === 'Done' ? null : LEVEL_COLOUR[level]);

// Every task due in [from, to], one entry per occurrence for a recurring task.
function dueItems(data: CalendarData, from: DayKey, to: DayKey, now: number, tz: string) {
  const today = dayKey(now, tz);
  const ticked = new Set(data.occurrences.map((o) => `${o.taskId}\u0000${o.date}`));
  return data.tasks.flatMap((t) => {
    if (!t.rrule) {
      if (t.due === null) return [];
      const date = dayKey(t.due, tz);
      return date < from || date > to ? [] : [{ task: t, date, level: heatOf(t, now).level as HeatLevel | null }];
    }
    return taskOccurrences(t, tz, from, to).map((o) => {
      const done = ticked.has(`${t.id}\u0000${o.date}`);
      const level: HeatLevel | null = done ? 'Done' : o.date < today ? null : heatOf({ ...t, due: o.at }, now).level;
      return { task: { ...t, due: o.at, done: done || level === null }, date: o.date, level };
    });
  });
}

export interface MonthCell {
  date: DayKey;
  inMonth: boolean;
  isToday: boolean;
  pills: Pill[];
  more: string | null;
}

/** The month of `anchor`: 42 days from the Sunday before the 1st. */
export function monthGrid(anchor: DayKey, data: CalendarData, now: number, tz: string): MonthCell[] {
  const { year, month } = keyParts(anchor);
  const first = keyOf(year, month, 1);
  const start = addDays(first, -weekdayOf(first));
  const items = dueItems(data, start, addDays(start, 41), now, tz);
  const today = dayKey(now, tz);
  return Array.from({ length: 42 }, (_, i) => {
    const date = addDays(start, i);
    const here = items.filter((x) => x.date === date);
    const sorted = byHeat(
      here.map((x) => x.task),
      now,
    );
    const pills = sorted.slice(0, PILLS_PER_CELL).map((t) => {
      const level = here.find((x) => x.task === t)!.level;
      return { taskId: t.id, title: t.title, level, colour: colourOf(level) };
    });
    const rest = sorted.length - pills.length;
    return {
      date,
      inMonth: keyParts(date).month === month,
      isToday: date === today,
      pills,
      more: rest > 0 ? copy.calendar.more(rest) : null,
    };
  });
}

/** The week of `anchor`, Sunday first. */
export function weekDays(anchor: DayKey): DayKey[] {
  const sunday = addDays(anchor, -weekdayOf(anchor));
  return Array.from({ length: 7 }, (_, i) => addDays(sunday, i));
}

export interface LaidOut {
  kind: 'event' | 'block';
  id: Id;
  title: string;
  top: number;
  height: number;
  /** A block's left border: its task's heat colour. Events are grey. */
  colour: string | null;
}

const toY = (min: number) => (min / 60) * GRID_HOUR_PX;

/** One day column: events, then blocks drawn over them, then deadline flags; and the all-day strip. */
export function dayLayout(day: DayKey, data: CalendarData, now: number, tz: string) {
  const dayStart = startOfDay(day, tz);
  const dayEnd = startOfDay(addDays(day, 1), tz);
  const overlapping = data.events.filter((e) => e.end > dayStart && e.start < dayEnd);
  const events: LaidOut[] = overlapping
    .filter((e) => !e.allDay)
    .sort((a, b) => a.start - b.start)
    .map((e) => {
      const from = e.start <= dayStart ? 0 : minuteOfDay(e.start, tz);
      const to = e.end >= dayEnd ? 1440 : minuteOfDay(e.end, tz);
      return { kind: 'event', id: e.id, title: e.title, top: toY(from), height: Math.max(MIN_ITEM_PX, toY(to - from)), colour: null };
    });
  const blocks: LaidOut[] = data.blocks
    .filter((b) => b.date === day)
    .sort((a, b) => a.start - b.start)
    .flatMap((b) => {
      const t = b.taskId === undefined ? undefined : data.tasks.find((x) => x.id === b.taskId);
      const title = t?.title ?? data.habits?.find((h) => h.id === b.habitId)?.title;
      if (title === undefined) return [];
      const colour = t ? colourOf(heatOf(withEffectiveDue(t, data.occurrences, now, tz), now).level) : null;
      return [{ kind: 'block' as const, id: b.id, title, top: toY(b.start), height: Math.max(MIN_ITEM_PX, toY(b.minutes)), colour }];
    });
  const due = dueItems(data, day, day, now, tz).filter((x) => x.level !== null && x.level !== 'Done');
  return {
    items: [...events, ...blocks],
    dueFlags: due.map((x) => ({
      taskId: x.task.id,
      title: x.task.title,
      top: toY(minuteOfDay(x.task.due!, tz)),
      label: copy.calendar.dueFlag(clock(minuteOfDay(x.task.due!, tz))),
      colour: colourOf(x.level),
    })),
    allDay: {
      due: due.map((x) => ({ taskId: x.task.id, title: x.task.title, colour: colourOf(x.level) })),
      beads: data.milestones.filter((m) => m.date === day),
      events: overlapping.filter((e) => e.allDay),
    },
  };
}

/** The tray left of the week: this week's open tasks with no block in it, in heat order. */
export function unscheduledTray(anchor: DayKey, data: CalendarData, now: number, tz: string): Task[] {
  const days = weekDays(anchor);
  const [first, last] = [days[0], days[6]];
  const blocked = new Set(data.blocks.filter((b) => b.date >= first && b.date <= last).map((b) => b.taskId));
  const inWeek = (d: DayKey | undefined) => d !== undefined && d >= first && d <= last;
  return byHeat(
    openTasks(data.tasks, data.occurrences, now, tz).filter(
      (t) => !blocked.has(t.id) && (inWeek(t.due === null ? undefined : dayKey(t.due, tz)) || inWeek(t.scheduledDate)),
    ),
    now,
  );
}

/** ← and →: a month (to its 1st), a week or a day. */
export function page(mode: CalendarMode, anchor: DayKey, dir: 1 | -1): DayKey {
  if (mode === 'day') return addDays(anchor, dir);
  if (mode === 'week') return addDays(anchor, 7 * dir);
  const { year, month } = keyParts(anchor);
  const m = month - 1 + dir;
  return keyOf(year + Math.floor(m / 12), (((m % 12) + 12) % 12) + 1, 1);
}

/** "October 2026", "Oct 4 – Oct 10", "Tuesday, October 6" */
export function calendarTitle(mode: CalendarMode, anchor: DayKey): string {
  if (mode === 'day') return longDay(anchor);
  if (mode === 'week') {
    const days = weekDays(anchor);
    return `${shortMonthDay(days[0])} – ${shortMonthDay(days[6])}`;
  }
  const { year, month } = keyParts(anchor);
  return `${MONTHS[month - 1]} ${year}`;
}

/** Calendar's "+" adds a task due 11:59 PM on the selected day. */
export function dueAtEndOfDay(day: DayKey, tz: string): number {
  return atMinute(day, 23 * 60 + 59, tz);
}
