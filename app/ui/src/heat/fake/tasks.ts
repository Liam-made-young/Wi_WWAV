// The fake core's Tasks: marking done, the estimate, the time a task took,
// the sidebar's lists in their display order, your average time, quick
// capture and the toolbar's Sync (docs/HEAT.md).

import { clock } from '../../shared/time/format';
import { dayKey, minuteOfDay } from '../../shared/time/zone';
import type { DayKey, Id, Task, TaskOccurrence } from '../client';
import { actualMin, estimateContext, formatMinutes } from '../model/estimate';
import { setTook } from '../model/focus';
import * as copy from '../model/copy';
import type * as M from '../model/records';
import { nextOpenOccurrence, recurs } from '../model/recurrence';
import { libraryLists } from '../model/spaces';
import { derive, refuse, register } from './core';
import { modelData, setSyncedAt } from './today';

// --- the lists and the averages ----------------------------------------------

derive((snap, fake) => {
  const { zone: tz, now } = fake;
  const data = modelData(snap.records, []);
  const lists = libraryLists(
    {
      tasks: data.tasks,
      occurrences: data.occurrences,
      captures: snap.records.capture as M.Capture[],
      projects: snap.records.project as M.Project[],
      milestones: snap.records.milestone as M.Milestone[],
      courses: snap.records.course as M.Course[],
    },
    now,
    tz,
  );
  const ids = (rows: { id: Id }[]) => rows.map((r) => r.id);
  snap.derived.lists = {
    inbox: ids(lists.inbox),
    allOpen: ids(lists.allOpen),
    hot: ids(lists.hot),
    dueThisWeek: ids(lists.dueThisWeek),
    scheduled: ids(lists.scheduled),
    someday: ids(lists.someday),
    done: ids([...lists.done].sort((a, b) => (b.doneAt ?? 0) - (a.doneAt ?? 0))),
  };

  const ctx = estimateContext(data.tasks, data.sessions);
  snap.derived.averages = snap.records.space.flatMap((space) =>
    space.types.flatMap((type) => {
      const a = ctx.averages.get(`${space.id}\u0000${type}`);
      return a ? [{ space: space.id, type, minutes: a.minutes, count: a.count }] : [];
    }),
  );
});

// --- done --------------------------------------------------------------------

register('heat.done', (args, fake) => {
  const taskId = args.taskId as Id;
  const task = fake.store.task.get(taskId);
  if (!task) refuse('No task has that id.');
  const done = args.done === true;
  const sessions = [...fake.store.focusSession.values()].filter((s) => s.taskId === taskId);
  const data = modelData(
    {
      task: [...fake.store.task.values()],
      taskOccurrence: [...fake.store.taskOccurrence.values()],
      timeBlock: [],
      focusSession: [...fake.store.focusSession.values()],
      habit: [],
    },
    [],
  );
  const model = task as unknown as M.Task;

  if (recurs(model)) {
    const next = nextOpenOccurrence(model, data.occurrences, fake.now, fake.zone);
    const day = (args.date as DayKey | undefined) ?? next?.date ?? dayKey(fake.now, fake.zone);
    const has = [...fake.store.taskOccurrence.values()].find((o) => o.taskId === taskId && o.date === day);
    if (done && has) return { task, undo: null };
    if (!done && !has) return { task, undo: null };
    const { undo } = fake.write(done ? 'mark done' : 'mark not done', ['taskOccurrence'], () => {
      if (done) {
        const o: TaskOccurrence = { id: fake.newId(), taskId, date: day, doneAt: fake.now };
        fake.store.taskOccurrence.set(o.id, o);
      } else fake.store.taskOccurrence.delete(has!.id);
    });
    const since = Math.max(
      -Infinity,
      ...[...fake.store.taskOccurrence.values()]
        .filter((o) => o.taskId === taskId && o.date !== day)
        .map((o) => o.doneAt),
    );
    const recent = sessions.filter((s) => s.endedAt > since);
    const took =
      done && recent.length > 0
        ? copy.check.done(formatMinutes(recent.reduce((n, s) => n + s.focusMin, 0)), recent.length)
        : undefined;
    return { task, ...(took ? { took } : {}), undo };
  }

  const record: Task = { ...task, done, doneAt: done ? fake.now : null };
  const { undo } = fake.write(done ? 'mark done' : 'mark not done', ['task'], () =>
    fake.store.task.set(taskId, record),
  );
  if (done && fake.state.currentTaskId === taskId) fake.state.currentTaskId = null;
  const minutes = actualMin(
    record as unknown as M.Task,
    sessions.map((s) => ({ ...s, room: 'heat' as const })),
  );
  const took = done && minutes > 0 ? copy.check.done(formatMinutes(minutes), sessions.length) : undefined;
  return { task: record, ...(took ? { took } : {}), undo };
});

