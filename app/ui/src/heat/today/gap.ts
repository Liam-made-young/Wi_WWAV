// Where a block goes when a task is placed rather than dropped (docs/SPEC.md
// 3.5). P puts the selected task into the next free gap after now: the first
// 15-minute mark, from now on, where a block as long as the task's estimate
// fits before midnight and clashes with no block or timed event. Dropping on
// the column places by position; this places by gap. Plan my day is the
// core's, not this.

import { addDays, type DayKey, minuteOfDay, startOfDay } from '../../shared/time/zone';
import type { CalendarEvent, TimeBlock } from '../client';

export const HOUR_PX = 44;
export const SNAP_MIN = 15;
/** The column runs from 7 AM to midnight. */
export const COLUMN_START = 7 * 60;
export const COLUMN_END = 24 * 60;
export const COLUMN_HEIGHT = ((COLUMN_END - COLUMN_START) / 60) * HOUR_PX;

export type Span = [number, number];

const roundUp = (min: number) => Math.ceil(min / SNAP_MIN) * SNAP_MIN;
const clamp = (x: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, x));

export const minutesToY = (min: number) => ((min - COLUMN_START) / 60) * HOUR_PX;
export const yToMinutes = (y: number) => COLUMN_START + (y / HOUR_PX) * 60;
/** The nearest 15-minute mark, as a drag snaps. */
export const snap = (min: number) => Math.round(min / SNAP_MIN) * SNAP_MIN;

/** A block as long as an estimate, rounded up to 15 minutes. */
export function blockLength(estimateMin: number): number {
  return Math.max(SNAP_MIN, roundUp(estimateMin));
}

/** The minutes of `date` already spoken for: every block and every timed event. */
export function busySpans(
  blocks: readonly TimeBlock[],
  events: readonly CalendarEvent[],
  date: DayKey,
  tz: string,
): Span[] {
  const dayStart = startOfDay(date, tz);
  const dayEnd = startOfDay(addDays(date, 1), tz);
  const spans: Span[] = blocks.filter((b) => b.date === date).map((b) => [b.start, b.start + b.minutes]);
  for (const e of events) {
    if (e.allDay || e.end <= dayStart || e.start >= dayEnd) continue;
    spans.push([e.start <= dayStart ? 0 : minuteOfDay(e.start, tz), e.end >= dayEnd ? 1440 : minuteOfDay(e.end, tz)]);
  }
  return spans;
}

/** The first 15-minute mark from `from` on where `length` fits before `until`, or null. */
export function firstGap(spans: readonly Span[], from: number, until: number, length: number): number | null {
  for (let s = roundUp(from); s + length <= until; s += SNAP_MIN) {
    if (spans.every(([a, b]) => s + length <= a || s >= b)) return s;
  }
  return null;
}

/** P: where a block of `length` minutes goes today, after now and never before the column's 7 AM. */
export function nextGap(
  blocks: readonly TimeBlock[],
  events: readonly CalendarEvent[],
  date: DayKey,
  tz: string,
  nowMin: number,
  length: number,
): number | null {
  return firstGap(busySpans(blocks, events, date, tz), Math.max(COLUMN_START, nowMin), COLUMN_END, length);
}

/** A task dropped on the column at `y` (px from the column's top): its block starts on the nearest mark, inside the column. */
export function dropStart(y: number, length: number): number {
  return clamp(snap(yToMinutes(y)), COLUMN_START, COLUMN_END - length);
}

/** Dragging a block's bottom edge to `bottomY`: the new length, never under 15 minutes or past midnight. */
export function resizedMinutes(start: number, bottomY: number): number {
  return clamp(snap(yToMinutes(bottomY) - start), SNAP_MIN, COLUMN_END - start);
}

/** Dragging a block's body so its top lands at `topY`. */
export function movedStart(minutes: number, topY: number): number {
  return clamp(snap(yToMinutes(topY)), COLUMN_START, COLUMN_END - minutes);
}

/** Scroll the column so now sits a third of the way down. */
export function initialScroll(nowMin: number, viewportPx: number): number {
  return clamp(minutesToY(nowMin) - viewportPx / 3, 0, Math.max(0, COLUMN_HEIGHT - viewportPx));
}

/** Where the 1 px red line marks the current minute; null outside the column. */
export function nowLineY(nowMin: number): number | null {
  return nowMin < COLUMN_START || nowMin >= COLUMN_END ? null : minutesToY(nowMin);
}
