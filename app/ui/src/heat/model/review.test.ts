import { describe, expect, it } from 'vitest';
import type { Milestone, Space, TaskOccurrence } from './records';
import { type ReviewData, factLines, lastWeekFacts } from './review';
import { NY, ny, session, task } from './testkit';

const now = ny('2026-10-12 10:00'); // a Monday; last week is October 5 to 11
const classes: Space = { id: 'classes', name: 'Classes', hue: 211, groupKind: 'course', groupLabel: 'Course', types: ['Homework', 'Other'], persona: '' };
const wwav: Space = { id: 'wwav', name: 'WWAV', hue: 6, groupKind: 'milestone', groupLabel: 'Milestone', types: ['Software', 'Other'], persona: '' };

function data(over: Partial<ReviewData> = {}): ReviewData {
  return { spaces: [classes, wwav], tasks: [], occurrences: [], sessions: [], milestones: [], ...over };
}

describe('the weekly review’s facts (3.13, step 2)', () => {
  const inWeek = ny('2026-10-08 15:00');

  it('counts tasks done and focus time per space, in the last 7 days only', () => {
    const a = task({ spaceId: 'classes', done: true, doneAt: inWeek });
    const b = task({ spaceId: 'classes', done: true, doneAt: ny('2026-10-05 00:00') });
    const early = task({ spaceId: 'classes', done: true, doneAt: ny('2026-10-04 23:59') });
    const today = task({ spaceId: 'classes', done: true, doneAt: ny('2026-10-12 00:00') });
    const c = task({ spaceId: 'wwav' });
    const series = task({ spaceId: 'wwav', rrule: 'FREQ=DAILY', due: ny('2026-10-01 09:00') });
    const occ: TaskOccurrence = { id: 'o', taskId: series.id, date: '2026-10-07', doneAt: inWeek };
    const sessions = [
      session({ taskId: a.id, focusMin: 25, endedAt: inWeek }),
      session({ taskId: c.id, focusMin: 50, endedAt: inWeek }),
      session({ taskId: c.id, focusMin: 50, endedAt: ny('2026-10-04 12:00') }),
      session({ habitId: 'kanji', focusMin: 20, endedAt: inWeek }),
    ];
    const facts = lastWeekFacts(data({ tasks: [a, b, early, today, c, series], occurrences: [occ], sessions }), now, NY);
    expect(facts.from).toBe('2026-10-05');
    expect(facts.to).toBe('2026-10-11');
    expect(facts.spaces).toEqual([
      { spaceId: 'classes', name: 'Classes', tasksDone: 2, focusMin: 25 },
      { spaceId: 'wwav', name: 'WWAV', tasksDone: 1, focusMin: 50 },
    ]);
    expect(facts.habitFocusMin).toBe(20);
  });

  it('lists the milestones reached that week', () => {
    const reached: Milestone = { id: 'm1', spaceId: 'wwav', title: 'Enclosure v2', date: '2026-10-09', done: true, order: 1 };
    const missed: Milestone = { id: 'm2', spaceId: 'wwav', title: 'Firmware 1.0', date: '2026-10-10', done: false, order: 2 };
    const old: Milestone = { id: 'm3', spaceId: 'wwav', title: 'Schematic', date: '2026-09-01', done: true, order: 0 };
    expect(lastWeekFacts(data({ milestones: [reached, missed, old] }), now, NY).milestonesReached).toEqual([reached]);
  });

  it('measures estimates against time taken: "Homework: estimated 1h 15m, took 1h 32m across 4"', () => {
    const homework = [
      [60, 80],
      [75, 95],
      [75, 100],
      [90, 93],
    ].map(([estMin, took]) => task({ type: 'Homework', estMin, adjustMin: took, done: true, doneAt: inWeek }));
    // Done before the week, so it is history, not part of the week's accuracy.
    const before = task({ type: 'Homework', estMin: 10, adjustMin: 500, done: true, doneAt: ny('2026-09-30 12:00') });
    const untimed = task({ type: 'Homework', estMin: 30, done: true, doneAt: inWeek });
    const facts = lastWeekFacts(data({ tasks: [...homework, before, untimed] }), now, NY);
    expect(facts.accuracy).toEqual([{ label: 'Homework', estimatedMin: 75, tookMin: 92, count: 4 }]);
    expect(factLines(facts)).toContain('Homework: estimated 1h 15m, took 1h 32m across 4');
  });

  it('estimates a task without estMin from the averages as they stood when the week began', () => {
    const history = task({ type: 'Homework', adjustMin: 40, done: true, doneAt: ny('2026-09-30 12:00') });
    const week = task({ type: 'Homework', difficulty: 5, adjustMin: 70, done: true, doneAt: inWeek });
    const facts = lastWeekFacts(data({ tasks: [history, week] }), now, NY);
    expect(facts.accuracy).toEqual([{ label: 'Homework', estimatedMin: 40, tookMin: 70, count: 1 }]);
  });

  it('names the space when two spaces share a type', () => {
    const a = task({ spaceId: 'classes', type: 'Other', estMin: 30, adjustMin: 30, done: true, doneAt: inWeek });
    const b = task({ spaceId: 'wwav', type: 'Other', estMin: 60, adjustMin: 45, done: true, doneAt: inWeek });
    expect(lastWeekFacts(data({ tasks: [a, b] }), now, NY).accuracy.map((x) => x.label)).toEqual([
      'Other (Classes)',
      'Other (WWAV)',
    ]);
  });

  it('writes the facts as plain lines and nothing else', () => {
    const reached: Milestone = { id: 'm1', spaceId: 'wwav', title: 'Enclosure v2', date: '2026-10-09', done: true, order: 1 };
    const a = task({ spaceId: 'classes', type: 'Homework', estMin: 60, adjustMin: 50, done: true, doneAt: inWeek });
    const sessions = [session({ habitId: 'kanji', focusMin: 80, endedAt: inWeek })];
    expect(factLines(lastWeekFacts(data({ tasks: [a], sessions, milestones: [reached] }), now, NY))).toEqual([
      'Classes: 1 task done, 0m of focus',
      'WWAV: 0 tasks done, 0m of focus',
      'Habits: 1h 20m of focus',
      'Milestone reached: Enclosure v2',
      'Homework: estimated 1h, took 50m across 1',
    ]);
  });
});
