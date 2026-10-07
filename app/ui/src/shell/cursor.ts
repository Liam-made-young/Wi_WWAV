// Where the playhead is between the engine's clock events (docs/SPEC.md
// 2.2, 9.2). The engine is the master clock: ten times a second it says
// where the playhead stands and how fast it travels, and between those
// anchors the strip moves the cursor on at that speed. It never counts on
// its own, so a cursor can't drift from what is heard, and a stopped
// clock leaves it still.

export interface Clock {
  sample: number;
  hostTimeNs: number;
  /** Samples per second of playhead travel; 0 when stopped. */
  rate: number;
  state: 'playing' | 'recording' | 'stopped';
}

/** A known position, `at` a local time in ms (performance.now()), moving at `speed` seconds per second. */
export interface Anchor {
  seconds: number;
  at: number;
  speed: number;
}

/**
 * `sampleRate` is the engine's device rate (engine.status), or 0 before it
 * is known, when a playing clock's own travel rate stands in for it.
 */
export function anchorFromClock(clock: Clock, sampleRate: number, at: number): Anchor {
  const moving = clock.state !== 'stopped' && clock.rate > 0;
  const perSecond = sampleRate > 0 ? sampleRate : clock.rate;
  if (perSecond <= 0) return { seconds: 0, at, speed: 0 };
  return { seconds: clock.sample / perSecond, at, speed: moving ? clock.rate / perSecond : 0 };
}

export function anchorFromState(state: { position: number; playing: boolean }, at: number): Anchor {
  return { seconds: state.position, at, speed: state.playing ? 1 : 0 };
}

export function cursorAt(a: Anchor, now: number, duration: number): number {
  const s = a.seconds + (Math.max(0, now - a.at) / 1000) * a.speed;
  return Math.min(Math.max(s, 0), duration);
}
