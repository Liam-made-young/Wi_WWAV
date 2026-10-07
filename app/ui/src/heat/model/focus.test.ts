import { describe, expect, it } from 'vitest';
import { actualMin } from './estimate';
import {
  FOCUS_LENGTHS,
  type FocusEvent,
  type FocusState,
  type FocusTarget,
  checkOff,
  finishWithTime,
  focusLcd,
  focusStep,
  focusStrip,
  initialFocus,
  isFocusLength,
  setTook,
} from './focus';
import type { FocusSession } from './records';
import { ny, session, task } from './testkit';

const MIN = 60_000;
const t0 = ny('2026-10-06 09:00');
const mix: FocusTarget = { kind: 'task', id: 'mix', title: 'Mix the second verse' };

// Runs events in order and gathers every effect.
function run(events: [number, FocusEvent][], state = initialFocus(), chime = false) {
  const effects = [];
  for (const [at, e] of events) {
    const r = focusStep(state, e, at, { chime });
    state = r.state;
    effects.push(...r.effects);
  }
  return { state, effects };
}

const press = (target?: FocusTarget): FocusEvent => ({ type: 'press', target, room: 'heat' });
const tick: FocusEvent = { type: 'tick' };

describe('lengths', () => {
  it('defaults to 25 minutes, offers 50, and takes a custom 10 to 90', () => {
    expect(initialFocus().focusMin).toBe(25);
    expect(FOCUS_LENGTHS).toEqual([25, 50]);
    expect([9, 10, 45, 90, 91, 12.5].map(isFocusLength)).toEqual([false, true, true, true, false, false]);
    expect(run([[t0, { type: 'setLength', minutes: 50 }]]).state.focusMin).toBe(50);
    expect(run([[t0, { type: 'setLength', minutes: 95 }]]).state.focusMin).toBe(25);
  });

  it('keeps a round in progress at its own length', () => {
    const { state } = run([
      [t0, press(mix)],
      [t0 + MIN, { type: 'setLength', minutes: 50 }],
    ]);
    expect(state.focusMin).toBe(25);
  });

  it('gives a 5-minute break, and 15 after every fourth round', () => {
    let state = initialFocus();
    const breaks: number[] = [];
    let at = t0;
    for (let round = 1; round <= 5; round++) {
      state = focusStep(state, press(mix), at).state;
      at += 25 * MIN;
      state = focusStep(state, tick, at).state;
      breaks.push(state.leftMs / MIN);
      state = focusStep(state, press(), at).state;
      at += state.leftMs;
      state = focusStep(state, tick, at).state;
    }
    expect(breaks).toEqual([5, 5, 5, 15, 5]);
    expect(state.round).toBe(2);
  });
});

describe('a round, as 1.6 runs it', () => {
  it('starts on F, reads 24:59 a second in, and logs 25m to the task when it ends', () => {
    let s = focusStep(initialFocus(), press(mix), t0).state;
    expect(focusLcd(s, t0 + 1000)).toEqual({
      digits: '24:59',
      line: 'Focus 1 of 4 · Mix the second verse',
      note: null,
      meter: (24 * MIN + 59_000) / (25 * MIN),
      paused: false,
    });
    const end = focusStep(s, tick, t0 + 25 * MIN);
    s = end.state;
    expect(end.effects).toEqual([
      {
        kind: 'log',
        session: { taskId: 'mix', startedAt: t0, endedAt: t0 + 25 * MIN, focusMin: 25, interruptions: 0, room: 'heat' },
      },
    ]);
    expect(focusLcd(s, t0 + 25 * MIN)).toEqual({
      digits: '5:00',
      line: 'Break 5:00. Press F to start it.',
      note: 'Focus done. 25m logged to Mix the second verse.',
      meter: 1,
      paused: false,
    });
  });

  it('logs to the end of the round even when the tick comes late', () => {
    const s = focusStep(initialFocus(), press(mix), t0).state;
    const { effects } = focusStep(s, tick, t0 + 40 * MIN);
    expect(effects[0]).toMatchObject({ kind: 'log', session: { endedAt: t0 + 25 * MIN, focusMin: 25 } });
  });

  it('pauses and resumes on F, and the pause is not focus time', () => {
    const { state, effects } = run([
      [t0, press(mix)],
      [t0 + 10 * MIN, press()],
      [t0 + 40 * MIN, press()],
      [t0 + 54 * MIN, tick],
      [t0 + 55 * MIN, tick],
    ]);
    expect(effects).toHaveLength(1);
    expect(effects[0]).toMatchObject({ session: { focusMin: 25, endedAt: t0 + 55 * MIN } });
    expect(state.phase).toBe('break');
  });

  it('stops and logs on ⇧F, rounding to the minute', () => {
    const { state, effects } = run([
      [t0, press(mix)],
      [t0 + 18 * MIN + 40_000, { type: 'stop' }],
    ]);
    expect(effects).toEqual([
      {
        kind: 'log',
        session: { taskId: 'mix', startedAt: t0, endedAt: t0 + 18 * MIN + 40_000, focusMin: 19, interruptions: 0, room: 'heat' },
      },
    ]);
    expect(state.phase).toBe('idle');
    expect(state.round).toBe(1);
    expect(focusLcd(state, t0 + 19 * MIN).note).toBe('Focus stopped. 19m logged to Mix the second verse.');
  });

  it('logs nothing for a stop under half a minute', () => {
    expect(run([[t0, press(mix)], [t0 + 20_000, { type: 'stop' }]]).effects).toEqual([]);
  });

  it('marks "Pulled away" with I: it pauses and records an interruption', () => {
    const { state, effects } = run([
      [t0, press(mix)],
      [t0 + 5 * MIN, { type: 'pulledAway' }],
      [t0 + 6 * MIN, { type: 'pulledAway' }],
      [t0 + 9 * MIN, press()],
      [t0 + 10 * MIN, { type: 'pulledAway' }],
      [t0 + 12 * MIN, press()],
      [t0 + 32 * MIN, tick],
    ]);
    expect(state.phase).toBe('break');
    expect(effects[0]).toMatchObject({ session: { interruptions: 2, focusMin: 25 } });
  });
});

