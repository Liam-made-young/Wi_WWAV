import { describe, expect, it } from 'vitest';
import type { HeatTimer, Task } from './client';
import { lcdLine, stripText, timerView } from './timer';

// docs/SPEC.md 3.5, 2.2. What a fail looks like: an LCD that reads anything
// but "24:59"; a meter that doesn't drain; "Break 5:00. Press F to start it."
// missing while a break waits; a strip that says "focus 18:42 left" while
// nothing runs, or says nothing while a round does.

const idle: HeatTimer = { phase: 'idle', round: 1, endsAt: null };
const running = (endsAt: number): HeatTimer => ({
  phase: 'focus',
  round: 1,
  endsAt,
  running: true,
  lengthMs: 25 * 60_000,
  focusMin: 25,
});
const task = { id: 't', title: 'Mix the second verse' } as Task;

describe('the timer as the snapshot carries it', () => {
  it('reads an idle timer as the length chosen, full', () => {
    expect(timerView(idle, 0, 25)).toMatchObject({
      phase: 'idle',
      digits: '25:00',
      meter: 1,
      running: false,
      waiting: false,
    });
    expect(timerView(idle, 0, 50).digits).toBe('50:00');
  });

  it('counts a running round down from its end, rounding up so 0:00 shows only at the end', () => {
    const t = running(25 * 60_000);
    expect(timerView(t, 0).digits).toBe('25:00');
    expect(timerView(t, 1).digits).toBe('25:00');
    expect(timerView(t, 61_000).digits).toBe('23:59');
    expect(timerView(t, 25 * 60_000 - 500).digits).toBe('0:01');
    expect(timerView(t, 25 * 60_000).digits).toBe('0:00');
    expect(timerView(t, 99 * 60_000).digits).toBe('0:00');
    expect(timerView(t, 12.5 * 60_000).meter).toBeCloseTo(0.5);
  });

  it('holds a paused round where it was', () => {
    const paused: HeatTimer = {
      phase: 'focus',
      round: 2,
      endsAt: null,
      running: false,
      leftMs: 10 * 60_000,
      lengthMs: 25 * 60_000,
    };
    expect(timerView(paused, 1e12)).toMatchObject({ digits: '10:00', running: false, round: 2 });
  });

  it('says a break waits for a press while it has run no time', () => {
    const waiting: HeatTimer = {
      phase: 'break',
      round: 1,
      endsAt: null,
      running: false,
      leftMs: 5 * 60_000,
      lengthMs: 5 * 60_000,
    };
    expect(timerView(waiting, 0).waiting).toBe(true);
    expect(lcdLine(timerView(waiting, 0), task)).toBe('Break 5:00. Press F to start it.');
    const started: HeatTimer = { ...waiting, endsAt: 300_000, running: true };
    expect(timerView(started, 0).waiting).toBe(false);
    expect(lcdLine(timerView(started, 0), task)).toBe('Break');
  });

  it('writes the LCD line, or says what to do when nothing is current', () => {
    expect(lcdLine(timerView(running(1e9), 0), task)).toBe('Focus 1 of 4 · Mix the second verse');
    expect(lcdLine(timerView(idle, 0), undefined)).toBe(
      'Nothing is current. Pick a task and press C, or drag one here.',
    );
  });

  it('gives the strip "focus 18:42 left" while a round is under way, and nothing otherwise', () => {
    expect(stripText(running(18 * 60_000 + 42_000), 0)).toBe('focus 18:42 left');
    expect(stripText({ ...running(0), running: false, endsAt: null, leftMs: 60_000 }, 0)).toBe('focus 1:00 left');
    expect(stripText(idle, 0)).toBeNull();
    expect(stripText(undefined, 0)).toBeNull();
    const waiting: HeatTimer = {
      phase: 'break',
      round: 1,
      endsAt: null,
      running: false,
      leftMs: 300_000,
      lengthMs: 300_000,
    };
    expect(stripText(waiting, 0)).toBeNull();
    expect(stripText({ ...waiting, endsAt: 192_000, running: true }, 0)).toBe('break 3:12 left');
  });
});
