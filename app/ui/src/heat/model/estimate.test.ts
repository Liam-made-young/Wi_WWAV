import { describe, expect, it } from 'vitest';
import { DAY_MS } from './heat';
import {
  actualMin,
  averageLines,
  estimateContext,
  estimateMin,
  formatMinutes,
  weeklyLoad,
  weeklyLoadLine,
} from './estimate';
import type { Space } from './records';
import { ny, session, task } from './testkit';

describe('minutes, as written everywhere', () => {
  it('writes 45m, 1h 15m, 3h 10m, 2h', () => {
    expect(formatMinutes(45)).toBe('45m');
    expect(formatMinutes(75)).toBe('1h 15m');
    expect(formatMinutes(190)).toBe('3h 10m');
    expect(formatMinutes(120)).toBe('2h');
    expect(formatMinutes(0)).toBe('0m');
    expect(formatMinutes(44.6)).toBe('45m');
  });
});

describe('actualMin', () => {
  it('is the sum of focus minutes plus the hand adjustment', () => {
    const t = task({ adjustMin: 10 });
    const sessions = [
      session({ taskId: t.id, focusMin: 25 }),
      session({ taskId: t.id, focusMin: 18 }),
      session({ taskId: 'other', focusMin: 50 }),
    ];
    expect(actualMin(t, sessions)).toBe(53);
  });
});

describe('the estimate chain (3.1)', () => {
  const classes = 'classes';
  const done = (type: string, minutes: number, spaceId = classes) =>
    task({ spaceId, type, done: true, doneAt: 1, adjustMin: minutes });

  it('takes the task’s own estMin first', () => {
    const history = [done('Homework', 60)];
    const t = task({ type: 'Homework', estMin: 45, difficulty: 4 });
    expect(estimateMin(t, estimateContext([...history, t], []))).toBe(45);
  });

  it('then the average for its type in its space', () => {
    const history = [done('Homework', 60), done('Homework', 90), done('Homework', 0), done('Homework', 500, 'wwav')];
    const t = task({ type: 'Homework', difficulty: 4 });
    // The task with no time and the other space's task don't count.
    expect(estimateMin(t, estimateContext([...history, t], []))).toBe(75);
  });

  it('then difficulty × 20 minutes', () => {
    const t = task({ type: 'Quiz', difficulty: 2 });
    expect(estimateMin(t, estimateContext([done('Homework', 60), t], []))).toBe(40);
  });

  it('learns from focus minutes, not only typed ones', () => {
    const past = task({ type: 'Lab', done: true, doneAt: 1 });
    const t = task({ type: 'Lab', difficulty: 1 });
    const sessions = [session({ taskId: past.id, focusMin: 25 }), session({ taskId: past.id, focusMin: 25 })];
    expect(estimateMin(t, estimateContext([past, t], sessions))).toBe(50);
  });

  it('makes a parent the sum of its open children, all the way down', () => {
    const parent = task({ estMin: 999 });
    const a = task({ parentTaskId: parent.id, estMin: 30 });
    const b = task({ parentTaskId: parent.id, estMin: 999 });
    const b1 = task({ parentTaskId: b.id, estMin: 20 });
    const b2 = task({ parentTaskId: b.id, estMin: 25 });
    const shut = task({ parentTaskId: parent.id, estMin: 60, done: true, doneAt: 1 });
    const ctx = estimateContext([parent, a, b, b1, b2, shut], []);
    expect(estimateMin(b, ctx)).toBe(45);
    expect(estimateMin(parent, ctx)).toBe(75);
  });

  it('falls back to the parent’s own chain once every child is done', () => {
    const parent = task({ estMin: 40 });
    const child = task({ parentTaskId: parent.id, estMin: 60, done: true, doneAt: 1 });
    expect(estimateMin(parent, estimateContext([parent, child], []))).toBe(40);
  });
});

describe('your average time', () => {
  it('reads "Homework 1h 15m (6)" in the space’s type order', () => {
    const space: Space = {
      id: 'classes',
      name: 'Classes',
      hue: 0,
      groupKind: 'course',
      groupLabel: 'Course',
      types: ['Homework', 'Quiz', 'Listening'],
      persona: '',
    };
    const homework = [60, 70, 80, 70, 80, 90].map((m) =>
      task({ type: 'Homework', done: true, doneAt: 1, adjustMin: m }),
    );
    const quiz = [task({ type: 'Quiz', done: true, doneAt: 1, adjustMin: 20 })];
    expect(averageLines(space, estimateContext([...quiz, ...homework], []))).toEqual([
      'Homework 1h 15m (6)',
      'Quiz 20m (1)',
    ]);
  });
});

describe('weekly load (3.1)', () => {
  const now = ny('2026-10-06 08:40');

  it('sums the estimates of open tasks due within 7 days', () => {
    const tasks = [
      task({ estMin: 60, due: now + DAY_MS }),
      task({ estMin: 45, due: now + 7 * DAY_MS }),
      task({ estMin: 30, due: now - DAY_MS }), // overdue is still this week's work
      task({ estMin: 500, due: now + 7 * DAY_MS + 1 }),
      task({ estMin: 500, due: null }),
      task({ estMin: 500, due: now + DAY_MS, done: true, doneAt: now }),
    ];
    expect(weeklyLoad(tasks, estimateContext(tasks, []), now)).toEqual({ minutes: 135, count: 3 });
  });

  it('counts a subtask once, inside its parent', () => {
    const parent = task({ due: now + 2 * DAY_MS });
    const child = task({ parentTaskId: parent.id, estMin: 40, due: now + DAY_MS });
    const tasks = [parent, child];
    expect(weeklyLoad(tasks, estimateContext(tasks, []), now)).toEqual({ minutes: 40, count: 1 });
  });

  it('reads "This week: 3h 20m across 5 tasks"', () => {
    expect(weeklyLoadLine({ minutes: 200, count: 5 })).toBe('This week: 3h 20m across 5 tasks');
    expect(weeklyLoadLine({ minutes: 45, count: 1 })).toBe('This week: 45m across 1 task');
  });
});
