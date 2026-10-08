// The Focus layout's rules (docs/FOCUS.md), as the core works them out in
// crates/wi-core/src/focus.rs. This file is the reference the Rust was
// ported from, and what the fake core answers with in tests and in
// `npm run dev`; the real views only read `snapshot.focus`.
//
// Three things are decided here, each from the snapshot alone:
//
//   entropy          how out of order things are, 0 to 1
//   nowOf            what Focus shows: a task, the fix, or nothing
//   shouldInterrupt  whether an event may put one line under the Now task
//
// Every number they turn on is in CONFIG.

import { clockAt } from '../shared/time/format';
import { addDays, atMinute, dayKey, daysBetween } from '../shared/time/zone';
import type { Id, Snapshot, Task } from '../heat/client';
import { copy, dueText, formatMinutes } from '../heat/fmt';

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

/** Every threshold of the Focus layout, in one place. The core sends its own copy in `snapshot.focus.config`. */
export const CONFIG = {
  /** "This week": a task due within this many days counts when it has no planned time. */
  horizonDays: 7,
  /** What one of each open loop weighs. */
  weights: { overdue: 3, unplanned: 2, mail: 1.5, grades: 0.5 },
  /** The weighted sum at which entropy is 1. */
  full: 12,
  /** Entropy at or above this is "busy"; below it, and above 0, "calm". */
  busyAt: 0.2,
  /** Entropy at or above this is "high": Focus shows the fix before the next task. */
  highAt: 0.5,
  /** Deadlines this near are checked for risk. */
  riskHorizonDays: 7,
  /** The stretch of a day that can be planned, in minutes after midnight. */
  dayStartsMin: 7 * 60,
  dayEndsMin: 22 * 60,
  /** No day gives more than this much working time, however empty it is. */
  plannablePerDayMin: 6 * 60,
  /** Work shorter than this is never "at risk". */
  riskFloorMin: 15,
  /** A due date that moved interrupts when it now falls within this many days. */
  dueChangedWithinDays: 7,
  /** A task from mail is urgent when it is due within this many hours. */
  urgentWithinHours: 48,
  /** "Time to leave" shows this many minutes before a commitment starts. */
  leaveLeadMin: 15,
  /** A queued event older than this is dropped unseen. */
  queueKeepDays: 7,
  /** The hint under the task shows for this many days or launches, whichever ends first. */
  hintDays: 7,
  hintLaunches: 20,
};
export type FocusConfig = typeof CONFIG;

export type EntropyLevel = 'clear' | 'calm' | 'busy' | 'high';

export interface Entropy {
  /** 0 when every loop is closed, 1 at `full` and beyond. */
  score: number;
  level: EntropyLevel;
  parts: { unplanned: number; overdue: number; mail: number; grades: number };
}

export interface Action {
  label: string;
  /**
   * `current`: make the task the Now task. `task`: show it in Tasks. `view`: open a tool. `break`: start the break.
   * `plan`: Plan my day. `command`: run one of Learn's own commands, for a view whose line has its own answer.
   */
  do: 'current' | 'task' | 'view' | 'break' | 'plan' | 'command';
  taskId?: Id;
  view?: string;
  /** A `command` action's command, one of `heat.*`, and what it is given. */
  cmd?: string;
  args?: Record<string, unknown>;
}

export interface Fix {
  kind: 'plan' | 'mail' | 'grades';
  /** "4 tasks this week have no plan." */
  line: string;
  /** "Plan them?" */
  ask: string;
  action: Action;
}

export type Now =
  | { kind: 'task'; taskId: Id; why: 'current' | 'heat'; fix: null }
  | { kind: 'fix'; taskId: null; why: null; fix: Fix }
  | { kind: 'clear'; taskId: null; why: null; fix: null };

export type Priority = 'low' | 'normal' | 'high';

/** Something that happened, or is so now, that might be worth one line. */
export type FocusEvent =
  | { kind: 'dueChanged'; taskId: Id; from: number | null; to: number | null; at: number }
  | { kind: 'atRisk'; taskId: Id; due: number; workLeftMin: number; plannableMin: number }
  | { kind: 'mailUrgent'; taskId: Id; at: number }
  | { kind: 'focusEnded'; key: string; note: string | null }
  | { kind: 'leaveFor'; eventId: Id; title: string; start: number }
  | { kind: 'gradeWaiting'; gradeId: Id; title: string; count: number }
  | {
      kind: 'custom';
      id: string;
      source: string;
      line: string;
      action: Action | null;
      changesNext: boolean;
      priority: Priority;
      at: number;
    };

