// Recurrence: the RRULE subset Heat needs, as the PKM stores it (docs/SPEC.md
// 3.6). FREQ is DAILY, WEEKLY, MONTHLY or YEARLY, with INTERVAL, BYDAY (plain
// weekdays), BYMONTHDAY, COUNT and UNTIL, read as RFC 5545 reads them.
// Anything else is refused, never guessed at.
//
// The series starts at the task's due date, at its wall-clock time in the
// person's zone, and every occurrence keeps that wall-clock time across DST.
// A task with a rule but no due date runs from its scheduled date. Each
// finished occurrence is a TaskOccurrence row; the series never flips to done.

import { addDays, atMinute, type DayKey, daysInMonth, dayKey, keyOf, keyParts, minuteOfDay, weekdayOf } from '../../shared/time/zone';
import * as copy from './copy';
import type { Id, Task, TaskOccurrence } from './records';

export type Freq = 'DAILY' | 'WEEKLY' | 'MONTHLY' | 'YEARLY';

export interface Rule {
  freq: Freq;
  interval: number;
  /** Weekdays, 0 for Sunday. */
  byDay: number[];
  /** Days of the month; -1 is the last. */
  byMonthDay: number[];
  count?: number;
  until?: { instant: number } | { day: DayKey };
}

export interface Occurrence {
  date: DayKey;
  at: number;
}

const WEEKDAY_CODES = ['SU', 'MO', 'TU', 'WE', 'TH', 'FR', 'SA'];
const FREQS: Freq[] = ['DAILY', 'WEEKLY', 'MONTHLY', 'YEARLY'];
// A rule that never matches (the 31st, every 12 months, from February) would
// otherwise search forever; a hundred years is past any plan.
const HORIZON_DAYS = 36_525;

export const RULE_PRESETS: { label: string; rule: string | null }[] = [
  { label: copy.repeat.none, rule: null },
  { label: copy.repeat.daily, rule: 'FREQ=DAILY' },
  { label: copy.repeat.weekdays, rule: 'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR' },
  { label: copy.repeat.weekly, rule: 'FREQ=WEEKLY' },
  { label: copy.repeat.monthly, rule: 'FREQ=MONTHLY' },
  { label: copy.repeat.custom, rule: null },
];

/** The preset a stored rule shows as; any other rule is "Custom…", as in the PKM. */
export function presetOf(rule: string | undefined): string {
  if (!rule) return copy.repeat.none;
  return RULE_PRESETS.find((p) => p.rule === rule)?.label ?? copy.repeat.custom;
}

function parseUntil(v: string): Rule['until'] | null {
  const m = /^(\d{4})(\d{2})(\d{2})(?:T(\d{2})(\d{2})(\d{2})Z)?$/.exec(v);
  if (!m) return null;
  const [, y, mo, d, h, mi, s] = m;
  if (h === undefined) return { day: `${y}-${mo}-${d}` };
  return { instant: Date.UTC(+y, +mo - 1, +d, +h, +mi, +s) };
}

const isInt = (v: string) => /^-?\d+$/.test(v);

/** Reads a stored rule, with or without "RRULE:"; null when it is outside the subset. */
export function parseRule(text: string): Rule | null {
  const body = text.trim().replace(/^RRULE:/i, '');
  if (!body) return null;
  const rule: Partial<Rule> = { interval: 1, byDay: [], byMonthDay: [] };
  for (const part of body.split(';')) {
    const [key, value = ''] = part.split('=');
    switch (key.toUpperCase()) {
      case 'FREQ': {
        const f = value.toUpperCase() as Freq;
        if (!FREQS.includes(f)) return null;
        rule.freq = f;
        break;
      }
      case 'INTERVAL':
        if (!isInt(value) || +value < 1) return null;
        rule.interval = +value;
        break;
      case 'BYDAY': {
        const days = value.toUpperCase().split(',').map((c) => WEEKDAY_CODES.indexOf(c));
        if (days.some((d) => d < 0)) return null;
        rule.byDay = [...new Set(days)].sort((a, b) => a - b);
        break;
      }
      case 'BYMONTHDAY': {
        const days = value.split(',');
        if (days.some((d) => !isInt(d) || +d === 0 || Math.abs(+d) > 31)) return null;
        rule.byMonthDay = days.map(Number);
        break;
      }
      case 'COUNT':
        if (!isInt(value) || +value < 1) return null;
        rule.count = +value;
        break;
      case 'UNTIL': {
        const until = parseUntil(value);
        if (!until) return null;
        rule.until = until;
        break;
      }
      default:
        return null;
    }
  }
  if (!rule.freq) return null;
  // RFC 5545 forbids both of these.
  if (rule.count !== undefined && rule.until !== undefined) return null;
  if (rule.freq === 'WEEKLY' && rule.byMonthDay!.length > 0) return null;
  return rule as Rule;
}

