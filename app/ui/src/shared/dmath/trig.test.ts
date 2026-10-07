import { describe, expect, test } from 'vitest';
import { atan2, cos, sin, sqrt } from './trig';
import { TRIG_FINGERPRINT, trigFingerprint } from './fingerprint';

// Distance in units in the last place between two doubles of the same sign.
function ulps(a: number, b: number): number {
  if (Object.is(a, b)) return 0;
  const view = new DataView(new ArrayBuffer(16));
  view.setFloat64(0, a);
  view.setFloat64(8, b);
  const d = view.getBigInt64(0) - view.getBigInt64(8);
  return Number(d < 0n ? -d : d);
}

// A deterministic sweep: no Math.random, so a failure names its input.
function sweep(from: number, to: number, n: number): number[] {
  const xs: number[] = [];
  for (let i = 0; i <= n; i++) xs.push(from + ((to - from) * i) / n);
  return xs;
}

describe('sin and cos', () => {
  test('give the exact values at the exact points', () => {
    expect(sin(0)).toBe(0);
    expect(Object.is(sin(-0), -0)).toBe(true);
    expect(cos(0)).toBe(1);
    expect(cos(-0)).toBe(1);
    expect(sin(Math.PI / 2)).toBe(1);
    expect(cos(Math.PI)).toBe(-1);
  });

  test('stay within one ulp of the engine across the range the sky uses', () => {
    const xs = [
      ...sweep(-7, 7, 20011),
      ...sweep(-2000, 2000, 20011),
      ...sweep(-1e5, 1e5, 4001),
      ...sweep(-1e-6, 1e-6, 101),
    ];
    let worst = 0;
    for (const x of xs) {
      worst = Math.max(worst, ulps(sin(x), Math.sin(x)), ulps(cos(x), Math.cos(x)));
    }
    expect(worst).toBeLessThanOrEqual(1);
  });

  test('keep sin² + cos² at 1', () => {
    for (const x of sweep(-50, 50, 997)) {
      expect(Math.abs(sin(x) * sin(x) + cos(x) * cos(x) - 1)).toBeLessThan(4e-16);
    }
  });

  test('pass NaN and infinities through as NaN', () => {
    expect(sin(NaN)).toBeNaN();
    expect(cos(Infinity)).toBeNaN();
    expect(sin(-Infinity)).toBeNaN();
  });
});

describe('atan2', () => {
  test('gives the exact quadrant values', () => {
    expect(atan2(0, 1)).toBe(0);
    expect(Object.is(atan2(-0, 1), -0)).toBe(true);
    expect(atan2(0, -1)).toBe(Math.PI);
    expect(atan2(-0, -1)).toBe(-Math.PI);
    expect(atan2(1, 0)).toBe(Math.PI / 2);
    expect(atan2(-1, 0)).toBe(-Math.PI / 2);
    expect(atan2(1, 1)).toBe(Math.PI / 4);
    expect(atan2(Infinity, -Infinity)).toBe((3 * Math.PI) / 4);
    expect(atan2(NaN, 1)).toBeNaN();
  });

  test('stays within one ulp of the engine all the way round', () => {
    let worst = 0;
    for (const a of sweep(-Math.PI, Math.PI, 7919)) {
      for (const r of [1e-3, 1, 640, 1580, 1e6]) {
        const y = r * Math.sin(a);
        const x = r * Math.cos(a);
        worst = Math.max(worst, ulps(atan2(y, x), Math.atan2(y, x)));
      }
    }
    expect(worst).toBeLessThanOrEqual(1);
  });
});

test('sqrt is the correctly rounded one', () => {
  expect(sqrt(2)).toBe(Math.SQRT2);
  expect(sqrt(640)).toBe(Math.sqrt(640));
});

// The bits of a fixed grid of results, hashed. Every engine that runs the
// app must print this same value: the browser check (e2e/space-layout)
// asserts it in Chromium, and the WebKit pass asserts it in WebKit.
test('the trig fingerprint is pinned', () => {
  expect(trigFingerprint()).toBe(TRIG_FINGERPRINT);
});
