// The Pomodoro timer (docs/SPEC.md 3.5), as a state machine. It moves only on
// explicit events, each stamped by the caller's clock, and returns the
// effects for the caller to carry out: a FocusSession to save, a habit to
// tick, the chime. Nothing starts without a press: only F sets it running.
//
// F starts or pauses. ⇧F stops and logs. I marks "Pulled away": it pauses
// and records an interruption. Focus is 25 minutes, 50, or a custom 10–90;
// breaks are 5 minutes and every fourth is 15. The state lives outside any
// room, so switching rooms doesn't touch it (2.3).

import { countdown } from '../../shared/time/format';
import * as copy from './copy';
import { actualMin, formatMinutes } from './estimate';
import type { FocusSession, Id, Room, Task } from './records';

const MIN = 60_000;
const ROUNDS = 4;
export const FOCUS_LENGTHS = [25, 50];

export interface FocusTarget {
  kind: 'task' | 'habit';
  id: Id;
  title: string;
  /** A habit's length: it ticks itself once a session on it reaches this. */
  minutes?: number;
}

export interface FocusState {
  phase: 'idle' | 'focus' | 'break';
  running: boolean;
  /** The focus round, 1 to 4. During a break, the round just finished. */
  round: number;
  focusMin: number;
  /** The whole length of the current focus or break. */
  lengthMs: number;
  /** While running. */
  endsAt: number | null;
  /** While paused or waiting for a press. */
  leftMs: number;
  target: FocusTarget | null;
  /** The session in progress: the part of the round spent on one target. */
  session: { startedAt: number; fromLeftMs: number; interruptions: number; habitTicked: boolean } | null;
  room: Room;
  /** What just ended ("Focus done. 25m logged to …"), until the next press. */
  note: string | null;
}

export type FocusEvent =
  | { type: 'press'; target?: FocusTarget; room?: Room }
  | { type: 'stop' }
  | { type: 'pulledAway' }
  | { type: 'tick' }
  | { type: 'setTarget'; target: FocusTarget | null }
  | { type: 'setLength'; minutes: number };

export type FocusEffect =
  | { kind: 'log'; session: Omit<FocusSession, 'id'> }
  | { kind: 'tickHabit'; habitId: Id }
  | { kind: 'chime' };

/** Heat's chime is off by default (8.7, Open #10). */
export interface FocusSettings {
  chime: boolean;
}

export function isFocusLength(minutes: number): boolean {
  return Number.isInteger(minutes) && minutes >= 10 && minutes <= 90;
}

export function initialFocus(): FocusState {
  return idle({ round: 1, focusMin: 25, target: null, room: 'heat', note: null });
}

function idle(s: Pick<FocusState, 'round' | 'focusMin' | 'target' | 'room' | 'note'>): FocusState {
  const lengthMs = s.focusMin * MIN;
  return { ...s, phase: 'idle', running: false, lengthMs, endsAt: null, leftMs: lengthMs, session: null };
}

function leftAt(s: FocusState, now: number): number {
  return s.running && s.endsAt !== null ? Math.max(0, s.endsAt - now) : s.leftMs;
}

const breakMs = (round: number) => (round % ROUNDS === 0 ? 15 : 5) * MIN;
const nextRound = (round: number) => (round % ROUNDS) + 1;

// A habit with a length ticks once the session on it reaches that length.
function habitTick(s: FocusState, leftNow: number): { state: FocusState; effects: FocusEffect[] } {
  const t = s.target;
  const session = s.session;
  if (!session || session.habitTicked || t?.kind !== 'habit' || t.minutes === undefined) {
    return { state: s, effects: [] };
  }
  if (session.fromLeftMs - leftNow < t.minutes * MIN) return { state: s, effects: [] };
  return {
    state: { ...s, session: { ...session, habitTicked: true } },
    effects: [{ kind: 'tickHabit', habitId: t.id }],
  };
}

// Closes the session in progress; it is logged if it rounds to a minute or more.
function closeSession(s: FocusState, endedAt: number, leftNow: number): { minutes: number; effects: FocusEffect[] } {
  const ticked = habitTick(s, leftNow);
  if (!s.session || !s.target) return { minutes: 0, effects: ticked.effects };
  const minutes = Math.round((s.session.fromLeftMs - leftNow) / MIN);
  if (minutes < 1) return { minutes, effects: ticked.effects };
  const owner = s.target.kind === 'task' ? { taskId: s.target.id } : { habitId: s.target.id };
  const session = {
    ...owner,
    startedAt: s.session.startedAt,
    endedAt,
    focusMin: minutes,
    interruptions: s.session.interruptions,
    room: s.room,
  };
  return { minutes, effects: [...ticked.effects, { kind: 'log', session }] };
}

// A running focus or break whose time is up ends, whatever event arrives.
function settle(s: FocusState, now: number, settings: FocusSettings): { state: FocusState; effects: FocusEffect[] } {
  if (!s.running || s.endsAt === null) return { state: s, effects: [] };
  if (now < s.endsAt) return s.phase === 'focus' ? habitTick(s, s.endsAt - now) : { state: s, effects: [] };
  if (s.phase === 'break') {
    const round = nextRound(s.round);
    return { state: idle({ ...s, round, note: copy.focus.breakDone(round, ROUNDS) }), effects: [] };
  }
  const { minutes, effects } = closeSession(s, s.endsAt, 0);
  const lengthMs = breakMs(s.round);
  return {
    state: {
      ...s,
      phase: 'break',
      running: false,
      lengthMs,
      endsAt: null,
      leftMs: lengthMs,
      session: null,
      note: minutes >= 1 ? copy.focus.done(formatMinutes(minutes), s.target?.title ?? '') : copy.focus.doneUnlogged,
    },
    effects: settings.chime ? [...effects, { kind: 'chime' }] : effects,
  };
}

