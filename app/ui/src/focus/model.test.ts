// The Focus layout's rules, on the fake core's snapshot: entropy, what Focus
// shows, and the one decision about interrupts.

import { describe, expect, it } from 'vitest';
import type { Snapshot, Task } from '../heat/client';
import { createFake, type Fake } from '../heat/fake/all';
import { snapshotOf } from '../heat/fake/core';
import { setEvents } from '../heat/fake/today';
import { epochOf } from '../shared/time/zone';
import { focusMemory } from './fake';
import {
  CONFIG,
  emptyMemory,
  entropyOf,
  eventsFromChanges,
  fixOf,
  focusState,
  nowOf,
  pick,
  plannableMin,
  shouldInterrupt,
  standingEvents,
} from './model';

const NY = 'America/New_York';
/** Wednesday 7 Oct 2026, 10:00 AM in New York. */
const NOW = epochOf({ year: 2026, month: 10, day: 7, hour: 10, minute: 0 }, NY);
const HOUR = 3_600_000;
const DAY = 24 * HOUR;

const task = (id: string, over: Partial<Task> = {}): Task => ({
  id,
  spaceId: 'sp',
  title: id,
  type: 'Homework',
  due: null,
  difficulty: 3,
  estMin: 60,
  adjustMin: 0,
  notes: '',
  done: false,
  doneAt: null,
  source: 'you',
  ...over,
});

function world(tasks: Task[], more: Parameters<typeof createFake>[0] = {}): { fake: Fake; snap: () => Snapshot } {
  const { fake } = createFake({ task: tasks, ...more }, { now: NOW, zone: NY });
  return { fake, snap: () => snapshotOf(fake) };
}

describe('the config', () => {
  // The core's `Config::default()` is pinned to these same keys and values (crates/wi-core/src/focus.rs).
  it('is the core’s, key for key', () => {
    expect(CONFIG).toEqual({
      horizonDays: 7,
      weights: { overdue: 3, unplanned: 2, mail: 1.5, grades: 0.5 },
      full: 12,
      busyAt: 0.2,
      highAt: 0.5,
      riskHorizonDays: 7,
      dayStartsMin: 420,
      dayEndsMin: 1320,
      plannablePerDayMin: 360,
      riskFloorMin: 15,
      dueChangedWithinDays: 7,
      urgentWithinHours: 48,
      leaveLeadMin: 15,
      queueKeepDays: 7,
      hintDays: 7,
      hintLaunches: 20,
    });
  });
});

describe('entropy', () => {
  it('is zero, and Focus is clear, when nothing is open', () => {
    const { snap } = world([]);
    const f = snap().focus!;
    expect(f.entropy).toEqual({ score: 0, level: 'clear', parts: { unplanned: 0, overdue: 0, mail: 0, grades: 0 } });
    expect(f.now.kind).toBe('clear');
    expect(f.interrupt).toBeNull();
  });

  it('counts a task due this week with no planned time, and stops once it has a block or a day', () => {
    const { fake, snap } = world([
      task('a', { due: NOW + 2 * DAY }),
      task('b', { due: NOW + 3 * DAY, scheduledDate: '2026-10-08' }),
      task('far', { due: NOW + 20 * DAY }),
      task('undated'),
    ]);
    expect(snap().focus!.entropy.parts.unplanned).toBe(1);
    fake.store.timeBlock.set('blk', { id: 'blk', taskId: 'a', date: '2026-10-07', start: 14 * 60, minutes: 60, origin: 'you' });
    expect(snap().focus!.entropy.parts.unplanned).toBe(0);
    expect(snap().focus!.entropy.score).toBe(0);
  });

  it('counts an overdue task as overdue, not as unplanned', () => {
    const { snap } = world([task('late', { due: NOW - 2 * HOUR })]);
    expect(snap().focus!.entropy.parts).toMatchObject({ overdue: 1, unplanned: 0 });
  });

  it('counts grades waiting and mail that still asks for something', () => {
    const { fake, snap } = world([], {
      grade: [{ id: 'g', courseId: 'c', categoryId: null, title: 'Quiz 3', score: null, outOf: 10, dropped: false, pending: true, source: 'mail' }],
      mailThread: [
        { id: 'm1', gmailThreadId: 'g1', subject: 'Form due', from: 'a@b', receivedAt: NOW, state: 'task', reason: '', recordedBy: 'claude' },
        { id: 'm2', gmailThreadId: 'g2', subject: 'News', from: 'a@b', receivedAt: NOW, state: 'nothing', reason: '', recordedBy: 'claude' },
      ],
    });
    void fake;
    expect(snap().focus!.entropy.parts).toMatchObject({ grades: 1, mail: 1 });
  });

  it('is normalised: it never passes 1, and its level follows the config', () => {
    const many = Array.from({ length: 12 }, (_, i) => task(`t${i}`, { due: NOW - HOUR }));
    const e = entropyOf(world(many).snap());
    expect(e.score).toBe(1);
    expect(e.level).toBe('high');
    const one = entropyOf(world([task('a', { due: NOW + DAY })]).snap());
    expect(one.score).toBeCloseTo(CONFIG.weights.unplanned / CONFIG.full, 2);
    expect(one.level).toBe('calm');
  });
});

