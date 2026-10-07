// The days Calendar shows (docs/SPEC.md 3.7): a month grid that starts on the
// Sunday before the 1st and always has six weeks, a week from Sunday, or one
// day; paging by a month (to its 1st), a week or a day; and the title.

import { longDay, MONTHS, shortMonthDay } from '../../shared/time/format';
import { addDays, type DayKey, keyOf, keyParts, weekdayOf } from '../../shared/time/zone';

export type Mode = 'month' | 'week' | 'day';

/** The six weeks of `anchor`'s month, 42 days from the Sunday on or before its 1st. */
export function monthDays(anchor: DayKey): DayKey[] {
  const { year, month } = keyParts(anchor);
  const first = keyOf(year, month, 1);
  const start = addDays(first, -weekdayOf(first));
  return Array.from({ length: 42 }, (_, i) => addDays(start, i));
}

/** The week of `anchor`, Sunday first. */
export function weekDays(anchor: DayKey): DayKey[] {
  const sunday = addDays(anchor, -weekdayOf(anchor));
  return Array.from({ length: 7 }, (_, i) => addDays(sunday, i));
}

/** The days a mode shows around `anchor`. */
export function daysOf(mode: Mode, anchor: DayKey): DayKey[] {
  return mode === 'month' ? monthDays(anchor) : mode === 'week' ? weekDays(anchor) : [anchor];
}

/** ← and →: a month (to its 1st), a week or a day. */
export function page(mode: Mode, anchor: DayKey, dir: 1 | -1): DayKey {
  if (mode === 'day') return addDays(anchor, dir);
  if (mode === 'week') return addDays(anchor, 7 * dir);
  const { year, month } = keyParts(anchor);
  const m = month - 1 + dir;
  return keyOf(year + Math.floor(m / 12), (((m % 12) + 12) % 12) + 1, 1);
}

/** "October 2026", "Oct 4 – Oct 10", "Tuesday, October 6" */
export function titleOf(mode: Mode, anchor: DayKey): string {
  if (mode === 'day') return longDay(anchor);
  if (mode === 'week') {
    const days = weekDays(anchor);
    return `${shortMonthDay(days[0])} – ${shortMonthDay(days[6])}`;
  }
  const { year, month } = keyParts(anchor);
  return `${MONTHS[month - 1]} ${year}`;
}

/** Moves the selected day by `by` days, and says which anchor keeps it in view. */
export function moveSelected(
  mode: Mode,
  anchor: DayKey,
  selected: DayKey,
  by: number,
): { selected: DayKey; anchor: DayKey } {
  const next = addDays(selected, by);
  const shown = daysOf(mode, anchor);
  if (shown.includes(next) && (mode !== 'month' || keyParts(next).month === keyParts(anchor).month)) {
    return { selected: next, anchor };
  }
  return { selected: next, anchor: mode === 'month' ? keyOf(keyParts(next).year, keyParts(next).month, 1) : next };
}
