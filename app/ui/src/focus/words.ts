// What the readout says (docs/FOCUS.md): the Now task and when it is due,
// the next commitment, and what is playing; or ALL CLEAR. Words only; the
// dots are focus/Readout.tsx's.

import { clockAt, WEEKDAYS } from '../shared/time/format';
import { dayKey, daysBetween, weekdayOf } from '../shared/time/zone';
import type { NextCommitment, Now } from './model';

export interface ReadoutInput {
  now: Now | null;
  /** The Now task's title and its due phrase ("Tomorrow 11:59 PM"), when Focus shows a task. */
  task: { title: string; due: string | null } | null;
  next: NextCommitment | null;
  /** The commitments' own line for what is next ("NEXT JPN 101 10:00 · LEAVE 9:35"), when the snapshot carries one. */
  nextLine?: string | null;
  /** What is playing, or null. */
  playing: string | null;
  /** The clock and the day, for "NEXT … 10:00 AM". */
  at: number;
  tz: string;
}

export interface ReadoutText {
  /** The main line, from the left. */
  left: string;
  /** What stands at the right, most wanted first: the readout drops from the end when it runs out of room. */
  right: string[];
  /** The whole of it in a sentence, for VoiceOver. */
  label: string;
}

const TITLE_MAX = 22;
/** The readout has few cells: a weekday is its first three letters there. */
const shortDays = (s: string) => WEEKDAYS.reduce((out, day) => out.replace(day, day.slice(0, 3)), s);
const clip = (s: string, n: number) => (s.length > n ? `${s.slice(0, n - 1).trimEnd()}…` : s);

/** "10:00 AM" today, "TOMORROW 10:00 AM", "FRI 10:00 AM". */
function when(start: number, at: number, tz: string): string {
  const time = clockAt(start, tz);
  const day = dayKey(start, tz);
  const ahead = daysBetween(dayKey(at, tz), day);
  if (ahead <= 0) return time;
  if (ahead === 1) return `Tomorrow ${time}`;
  return `${WEEKDAYS[weekdayOf(day)].slice(0, 3)} ${time}`;
}

export function readoutText(input: ReadoutInput): ReadoutText {
  const { now, task, next, playing } = input;
  const nextText = input.nextLine
    ? input.nextLine.replace(/^next\b/i, 'Next')
    : next
      ? `Next ${clip(next.title, TITLE_MAX)} ${when(next.start, input.at, input.tz)}`
      : null;
  const playingText = playing ? `♪ ${clip(playing, TITLE_MAX)}` : null;
  const up = (s: string) => s.toUpperCase();

  if (!now || now.kind === 'clear') {
    const left = nextText ? `All clear · ${nextText}` : 'All clear';
    return {
      left: up(left),
      right: playingText ? [up(playingText)] : [],
      label: [nextText ? `All clear. ${nextText}.` : 'All clear.', playing && `Playing ${playing}.`].filter(Boolean).join(' '),
    };
  }
  const main =
    now.kind === 'fix'
      ? now.fix.line.replace(/\.$/, '')
      : task
        ? [task.title, task.due && shortDays(task.due)].filter(Boolean).join(' · ')
        : '';
  return {
    left: up(main),
    right: [nextText, playingText].filter((s): s is string => s !== null).map(up),
    label: [
      now.kind === 'fix' ? now.fix.line : task ? `Now: ${task.title}${task.due ? `, ${task.due}` : ''}.` : '',
      nextText && `${nextText}.`,
      playing && `Playing ${playing}.`,
    ]
      .filter(Boolean)
      .join(' '),
  };
}

/** The cells of one row of the readout: `left` from the left, as much of `right` as fits from the right. */
export function layoutCells(text: ReadoutText, cells: number): string {
  if (cells <= 0) return '';
  const gap = 3;
  const kept: string[] = [];
  // The main line keeps three fifths of the row, or all it needs if that is less.
  const floor = Math.min(text.left.length, Math.ceil(cells * 0.6));
  let used = 0;
  for (const part of text.right) {
    const more = part.length + (kept.length ? gap : 0);
    if (floor + gap + used + more > cells) break;
    kept.push(part);
    used += more;
  }
  const right = kept.join(' '.repeat(gap));
  const room = cells - (right ? right.length + gap : 0);
  const left = text.left.length > room ? `${text.left.slice(0, Math.max(0, room - 1))}…` : text.left;
  return right ? left.padEnd(cells - right.length, ' ') + right : left.padEnd(cells, ' ');
}