export interface Interrupt {
  id: string;
  /** The event's kind, or a custom event's own source: what a view registers to be opened by. */
  source: string;
  line: string;
  action: Action | null;
  priority: Priority;
}

/** What a commitment is until commitments have a record of their own: the next calendar event. */
export interface NextCommitment {
  id: Id;
  title: string;
  start: number;
}

export interface FocusState {
  entropy: Entropy;
  now: Now;
  next: NextCommitment | null;
  interrupt: Interrupt | null;
  /** How many more are waiting behind the one shown. */
  queued: number;
  config: FocusConfig;
}

/** What the core remembers between snapshots; none of it is journaled. */
export interface FocusMemory {
  /** Events that happened: a due date moved, mail made a task, something raised by a view. */
  queue: FocusEvent[];
  /** Interrupts sent away, by id, with when. */
  dismissed: Record<string, number>;
  /** The day the fix was last put off, so it isn't offered again that day. */
  fixSnoozed: string | null;
}

export const emptyMemory = (): FocusMemory => ({ queue: [], dismissed: {}, fixSnoozed: null });

const round2 = (x: number) => Math.round(x * 100) / 100;

function openIds(snap: Snapshot): Id[] {
  return snap.derived.lists.allOpen;
}

function taskMap(snap: Snapshot): Map<Id, Task> {
  return new Map(snap.records.task.map((t) => [t.id, t]));
}

/** Whether a task has time set aside from today on: a block, or a day it is scheduled for. */
function planned(snap: Snapshot, t: Task): boolean {
  if (t.scheduledDate !== undefined && t.scheduledDate >= snap.date) return true;
  return snap.records.timeBlock.some((b) => b.taskId === t.id && b.date >= snap.date);
}

/** A mail thread still asking for something: it made no task yet, or it is pressing and unread. */
function mailNeedsAction(snap: Snapshot): number {
  return snap.records.mailThread.filter((m) => {
    const place = snap.mailState?.[m.gmailThreadId];
    if (place?.archived) return false;
    if (m.state === 'task' && !m.taskId) return true;
    return place?.unread === true && (m.priority === 'urgent' || m.priority === 'high');
  }).length;
}

export function entropyOf(snap: Snapshot, cfg: FocusConfig = CONFIG): Entropy {
  const tasks = taskMap(snap);
  let overdue = 0;
  let unplanned = 0;
  for (const id of openIds(snap)) {
    const t = tasks.get(id);
    if (!t) continue;
    if (snap.derived.tasks[id]?.heat.level === 'Overdue') {
      overdue += 1;
      continue;
    }
    if (t.rrule || t.due === null) continue;
    if (t.due - snap.now <= cfg.horizonDays * DAY && !planned(snap, t)) unplanned += 1;
  }
  const parts = {
    unplanned,
    overdue,
    mail: mailNeedsAction(snap),
    grades: snap.records.grade.filter((g) => g.pending).length,
  };
  const w = cfg.weights;
  const sum = parts.overdue * w.overdue + parts.unplanned * w.unplanned + parts.mail * w.mail + parts.grades * w.grades;
  const score = round2(Math.min(1, sum / cfg.full));
  const level: EntropyLevel = sum === 0 ? 'clear' : score >= cfg.highAt ? 'high' : score >= cfg.busyAt ? 'busy' : 'calm';
  return { score, level, parts };
}

const count = (n: number, one: string, many: string) => (n === 1 ? one : many.replace('#', String(n)));