describe('nothing starts without a press', () => {
  it('waits after a round, and after a break', () => {
    let s = run([
      [t0, press(mix)],
      [t0 + 25 * MIN, tick],
    ]).state;
    for (let m = 26; m < 200; m++) s = focusStep(s, tick, t0 + m * MIN).state;
    expect(s).toMatchObject({ phase: 'break', running: false });
    s = focusStep(s, press(), t0 + 200 * MIN).state;
    s = focusStep(s, tick, t0 + 205 * MIN).state;
    expect(s).toMatchObject({ phase: 'idle', running: false, round: 2 });
    expect(focusLcd(s, t0 + 205 * MIN).note).toBe('Break done. Press F to start focus 2 of 4.');
    for (let m = 206; m < 400; m++) s = focusStep(s, tick, t0 + m * MIN).state;
    expect(s).toMatchObject({ phase: 'idle', running: false });
  });

  it('never needs a target to stay still, and F with no current task does nothing', () => {
    expect(focusStep(initialFocus(), press(), t0).state).toEqual(initialFocus());
  });

  it('holds over thousands of random events: only F ever sets the timer running', () => {
    let seed = 7;
    const random = () => {
      seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648;
      return seed / 2_147_483_648;
    };
    const events: FocusEvent[] = [
      press(mix),
      press(),
      tick,
      tick,
      tick,
      { type: 'stop' },
      { type: 'pulledAway' },
      { type: 'setLength', minutes: 10 },
      { type: 'setTarget', target: { kind: 'habit', id: 'kanji', title: 'Practise kanji', minutes: 20 } },
      { type: 'setTarget', target: mix },
    ];
    let state: FocusState = initialFocus();
    let at = t0;
    let starts = 0;
    for (let i = 0; i < 5000; i++) {
      at += Math.floor(random() * 40 * MIN);
      const e = events[Math.floor(random() * events.length)];
      const next = focusStep(state, e, at).state;
      if (!state.running && next.running) {
        expect(e.type).toBe('press');
        starts += 1;
      }
      state = next;
    }
    expect(starts).toBeGreaterThan(100);
  });
});

