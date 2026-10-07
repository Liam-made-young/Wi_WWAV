import { describe, expect, it } from 'vitest';
import {
  DAY_MS,
  LEVEL_COLOUR,
  TUBE,
  byHeat,
  duePhrase,
  heatOf,
  heatThresholds,
  nextHeatChange,
  runway,
  tubeFill,
} from './heat';
import { NY, ny, task } from './testkit';

describe('the heat algorithm (3.1)', () => {
  const now = ny('2026-10-06 08:40');

  it('gives a done task no heat', () => {
    expect(heatOf(task({ done: true, due: now - DAY_MS }), now)).toEqual({ level: 'Done', v: null });
  });

  it('makes an undated task Cool at 0.05', () => {
    expect(heatOf(task({ due: null }), now)).toEqual({ level: 'Cool', v: 0.05 });
  });

  it('makes anything past its due Overdue at 1.1', () => {
    expect(heatOf(task({ due: now - 1 }), now)).toEqual({ level: 'Overdue', v: 1.1 });
    expect(heatOf(task({ due: now - 3 * DAY_MS }), now)).toEqual({ level: 'Overdue', v: 1.1 });
  });

  it('is Hot at 1 on the due instant itself, since days is 0, not below it', () => {
    expect(heatOf(task({ due: now }), now)).toEqual({ level: 'Hot', v: 1 });
  });

  it('gives a runway of difficulty × 2 + 1 days', () => {
    expect([1, 2, 3, 4, 5].map(runway)).toEqual([3, 5, 7, 9, 11]);
  });

  it('computes v = 1 - days / runway and its levels', () => {
    // difficulty 3, runway 7: 3.5 days left is v 0.5, Warm.
    const h = heatOf(task({ difficulty: 3, due: now + 3.5 * DAY_MS }), now);
    expect(h.level).toBe('Warm');
    expect(h.v).toBeCloseTo(0.5, 12);
    // 1 day left is v 0.857, Hot.
    expect(heatOf(task({ difficulty: 3, due: now + DAY_MS }), now).level).toBe('Hot');
    // 6 days left is v 0.143, Cool.
    const cool = heatOf(task({ difficulty: 3, due: now + 6 * DAY_MS }), now);
    expect(cool.level).toBe('Cool');
    expect(cool.v).toBeCloseTo(1 / 7, 12);
  });

  it('floors a Cool v at 0.05', () => {
    expect(heatOf(task({ difficulty: 1, due: now + 30 * DAY_MS }), now)).toEqual({ level: 'Cool', v: 0.05 });
    expect(heatOf(task({ difficulty: 5, due: now + 10.9 * DAY_MS }), now).v).toBe(0.05);
  });

  it('holds difficulty to 1..5', () => {
    expect(runway(0)).toBe(3);
    expect(runway(9)).toBe(11);
  });
});

describe('Warm at / Hot at (3.1 table)', () => {
  const table = {
    warm: [1.98, 3.3, 4.62, 5.94, 7.26],
    hot: [0.9, 1.5, 2.1, 2.7, 3.3],
  };

  it('prints every number of the table', () => {
    for (let d = 1; d <= 5; d++) {
      expect(heatThresholds(d)).toEqual({ warmAt: table.warm[d - 1], hotAt: table.hot[d - 1] });
    }
  });

  it('turns each difficulty Warm, then Hot, at exactly those days left', () => {
    const due = ny('2026-10-20 23:59');
    for (let d = 1; d <= 5; d++) {
      const t = task({ difficulty: d, due });
      const start = due - 20 * DAY_MS;
      expect(heatOf(t, start).level).toBe('Cool');
      const warm = nextHeatChange([t], start)!;
      expect(heatOf(t, warm - 1).level).toBe('Cool');
      expect(heatOf(t, warm).level).toBe('Warm');
      expect(Math.abs((due - warm) / DAY_MS - table.warm[d - 1])).toBeLessThan(1e-6);
      const hot = nextHeatChange([t], warm)!;
      expect(heatOf(t, hot - 1).level).toBe('Warm');
      expect(heatOf(t, hot).level).toBe('Hot');
      expect(Math.abs((due - hot) / DAY_MS - table.hot[d - 1])).toBeLessThan(1e-6);
      const overdue = nextHeatChange([t], hot)!;
      expect(overdue).toBe(due + 1);
      expect(heatOf(t, overdue).level).toBe('Overdue');
      expect(nextHeatChange([t], overdue)).toBeNull();
    }
  });
});

