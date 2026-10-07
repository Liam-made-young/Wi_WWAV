// The Pomodoro timer as the snapshot carries it (docs/SPEC.md 3.5): the
// core's state machine moves it; these functions only read it, for the LCD
// panel, the Now widget and the Now strip's "focus 18:42 left".

import { countdown } from '../shared/time/format';
import type { HeatTimer, Task } from './client';
import { copy } from './fmt';

export const ROUNDS = 4;

export interface TimerView {
  phase: HeatTimer['phase'];
  running: boolean;
  round: number;
  /** Time left; for an idle timer, the whole length. */
  leftMs: number;
  lengthMs: number;
  /** "24:59" */
  digits: string;
  /** The meter drains from 1 to 0. */
  meter: number;
  /** A break that waits for a press: "Break 5:00. Press F to start it." */
  waiting: boolean;
  note: string | null;
  focusMin: number;
}

export function timerView(t: HeatTimer, now: number, chosenMin = 25): TimerView {
  const focusMin = t.focusMin ?? chosenMin;
  const running = t.running ?? t.endsAt !== null;
  const lengthMs = t.phase === 'idle' ? chosenMin * 60_000 : (t.lengthMs ?? focusMin * 60_000);
  const leftMs =
    t.phase === 'idle' ? lengthMs : running && t.endsAt !== null ? Math.max(0, t.endsAt - now) : (t.leftMs ?? lengthMs);
  return {
    phase: t.phase,
    running,
    round: t.round,
    leftMs,
    lengthMs,
    digits: countdown(leftMs),
    meter: t.phase === 'idle' ? 1 : lengthMs > 0 ? leftMs / lengthMs : 0,
    waiting: t.phase === 'break' && !running && leftMs === lengthMs,
    note: t.note ?? null,
    focusMin,
  };
}

/** The LCD's one line: "Focus 1 of 4 · Mix the second verse", or what to do when nothing is current. */
export function lcdLine(v: TimerView, target: Task | undefined): string {
  if (v.phase === 'break') return v.waiting ? copy.focus.breakWaits(countdown(v.lengthMs)) : copy.focus.breakLine;
  return target ? copy.focus.line(v.round, ROUNDS, target.title) : copy.widgets.nowEmpty;
}

/** The Now strip's tail: "focus 18:42 left", "break 3:12 left", or null when nothing is under way. */
export function stripText(t: HeatTimer | undefined, now: number): string | null {
  if (!t) return null;
  const v = timerView(t, now);
  if (v.phase === 'focus') return copy.focus.strip(countdown(v.leftMs));
  if (v.phase === 'break' && (v.running || v.leftMs < v.lengthMs)) return copy.focus.breakStrip(countdown(v.leftMs));
  return null;
}