function pause(s: FocusState, now: number): FocusState {
  return { ...s, running: false, leftMs: leftAt(s, now), endsAt: null };
}

function start(s: FocusState, now: number): FocusState {
  return { ...s, running: true, endsAt: now + s.leftMs, note: null };
}

export function focusStep(
  state: FocusState,
  event: FocusEvent,
  now: number,
  settings: FocusSettings = { chime: false },
): { state: FocusState; effects: FocusEffect[] } {
  const settled = settle(state, now, settings);
  const s = settled.state;
  const done = (next: FocusState, effects: FocusEffect[] = []) => ({
    state: next,
    effects: [...settled.effects, ...effects],
  });

  switch (event.type) {
    case 'tick':
      return done(s);

    case 'press': {
      if (s.phase === 'idle') {
        const target = event.target ?? s.target;
        if (!target) return done(s);
        const begun = { ...s, target, room: event.room ?? s.room, phase: 'focus' as const };
        return done({
          ...start(begun, now),
          session: { startedAt: now, fromLeftMs: s.leftMs, interruptions: 0, habitTicked: false },
        });
      }
      return done(s.running ? pause(s, now) : start(s, now));
    }

    case 'pulledAway': {
      if (s.phase !== 'focus' || !s.running || !s.session) return done(s);
      const paused = pause(s, now);
      return done({ ...paused, session: { ...s.session, interruptions: s.session.interruptions + 1 } });
    }

    case 'stop': {
      if (s.phase === 'break') return done(idle({ ...s, round: nextRound(s.round), note: null }));
      if (s.phase !== 'focus') return done(s);
      const { minutes, effects } = closeSession(s, now, leftAt(s, now));
      const note = minutes >= 1 ? copy.focus.stopped(formatMinutes(minutes), s.target?.title ?? '') : null;
      return done(idle({ ...s, note }), effects);
    }

    case 'setTarget': {
      if (s.phase !== 'focus') return done({ ...s, target: event.target });
      // Every session belongs to one task: a new current task closes this
      // one and opens the next, and the round runs on.
      if (!event.target || event.target.id === s.target?.id) return done(s);
      const leftNow = leftAt(s, now);
      const { effects } = closeSession(s, now, leftNow);
      return done(
        {
          ...s,
          target: event.target,
          session: { startedAt: now, fromLeftMs: leftNow, interruptions: 0, habitTicked: false },
        },
        effects,
      );
    }

    case 'setLength': {
      if (s.phase === 'focus' || !isFocusLength(event.minutes)) return done(s);
      const next = { ...s, focusMin: event.minutes };
      return done(s.phase === 'idle' ? idle(next) : next);
    }
  }
}

export interface FocusLcd {
  digits: string;
  line: string;
  note: string | null;
  /** The meter drains from 1 to 0. */
  meter: number;
  paused: boolean;
}

/** The olive LCD panel: 32 px digits, one line, a meter. */
export function focusLcd(s: FocusState, now: number): FocusLcd {
  const left = leftAt(s, now);
  const digits = countdown(left);
  const meter = s.lengthMs > 0 ? left / s.lengthMs : 0;
  const focusLine = s.target ? copy.focus.line(s.round, ROUNDS, s.target.title) : copy.widgets.nowEmpty;
  if (s.phase === 'idle') return { digits, line: focusLine, note: s.note, meter: 1, paused: false };
  if (s.phase === 'focus') return { digits, line: focusLine, note: s.note, meter, paused: !s.running };
  const waiting = !s.running && left === s.lengthMs;
  return {
    digits,
    line: waiting ? copy.focus.breakWaits(countdown(s.lengthMs)) : copy.focus.breakLine,
    note: s.note,
    meter,
    paused: !s.running && !waiting,
  };
}

/** The Now strip's focus half: "focus 18:40 left". */
export function focusStrip(s: FocusState, now: number): string | null {
  if (s.phase === 'focus') return copy.focus.strip(countdown(leftAt(s, now)));
  if (s.phase === 'break' && (s.running || leftAt(s, now) < s.lengthMs))
    return copy.focus.breakStrip(countdown(leftAt(s, now)));
  return null;
}

export type CheckOff = { kind: 'done'; task: Task; message: string } | { kind: 'ask'; title: string; hint: string };

/**
 * Checking a task off. With logged time it is done at once and the status
 * bar says what it took; only a task with no logged time still asks.
 */
export function checkOff(task: Task, sessions: readonly FocusSession[], now: number): CheckOff {
  const count = sessions.filter((s) => s.taskId === task.id).length;
  const took = actualMin(task, sessions);
  if (count === 0 && took <= 0) return { kind: 'ask', title: copy.check.title, hint: copy.check.hint };
  return {
    kind: 'done',
    task: { ...task, done: true, doneAt: now },
    message: copy.check.done(formatMinutes(took), count),
  };
}

/** The answer to "Time it took", for a task with no logged time. */
export function finishWithTime(task: Task, minutes: number, now: number): Task {
  return { ...task, adjustMin: minutes, done: true, doneAt: now };
}

/** Get Info's "Took": set the total by hand; focus minutes stay as logged. */
export function setTook(task: Task, sessions: readonly FocusSession[], minutes: number): Task {
  return { ...task, adjustMin: minutes - (actualMin(task, sessions) - task.adjustMin) };
}
