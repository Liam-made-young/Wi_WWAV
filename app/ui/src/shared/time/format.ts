// Dates and times in the words the app prints. The clock is built by hand
// rather than by Intl's formatter, whose space before "PM" changed to a
// narrow no-break space in newer ICU releases.

import { type DayKey, keyParts, minuteOfDay, weekdayOf } from './zone';

export const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
export const MONTHS = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
];

/** "4:00 PM" for minutes after midnight. */
export function clock(minutes: number): string {
  const m = ((Math.round(minutes) % 1440) + 1440) % 1440;
  const h = Math.floor(m / 60);
  const h12 = h % 12 === 0 ? 12 : h % 12;
  return `${h12}:${String(m % 60).padStart(2, '0')} ${h < 12 ? 'AM' : 'PM'}`;
}

/** "11:59 PM" for an instant in `tz`. */
export function clockAt(ms: number, tz: string): string {
  return clock(minuteOfDay(ms, tz));
}

/** "Tuesday, October 6" */
export function longDay(key: DayKey): string {
  return `${WEEKDAYS[weekdayOf(key)]}, ${monthDay(key)}`;
}

/** "August 26" */
export function monthDay(key: DayKey): string {
  const { month, day } = keyParts(key);
  return `${MONTHS[month - 1]} ${day}`;
}

/** "Oct 21" */
export function shortMonthDay(key: DayKey): string {
  const { month, day } = keyParts(key);
  return `${MONTHS[month - 1].slice(0, 3)} ${day}`;
}

/** "24:59": time left, rounded up to the second so 0:00 shows only at the end. */
export function countdown(ms: number): string {
  const s = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}
