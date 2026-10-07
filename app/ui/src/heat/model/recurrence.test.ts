import { describe, expect, it } from 'vitest';
import { heatOf } from './heat';
import {
  RULE_PRESETS,
  checkOccurrence,
  nextOpenOccurrence,
  occurrencesBetween,
  openTasks,
  parseRule,
  presetOf,
  reopenOccurrence,
  withEffectiveDue,
} from './recurrence';
import type { TaskOccurrence } from './records';
import { NY, ids, ny, task } from './testkit';

// The series' first occurrence: a day and a minute of that day, in New York.
const dates = (rrule: string, day: string, minute: number, from: string, to: string) =>
  occurrencesBetween(parseRule(rrule)!, { day, minute }, NY, from, to).map((o) => o.date);

describe('parsing the RRULE subset', () => {
  it('reads FREQ, INTERVAL, BYDAY, BYMONTHDAY, COUNT and UNTIL', () => {
    expect(parseRule('FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR')).toEqual({
      freq: 'WEEKLY',
      interval: 1,
      byDay: [1, 2, 3, 4, 5],
      byMonthDay: [],
    });
    expect(parseRule('RRULE:FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=1,-1;COUNT=6')).toEqual({
      freq: 'MONTHLY',
      interval: 2,
      byDay: [],
      byMonthDay: [1, -1],
      count: 6,
    });
    expect(parseRule('FREQ=DAILY;UNTIL=20261231T235959Z')?.until).toEqual({
      instant: Date.parse('2026-12-31T23:59:59Z'),
    });
    expect(parseRule('FREQ=DAILY;UNTIL=20261231')?.until).toEqual({ day: '2026-12-31' });
  });

  it('refuses what Learn does not handle rather than guessing', () => {
    for (const bad of [
      '',
      'nonsense',
      'FREQ=HOURLY',
      'FREQ=MONTHLY;BYDAY=2TU',
      'FREQ=WEEKLY;BYSETPOS=1',
      'FREQ=WEEKLY;BYMONTHDAY=3',
      'FREQ=DAILY;INTERVAL=0',
      'FREQ=DAILY;COUNT=2;UNTIL=20261231',
      'FREQ=MONTHLY;BYMONTHDAY=32',
      'INTERVAL=2',
    ]) {
      expect(parseRule(bad), bad).toBeNull();
    }
  });

  it('names the PKM presets, with "Every weekday"', () => {
    expect(RULE_PRESETS.map((p) => p.label)).toEqual([
      'Does not repeat',
      'Daily',
      'Every weekday',
      'Weekly',
      'Monthly',
      'Custom…',
    ]);
    expect(presetOf('FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR')).toBe('Every weekday');
    expect(presetOf(undefined)).toBe('Does not repeat');
    expect(presetOf('FREQ=WEEKLY;INTERVAL=2;BYDAY=TU')).toBe('Custom…');
  });
});

