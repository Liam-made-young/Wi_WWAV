// The fake core's Today: the plan, the blocks, the focus timer, the current
// task, the Today lists and every task's derived values (docs/HEAT.md). The
// rules come from the TS model, the reference the Rust was ported from; the
// views never call it.

import { clock } from '../../shared/time/format';
import { addDays, dayKey, minuteOfDay } from '../../shared/time/zone';
import type { CalendarEvent, DayKey, FocusSession, HeatTimer, Id, Task, TimeBlock } from '../client';
import { actualMin, estimateContext, estimateMin } from '../model/estimate';
import { type FocusEffect, type FocusState, type FocusTarget, focusStep, initialFocus } from '../model/focus';
import { byHeat, heatOf } from '../model/heat';
import type * as M from '../model/records';
import { nextOpenOccurrence, openTasks, recurs, taskOccurrences } from '../model/recurrence';
import { acceptDraft, planMyDay, type PlanData, planSections, planSubtitle } from '../model/plan';
import { COLUMN_END, COLUMN_START, busySpans, firstGap } from '../today/gap';
import { type Fake, derive, refuse, register } from './core';

/** What the fake keeps beyond the records: other calendars' events, and the timer's whole state. */
interface Extra {
  events: CalendarEvent[];
  focus: FocusState;
  syncedAt: number | null;
}

const extras = new WeakMap<Fake, Extra>();

export function extra(fake: Fake): Extra {
  let e = extras.get(fake);
  if (!e) {
    e = { events: [], focus: initialFocus(), syncedAt: null };
    extras.set(fake, e);
  }
  return e;
}

export function setEvents(fake: Fake, events: CalendarEvent[]) {
  extra(fake).events = events;
}

export function setSyncedAt(fake: Fake, at: number) {
  extra(fake).syncedAt = at;
}

// The model reads records a little differently (its TaskSource has no 'claude', its sessions name a room),
// so the records are handed over as it expects them.
export function modelData(
  records: {
    task: Task[];
    taskOccurrence: { id: Id; taskId: Id; date: DayKey; doneAt: number }[];
    timeBlock: TimeBlock[];
    focusSession: FocusSession[];
    habit: { id: Id; title: string; minutes?: number; log: Record<DayKey, true>; showCounter: boolean }[];
  },
  events: CalendarEvent[],
): PlanData {
  return {
    tasks: records.task as unknown as M.Task[],
    occurrences: records.taskOccurrence as M.TaskOccurrence[],
    blocks: records.timeBlock as M.TimeBlock[],
    events: events as M.CalendarEvent[],
    sessions: records.focusSession.map((s) => ({ ...s, view: 'heat' as const })) as M.FocusSession[],
    habits: records.habit as M.Habit[],
  };
}

const storeData = (fake: Fake): PlanData =>
  modelData(
    {
      task: [...fake.store.task.values()],
      taskOccurrence: [...fake.store.taskOccurrence.values()],
      timeBlock: [...fake.store.timeBlock.values()],
      focusSession: [...fake.store.focusSession.values()],
      habit: [...fake.store.habit.values()],
    },
    extra(fake).events,
  );

// --- the snapshot ------------------------------------------------------------

derive((snap, fake) => {
  const { zone: tz, now } = fake;
  const data = modelData(snap.records, extra(fake).events);
  const ctx = estimateContext(data.tasks, data.sessions);
  const open = new Map(openTasks(data.tasks, data.occurrences, now, tz).map((t) => [t.id, t]));

  for (const t of data.tasks) {
    const series = recurs(t);
    const seen = series ? (open.get(t.id) ?? { ...t, due: null }) : t;
    const heat = heatOf(seen, now);
    const own = t as unknown as Task;
    const next = series ? (nextOpenOccurrence(t, data.occurrences, now, tz)?.date ?? null) : undefined;
    snap.derived.tasks[t.id] = {
      heat: { v: heat.v, level: heat.level },
      actualMin: actualMin(t, data.sessions),
      estimate: {
        min: estimateMin(t, ctx),
        by: own.estBy ?? (t.estMin !== null && t.estMin > 0 ? 'you' : 'default'),
        reason: own.estReason ?? null,
      },
      ...(next !== undefined ? { next } : {}),
    };
  }

  const sections = planSections(data, now, tz);
  const items = <K extends string>(kind: K) => sections.find((s) => s.kind === kind)?.items ?? [];
  snap.derived.today = {
    header: planSubtitle(data, now, tz),
    planned: (items('planned') as { block: M.TimeBlock }[]).map((r) => r.block.id),
    dueToday: (items('dueToday') as M.Task[]).map((t) => t.id),
    recurringToday: (items('recurring') as { kind: 'task' | 'habit'; id: Id }[]).map((r) => ({
      kind: r.kind,
      id: r.id,
    })),
    hotUnplanned: (items('hot') as M.Task[]).map((t) => t.id),
  };
  snap.derived.hotTasks = byHeat([...open.values()], now)
    .filter((t) => ['Hot', 'Overdue'].includes(heatOf(t, now).level))
    .slice(0, 5)
    .map((t) => t.id);

  snap.events = extra(fake).events;
  const synced = extra(fake).syncedAt;
  snap.derived.status =
    synced === null ? 'Saved on this Mac' : `Saved on this Mac · Synced ${clock(minuteOfDay(synced, tz))}`;

  // Calendar's recurring pills: each series' occurrences from 45 days back to 130 ahead of today.
  const from = addDays(snap.date, -45);
  const to = addDays(snap.date, 130);
  const ticked = new Set(snap.records.taskOccurrence.map((o) => `${o.taskId}\u0000${o.date}`));
  snap.derived.occurrences = data.tasks
    .filter((t) => !t.done && recurs(t))
    .flatMap((t) =>
      taskOccurrences(t, tz, from, to).map((o) => ({
        taskId: t.id,
        date: o.date,
        done: ticked.has(`${t.id}\u0000${o.date}`),
      })),
    );
});

