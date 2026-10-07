import { describe, expect, it } from 'vitest';
import {
  addDays,
  dayKey,
  daysBetween,
  epochOf,
  minuteOfDay,
  startOfDay,
  wallTime,
  weekdayOf,
  atMinute,
} from './zone';

const NY = 'America/New_York';

describe('wall time in a zone', () => {
  it('reads the wall clock of an instant', () => {
    // 1.6's morning: Tuesday, October 6, 8:40 AM in New York (EDT, UTC-4).
    const t = Date.parse('2026-10-06T12:40:00Z');
    expect(wallTime(t, NY)).toEqual({ year: 2026, month: 10, day: 6, hour: 8, minute: 40, second: 0 });
    expect(dayKey(t, NY)).toBe('2026-10-06');
    expect(minuteOfDay(t, NY)).toBe(8 * 60 + 40);
  });

  it('puts an instant on the day it is in that zone, not in UTC', () => {
    // 11:59 PM Wednesday in New York is already Thursday in UTC.
    const t = Date.parse('2026-10-08T03:59:00Z');
    expect(dayKey(t, NY)).toBe('2026-10-07');
    expect(dayKey(t, 'UTC')).toBe('2026-10-08');
  });

  it('turns a wall clock back into the same instant', () => {
    expect(epochOf({ year: 2026, month: 10, day: 7, hour: 23, minute: 59 }, NY)).toBe(
      Date.parse('2026-10-08T03:59:00Z'),
    );
    expect(epochOf({ year: 2026, month: 12, day: 1, hour: 9, minute: 0 }, NY)).toBe(
      Date.parse('2026-12-01T14:00:00Z'),
    );
  });

  it('moves a time that does not exist forward by the gap, as RFC 5545 does', () => {
    // March 8, 2026: 2:00 AM jumps to 3:00 AM, so 2:30 AM is read as 3:30 AM EDT.
    expect(epochOf({ year: 2026, month: 3, day: 8, hour: 2, minute: 30 }, NY)).toBe(
      Date.parse('2026-03-08T07:30:00Z'),
    );
  });

  it('takes the first of a time that happens twice', () => {
    // November 1, 2026: 1:30 AM happens in EDT, then again in EST.
    expect(epochOf({ year: 2026, month: 11, day: 1, hour: 1, minute: 30 }, NY)).toBe(
      Date.parse('2026-11-01T05:30:00Z'),
    );
  });

  it('finds local midnight and a minute of the day across a DST change', () => {
    expect(startOfDay('2026-11-01', NY)).toBe(Date.parse('2026-11-01T04:00:00Z'));
    expect(startOfDay('2026-11-02', NY)).toBe(Date.parse('2026-11-02T05:00:00Z'));
    expect(atMinute('2026-11-01', 23 * 60 + 59, NY)).toBe(Date.parse('2026-11-02T04:59:00Z'));
    expect(atMinute('2026-10-06', 24 * 60, NY)).toBe(startOfDay('2026-10-07', NY));
  });
});

describe('day keys', () => {
  it('adds days across months and years', () => {
    expect(addDays('2026-10-06', 1)).toBe('2026-10-07');
    expect(addDays('2026-10-31', 1)).toBe('2026-11-01');
    expect(addDays('2026-12-31', 1)).toBe('2027-01-01');
    expect(addDays('2028-03-01', -1)).toBe('2028-02-29');
    expect(addDays('2026-10-06', -400)).toBe('2025-09-01');
  });

  it('counts days between keys', () => {
    expect(daysBetween('2026-10-06', '2026-10-07')).toBe(1);
    expect(daysBetween('2026-10-06', '2026-08-26')).toBe(-41);
    expect(daysBetween('2026-03-07', '2026-03-09')).toBe(2);
  });

  it('gives the weekday from Sunday', () => {
    expect(weekdayOf('2026-10-04')).toBe(0);
    expect(weekdayOf('2026-10-06')).toBe(2);
    expect(weekdayOf('2026-10-10')).toBe(6);
  });
});