register('heat.estimate', (args, fake) => {
  const task = fake.store.task.get(args.taskId as Id);
  if (!task) refuse('No task has that id.');
  let clamped = false;
  const record: Task = { ...task };
  if (typeof args.difficulty === 'number') {
    const d = Math.min(5, Math.max(1, Math.round(args.difficulty)));
    clamped ||= d !== args.difficulty;
    record.difficulty = d;
  }
  if (typeof args.estMin === 'number') {
    const m = Math.min(600, Math.max(5, Math.round(args.estMin)));
    clamped ||= m !== args.estMin;
    record.estMin = m;
  }
  record.estBy = 'you';
  delete record.estReason;
  const { undo } = fake.write('estimate', ['task'], () => fake.store.task.set(record.id, record));
  return { task: record, clamped, undo };
});

register('heat.tookTime', (args, fake) => {
  const task = fake.store.task.get(args.taskId as Id);
  if (!task) refuse('No task has that id.');
  const minutes = Math.max(0, Math.round(args.minutes as number));
  const sessions = [...fake.store.focusSession.values()].map((s) => ({ ...s, room: 'heat' as const }));
  const record = setTook(task as unknown as M.Task, sessions, minutes) as unknown as Task;
  const { undo } = fake.write('change time taken', ['task'], () => fake.store.task.set(record.id, record));
  return { task: record, undo };
});

// --- capture -----------------------------------------------------------------

const inboxLine = (n: number) => copy.capture.footer(n, true);

register('heat.capture.add', (args, fake) => {
  const text = String(args.text ?? '').trim();
  if (!text) refuse('Type something to capture.');
  const capture = { id: fake.newId(), text, ...(args.link ? { link: args.link as never } : {}) };
  const { undo } = fake.write('capture', ['capture'], () => fake.store.capture.set(capture.id, capture));
  const inbox = [...fake.store.capture.values()].filter((c) => !c.triagedAt).length;
  return { capture, inbox: inboxLine(inbox), undo };
});

register('heat.capture.triage', (args, fake) => {
  const capture = fake.store.capture.get(args.id as Id);
  if (!capture) refuse('No capture has that id.');
  const to = args.to as 'task' | 'note' | 'project' | 'upload';
  const first = [...fake.store.space.values()][0];
  const made = (args.record ?? {}) as Record<string, unknown>;
  let result: { type: typeof to; id: Id | null } = { type: to, id: null };
  const { undo } = fake.write('triage capture', ['capture', 'task', 'note', 'project'], () => {
    if (to === 'task') {
      const task: Task = {
        id: fake.newId(),
        spaceId: first?.id ?? '',
        title: capture.text,
        type: first?.types[0] ?? 'Other',
        due: null,
        difficulty: 3,
        estMin: null,
        adjustMin: 0,
        notes: '',
        done: false,
        doneAt: null,
        source: 'capture',
        public: false,
        ...made,
      } as Task;
      fake.store.task.set(task.id, task);
      result = { type: to, id: task.id };
    } else if (to === 'note') {
      const note = { id: fake.newId(), markdown: capture.text, public: false, ...made };
      fake.store.note.set(note.id, note);
      result = { type: to, id: note.id };
    } else if (to === 'project') {
      const project = {
        id: fake.newId(),
        spaceId: first?.id ?? '',
        title: capture.text,
        status: 'active' as const,
        public: false,
        ...made,
      };
      fake.store.project.set(project.id, project);
      result = { type: to, id: project.id };
    }
    fake.store.capture.set(capture.id, {
      ...capture,
      triagedAt: fake.now,
      resultType: to,
      ...(result.id ? { resultId: result.id } : {}),
    });
  });
  return { result, undo };
});

// --- Sync --------------------------------------------------------------------

register('heat.calendars.sync', (_args, fake) => {
  setSyncedAt(fake, fake.now);
  fake.emit([]);
  return {
    line: copy.sync.synced(clock(minuteOfDay(fake.now, fake.zone)), { newTasks: 0, dateChanges: 0, newGrades: 0 }),
  };
});
