// Adversarial review of build/heatmodel. Each test states what the spec asks
// for and fails against the model as built; each is skipped with the finding
// it exposes, so the suite stays green until the finding is fixed. Remove the
// `.skip` together with the fix.

import { describe, expect, it } from 'vitest';
import { checkOff, focusStep, initialFocus, type FocusEffect, type FocusEvent, type FocusState } from './focus';
import { currentPct, letterFor, whatItWouldTake } from './grades';
import { byHeat } from './heat';
import { importArtifact, parseHeatExport } from './importArtifact';
import { planMyDay, planNext, type PlanData } from './plan';
import type { Course, Grade, GradeCategory } from './records';
import { openTasks } from './recurrence';
import { NY, ids, ny, session, task } from './testkit';

const cat = (id: string, weight: number): GradeCategory => ({ id, name: id, weight, keywords: [] });
const course = (categories: GradeCategory[]): Course => ({
  id: 'c',
  termId: 't',
  code: 'JPN 201',
  name: 'Intermediate Japanese',
  categories,
  notes: '',
});
const grade = (categoryId: string, score: number, outOf = 100): Grade => ({
  id: `${categoryId}-${score}-${outOf}`,
  courseId: 'c',
  categoryId,
  title: categoryId,
  score,
  outOf,
  dropped: false,
  pending: false,
  source: 'you',
});

describe('S2.1: grade letters at exact edges (3.1)', () => {
  // Finding: currentPct sums weight × category % in floating point, so a
  // course that stands at exactly 60% (0.2 × 68 + 0.8 × 58) comes out as
  // 59.99999999999999 and letterFor gives F instead of D. Same for 67% → D.
  it.skip('gives D at exactly 60% and D+ at exactly 67%', () => {
    const sixty = course([cat('hw', 20), cat('exams', 80)]);
    expect(letterFor(currentPct(sixty, [grade('hw', 68), grade('exams', 58)])!)).toBe('D');
    const sixtySeven = course([cat('hw', 25), cat('exams', 75)]);
    expect(letterFor(currentPct(sixtySeven, [grade('hw', 94), grade('exams', 58)])!)).toBe('D+');
  });
});

describe('S2.1: what it would take, when 100% on the rest is exactly enough (3.8)', () => {
  // Finding: whatItWouldTake tests need > 100 on a floating-point need, so a
  // target reachable with exactly 100% is called out of reach, and the line
  // contradicts itself: "A B- is out of reach; the highest possible is 80% (B-)."
  it.skip('says you need 100%, not that the letter is out of reach', () => {
    // 2/3 on a 60% category: 0.6 × 66.67 + 40 = 80 exactly.
    const c = course([cat('hw', 60), cat('final', 40)]);
    expect(whatItWouldTake(c, [grade('hw', 2, 3)], 'B-')).toBe(
      'To finish with a B- (80%), you need 100% on the remaining 40%.',
    );
    // 50% on a 34% category: 17 + 66 = 83 exactly.
    const d = course([cat('mid', 34), cat('final', 66)]);
    expect(whatItWouldTake(d, [grade('mid', 50)], 'B')).toBe(
      'To finish with a B (83%), you need 100% on the remaining 66%.',
    );
  });
});

describe('3.6: a recurring task never vanishes', () => {
  // Finding: openTasks drops a task whose rule is readable but which has no
  // due date and no scheduled date (seriesStart is null, so there is no next
  // occurrence). It then shows in no list, count, plan or strip, and not in
  // Done either. The PKM sets scheduledDate to today in this case.
  it.skip('keeps "Every weekday" with no dates in the open tasks', () => {
    const t = task({ title: 'Practise scales', rrule: 'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR', due: null });
    expect(openTasks([t], [], ny('2026-10-06 08:40'), NY).map((x) => x.id)).toEqual([t.id]);
  });
});

describe('3.6: the series never flips to done', () => {
  // Finding: checkOff completes any task with logged time by returning it
  // with done: true, recurring or not. Checking a recurring task that has
  // focus sessions therefore ends the whole series instead of writing a
  // TaskOccurrence row (checkOccurrence).
  it.skip('does not mark a recurring task done when it is checked off', () => {
    const t = task({ rrule: 'FREQ=DAILY', due: ny('2026-10-06 17:00') });
    const result = checkOff(t, [session({ taskId: t.id })], ny('2026-10-06 18:00'));
    expect(result.kind === 'done' && result.task.done).toBe(false);
  });
});