/** The one thing that would put the most back in order: the heaviest part that has a fix. Overdue work has none but doing it. */
export function fixOf(e: Entropy, cfg: FocusConfig = CONFIG): Fix | null {
  const w = cfg.weights;
  const options: { weight: number; fix: Fix }[] = [
    {
      weight: e.parts.unplanned * w.unplanned,
      fix: {
        kind: 'plan',
        line: count(e.parts.unplanned, '1 task this week has no plan.', '# tasks this week have no plan.'),
        ask: count(e.parts.unplanned, 'Plan it?', 'Plan them?'),
        action: { label: 'Plan my day', do: 'plan' },
      },
    },
    {
      weight: e.parts.mail * w.mail,
      fix: {
        kind: 'mail',
        line: count(e.parts.mail, '1 mail needs you.', '# mails need you.'),
        ask: count(e.parts.mail, 'Read it?', 'Read them?'),
        action: { label: 'Open Mail', do: 'view', view: 'mail' },
      },
    },
    {
      weight: e.parts.grades * w.grades,
      fix: {
        kind: 'grades',
        line: count(e.parts.grades, '1 grade is waiting for its score.', '# grades are waiting for their scores.'),
        ask: count(e.parts.grades, 'Enter it?', 'Enter them?'),
        action: { label: 'Open Grades', do: 'view', view: 'grades' },
      },
    },
  ];
  let best: { weight: number; fix: Fix } | null = null;
  for (const o of options) if (o.weight > 0 && (best === null || o.weight > best.weight)) best = o;
  return best?.fix ?? null;
}

/**
 * What Focus shows. The current task if one is set; when entropy is high,
 * the fix; else the top open task by heat; else the fix for what is left;
 * else nothing. A fix put off today stays put off until tomorrow.
 */
export function nowOf(snap: Snapshot, e: Entropy, mem: FocusMemory, cfg: FocusConfig = CONFIG): Now {
  const open = openIds(snap);
  const current = snap.heatState.currentTaskId;
  if (current && open.includes(current)) return { kind: 'task', taskId: current, why: 'current', fix: null };
  const fix = mem.fixSnoozed === snap.date ? null : fixOf(e, cfg);
  if (fix && e.score >= cfg.highAt) return { kind: 'fix', taskId: null, why: null, fix };
  // `allOpen` is in heat order already (heat/model/spaces.ts).
  if (open.length > 0) return { kind: 'task', taskId: open[0], why: 'heat', fix: null };
  if (fix) return { kind: 'fix', taskId: null, why: null, fix };
  return { kind: 'clear', taskId: null, why: null, fix: null };
}

export function nextCommitment(snap: Snapshot): NextCommitment | null {
  let next: NextCommitment | null = null;
  for (const e of snap.events) {
    if (e.allDay || e.start <= snap.now) continue;
    if (next === null || e.start < next.start) next = { id: e.id, title: e.title, start: e.start };
  }
  return next;
}

/** Minutes that can still be planned between now and `due`, with the calendar and other tasks' blocks taken out. */
export function plannableMin(snap: Snapshot, taskId: Id, due: number, cfg: FocusConfig = CONFIG): number {
  const tz = snap.zone;
  const last = dayKey(due, tz);
  let total = 0;
  for (let day = snap.date, n = 0; day <= last && n <= cfg.riskHorizonDays; day = addDays(day, 1), n += 1) {
    const lo = Math.max(atMinute(day, cfg.dayStartsMin, tz), snap.now);
    const hi = Math.min(atMinute(day, cfg.dayEndsMin, tz), due);
    if (hi <= lo) continue;
    const busy: [number, number][] = [];
    for (const e of snap.events) if (!e.allDay) busy.push([e.start, e.end]);
    for (const b of snap.records.timeBlock) {
      if (b.date !== day || b.taskId === taskId) continue;
      const start = atMinute(day, b.start, tz);
      busy.push([start, start + b.minutes * MIN]);
    }
    busy.sort((a, b) => a[0] - b[0]);
    let taken = 0;
    let cursor = lo;
    for (const [s, t] of busy) {
      const from = Math.max(s, cursor);
      const to = Math.min(t, hi);
      if (to > from) {
        taken += to - from;
        cursor = to;
      }
    }
    total += Math.min(cfg.plannablePerDayMin, Math.floor((hi - lo - taken) / MIN));
  }
  return total;
}