describe('expanding occurrences', () => {
  it('repeats daily, and every other day up to a count', () => {
    expect(dates('FREQ=DAILY', '2026-10-06', 0, '2026-10-06', '2026-10-09')).toEqual([
      '2026-10-06',
      '2026-10-07',
      '2026-10-08',
      '2026-10-09',
    ]);
    expect(dates('FREQ=DAILY;INTERVAL=2;COUNT=3', '2026-10-06', 0, '2026-10-01', '2026-12-31')).toEqual([
      '2026-10-06',
      '2026-10-08',
      '2026-10-10',
    ]);
  });

  it('counts COUNT from the start of the series, not the window', () => {
    expect(dates('FREQ=DAILY;COUNT=5', '2026-10-06', 0, '2026-10-09', '2026-10-20')).toEqual([
      '2026-10-09',
      '2026-10-10',
    ]);
  });

  it('repeats weekly on the start’s weekday, or on BYDAY, never before the start', () => {
    expect(dates('FREQ=WEEKLY', '2026-10-06', 0, '2026-10-01', '2026-10-20')).toEqual([
      '2026-10-06',
      '2026-10-13',
      '2026-10-20',
    ]);
    expect(dates('FREQ=WEEKLY;BYDAY=MO,WE', '2026-10-06', 0, '2026-10-01', '2026-10-14')).toEqual([
      '2026-10-07',
      '2026-10-12',
      '2026-10-14',
    ]);
  });

  it('counts INTERVAL in weeks that start on Monday', () => {
    expect(dates('FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH', '2026-10-06', 0, '2026-10-01', '2026-10-31')).toEqual([
      '2026-10-06',
      '2026-10-08',
      '2026-10-20',
      '2026-10-22',
    ]);
  });

  it('gives "Every weekday" Monday to Friday', () => {
    expect(dates('FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR', '2026-10-09', 0, '2026-10-09', '2026-10-19')).toEqual([
      '2026-10-09',
      '2026-10-12',
      '2026-10-13',
      '2026-10-14',
      '2026-10-15',
      '2026-10-16',
      '2026-10-19',
    ]);
  });

  it('repeats monthly on the start’s day, skipping months without it', () => {
    expect(dates('FREQ=MONTHLY', '2026-01-31', 0, '2026-01-01', '2026-06-30')).toEqual([
      '2026-01-31',
      '2026-03-31',
      '2026-05-31',
    ]);
  });

  it('takes BYMONTHDAY, counting negatives from the month’s end', () => {
    expect(dates('FREQ=MONTHLY;BYMONTHDAY=1,15', '2026-10-06', 0, '2026-10-01', '2026-11-30')).toEqual([
      '2026-10-15',
      '2026-11-01',
      '2026-11-15',
    ]);
    expect(dates('FREQ=MONTHLY;BYMONTHDAY=-1', '2026-01-10', 0, '2026-01-01', '2026-04-30')).toEqual([
      '2026-01-31',
      '2026-02-28',
      '2026-03-31',
      '2026-04-30',
    ]);
  });

  it('takes BYDAY and BYMONTHDAY together as both', () => {
    expect(dates('FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13', '2026-01-01', 0, '2026-01-01', '2026-12-31')).toEqual([
      '2026-02-13',
      '2026-03-13',
      '2026-11-13',
    ]);
  });

  it('repeats yearly, with February 29 only in leap years', () => {
    expect(dates('FREQ=YEARLY', '2026-10-06', 0, '2026-01-01', '2028-12-31')).toEqual([
      '2026-10-06',
      '2027-10-06',
      '2028-10-06',
    ]);
    expect(dates('FREQ=YEARLY', '2028-02-29', 0, '2028-01-01', '2032-12-31')).toEqual(['2028-02-29', '2032-02-29']);
  });

  it('stops at UNTIL, a day or an instant, inclusive', () => {
    expect(dates('FREQ=DAILY;UNTIL=20261008', '2026-10-06', 23 * 60 + 59, '2026-10-01', '2026-10-31')).toEqual([
      '2026-10-06',
      '2026-10-07',
      '2026-10-08',
    ]);
    // 03:59 UTC on October 8 is 11:59 PM on October 7 in New York.
    expect(dates('FREQ=DAILY;UNTIL=20261008T035900Z', '2026-10-06', 23 * 60 + 59, '2026-10-01', '2026-10-31')).toEqual([
      '2026-10-06',
      '2026-10-07',
    ]);
  });

  it('keeps the wall-clock time across DST', () => {
    const fridays = occurrencesBetween(
      parseRule('FREQ=WEEKLY')!,
      { day: '2026-10-23', minute: 23 * 60 + 59 },
      NY,
      '2026-10-01',
      '2026-11-13',
    );
    expect(fridays.map((o) => o.at)).toEqual([
      ny('2026-10-23 23:59'),
      ny('2026-10-30 23:59'),
      ny('2026-11-06 23:59'),
      ny('2026-11-13 23:59'),
    ]);
    // Across November 1 the gap is a week and an hour.
    expect(fridays[2].at - fridays[1].at).toBe(7 * 86_400_000 + 3_600_000);
  });

  it('moves a time inside the spring gap forward by the gap', () => {
    const days = occurrencesBetween(
      parseRule('FREQ=DAILY')!,
      { day: '2026-03-07', minute: 2 * 60 + 30 },
      NY,
      '2026-03-07',
      '2026-03-09',
    );
    expect(days.map((o) => o.at)).toEqual([ny('2026-03-07 02:30'), ny('2026-03-08 03:30'), ny('2026-03-09 02:30')]);
  });
});