describe('what Focus shows', () => {
  const four = [1, 2, 3, 4].map((n) => task(`t${n}`, { due: NOW + n * DAY }));

  it('shows the current task whatever the entropy', () => {
    const { fake, snap } = world(four);
    fake.state.currentTaskId = 't3';
    expect(snap().focus!.now).toMatchObject({ kind: 'task', taskId: 't3', why: 'current' });
  });

  it('shows the fix, not the mess, when entropy is high', () => {
    const f = world(four).snap().focus!;
    expect(f.entropy.level).toBe('high');
    expect(f.now.kind).toBe('fix');
    expect(f.now.fix).toMatchObject({
      kind: 'plan',
      line: '4 tasks this week have no plan.',
      ask: 'Plan them?',
      action: { label: 'Plan my day', do: 'plan' },
    });
  });

  it('shows the top task by heat once the fix is put off for the day', () => {
    const { snap } = world(four);
    const s = snap();
    const mem = { ...emptyMemory(), fixSnoozed: s.date };
    expect(nowOf(s, entropyOf(s), mem)).toMatchObject({ kind: 'task', taskId: 't1', why: 'heat' });
  });

  it('shows the top task by heat when entropy is not high', () => {
    const f = world([task('soon', { due: NOW + DAY }), task('later', { due: NOW + 5 * DAY, scheduledDate: '2026-10-09' })]).snap().focus!;
    expect(f.now).toMatchObject({ kind: 'task', taskId: 'soon', why: 'heat' });
  });

  it('offers the fix for what is left when no task is open', () => {
    const s = world([], {
      grade: [{ id: 'g', courseId: 'c', categoryId: null, title: 'Quiz 3', score: null, outOf: 10, dropped: false, pending: true, source: 'mail' }],
    }).snap();
    expect(s.focus!.now).toMatchObject({ kind: 'fix', fix: { kind: 'grades', line: '1 grade is waiting for its score.' } });
  });

  it('has no fix for overdue work: the fix is doing it', () => {
    expect(fixOf({ score: 1, level: 'high', parts: { overdue: 5, unplanned: 0, mail: 0, grades: 0 } })).toBeNull();
  });
});

