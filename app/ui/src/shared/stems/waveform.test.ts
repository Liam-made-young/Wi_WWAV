import { describe, expect, test } from 'vitest';
import { freshMix } from './mix';
import { WAVE_BUCKETS, computePeaks, fullMixNormaliser, waveform } from './waveform';

// Four stems with recognisable shapes: each loud in its own quarter.
function peaks(): Record<'vocals' | 'drums' | 'other' | 'bass', Float32Array> {
  const make = (q: number) => Float32Array.from({ length: WAVE_BUCKETS }, (_, b) => (Math.floor(b / 100) === q ? 1 : 0.25));
  return { vocals: make(0), drums: make(1), other: make(2), bass: make(3) };
}

describe('the waveform', () => {
  test('has 400 buckets', () => {
    const p = peaks();
    expect(waveform(freshMix(), p, fullMixNormaliser(p))).toHaveLength(400);
  });

  test("is drawn from the audible stems only: mute the drums and the drum shape vanishes", () => {
    const p = peaks();
    const n = fullMixNormaliser(p);
    const mix = freshMix();
    mix.drums.muted = true;
    const w = waveform(mix, p, n);
    // Bucket 150 is in the drums' loud quarter; with them muted it reads
    // like any quiet bucket of the other three.
    expect(w[150]).toBeCloseTo(w[350] - (0.8 * 0.75) / n, 6);
    expect(w[150]).toBeCloseTo((3 * 0.8 * 0.25) / n, 6);
  });

  test('keeps its normaliser fixed to the full mix, so a mute rescales nothing else', () => {
    const p = peaks();
    const n = fullMixNormaliser(p);
    expect(n).toBeCloseTo(0.8 * (1 + 3 * 0.25), 6);
    const all = waveform(freshMix(), p, n);
    const mix = freshMix();
    mix.drums.muted = true;
    const without = waveform(mix, p, n);
    expect(without[50]).toBeCloseTo(all[50] - (0.8 * 0.25) / n, 6);
  });

  test('under a solo, only the soloed stem remains', () => {
    const p = peaks();
    const n = fullMixNormaliser(p);
    const mix = freshMix();
    mix.bass.soloed = true;
    const w = waveform(mix, p, n);
    expect(w[350]).toBeCloseTo(0.8 / n, 6);
    expect(w[50]).toBeCloseTo((0.8 * 0.25) / n, 6);
  });

  test('weights each stem by its level and never passes 1', () => {
    const p = peaks();
    const n = fullMixNormaliser(p);
    const mix = freshMix();
    for (const s of ['vocals', 'drums', 'other', 'bass'] as const) mix[s].level = 1;
    expect(Math.max(...waveform(mix, p, n))).toBeLessThanOrEqual(1);
  });
});

describe('peaks', () => {
  test('are the loudest sample per bucket across up to two channels, scaled to the stem’s own peak', () => {
    const left = Float32Array.from({ length: 800 }, (_, i) => (i === 10 ? -0.5 : i === 700 ? 0.25 : 0));
    const right = Float32Array.from({ length: 800 }, (_, i) => (i === 11 ? 0.1 : 0));
    const ignored = Float32Array.from({ length: 800 }, () => 0.9);
    const p = computePeaks([left, right, ignored], 400);
    expect(p).toHaveLength(400);
    expect(p[5]).toBe(1);
    expect(p[350]).toBe(0.5);
    expect(p[0]).toBe(0);
  });

  test('a silent stem stays flat rather than dividing by zero', () => {
    expect([...computePeaks([new Float32Array(400)], 400)].every((v) => v === 0)).toBe(true);
  });
});
