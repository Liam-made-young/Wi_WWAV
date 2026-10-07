import { describe, expect, it } from 'vitest';
import { focusStep, focusStrip, initialFocus } from './focus';
import { type LcdData, taskHalf } from './lcd';
import type { Course } from './records';
import { NY, ny, task } from './testkit';

const jpn102: Course = { id: 'jpn102', termId: 't', code: 'JPN 102', name: 'Japanese', categories: [], notes: '' };
const jpn201: Course = { id: 'jpn201', termId: 't', code: 'JPN 201', name: 'Japanese', categories: [], notes: '' };

function data(over: Partial<LcdData> = {}): LcdData {
  return { tasks: [], occurrences: [], sessions: [], courses: [jpn102, jpn201], milestones: [], ...over };
}

describe('the Now strip’s task half (2.2, from Heat’s LCD in 3.1)', () => {
  const now = ny('2026-10-06 12:30');

  it('shows the hottest open task, its due and group, and the week’s load', () => {
    const quiz = task({
      title: 'Grammar quiz 4',
      courseId: 'jpn102',
      difficulty: 2,
      due: ny('2026-10-07 23:59'),
      estMin: 45,
    });
    const others = [40, 35, 60, 20].map((estMin, i) =>
      task({ estMin, difficulty: 1, due: ny('2026-10-10 12:00') + i }),
    );
    const half = taskHalf(data({ tasks: [...others, quiz] }), now, NY, null, null);
    expect(half).toEqual({
      taskId: quiz.id,
      line1: 'Hot: Grammar quiz 4',
      line2: 'Tomorrow 11:59 PM, JPN 102. This week: 3h 20m across 5 tasks',
      meter: expect.closeTo(1 - 35.483 / 24 / 5, 3),
    });
  });

  it('shows the current task over the hottest, with the focus countdown', () => {
    const hot = task({ title: 'Problem set', difficulty: 5, due: ny('2026-10-06 18:00') });
    const quiz = task({ title: 'Grammar quiz 4', courseId: 'jpn201', difficulty: 3, due: ny('2026-10-06 16:00') });
    const focus = focusStep(
      initialFocus(),
      { type: 'press', target: { kind: 'task', id: quiz.id, title: quiz.title } },
      now,
    ).state;
    const half = taskHalf(
      data({ tasks: [hot, quiz] }),
      now + 6 * 60_000 + 20_000,
      NY,
      quiz.id,
      focusStrip(focus, now + 6 * 60_000 + 20_000),
    );
    expect(half.line1).toBe('Hot: Grammar quiz 4');
    expect(half.line2).toBe('Today 4:00 PM, JPN 201 · focus 18:40 left');
  });

  it('falls back to the hottest task once the current one is done', () => {
    const shut = task({ title: 'Shipped', done: true, doneAt: now });
    const next = task({ title: 'Next', due: ny('2026-10-09 12:00') });
    expect(taskHalf(data({ tasks: [shut, next] }), now, NY, shut.id, null).taskId).toBe(next.id);
  });

  it('leaves out what a task doesn’t have', () => {
    const plain = task({ title: 'Call the bank', difficulty: 1 });
    expect(taskHalf(data({ tasks: [plain] }), now, NY, null, null).line2).toBe('This week: 0m across 0 tasks');
    expect(taskHalf(data({ tasks: [plain] }), now, NY, plain.id, 'focus 3:00 left').line2).toBe('focus 3:00 left');
    expect(taskHalf(data({ tasks: [plain] }), now, NY, null, null).line1).toBe('Cool: Call the bank');
  });

  it('says "All clear" when nothing is open', () => {
    expect(taskHalf(data(), now, NY, null, null)).toEqual({
      taskId: null,
      line1: 'All clear',
      line2: 'Nothing open right now.',
      meter: 0,
    });
  });
});
