// The heat algorithm, exactly as Heat runs it today (docs/SPEC.md 3.1):
//
//   done         -> no heat ("Done")
//   no due date  -> v = 0.05, "Cool"
//   days   = (due - now) / 86_400_000
//   days < 0     -> v = 1.1, "Overdue"
//   runway = difficulty * 2 + 1
//   v      = clamp(1 - days / runway, 0, 1)
//   v >= 0.70 -> "Hot";  v >= 0.34 -> "Warm";  else "Cool" (v floored at 0.05)
//
// A recurring task's heat comes from its next occurrence: pass it through
// recurrence.ts's withEffectiveDue first.

import tokens from '../../../../../design/tokens.json' with { type: 'json' };
import { clockAt, shortMonthDay, WEEKDAYS } from '../../shared/time/format';
import { dayKey, daysBetween, wallTime, weekdayOf } from '../../shared/time/zone';
import * as copy from './copy';

export const DAY_MS = 86_400_000;
const HOT = 0.7;
const WARM = 0.34;
const FLOOR = 0.05;
const OVERDUE = 1.1;

export type HeatLevel = 'Overdue' | 'Hot' | 'Warm' | 'Cool' | 'Done';

export interface Heat {
  level: HeatLevel;
  v: number | null;
}

export interface HeatInput {
  done: boolean;
  due: number | null;
  difficulty: number;
}

// The colours live in the token file, like every other (docs/SPEC.md 8.11).
export const LEVEL_COLOUR: Record<Exclude<HeatLevel, 'Done'>, string> = {
  Overdue: tokens.heat.overdue,
  Hot: tokens.heat.hot,
  Warm: tokens.heat.warm,
  Cool: tokens.heat.cool,
};

/** The badge: a glossy tube filled to v. */
export const TUBE = { width: 46, height: 11 };

/** A due date heat can read. One that isn't a finite number (a corrupt row) counts as none, so it can't upset the sort. */
const dated = (due: number | null): due is number => due !== null && Number.isFinite(due);

/** Days of runway: difficulty 1..5 gives 3, 5, 7, 9, 11. */
export function runway(difficulty: number): number {
  const d = Math.min(5, Math.max(1, Math.round(difficulty)));
  return d * 2 + 1;
}

export function heatOf(t: HeatInput, now: number): Heat {
  if (t.done) return { level: 'Done', v: null };
  if (!dated(t.due)) return { level: 'Cool', v: FLOOR };
  const days = (t.due - now) / DAY_MS;
  if (days < 0) return { level: 'Overdue', v: OVERDUE };
  const v = Math.min(1, Math.max(0, 1 - days / runway(t.difficulty)));
  if (v >= HOT) return { level: 'Hot', v };
  if (v >= WARM) return { level: 'Warm', v };
  return { level: 'Cool', v: Math.max(v, FLOOR) };
}

/** Days left when a task turns Warm and Hot, as 3.1's table prints them. */
export function heatThresholds(difficulty: number): { warmAt: number; hotAt: number } {
  const r = runway(difficulty);
  const round2 = (x: number) => Math.round(x * 100) / 100;
  return { warmAt: round2((1 - WARM) * r), hotAt: round2((1 - HOT) * r) };
}

/** How full the tube is: v, capped at full; a done task's tube is empty. */
export function tubeFill(h: Heat): number {
  return h.v === null ? 0 : Math.min(1, h.v);
}

/** Open tasks by heat descending, then by due date, undated last; done tasks after them. */
export function byHeat<T extends HeatInput>(items: readonly T[], now: number): T[] {
  const keyed = items.map((t) => ({ t, v: heatOf(t, now).v }));
  keyed.sort((a, b) => {
    if (a.v === null || b.v === null) return (a.v === null ? 1 : 0) - (b.v === null ? 1 : 0);
    if (a.v !== b.v) return b.v - a.v;
    if (!dated(a.t.due) || !dated(b.t.due)) return (dated(a.t.due) ? 0 : 1) - (dated(b.t.due) ? 0 : 1);
    return a.t.due - b.t.due;
  });
  return keyed.map((k) => k.t);
}

/** "Today 4:00 PM", "Tomorrow 11:59 PM", "Wednesday 11:59 PM", "3h overdue", "2d overdue". */
export function duePhrase(due: number, now: number, tz: string): string {
  if (due < now) {
    const minutes = Math.floor((now - due) / 60_000);
    if (minutes < 60) return copy.due.overdue(`${Math.max(1, minutes)}m`);
    if (minutes < 24 * 60) return copy.due.overdue(`${Math.floor(minutes / 60)}h`);
    return copy.due.overdue(`${Math.floor(minutes / (24 * 60))}d`);
  }
  const time = clockAt(due, tz);
  const day = dayKey(due, tz);
  const ahead = daysBetween(dayKey(now, tz), day);
  if (ahead === 0) return copy.due.today(time);
  if (ahead === 1) return copy.due.tomorrow(time);
  if (ahead <= 6) return copy.due.onDay(WEEKDAYS[weekdayOf(day)], time);
  const year = wallTime(due, tz).year;
  const date = year === wallTime(now, tz).year ? shortMonthDay(day) : `${shortMonthDay(day)}, ${year}`;
  return copy.due.onDay(date, time);
}

/**
 * The first millisecond after `now` at which any task's level changes, or
 * null when none will. Levels only climb (Cool, Warm, Hot, Overdue), so a
 * binary search between now and just past the due date finds the exact
 * millisecond, with v computed exactly as heatOf computes it.
 */
export function nextHeatChange(items: readonly HeatInput[], now: number): number | null {
  let soonest: number | null = null;
  for (const t of items) {
    if (t.done || !dated(t.due) || t.due < now) continue;
    const level = heatOf(t, now).level;
    let lo = now;
    let hi = t.due + 1;
    while (hi - lo > 1) {
      const mid = Math.floor((lo + hi) / 2);
      if (heatOf(t, mid).level === level) lo = mid;
      else hi = mid;
    }
    if (soonest === null || hi < soonest) soonest = hi;
  }
  return soonest;
}