describe('minutes go to the current task', () => {
  it('adds a finished round to the task’s actualMin', () => {
    const t = task({ id: 'mix', adjustMin: 5 });
    const logs: FocusSession[] = [];
    const { effects } = run([
      [t0, press(mix)],
      [t0 + 25 * MIN, tick],
    ]);
    for (const e of effects) if (e.kind === 'log') logs.push({ id: 's', ...e.session });
    expect(actualMin(t, logs)).toBe(30);
  });

  it('splits a round when the current task changes, so each task gets its own minutes', () => {
    const quiz: FocusTarget = { kind: 'task', id: 'quiz', title: 'Grammar quiz 4' };
    const { effects, state } = run([
      [t0, press(mix)],
      [t0 + 10 * MIN, { type: 'setTarget', target: quiz }],
      [t0 + 25 * MIN, tick],
    ]);
    expect(effects).toEqual([
      { kind: 'log', session: { taskId: 'mix', startedAt: t0, endedAt: t0 + 10 * MIN, focusMin: 10, interruptions: 0, room: 'heat' } },
      { kind: 'log', session: { taskId: 'quiz', startedAt: t0 + 10 * MIN, endedAt: t0 + 25 * MIN, focusMin: 15, interruptions: 0, room: 'heat' } },
    ]);
    expect(focusLcd(state, t0 + 25 * MIN).note).toBe('Focus done. 15m logged to Grammar quiz 4.');
  });

  it('says only "Focus done." when the round’s last session was too short to log', () => {
    const quiz: FocusTarget = { kind: 'task', id: 'quiz', title: 'Grammar quiz 4' };
    const { effects, state } = run([
      [t0, press(mix)],
      [t0 + 24 * MIN + 50_000, { type: 'setTarget', target: quiz }],
      [t0 + 25 * MIN, tick],
    ]);
    expect(effects).toEqual([
      { kind: 'log', session: { taskId: 'mix', startedAt: t0, endedAt: t0 + 24 * MIN + 50_000, focusMin: 25, interruptions: 0, room: 'heat' } },
    ]);
    expect(focusLcd(state, t0 + 25 * MIN).note).toBe('Focus done.');
  });

  it('logs a habit’s minutes to the habit, and ticks it once the session reaches its length', () => {
    const kanji: FocusTarget = { kind: 'habit', id: 'kanji', title: 'Practise kanji', minutes: 20 };
    const r1 = run([
      [t0, press(kanji)],
      [t0 + 19 * MIN, tick],
    ]);
    expect(r1.effects).toEqual([]);
    const r2 = focusStep(r1.state, tick, t0 + 20 * MIN);
    expect(r2.effects).toEqual([{ kind: 'tickHabit', habitId: 'kanji' }]);
    const r3 = focusStep(r2.state, tick, t0 + 25 * MIN);
    expect(r3.effects).toEqual([
      { kind: 'log', session: { habitId: 'kanji', startedAt: t0, endedAt: t0 + 25 * MIN, focusMin: 25, interruptions: 0, room: 'heat' } },
    ]);
  });
});

describe('the chime', () => {
  it('is off by default, and never sounds while off', () => {
    const { effects } = run([
      [t0, press(mix)],
      [t0 + 25 * MIN, tick],
      [t0 + 26 * MIN, press()],
      [t0 + 31 * MIN, tick],
    ]);
    expect(effects.filter((e) => e.kind === 'chime')).toEqual([]);
  });

  it('sounds once when a focus round ends if switched on, and never to end a break', () => {
    const { effects } = run(
      [
        [t0, press(mix)],
        [t0 + 25 * MIN, tick],
        [t0 + 26 * MIN, press()],
        [t0 + 31 * MIN, tick],
      ],
      initialFocus(),
      true,
    );
    expect(effects.map((e) => e.kind)).toEqual(['log', 'chime']);
  });
});

describe('the Now strip', () => {
  it('reads "focus 18:40 left" during focus, paused or not', () => {
    const s = focusStep(initialFocus(), press(mix), t0).state;
    expect(focusStrip(s, t0 + 6 * MIN + 20_000)).toBe('focus 18:40 left');
    const paused = focusStep(s, press(), t0 + 6 * MIN + 20_000).state;
    expect(focusStrip(paused, t0 + 60 * MIN)).toBe('focus 18:40 left');
    expect(focusStrip(initialFocus(), t0)).toBeNull();
  });
});

describe('checking a task off (3.5)', () => {
  const now = ny('2026-10-06 15:00');

  it('completes at once when it has logged time, with "Done. Took 1h 15m across 3 focus sessions."', () => {
    const t = task();
    const sessions = [25, 25, 25].map((focusMin) => session({ taskId: t.id, focusMin }));
    expect(checkOff(t, sessions, now)).toEqual({
      kind: 'done',
      task: { ...t, done: true, doneAt: now },
      message: 'Done. Took 1h 15m across 3 focus sessions.',
    });
    expect(checkOff(t, sessions.slice(0, 1), now)).toMatchObject({ message: 'Done. Took 25m across 1 focus session.' });
  });

  it('asks "Time it took" only when nothing is logged', () => {
    expect(checkOff(task(), [], now)).toEqual({
      kind: 'ask',
      title: 'Time it took',
      hint: 'This trains your time averages for this type of task.',
    });
  });

  it('takes the answer as the time, and Get Info’s "Took" adjusts it by hand', () => {
    const t = task();
    const done = finishWithTime(t, 40, now);
    expect(done).toEqual({ ...t, adjustMin: 40, done: true, doneAt: now });
    const sessions = [session({ taskId: t.id, focusMin: 50 })];
    const took = setTook(t, sessions, 75);
    expect(took.adjustMin).toBe(25);
    expect(actualMin(took, sessions)).toBe(75);
  });
});
