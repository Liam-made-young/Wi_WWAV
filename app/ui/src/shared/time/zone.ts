// Time zones with Intl and nothing else. An instant is epoch ms; a day is a
// "YYYY-MM-DD" key in the person's zone (the system's, 3.11). Day keys are
// plain calendar dates, so their arithmetic runs in UTC and never meets DST.

export type DayKey = string;

export interface WallTime {
  year: number;
  month: number; // 1-12
  day: number;
  hour: number;
  minute: number;
  second: number;
}

const DAY_MS = 86_400_000;
const formatters = new Map<string, Intl.DateTimeFormat>();

function formatter(tz: string): Intl.DateTimeFormat {
  let f = formatters.get(tz);
  if (!f) {
    f = new Intl.DateTimeFormat('en-US', {
      timeZone: tz,
      hourCycle: 'h23',
      year: 'numeric',
      month: 'numeric',
      day: 'numeric',
      hour: 'numeric',
      minute: 'numeric',
      second: 'numeric',
    });
    formatters.set(tz, f);
  }
  return f;
}

/** The wall clock in `tz` at instant `ms`. */
export function wallTime(ms: number, tz: string): WallTime {
  const out: WallTime = { year: 0, month: 0, day: 0, hour: 0, minute: 0, second: 0 };
  for (const p of formatter(tz).formatToParts(ms)) {
    if (p.type in out) out[p.type as keyof WallTime] = Number(p.value);
  }
  return out;
}

// How far the wall clock runs ahead of UTC at an instant.
function offsetAt(ms: number, tz: string): number {
  const w = wallTime(ms, tz);
  const asUtc = Date.UTC(w.year, w.month - 1, w.day, w.hour, w.minute, w.second);
  return asUtc - Math.floor(ms / 1000) * 1000;
}

/**
 * The instant a wall clock names in `tz`. A time that happens twice (the
 * autumn hour) is the first one. A time that never happens (the spring gap)
 * is read with the offset before the gap, so it lands that far after it, as
 * RFC 5545 reads it.
 */
export function epochOf(
  w: { year: number; month: number; day: number; hour: number; minute: number; second?: number },
  tz: string,
): number {
  const asUtc = Date.UTC(w.year, w.month - 1, w.day, w.hour, w.minute, w.second ?? 0);
  const before = offsetAt(asUtc - DAY_MS, tz);
  const after = offsetAt(asUtc + DAY_MS, tz);
  const valid = [before, after]
    .map((o) => asUtc - o)
    .filter((t) => offsetAt(t, tz) === asUtc - t)
    .sort((a, b) => a - b);
  return valid.length > 0 ? valid[0] : asUtc - before;
}

function pad(n: number): string {
  return String(n).padStart(2, '0');
}

export function keyOf(year: number, month: number, day: number): DayKey {
  return `${year}-${pad(month)}-${pad(day)}`;
}

export function keyParts(key: DayKey): { year: number; month: number; day: number } {
  const [year, month, day] = key.split('-').map(Number);
  return { year, month, day };
}

/** The day an instant falls on in `tz`. */
export function dayKey(ms: number, tz: string): DayKey {
  const w = wallTime(ms, tz);
  return keyOf(w.year, w.month, w.day);
}

/** Minutes after local midnight, by the wall clock. */
export function minuteOfDay(ms: number, tz: string): number {
  const w = wallTime(ms, tz);
  return w.hour * 60 + w.minute;
}

function keyToUtc(key: DayKey): number {
  const { year, month, day } = keyParts(key);
  return Date.UTC(year, month - 1, day);
}

function utcToKey(ms: number): DayKey {
  const d = new Date(ms);
  return keyOf(d.getUTCFullYear(), d.getUTCMonth() + 1, d.getUTCDate());
}

export function addDays(key: DayKey, n: number): DayKey {
  return utcToKey(keyToUtc(key) + n * DAY_MS);
}

/** Whole days from `a` to `b`. */
export function daysBetween(a: DayKey, b: DayKey): number {
  return Math.round((keyToUtc(b) - keyToUtc(a)) / DAY_MS);
}

/** 0 is Sunday, as the calendar's weeks start (3.1). */
export function weekdayOf(key: DayKey): number {
  return new Date(keyToUtc(key)).getUTCDay();
}

export function daysInMonth(year: number, month: number): number {
  return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

/** The instant a day's wall clock reads `minutes` after midnight; 1440 is the next midnight. */
export function atMinute(key: DayKey, minutes: number, tz: string): number {
  const day = addDays(key, Math.floor(minutes / 1440));
  const m = ((minutes % 1440) + 1440) % 1440;
  const { year, month, day: d } = keyParts(day);
  return epochOf({ year, month, day: d, hour: Math.floor(m / 60), minute: m % 60 }, tz);
}

export function startOfDay(key: DayKey, tz: string): number {
  return atMinute(key, 0, tz);
}
