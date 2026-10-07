import { describe, expect, it } from 'vitest';
import { anchorFromClock, anchorFromState, cursorAt } from './cursor';

// 2.2 and 9.2: the engine is the master clock; the strip extrapolates the
// cursor between the clock's anchors. What a fail looks like: the cursor
// standing still between anchors while playing, moving while stopped,
// running past the song's end, or jumping by a sample-rate mismatch.

describe('the cursor between clock anchors', () => {
  it('reads an anchor in seconds at the engine’s rate and moves at playing speed', () => {
    const a = anchorFromClock({ sample: 96_000, hostTimeNs: 0, rate: 48_000, state: 'playing' }, 48_000, 1_000);
    expect(a).toEqual({ seconds: 2, at: 1_000, speed: 1 });
    expect(cursorAt(a, 1_250, 60)).toBeCloseTo(2.25, 6);
  });

  it('stands still while stopped', () => {
    const a = anchorFromClock({ sample: 48_000, hostTimeNs: 0, rate: 0, state: 'stopped' }, 48_000, 0);
    expect(cursorAt(a, 5_000, 60)).toBe(1);
  });

  it('never runs past the end or before the start', () => {
    const a = { seconds: 59.9, at: 0, speed: 1 };
    expect(cursorAt(a, 1_000, 60)).toBe(60);
    expect(cursorAt({ seconds: -0.2, at: 0, speed: 0 }, 0, 60)).toBe(0);
  });

  it('takes the clock’s own travel rate when the device rate is not known yet', () => {
    const a = anchorFromClock({ sample: 44_100, hostTimeNs: 0, rate: 44_100, state: 'playing' }, 0, 0);
    expect(a).toEqual({ seconds: 1, at: 0, speed: 1 });
  });

  it('anchors on a player state too', () => {
    expect(anchorFromState({ position: 12.5, playing: true }, 40)).toEqual({ seconds: 12.5, at: 40, speed: 1 });
    expect(anchorFromState({ position: 12.5, playing: false }, 40)).toEqual({ seconds: 12.5, at: 40, speed: 0 });
  });
});