describe('recurring tasks', () => {
  // A quiz every Friday at 11:59 PM, from October 2.
  const quiz = task({ title: 'Weekly quiz', difficulty: 2, due: ny('2026-10-02 23:59'), rrule: 'FREQ=WEEKLY' });

  it('take their heat from the next occurrence', () => {
    const now = ny('2026-10-08 08:00');
    expect(nextOpenOccurrence(quiz, [], now, NY)).toEqual({ date: '2026-10-09', at: ny('2026-10-09 23:59') });
    const effective = withEffectiveDue(quiz, [], now, NY);
    expect(effective.due).toBe(ny('2026-10-09 23:59'));
    // Not Overdue from October 2: 1.67 days left at difficulty 2 is Warm.
    expect(heatOf(quiz, now).level).toBe('Overdue');
    expect(heatOf(effective, now).level).toBe('Warm');
  });

  it('skip a finished occurrence, and leave past days behind', () => {
    const done: TaskOccurrence[] = [{ id: 'o1', taskId: quiz.id, date: '2026-10-09', doneAt: ny('2026-10-09 20:00') }];
    expect(nextOpenOccurrence(quiz, done, ny('2026-10-08 08:00'), NY)?.date).toBe('2026-10-16');
    // October 9 went by unticked; on the 12th the next is the 16th, not a week overdue.
    expect(nextOpenOccurrence(quiz, [], ny('2026-10-12 08:00'), NY)?.date).toBe('2026-10-16');
  });

  it('keep today’s occurrence until midnight, overdue once its time passes', () => {
    const fourPm = task({ due: ny('2026-10-05 16:00'), rrule: 'FREQ=DAILY' });
    const now = ny('2026-10-06 17:00');
    const effective = withEffectiveDue(fourPm, [], now, NY);
    expect(effective.due).toBe(ny('2026-10-06 16:00'));
    expect(heatOf(effective, now).level).toBe('Overdue');
  });

  it('end with the series', () => {
    const three = task({ due: ny('2026-10-01 09:00'), rrule: 'FREQ=DAILY;COUNT=3' });
    expect(nextOpenOccurrence(three, [], ny('2026-10-06 08:00'), NY)).toBeNull();
    expect(withEffectiveDue(three, [], ny('2026-10-06 08:00'), NY).due).toBeNull();
  });

  it('tick as rows of their own, and the series never flips to done', () => {
    const now = ny('2026-10-08 08:00');
    const newId = ids('occ');
    const first = checkOccurrence(quiz, [], now, NY, newId)!;
    expect(first).toEqual({ id: 'occ-1', taskId: quiz.id, date: '2026-10-09', doneAt: now });
    const second = checkOccurrence(quiz, [first], now, NY, newId)!;
    expect(second.date).toBe('2026-10-16');
    expect(quiz.done).toBe(false);
    expect(withEffectiveDue(quiz, [first, second], now, NY).done).toBe(false);
    expect(reopenOccurrence([first, second], quiz.id, '2026-10-09')).toEqual([second]);
  });

  it('leave a task without a rule as it is', () => {
    const plain = task({ due: ny('2026-10-09 23:59') });
    expect(withEffectiveDue(plain, [], ny('2026-10-06 08:00'), NY)).toBe(plain);
  });

  it('run a series without a due date from its scheduled date', () => {
    const scales = task({ scheduledDate: '2026-10-05', rrule: 'FREQ=DAILY' });
    expect(nextOpenOccurrence(scales, [], ny('2026-10-06 08:00'), NY)).toEqual({
      date: '2026-10-06',
      at: ny('2026-10-06 00:00'),
    });
    expect(withEffectiveDue(scales, [], ny('2026-10-06 08:00'), NY).due).toBeNull();
  });
});

describe('openTasks', () => {
  it('keeps a task whose rule Learn can’t read as a plain task, rather than losing it', () => {
    const now = ny('2026-10-06 08:00');
    const odd = task({ due: ny('2026-10-09 23:59'), rrule: 'FREQ=HOURLY' });
    expect(openTasks([odd], [], now, NY)).toEqual([odd]);
    expect(withEffectiveDue(odd, [], now, NY)).toBe(odd);
  });

  it('drops done tasks and ended series, and moves a series’ due to its next occurrence', () => {
    const now = ny('2026-10-06 08:00');
    const plain = task({ due: ny('2026-10-09 23:59') });
    const done = task({ done: true, doneAt: now });
    const ended = task({ due: ny('2026-10-01 09:00'), rrule: 'FREQ=DAILY;COUNT=3' });
    const weekly = task({ due: ny('2026-10-02 23:59'), rrule: 'FREQ=WEEKLY' });
    const open = openTasks([plain, done, ended, weekly], [], now, NY);
    expect(open.map((t) => t.id)).toEqual([plain.id, weekly.id]);
    expect(open[1].due).toBe(ny('2026-10-09 23:59'));
  });
});

describe('a window far from the start', () => {
  it('gives exactly what walking the whole series from its start gives', () => {
    const rules = [
      'FREQ=DAILY;INTERVAL=3',
      'FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH',
      'FREQ=WEEKLY;INTERVAL=3',
      'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR',
      'FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=31',
      'FREQ=MONTHLY;BYMONTHDAY=-1',
      'FREQ=MONTHLY;INTERVAL=5',
      'FREQ=YEARLY;INTERVAL=4',
      'FREQ=YEARLY;BYMONTHDAY=15',
      'FREQ=DAILY;INTERVAL=2;UNTIL=20290301',
    ];
    const starts = ['2019-01-31', '2020-02-29', '2023-06-13'];
    const windows: [string, string][] = [
      ['2026-10-01', '2026-11-15'],
      ['2028-02-01', '2028-03-31'],
      ['2029-02-20', '2029-03-10'],
    ];
    for (const text of rules) {
      const rule = parseRule(text)!;
      for (const day of starts) {
        const start = { day, minute: 9 * 60 };
        for (const [from, to] of windows) {
          const walked = occurrencesBetween(rule, start, NY, day, to).filter((o) => o.date >= from);
          expect(occurrencesBetween(rule, start, NY, from, to), `${text} from ${day}, ${from}..${to}`).toEqual(walked);
        }
      }
    }
  });
});