/** What is so right now that might be worth a line: deadlines at risk, a break that waits, a commitment near, grades waiting. */
export function standingEvents(snap: Snapshot, cfg: FocusConfig = CONFIG): FocusEvent[] {
  const events: FocusEvent[] = [];
  const tasks = taskMap(snap);
  for (const id of openIds(snap)) {
    const t = tasks.get(id);
    const d = snap.derived.tasks[id];
    if (!t || !d || t.rrule || t.due === null || t.due <= snap.now) continue;
    if (t.due - snap.now > cfg.riskHorizonDays * DAY) continue;
    const workLeftMin = Math.max(0, Math.round(d.estimate.min - d.actualMin));
    if (workLeftMin < cfg.riskFloorMin) continue;
    const free = plannableMin(snap, id, t.due, cfg);
    if (workLeftMin > free) events.push({ kind: 'atRisk', taskId: id, due: t.due, workLeftMin, plannableMin: free });
  }

  const timer = snap.heatState.timer;
  const running = timer.running ?? timer.endsAt !== null;
  if (timer.phase === 'break' && !running && (timer.leftMs ?? timer.lengthMs) === timer.lengthMs) {
    const last = snap.records.focusSession.reduce((m, s) => Math.max(m, s.endedAt), 0);
    events.push({ kind: 'focusEnded', key: `${timer.round}:${last}`, note: timer.note ?? null });
  }

  const next = nextCommitment(snap);
  if (next && next.start - snap.now <= cfg.leaveLeadMin * MIN) {
    events.push({ kind: 'leaveFor', eventId: next.id, title: next.title, start: next.start });
  }

  const pending = snap.records.grade.filter((g) => g.pending);
  if (pending.length > 0) {
    const newest = pending.reduce((a, b) => ((b.postedAt ?? 0) > (a.postedAt ?? 0) || (b.postedAt === a.postedAt && b.id > a.id) ? b : a));
    events.push({ kind: 'gradeWaiting', gradeId: newest.id, title: newest.title, count: pending.length });
  }
  return events;
}

export interface InterruptContext {
  snap: Snapshot;
  /** The task Focus shows, if it shows one. */
  nowTaskId: Id | null;
  cfg: FocusConfig;
}

/** A focus round is running: only what can't wait gets through. */
function focusing(snap: Snapshot): boolean {
  const t = snap.heatState.timer;
  return t.phase === 'focus' && (t.running ?? t.endsAt !== null);
}

/**
 * The one decision: may this event interrupt? Only if it changes what the
 * person should do next. Answers the line to show, or null. An event that
 * gets null is not lost: a standing one is asked about again at the next
 * snapshot, and a queued one waits its turn.
 */
export function shouldInterrupt(event: FocusEvent, ctx: InterruptContext): Interrupt | null {
  const { snap, cfg } = ctx;
  const tz = snap.zone;
  const open = new Set(openIds(snap));
  const task = (id: Id) => (open.has(id) ? snap.records.task.find((t) => t.id === id) : undefined);
  const made = ((): Interrupt | null => {
    switch (event.kind) {
      case 'dueChanged': {
        const t = task(event.taskId);
        // Done since, or moved again: the line would no longer be true.
        if (!t || t.due !== event.to || event.to === event.from) return null;
        const near = (due: number | null, span: number) => due !== null && due - snap.now <= span;
        const changes =
          event.taskId === ctx.nowTaskId ||
          near(event.to, cfg.dueChangedWithinDays * DAY) ||
          near(event.from, cfg.urgentWithinHours * HOUR);
        if (!changes) return null;
        return {
          id: `due:${t.id}:${event.to}`,
          source: 'dueChanged',
          line:
            event.to === null
              ? `Due date removed: ${t.title} has no due date now.`
              : `Due date moved: ${t.title} is now due ${dueText(event.to, snap.now, tz)}.`,
          action: { label: 'Show it', do: 'task', taskId: t.id },
          priority: near(event.to, cfg.urgentWithinHours * HOUR) ? 'high' : 'normal',
        };
      }
      case 'atRisk': {
        const t = task(event.taskId);
        // Already the thing being done: nothing about what comes next changes.
        if (!t || t.due !== event.due || event.taskId === ctx.nowTaskId) return null;
        if (event.workLeftMin <= event.plannableMin) return null;
        return {
          id: `risk:${t.id}:${event.due}`,
          source: 'atRisk',
          line: `At risk: ${t.title} needs ${formatMinutes(event.workLeftMin)}, and ${
            event.plannableMin > 0 ? `only ${formatMinutes(event.plannableMin)} is` : 'no time is'
          } free before it is due.`,
          action: { label: 'Do it now', do: 'current', taskId: t.id },
          priority: 'high',
        };
      }
      case 'mailUrgent': {
        const t = task(event.taskId);
        if (!t || event.taskId === ctx.nowTaskId) return null;
        const thread = snap.records.mailThread.find((m) => m.taskId === t.id);
        const soon = t.due !== null && t.due - snap.now <= cfg.urgentWithinHours * HOUR;
        if (!soon && thread?.priority !== 'urgent') return null;
        return {
          id: `mail:${t.id}`,
          source: 'mailUrgent',
          line:
            t.due === null
              ? `From mail, and urgent: ${t.title}.`
              : `From mail: ${t.title}, due ${dueText(t.due, snap.now, tz)}.`,
          action: { label: 'Do it now', do: 'current', taskId: t.id },
          priority: 'high',
        };
      }
      case 'focusEnded':
        return {
          id: `focus:${event.key}`,
          source: 'focusEnded',
          line: event.note ?? copy.focus.doneUnlogged,
          action: { label: 'Start break', do: 'break' },
          priority: 'high',
        };
      case 'leaveFor': {
        const lead = event.start - snap.now;
        if (lead <= 0 || lead > cfg.leaveLeadMin * MIN) return null;
        return {
          id: `leave:${event.eventId}:${event.start}`,
          source: 'leaveFor',
          line: `Time to leave: ${event.title} starts at ${clockAt(event.start, tz)}.`,
          action: { label: 'Open Calendar', do: 'view', view: 'calendar' },
          priority: 'high',
        };
      }
      case 'gradeWaiting':
        return {
          id: `grade:${event.gradeId}:${event.count}`,
          source: 'gradeWaiting',
          line:
            event.count === 1
              ? `A grade is waiting for its score: ${event.title}.`
              : `${event.count} grades are waiting for their scores.`,
          action: { label: 'Open Grades', do: 'view', view: 'grades' },
          priority: 'low',
        };
      case 'custom':
        if (!event.changesNext) return null;
        return { id: event.id, source: event.source, line: event.line, action: event.action, priority: event.priority };
    }
  })();
  if (made && focusing(snap) && made.priority !== 'high') return null;
  return made;
}

