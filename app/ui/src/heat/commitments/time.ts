// Minutes after midnight as a time field reads and writes them, and the
// Monday a week is named by (docs/COMMITMENTS.md: `start`, `end`, `weekOf`).

import { addDays, type DayKey, weekdayOf } from '../../shared/time/zone';

const pad = (n: number) => String(n).padStart(2, '0');

/** "10:00" for 600, as a time field reads it. */
export const timeField = (min: number) => `${pad(Math.floor(min / 60) % 24)}:${pad(min % 60)}`;

/** The minutes a time field names; null while it is empty or half typed. */
export function fieldMinutes(text: string): number | null {
  const m = /^(\d{1,2}):(\d{2})$/.exec(text.trim());
  if (!m) return null;
  const min = Number(m[1]) * 60 + Number(m[2]);
  return min < 1440 ? min : null;
}

/** The Monday of `day`'s week. */
export const mondayOf = (day: DayKey): DayKey => addDays(day, -((weekdayOf(day) + 6) % 7));