// --- blocks ------------------------------------------------------------------

const snap15 = (n: number) => Math.round(n / 15) * 15;
const clamp = (x: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, x));

register('heat.block.put', (args, fake) => {
  const a = args as { id?: Id; taskId?: Id; habitId?: Id; date: DayKey; start: number; minutes: number };
  if (!a.taskId && !a.habitId && !a.id) refuse('A block needs a task or a habit.');
  const was = a.id ? fake.store.timeBlock.get(a.id) : undefined;
  const taskId = a.taskId ?? was?.taskId;
  const habitId = a.habitId ?? was?.habitId;
  if (taskId && !fake.store.task.has(taskId)) refuse('No task has that id.');
  const start = clamp(snap15(a.start), COLUMN_START, COLUMN_END - 15);
  const minutes = clamp(snap15(a.minutes), 15, COLUMN_END - start);
  const block: TimeBlock = {
    id: was?.id ?? a.id ?? fake.newId(),
    ...(taskId ? { taskId } : {}),
    ...(habitId ? { habitId } : {}),
    date: a.date,
    start,
    minutes,
    origin: was?.origin ?? 'you',
  };
  const label = !was
    ? 'add block'
    : was.start !== start || was.date !== a.date
      ? 'move block'
      : was.minutes !== minutes
        ? 'resize block'
        : 'edit block';
  const { undo } = fake.write(label, ['timeBlock'], () => fake.store.timeBlock.set(block.id, block));
  return { block, undo };
});

// --- Plan my day -------------------------------------------------------------

register('heat.plan.make', (args, fake) => {
  const date = (args.date as DayKey) ?? dayKey(fake.now, fake.zone);
  const dayEndsAt = typeof args.dayEnds === 'number' ? args.dayEnds : undefined;
  const data = storeData(fake);
  const drafts = planMyDay(data, fake.now, fake.zone, { dayEndsAt });
  fake.state.planDrafts = drafts;
  fake.emit([]);

  const planned = new Set(data.blocks.filter((b) => b.date === date).map((b) => b.taskId));
  const parents = new Set(data.tasks.filter((t) => !t.done && t.parentTaskId).map((t) => t.parentTaskId));
  const drafted = new Set(drafts.map((d) => d.taskId));
  const unplanned = openTasks(data.tasks, data.occurrences, fake.now, fake.zone)
    .filter((t) => !planned.has(t.id) && !parents.has(t.id) && !drafted.has(t.id))
    .map((t) => t.id);

  // The 15-minute marks still free between now and the day's end, after the drafts.
  const spans = [
    ...busySpans([...fake.store.timeBlock.values()], extra(fake).events, date, fake.zone),
    ...drafts.map((d) => [d.start, d.start + d.minutes] as [number, number]),
  ];
  let minutesLeft = 0;
  for (
    let s = Math.max(COLUMN_START, Math.ceil(minuteOfDay(fake.now, fake.zone) / 15) * 15);
    s + 15 <= (dayEndsAt ?? 23 * 60);
    s += 15
  ) {
    if (firstGap(spans, s, s + 15, 15) !== null) minutesLeft += 15;
  }
  return { drafts, unplanned, minutesLeft };
});

