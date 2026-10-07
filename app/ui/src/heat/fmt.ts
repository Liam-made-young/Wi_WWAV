// Words for what the snapshot carries. The snapshot gives every derived
// value and every list's order (docs/HEAT.md); this file only writes them
// out: "Today 4:00 PM", "1h 15m", "Oct 9". It never sorts, clamps, plans or
// works out a heat value.

import { clock, shortMonthDay } from '../shared/time/format';
import { addDays, atMinute, type DayKey, daysBetween, minuteOfDay } from '../shared/time/zone';
import type { Task, TaskDerived } from './client';
import * as copy from './model/copy';
import { formatMinutes } from './model/estimate';
import { duePhrase } from './model/heat';

export { clock, copy, formatMinutes };

export const plural = copy.plural;

/** "Today 4:00 PM", "Tomorrow 11:59 PM", "2d overdue"; "No due date" has no phrase. */
export function dueText(due: number, now: number, tz: string): string {
  return duePhrase(due, now, tz);
}

/**
 * The instant a task's heat reads from: its own due date, or for a
 * recurring task the next occurrence's day at the due date's wall clock.
 */
export function effectiveDue(task: Task, derived: TaskDerived | undefined, tz: string): number | null {
  if (task.rrule && derived?.next && task.due !== null) return atMinute(derived.next, minuteOfDay(task.due, tz), tz);
  if (task.rrule && derived?.next) return atMinute(derived.next, 0, tz);
  return task.due;
}

/** "Today", "Tomorrow", "Oct 9": a day without a time, for the When column. */
export function dayText(day: DayKey, today: DayKey): string {
  const ahead = daysBetween(today, day);
  if (ahead === 0) return 'Today';
  if (ahead === 1) return 'Tomorrow';
  if (ahead === -1) return 'Yesterday';
  return shortMonthDay(day);
}

/** A course's two names without the spaces and the case: "jpn101" and "JPN 101" say the same thing. */
const squashed = (text: string) => text.replace(/\s+/g, '').toLowerCase();

/**
 * A course's name, written once: "ELE 209 · Intro to Computer Systems Lab",
 * or the code alone when the name is empty or only says the code again. The
 * snapshot carries it as `derived.courses[id].label`; this is for when it
 * doesn't.
 */
export function courseLabel(course: { code: string; name?: string | null }): string {
  const code = course.code.trim();
  const name = (course.name ?? '').trim();
  if (!name || squashed(name) === squashed(code)) return code;
  return code ? copy.homes.label(code, name) : name;
}

/** Where a task belongs, as Get Info names it: its course, else its project, else its space. */
export function homeLabel(derived: TaskDerived | undefined, fallback: { course?: string; project?: string; space?: string }): string {
  return derived?.home?.label ?? fallback.course ?? fallback.project ?? fallback.space ?? '';
}

/** The Edit menu's words and the like, from "Undo mark done" to the toast's text. */
export const undoSentence = (undo: string | null | undefined) => (undo ? undo : null);

/** "1:00 PM – 1:45 PM" for a block's minutes after midnight. */
export function spanText(start: number, minutes: number): string {
  return `${clock(start)} – ${clock(start + minutes)}`;
}

/** The first and last day of a week-wide window around `day`, for a snapshot request. */
export function around(day: DayKey, before: number, after: number): { from: DayKey; to: DayKey } {
  return { from: addDays(day, -before), to: addDays(day, after) };
}

/** An error as one sentence. */
export function sentenceOf(e: unknown): string {
  if (e instanceof Error && e.message) return e.message;
  if (e && typeof e === 'object' && 'message' in e) return String((e as { message: unknown }).message);
  return copy.status.failed;
}

/** "3 tasks", "1 task". */
export const tasksCount = (n: number) => plural(n, 'task');