const RANK: Record<Priority, number> = { high: 0, normal: 1, low: 2 };

/** At most one interrupt, and how many wait behind it: the most pressing first, then the oldest. */
export function pick(
  events: FocusEvent[],
  ctx: InterruptContext,
  dismissed: Record<string, number>,
): { interrupt: Interrupt | null; queued: number } {
  const seen = new Set<string>();
  const live: Interrupt[] = [];
  for (const e of events) {
    const i = shouldInterrupt(e, ctx);
    if (!i || i.id in dismissed || seen.has(i.id)) continue;
    seen.add(i.id);
    live.push(i);
  }
  // A stable sort: within a priority, events keep the order they came in.
  live.sort((a, b) => RANK[a.priority] - RANK[b.priority]);
  return { interrupt: live[0] ?? null, queued: Math.max(0, live.length - 1) };
}

/** Everything `snapshot.focus` holds. */
export function focusState(snap: Snapshot, mem: FocusMemory, cfg: FocusConfig = CONFIG): FocusState {
  const entropy = entropyOf(snap, cfg);
  const now = nowOf(snap, entropy, mem, cfg);
  const keep = snap.now - cfg.queueKeepDays * DAY;
  const queue = mem.queue.filter((e) => !('at' in e) || e.at >= keep);
  const { interrupt, queued } = pick(
    [...queue, ...standingEvents(snap, cfg)],
    { snap, nowTaskId: now.taskId, cfg },
    mem.dismissed,
  );
  return { entropy, now, next: nextCommitment(snap), interrupt, queued, config: cfg };
}

/** One record a journal entry changed, as both cores see it. */
export interface Change {
  kind: string;
  before: unknown;
  after: unknown;
}

/**
 * What someone other than the person changed (Claude, reading mail), turned
 * into events for the queue: a task's due date moved, or mail made a task.
 */
export function eventsFromChanges(changes: Change[], at: number): FocusEvent[] {
  const events: FocusEvent[] = [];
  for (const c of changes) {
    if (c.kind !== 'task') continue;
    const before = c.before as Task | null | undefined;
    const after = c.after as Task | null | undefined;
    if (!after || after.done) continue;
    if (!before) {
      if (after.source === 'mail') events.push({ kind: 'mailUrgent', taskId: after.id, at });
      continue;
    }
    const from = typeof before.due === 'number' ? before.due : null;
    const to = typeof after.due === 'number' ? after.due : null;
    if (from !== to) events.push({ kind: 'dueChanged', taskId: after.id, from, to, at });
  }
  return events;
}

/** Days from the snapshot's day to `ms`: 0 is today. For the readout's "NEXT". */
export function daysAhead(snap: Snapshot, ms: number): number {
  return daysBetween(snap.date, dayKey(ms, snap.zone));
}