register('heat.plan.accept', (args, fake) => {
  const names = args.taskIds as Id[] | undefined;
  const drafts = fake.state.planDrafts.filter((d) => !names || names.includes(d.taskId));
  if (drafts.length === 0) refuse('There are no drafts to accept.');
  const blocks = drafts.map((d) => acceptDraft(d, () => fake.newId())) as unknown as TimeBlock[];
  const { undo } = fake.write(names ? 'accept draft' : 'plan my day', ['timeBlock'], () => {
    for (const b of blocks) fake.store.timeBlock.set(b.id, b);
  });
  const left = new Set(drafts.map((d) => d.taskId));
  fake.state.planDrafts = fake.state.planDrafts.filter((d) => !left.has(d.taskId));
  return { blocks, undo };
});

register('heat.plan.clear', (_args, fake) => {
  fake.state.planDrafts = [];
  fake.emit([]);
  return {};
});

// --- the current task and the timer ------------------------------------------

function target(fake: Fake, id: Id | undefined | null): FocusTarget | null {
  const t = id ? fake.store.task.get(id) : undefined;
  return t ? { kind: 'task', id: t.id, title: t.title } : null;
}

// What the snapshot carries of the timer, from the model's state.
function show(fake: Fake, s: FocusState) {
  const timer: HeatTimer = {
    phase: s.phase,
    round: s.round,
    endsAt: s.running ? s.endsAt : null,
    running: s.running,
    leftMs: s.leftMs,
    lengthMs: s.lengthMs,
    focusMin: s.focusMin,
    taskId: s.target?.kind === 'task' ? s.target.id : null,
    note: s.note,
  };
  fake.state.timer = timer;
}

function apply(fake: Fake, effects: FocusEffect[]): FocusSession | undefined {
  let logged: FocusSession | undefined;
  for (const e of effects) {
    if (e.kind !== 'log') continue;
    const session: FocusSession = {
      id: fake.newId(),
      ...(e.session.taskId ? { taskId: e.session.taskId } : {}),
      ...(e.session.habitId ? { habitId: e.session.habitId } : {}),
      startedAt: e.session.startedAt,
      endedAt: e.session.endedAt,
      focusMin: e.session.focusMin,
      interruptions: e.session.interruptions,
      source: 'timer',
      public: false,
    };
    fake.write('focus session', ['focusSession'], () => fake.store.focusSession.set(session.id, session));
    logged = session;
  }
  return logged;
}

function step(fake: Fake, event: Parameters<typeof focusStep>[1]) {
  const e = extra(fake);
  const { state, effects } = focusStep(e.focus, event, fake.now);
  e.focus = state;
  show(fake, state);
  const logged = apply(fake, effects);
  fake.emit([]);
  return { heatState: fake.state, ...(logged ? { logged } : {}) };
}

register('heat.current.set', (args, fake) => {
  const id = (args.taskId as Id | null) ?? null;
  if (id && !fake.store.task.has(id)) refuse('No task has that id.');
  const e = extra(fake);
  fake.state.currentTaskId = id;
  // Changing the current task mid-round logs the part so far and runs on for the new one.
  if (e.focus.phase === 'focus' && id) step(fake, { type: 'setTarget', target: target(fake, id) });
  else fake.emit([]);
  return {};
});

register('heat.focus.start', (args, fake) => {
  const e = extra(fake);
  const id = (args.taskId as Id | undefined) ?? fake.state.currentTaskId ?? undefined;
  const t = target(fake, id);
  if (e.focus.phase === 'idle') {
    if (!t) refuse('Pick a task and press C first.');
    if (typeof args.length === 'number') step(fake, { type: 'setLength', minutes: args.length });
    fake.state.currentTaskId = t.id;
    return step(fake, { type: 'press', target: t });
  }
  // A break that waits for a press starts with the same call.
  if (e.focus.phase === 'break' && !e.focus.running) return step(fake, { type: 'press' });
  return { heatState: fake.state };
});

register('heat.focus.pause', (_args, fake) =>
  extra(fake).focus.running ? step(fake, { type: 'press' }) : { heatState: fake.state },
);
register('heat.focus.resume', (_args, fake) => {
  const s = extra(fake).focus;
  return s.phase !== 'idle' && !s.running ? step(fake, { type: 'press' }) : { heatState: fake.state };
});
register('heat.focus.interrupt', (_args, fake) => step(fake, { type: 'pulledAway' }));
register('heat.focus.stop', (_args, fake) => step(fake, { type: 'stop' }));
register('heat.focus.finish', (_args, fake) => step(fake, { type: 'tick' }));