describe('S2.2: drafts and P land in the time column (3.5)', () => {
  // Finding: planMyDay and planNext start from now with no floor at the
  // column's 7 AM, so planning after midnight drafts blocks at 1:15 AM, which
  // the 7 AM to midnight column can't show or drag.
  const data = (t: ReturnType<typeof task>): PlanData => ({
    tasks: [t],
    occurrences: [],
    blocks: [],
    events: [],
    sessions: [],
    habits: [],
  });

  it.skip('never places a block before 7 AM', () => {
    const now = ny('2026-10-06 01:10');
    const t = task({ estMin: 45, due: ny('2026-10-07 23:59') });
    for (const d of planMyDay(data(t), now, NY)) expect(d.start).toBeGreaterThanOrEqual(7 * 60);
    const p = planNext({ taskId: t.id }, data(t), now, NY, ids());
    expect(p === null || p.start >= 7 * 60).toBe(true);
  });
});

describe('S2.3: minutes go to the current task (3.5)', () => {
  // Finding: closeSession rounds each part of a split round to the minute on
  // its own, so a 25-minute round split at 12:30 logs 13 + 13 = 26 minutes to
  // actualMin (and three parts of 8:20 log 24).
  it.skip('logs exactly the round’s 25 minutes when the current task changes halfway', () => {
    const t0 = ny('2026-10-06 09:00');
    let s: FocusState = initialFocus();
    const effects: FocusEffect[] = [];
    const step = (e: FocusEvent, at: number) => {
      const r = focusStep(s, e, at);
      s = r.state;
      effects.push(...r.effects);
    };
    step({ type: 'press', target: { kind: 'task', id: 'a', title: 'Grammar quiz 4' } }, t0);
    step({ type: 'setTarget', target: { kind: 'task', id: 'b', title: 'Mix the second verse' } }, t0 + 12.5 * 60_000);
    step({ type: 'tick' }, t0 + 25 * 60_000);
    const logged = effects.reduce((sum, e) => sum + (e.kind === 'log' ? e.session.focusMin : 0), 0);
    expect(logged).toBe(25);
  });
});

describe('3.15: moving in refuses a malformed export instead of half-reading it', () => {
  // Finding: parseHeatExport checks only that the lists exist. A row that is
  // null throws a TypeError out of the parser, and a due that isn't a date is
  // accepted and imported as NaN. A NaN due makes heatOf's v NaN, and byHeat's
  // comparator then misorders every other task (a task due in 2 hours sorts
  // after one due tomorrow), so Plan my day drafts in the wrong order.
  const base = {
    format: 'heat-export',
    version: 1,
    exportedAt: '2026-10-06T12:40:00.000Z',
    timeZone: 'America/New_York',
    workspaces: [{ key: 'classes', name: 'Classes', groupLabel: 'Course', types: ['Quiz'], persona: '' }],
    milestones: [],
    habits: [],
    term: 'Fall 2026',
    courses: [],
    grades: [],
    processedMailIds: [],
    lastSyncAt: null,
  };
  const row = (due: string) => ({
    id: 'em-1',
    workspace: 'classes',
    title: 'Grammar quiz 4',
    group: null,
    type: 'Quiz',
    due,
    difficulty: 2,
    estMin: null,
    actualMin: null,
    notes: '',
    done: false,
    doneAt: null,
    source: 'calendar',
  });

  it.skip('returns an error for a null row rather than throwing', () => {
    expect(() => parseHeatExport({ ...base, tasks: [null] })).not.toThrow();
  });

  it.skip('never imports a due that is not a number', () => {
    const parsed = parseHeatExport({ ...base, tasks: [row('next Wednesday')] });
    if ('error' in parsed) return;
    const due = importArtifact(parsed, [], ids()).tasks[0].due;
    expect(due === null || Number.isFinite(due)).toBe(true);
  });

  it.skip('keeps the other tasks in heat order when one due is NaN', () => {
    const now = ny('2026-10-06 08:00');
    const soon = task({ due: ny('2026-10-06 10:00') });
    const tomorrow = task({ due: ny('2026-10-07 10:00') });
    const broken = task({ due: Number.NaN });
    const order = byHeat([tomorrow, broken, soon], now).map((t) => t.id);
    expect(order.indexOf(soon.id)).toBeLessThan(order.indexOf(tomorrow.id));
  });
});