describe('1.6: Grammar quiz 4, difficulty 2, due Wednesday 11:59 PM', () => {
  const quiz = task({ title: 'Grammar quiz 4', difficulty: 2, due: ny('2026-10-07 23:59') });

  it('is amber, Warm, at 8:40 on Tuesday', () => {
    expect(heatOf(quiz, ny('2026-10-06 08:40')).level).toBe('Warm');
  });

  it('turns Hot with 1.5 days left, at 11:59 Tuesday morning', () => {
    expect(nextHeatChange([quiz], ny('2026-10-06 08:40'))).toBe(ny('2026-10-06 11:59'));
    expect(heatOf(quiz, ny('2026-10-06 11:59') - 1).level).toBe('Warm');
    expect(heatOf(quiz, ny('2026-10-06 11:59'))).toEqual({ level: 'Hot', v: 0.7 });
  });
});

describe('colours and the tube', () => {
  it('uses the four level colours', () => {
    expect(LEVEL_COLOUR).toEqual({ Overdue: '#8f1d16', Hot: '#e0402c', Warm: '#efa431', Cool: '#4f9be6' });
  });

  it('fills a 46×11 tube to v, full when overdue, empty when done', () => {
    expect(TUBE).toEqual({ width: 46, height: 11 });
    expect(tubeFill({ level: 'Warm', v: 0.5 })).toBe(0.5);
    expect(tubeFill({ level: 'Overdue', v: 1.1 })).toBe(1);
    expect(tubeFill({ level: 'Cool', v: 0.05 })).toBe(0.05);
    expect(tubeFill({ level: 'Done', v: null })).toBe(0);
  });
});

describe('the sort', () => {
  it('orders by heat, then due, with undated last and done after that', () => {
    const now = ny('2026-10-06 08:40');
    const undated = task({ title: 'undated' });
    const farA = task({ title: 'far A', difficulty: 1, due: now + 20 * DAY_MS });
    const farB = task({ title: 'far B', difficulty: 1, due: now + 10 * DAY_MS });
    const warm = task({ title: 'warm', difficulty: 3, due: now + 3 * DAY_MS });
    const hot = task({ title: 'hot', difficulty: 3, due: now + DAY_MS });
    const late = task({ title: 'late', due: now - DAY_MS });
    const later = task({ title: 'later', due: now - 2 * DAY_MS });
    const done = task({ title: 'done', done: true, due: now - DAY_MS });
    const sorted = byHeat([undated, done, farA, warm, late, hot, farB, later], now);
    expect(sorted.map((t) => t.title)).toEqual(['later', 'late', 'hot', 'warm', 'far B', 'far A', 'undated', 'done']);
  });
});

describe('due phrases', () => {
  const now = ny('2026-10-06 08:40');

  it('names today, tomorrow and the days of this week', () => {
    expect(duePhrase(ny('2026-10-06 16:00'), now, NY)).toBe('Today 4:00 PM');
    expect(duePhrase(ny('2026-10-07 23:59'), now, NY)).toBe('Tomorrow 11:59 PM');
    expect(duePhrase(ny('2026-10-08 23:59'), now, NY)).toBe('Thursday 11:59 PM');
    expect(duePhrase(ny('2026-10-12 23:59'), now, NY)).toBe('Monday 11:59 PM');
  });

  it('gives a date past this week, and the year when it differs', () => {
    expect(duePhrase(ny('2026-10-13 23:59'), now, NY)).toBe('Oct 13 11:59 PM');
    expect(duePhrase(ny('2027-01-08 09:00'), now, NY)).toBe('Jan 8, 2027 9:00 AM');
  });

  it('counts overdue in minutes, then hours, then days', () => {
    expect(duePhrase(now - 20 * 60_000, now, NY)).toBe('20m overdue');
    expect(duePhrase(now - 1, now, NY)).toBe('1m overdue');
    expect(duePhrase(now - 3 * 3_600_000 - 59 * 60_000, now, NY)).toBe('3h overdue');
    expect(duePhrase(now - 2 * DAY_MS - 5 * 3_600_000, now, NY)).toBe('2d overdue');
  });

  it('reads the day in the zone, not in UTC', () => {
    // 9 PM Tuesday in New York is already Wednesday in UTC.
    expect(duePhrase(ny('2026-10-06 21:00'), now, NY)).toBe('Today 9:00 PM');
  });
});

describe('nextHeatChange', () => {
  const now = ny('2026-10-06 08:40');

  it('gives the soonest change in a list, so the 60-second render can land on it', () => {
    const a = task({ difficulty: 2, due: ny('2026-10-07 23:59') }); // Hot at 11:59 today
    const b = task({ difficulty: 1, due: ny('2026-10-06 10:00') }); // Overdue at 10:00:00.001
    expect(nextHeatChange([a, b], now)).toBe(ny('2026-10-06 10:00') + 1);
  });

  it('gives nothing when no level will change', () => {
    expect(nextHeatChange([task({ due: null }), task({ done: true, due: now + DAY_MS }), task({ due: now - 1 })], now)).toBeNull();
  });
});