describe('should_interrupt', () => {
  it('lets a due date Claude moved through as one line, and not a change the person made', () => {
    const { fake, snap } = world([task('essay', { title: 'Essay 2', due: NOW + 5 * DAY, scheduledDate: '2026-10-08' })]);
    const move = (actor: 'you' | 'claude', due: number) =>
      fake.write('edit task', ['task'], () => fake.store.task.set('essay', { ...fake.store.task.get('essay')!, due }), { actor });
    move('you', NOW + 4 * DAY);
    expect(snap().focus!.interrupt).toBeNull();
    move('claude', NOW + 2 * DAY);
    const f = snap().focus!;
    expect(f.interrupt).toMatchObject({ source: 'dueChanged', action: { do: 'task', taskId: 'essay' } });
    expect(f.interrupt!.line).toBe('Due date moved: Essay 2 is now due Friday 10:00 AM.');
    expect(f.queued).toBe(0);
  });

  it('drops a due-date line that is no longer true', () => {
    const { fake, snap } = world([task('essay', { due: NOW + 5 * DAY, scheduledDate: '2026-10-08' })]);
    fake.write('edit task', ['task'], () => fake.store.task.set('essay', { ...fake.store.task.get('essay')!, due: NOW + 2 * DAY }), { actor: 'claude' });
    expect(snap().focus!.interrupt).not.toBeNull();
    fake.store.task.set('essay', { ...fake.store.task.get('essay')!, done: true });
    expect(snap().focus!.interrupt).toBeNull();
  });

  it('does not interrupt for a date that moved far away on a task that is not the Now task', () => {
    const s = world([task('a', { due: NOW + 40 * DAY }), task('now', { due: NOW + DAY, scheduledDate: '2026-10-07' })]).snap();
    const ctx = { snap: s, nowTaskId: 'now', cfg: CONFIG };
    expect(shouldInterrupt({ kind: 'dueChanged', taskId: 'a', from: NOW + 30 * DAY, to: NOW + 40 * DAY, at: NOW }, ctx)).toBeNull();
  });

  it('raises a deadline at risk when the work left is more than the time that can be planned', () => {
    const { fake, snap } = world([
      task('big', { title: 'Lab report', due: NOW + 3 * HOUR, estMin: 300, scheduledDate: '2026-10-07' }),
      task('now', { due: NOW + HOUR, estMin: 10, scheduledDate: '2026-10-07' }),
    ]);
    fake.state.currentTaskId = 'now';
    const f = snap().focus!;
    expect(f.interrupt).toMatchObject({ source: 'atRisk', priority: 'high', action: { label: 'Do it now', do: 'current', taskId: 'big' } });
    expect(f.interrupt!.line).toBe('At risk: Lab report needs 5h, and only 3h is free before it is due.');
    // The task being done is never "at risk": nothing about what comes next would change.
    fake.state.currentTaskId = 'big';
    expect(snap().focus!.interrupt?.source).not.toBe('atRisk');
  });

  it('takes the calendar out of the time that can be planned', () => {
    const { fake, snap } = world([task('a', { due: NOW + 4 * HOUR })]);
    expect(plannableMin(snap(), 'a', NOW + 4 * HOUR)).toBe(240);
    setEvents(fake, [{ id: 'e', title: 'Class', start: NOW + HOUR, end: NOW + 2 * HOUR, allDay: false }]);
    expect(plannableMin(snap(), 'a', NOW + 4 * HOUR)).toBe(180);
  });

  it('says when it is time to leave for the next commitment, and not before', () => {
    const { fake, snap } = world([]);
    setEvents(fake, [{ id: 'e', title: 'JPN 101', start: NOW + 40 * 60_000, end: NOW + 2 * HOUR, allDay: false }]);
    expect(snap().focus!.interrupt).toBeNull();
    expect(snap().focus!.next).toMatchObject({ title: 'JPN 101' });
    fake.now = NOW + 30 * 60_000;
    expect(snap().focus!.interrupt).toMatchObject({ source: 'leaveFor', line: 'Time to leave: JPN 101 starts at 10:40 AM.' });
  });

  it('shows one at a time, the most pressing first, and counts the rest', () => {
    const { fake, snap } = world([], {
      grade: [{ id: 'g', courseId: 'c', categoryId: null, title: 'Quiz 3', score: null, outOf: 10, dropped: false, pending: true, source: 'mail' }],
    });
    const m = focusMemory(fake);
    m.queue.push({ kind: 'custom', id: 'n1', source: 'notes', line: 'A note needs you.', action: null, changesNext: true, priority: 'normal', at: NOW });
    m.queue.push({ kind: 'custom', id: 'n2', source: 'notes', line: 'Unimportant.', action: null, changesNext: false, priority: 'high', at: NOW });
    const f = snap().focus!;
    expect(f.interrupt).toMatchObject({ id: 'n1' });
    expect(f.queued).toBe(1);
    m.dismissed.n1 = NOW;
    expect(snap().focus!.interrupt).toMatchObject({ source: 'gradeWaiting', priority: 'low' });
    expect(snap().focus!.queued).toBe(0);
  });

  it('holds back all but what cannot wait while a focus round runs', () => {
    const { fake, snap } = world([task('a', { due: NOW + 9 * DAY })], {
      grade: [{ id: 'g', courseId: 'c', categoryId: null, title: 'Quiz 3', score: null, outOf: 10, dropped: false, pending: true, source: 'mail' }],
    });
    expect(snap().focus!.interrupt?.source).toBe('gradeWaiting');
    fake.state.timer = { phase: 'focus', round: 1, endsAt: NOW + 25 * 60_000, running: true, taskId: 'a' };
    expect(snap().focus!.interrupt).toBeNull();
    setEvents(fake, [{ id: 'e', title: 'Class', start: NOW + 10 * 60_000, end: NOW + HOUR, allDay: false }]);
    expect(snap().focus!.interrupt?.source).toBe('leaveFor');
  });

  it('offers the break when a focus round has ended', () => {
    const { fake, snap } = world([task('a')]);
    fake.state.timer = { phase: 'break', round: 1, endsAt: null, running: false, leftMs: 300_000, lengthMs: 300_000, note: 'Focus done. 25m logged to a.' };
    expect(snap().focus!.interrupt).toMatchObject({ source: 'focusEnded', line: 'Focus done. 25m logged to a.', action: { do: 'break' } });
  });

  it('turns what Claude changed into events: a moved due date, a task from mail', () => {
    const before = task('a', { due: 1 });
    expect(
      eventsFromChanges(
        [
          { kind: 'task', before, after: { ...before, due: 2 } },
          { kind: 'task', before: null, after: task('m', { source: 'mail' }) },
          { kind: 'task', before, after: { ...before, title: 'renamed' } },
          { kind: 'grade', before: null, after: {} },
        ],
        9,
      ),
    ).toEqual([
      { kind: 'dueChanged', taskId: 'a', from: 1, to: 2, at: 9 },
      { kind: 'mailUrgent', taskId: 'm', at: 9 },
    ]);
  });

  it('keeps `pick` and `focusState` in step', () => {
    const s = world([task('a', { due: NOW + DAY })]).snap();
    const state = focusState(s, emptyMemory());
    const picked = pick(standingEvents(s), { snap: s, nowTaskId: state.now.taskId, cfg: CONFIG }, {});
    expect(picked.interrupt).toEqual(state.interrupt);
  });
});