// The days of the k-th period of the series, in order.
function periodDays(rule: Rule, start: DayKey, k: number): DayKey[] {
  const s = keyParts(start);
  const step = k * rule.interval;
  switch (rule.freq) {
    case 'DAILY':
      return [addDays(start, step)];
    case 'WEEKLY': {
      // Weeks start on Monday, RFC 5545's default WKST.
      const monday = addDays(start, -((weekdayOf(start) + 6) % 7) + 7 * step);
      return Array.from({ length: 7 }, (_, i) => addDays(monday, i));
    }
    case 'MONTHLY': {
      const m = s.month - 1 + step;
      const year = s.year + Math.floor(m / 12);
      const month = (m % 12) + 1;
      return Array.from({ length: daysInMonth(year, month) }, (_, i) => keyOf(year, month, i + 1));
    }
    case 'YEARLY': {
      const year = s.year + step;
      // With no BYDAY or BYMONTHDAY, a yearly rule falls on the start's date.
      if (rule.byDay.length === 0 && rule.byMonthDay.length === 0) {
        return s.day <= daysInMonth(year, s.month) ? [keyOf(year, s.month, s.day)] : [];
      }
      const first = keyOf(year, 1, 1);
      const length = daysInMonth(year, 2) === 29 ? 366 : 365;
      return Array.from({ length }, (_, i) => addDays(first, i));
    }
  }
}

function matches(rule: Rule, start: DayKey, day: DayKey): boolean {
  const byDay = rule.freq === 'WEEKLY' && rule.byDay.length === 0 ? [weekdayOf(start)] : rule.byDay;
  if (byDay.length > 0 && !byDay.includes(weekdayOf(day))) return false;
  const byMonthDay =
    rule.freq === 'MONTHLY' && rule.byDay.length === 0 && rule.byMonthDay.length === 0
      ? [keyParts(start).day]
      : rule.byMonthDay;
  if (byMonthDay.length === 0) return true;
  const { year, month, day: d } = keyParts(day);
  const last = daysInMonth(year, month);
  return byMonthDay.some((md) => (md > 0 ? md === d : last + md + 1 === d));
}

/** Every occurrence of the series, in order, until COUNT, UNTIL or the horizon. */
function* expand(rule: Rule, start: { day: DayKey; minute: number }, tz: string): Generator<Occurrence> {
  const end = addDays(start.day, HORIZON_DAYS);
  let emitted = 0;
  for (let k = 0; ; k++) {
    const days = periodDays(rule, start.day, k);
    if (days.length > 0 && days[0] > end) return;
    for (const date of days) {
      if (date < start.day || !matches(rule, start.day, date)) continue;
      const at = atMinute(date, start.minute, tz);
      if (rule.until && ('day' in rule.until ? date > rule.until.day : at > rule.until.instant)) return;
      yield { date, at };
      emitted += 1;
      if (rule.count !== undefined && emitted >= rule.count) return;
    }
  }
}

/** The occurrences whose dates fall in [from, to], both inclusive. */
export function occurrencesBetween(
  rule: Rule,
  start: { day: DayKey; minute: number },
  tz: string,
  from: DayKey,
  to: DayKey,
): Occurrence[] {
  const out: Occurrence[] = [];
  for (const o of expand(rule, start, tz)) {
    if (o.date > to) break;
    if (o.date >= from) out.push(o);
  }
  return out;
}

/** Where a task's series starts: its due date and time, else its scheduled date. */
export function seriesStart(task: Task, tz: string): { day: DayKey; minute: number } | null {
  if (task.due !== null) return { day: dayKey(task.due, tz), minute: minuteOfDay(task.due, tz) };
  if (task.scheduledDate) return { day: task.scheduledDate, minute: 0 };
  return null;
}

/** A recurring task's occurrences in [from, to]; empty for a task without a readable rule. */
export function taskOccurrences(task: Task, tz: string, from: DayKey, to: DayKey): Occurrence[] {
  const rule = task.rrule ? parseRule(task.rrule) : null;
  const start = seriesStart(task, tz);
  return rule && start ? occurrencesBetween(rule, start, tz, from, to) : [];
}

/**
 * The next occurrence still to do: the earliest one from today on that has no
 * TaskOccurrence row. Today's stays until midnight, so it can read "1h
 * overdue"; an earlier day's that went by unticked is left behind rather
 * than staying overdue for ever.
 */
export function nextOpenOccurrence(
  task: Task,
  occurrences: readonly TaskOccurrence[],
  now: number,
  tz: string,
): Occurrence | null {
  const rule = task.rrule ? parseRule(task.rrule) : null;
  const start = seriesStart(task, tz);
  if (!rule || !start) return null;
  const today = dayKey(now, tz);
  const done = new Set(occurrences.filter((o) => o.taskId === task.id).map((o) => o.date));
  for (const o of expand(rule, start, tz)) {
    if (o.date >= today && !done.has(o.date)) return o;
  }
  return null;
}

/**
 * The task as heat sees it: a recurring task's due becomes its next open
 * occurrence (null once the series has ended). A task without a rule comes
 * back unchanged.
 */
export function withEffectiveDue(task: Task, occurrences: readonly TaskOccurrence[], now: number, tz: string): Task {
  if (!task.rrule) return task;
  const next = task.due === null ? null : nextOpenOccurrence(task, occurrences, now, tz);
  return { ...task, due: next ? next.at : null };
}

/** Checking a recurring task ticks its next open occurrence as a row of its own. */
export function checkOccurrence(
  task: Task,
  occurrences: readonly TaskOccurrence[],
  now: number,
  tz: string,
  newId: () => Id,
): TaskOccurrence | null {
  const next = nextOpenOccurrence(task, occurrences, now, tz);
  return next ? { id: newId(), taskId: task.id, date: next.date, doneAt: now } : null;
}

/** Unticking an occurrence removes its row. */
export function reopenOccurrence(occurrences: readonly TaskOccurrence[], taskId: Id, date: DayKey): TaskOccurrence[] {
  return occurrences.filter((o) => !(o.taskId === taskId && o.date === date));
}
