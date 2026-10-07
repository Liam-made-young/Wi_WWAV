import { describe, expect, it } from 'vitest';
import { clock, clockAt, countdown, longDay, monthDay, shortMonthDay } from './format';

describe('clock times', () => {
  it('writes minutes after midnight as a 12-hour clock', () => {
    expect(clock(0)).toBe('12:00 AM');
    expect(clock(9 * 60)).toBe('9:00 AM');
    expect(clock(12 * 60)).toBe('12:00 PM');
    expect(clock(16 * 60)).toBe('4:00 PM');
    expect(clock(15 * 60 + 30)).toBe('3:30 PM');
    expect(clock(23 * 60 + 59)).toBe('11:59 PM');
    expect(clock(24 * 60)).toBe('12:00 AM');
  });

  it('writes an instant in its zone with a plain space before AM or PM', () => {
    expect(clockAt(Date.parse('2026-10-08T03:59:00Z'), 'America/New_York')).toBe('11:59 PM');
    expect(clockAt(Date.parse('2026-10-06T19:41:00Z'), 'America/New_York')).toBe('3:41 PM');
  });
});

describe('dates', () => {
  it('writes the long day, the month day and the short month day', () => {
    expect(longDay('2026-10-06')).toBe('Tuesday, October 6');
    expect(monthDay('2026-08-26')).toBe('August 26');
    expect(shortMonthDay('2026-10-21')).toBe('Oct 21');
  });
});

describe('countdown', () => {
  it('rounds up to the second, so 0:00 shows only at the end', () => {
    expect(countdown(25 * 60_000)).toBe('25:00');
    expect(countdown(25 * 60_000 - 1)).toBe('25:00');
    expect(countdown(25 * 60_000 - 1000)).toBe('24:59');
    expect(countdown(5 * 60_000)).toBe('5:00');
    expect(countdown(18 * 60_000 + 40_000)).toBe('18:40');
    expect(countdown(0)).toBe('0:00');
    expect(countdown(-5)).toBe('0:00');
  });
});
