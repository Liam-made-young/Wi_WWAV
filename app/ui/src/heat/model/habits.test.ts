import { describe, expect, it } from 'vitest';
import { addDays } from '../../shared/time/zone';
import {
  HABIT_LIMIT,
  addHabit,
  doneRecord,
  habitLabel,
  habitLine,
  lastFourteen,
  streak,
  todayOrbs,
  toggleHabit,
  yearGrid,
} from './habits';
import type { Habit } from './records';
import { NY, habit, ids, ny } from './testkit';

const TODAY = '2026-10-06';

function logOf(days: string[]): Habit['log'] {
  return Object.fromEntries(days.map((d) => [d, true as const]));
}

describe('adding habits', () => {
  it('keeps the limit of 6: a 7th doesn’t fit, and says why', () => {
    expect(HABIT_LIMIT).toBe(6);
    const newId = ids('habit');
    let habits: Habit[] = [];
    for (let i = 0; i < 6; i++) {
      const r = addHabit(habits, { title: `Habit ${i}` }, newId);
      if (!('habit' in r)) throw new Error(r.error);
      habits = [...habits, r.habit];
    }
    expect(addHabit(habits, { title: 'One more' }, newId)).toEqual({ error: 'Habit limit reached' });
  });

  it('makes a habit with the counter off and an empty log', () => {
    expect(addHabit([], { title: 'Practise kanji', minutes: 20 }, ids('h'))).toEqual({
      habit: { id: 'h-1', title: 'Practise kanji', minutes: 20, log: {}, showCounter: false },
    });
  });

  it('asks for a name first', () => {
    expect(addHabit([], { title: '  ' }, ids())).toEqual({ error: 'Give the habit a name first.' });
  });

  it('writes a habit with a length as "Practise kanji, 20m"', () => {
    expect(habitLabel(habit({ title: 'Practise kanji', minutes: 20 }))).toBe('Practise kanji, 20m');
    expect(habitLabel(habit({ title: 'Stretch' }))).toBe('Stretch');
  });
});

describe('the log', () => {
  it('toggles a day on and off', () => {
    const h = habit();
    const on = toggleHabit(h, TODAY);
    expect(on.log).toEqual({ [TODAY]: true });
    expect(toggleHabit(on, TODAY).log).toEqual({});
  });

  it('is never pruned at 400 days', () => {
    const old = Array.from({ length: 800 }, (_, i) => addDays(TODAY, -i - 1));
    const h = toggleHabit(habit({ log: logOf(old) }), TODAY);
    expect(Object.keys(h.log)).toHaveLength(801);
    expect(h.log[addDays(TODAY, -800)]).toBe(true);
  });
});

describe('today’s orbs', () => {
  it('shows each habit’s orb for today and "3 of 5 done"', () => {
    const habits = [true, true, false, true, false].map((done, i) =>
      habit({ title: `h${i}`, log: done ? logOf([TODAY]) : {} }),
    );
    const { orbs, line } = todayOrbs(habits, ny('2026-10-06 21:00'), NY);
    expect(orbs.map((o) => o.done)).toEqual([true, true, false, true, false]);
    expect(line).toBe('3 of 5 done');
    expect(todayOrbs([], ny('2026-10-06 21:00'), NY).line).toBeNull();
  });

  it('reads today in the person’s zone', () => {
    // 9 PM in New York is already October 7 in UTC.
    const h = habit({ log: logOf([TODAY]) });
    expect(todayOrbs([h], ny('2026-10-06 21:00'), NY).orbs[0].done).toBe(true);
  });
});

describe('the grids', () => {
  const h = habit({ log: logOf(['2026-09-23', '2026-10-01', TODAY]) });

  it('draws the last 14 days, oldest first, ending today', () => {
    const days = lastFourteen(h, TODAY);
    expect(days).toHaveLength(14);
    expect(days[0]).toEqual({ date: '2026-09-23', done: true });
    expect(days[13]).toEqual({ date: TODAY, done: true });
    expect(days.filter((d) => d.done)).toHaveLength(3);
  });

  it('"Show the year" draws 53 weeks of 7 days from Sunday, ending this week, with nothing after today', () => {
    const grid = yearGrid(h, TODAY);
    expect(grid).toHaveLength(53);
    expect(grid.every((week) => week.length === 7)).toBe(true);
    // Today, a Tuesday, sits in the last column; the rest of its week is still to come.
    expect(grid[52].map((d) => d?.date ?? null)).toEqual(['2026-10-04', '2026-10-05', TODAY, null, null, null, null]);
    expect(grid[52][2]).toEqual({ date: TODAY, done: true });
    // The first column starts on the Sunday 52 weeks before this week's.
    expect(grid[0][0]).toEqual({ date: '2025-10-05', done: false });
  });
});

describe('the record and the counter (Open #9)', () => {
  const since = Array.from({ length: 41 }, (_, i) => addDays('2026-08-26', i));
  const h = habit({ log: logOf(since) });

  it('shows the growing record by default: "Done 41 days since August 26"', () => {
    expect(h.showCounter).toBe(false);
    expect(habitLine(h, TODAY)).toBe('Done 41 days since August 26');
    expect(doneRecord(habit({ log: logOf([TODAY]) }), TODAY)).toBe('Done 1 day since October 6');
    expect(doneRecord(habit({ log: logOf(['2025-08-26', TODAY]) }), TODAY)).toBe('Done 2 days since August 26, 2025');
    expect(doneRecord(habit(), TODAY)).toBe('Not done yet');
  });

  it('shows the streak counter only when switched on for that habit', () => {
    expect(habitLine({ ...h, showCounter: true }, '2026-10-05')).toBe('41-day streak');
  });

  it('keeps a streak alive until midnight', () => {
    const run = logOf(['2026-10-03', '2026-10-04', '2026-10-05']);
    // Not done yet today: the streak still stands on yesterday's.
    expect(streak(habit({ log: run }), TODAY)).toBe(3);
    expect(streak(habit({ log: { ...run, [TODAY]: true } }), TODAY)).toBe(4);
    // Missed yesterday: it is gone.
    expect(streak(habit({ log: logOf(['2026-10-03', '2026-10-04']) }), TODAY)).toBe(0);
    expect(habitLine(habit({ showCounter: true, log: logOf(['2026-10-01']) }), TODAY)).toBeNull();
  });
});
