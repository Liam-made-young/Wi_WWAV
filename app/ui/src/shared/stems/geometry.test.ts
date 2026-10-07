import { describe, expect, test } from 'vitest';
import { freshMix } from './mix';
import {
  ARMS,
  FX_DIRECTIONS,
  LIGHT_HIT,
  fxCentre,
  fxReach,
  levelAt,
  lightAt,
  moonCentre,
  stageGeometry,
} from './geometry';

describe('the planet player on a 560 pt stage', () => {
  const g = stageGeometry(560, 560);

  test('has the shared proportions: dim 515, planet 88, moons 23 on arms 139→242', () => {
    expect(g.dim).toBeCloseTo(515.2, 9);
    expect(g.planetR).toBeCloseTo(87.584, 9);
    expect(g.moonR).toBeCloseTo(23.184, 9);
    expect(g.rMin).toBeCloseTo(139.104, 9);
    expect(g.rMax).toBeCloseTo(242.144, 9);
    expect(g.span).toBeCloseTo(103.04, 9);
    expect(g.centre).toEqual({ x: 280, y: 280 });
  });

  test('fits the smaller side of a wider stage', () => {
    expect(stageGeometry(900, 560).dim).toBeCloseTo(515.2, 9);
    expect(stageGeometry(900, 560).centre).toEqual({ x: 450, y: 280 });
  });

  test('puts vocals up, drums down, bass left and other right', () => {
    expect(ARMS).toEqual({ vocals: { x: 0, y: -1 }, drums: { x: 0, y: 1 }, bass: { x: -1, y: 0 }, other: { x: 1, y: 0 } });
    expect(moonCentre(g, 'vocals', 0)).toEqual({ x: 280, y: 280 - g.rMin });
    expect(moonCentre(g, 'other', 1)).toEqual({ x: expect.closeTo(280 + g.rMax, 9), y: 280 });
  });

  test("a moon's distance from the planet is its volume, and nothing else is", () => {
    for (const level of [0, 0.25, 0.7, 1]) {
      for (const stem of ['vocals', 'drums', 'other', 'bass'] as const) {
        const c = moonCentre(g, stem, level);
        const distance = Math.sqrt((c.x - 280) ** 2 + (c.y - 280) ** 2);
        expect(distance).toBeCloseTo(g.rMin + level * g.span, 9);
        expect(levelAt(g, distance)).toBeCloseTo(level, 12);
      }
    }
    expect(levelAt(g, 0)).toBe(0);
    expect(levelAt(g, 9999)).toBe(1);
  });
});

describe('the FX moons', () => {
  const g = stageGeometry(560, 560);

  test('bloom on the diagonals: reverb upper left, delay upper right, distortion lower left, tremolo lower right', () => {
    const d = 1 / Math.sqrt(2);
    expect(FX_DIRECTIONS.reverb.x).toBeCloseTo(-d, 12);
    expect(FX_DIRECTIONS.reverb.y).toBeCloseTo(-d, 12);
    expect(FX_DIRECTIONS.delay).toMatchObject({ x: expect.closeTo(d, 12), y: expect.closeTo(-d, 12) });
    expect(FX_DIRECTIONS.distortion).toMatchObject({ x: expect.closeTo(-d, 12), y: expect.closeTo(d, 12) });
    expect(FX_DIRECTIONS.tremolo).toMatchObject({ x: expect.closeTo(d, 12), y: expect.closeTo(d, 12) });
  });

  test("a stem's sit round its moon and the master's round the planet; distance is dry/wet", () => {
    const mix = freshMix();
    const stem = fxReach(g, 'vocals', mix);
    expect(stem.fxR).toBeCloseTo(g.moonR * 0.46, 12);
    expect(stem.parent).toEqual(moonCentre(g, 'vocals', mix.vocals.level));
    expect(stem.rMin).toBeCloseTo(g.moonR + stem.fxR * 2.1, 12);
    expect(stem.rMax).toBeCloseTo(g.moonR + stem.fxR * 6.2, 12);
    const master = fxReach(g, 'master', mix);
    expect(master.fxR).toBeCloseTo(g.moonR * 0.62, 12);
    expect(master.parent).toEqual(g.centre);
    expect(master.rMin).toBeCloseTo(g.planetR + master.fxR * 2.1, 12);
    mix.vocals.fx.delay = 0.5;
    const c = fxCentre(g, 'vocals', 'delay', mix);
    const dist = Math.sqrt((c.x - stem.parent.x) ** 2 + (c.y - stem.parent.y) ** 2);
    expect(dist).toBeCloseTo((stem.rMin + stem.rMax) / 2, 9);
  });
});

describe('the stem lights', () => {
  test('each answer to a 44 × 44 pt square round the 8 pt light', () => {
    expect(LIGHT_HIT).toBe(44);
    const centres = { vocals: { x: 22, y: 22 }, drums: { x: 66, y: 22 }, other: { x: 110, y: 22 }, bass: { x: 154, y: 22 } };
    expect(lightAt(centres, { x: 0.5, y: 0.5 })).toBe('vocals');
    expect(lightAt(centres, { x: 43.5, y: 43.5 })).toBe('vocals');
    expect(lightAt(centres, { x: 44.5, y: 30 })).toBe('drums');
    expect(lightAt(centres, { x: 175.5, y: 1 })).toBe('bass');
    expect(lightAt(centres, { x: 176.5, y: 22 })).toBeNull();
    expect(lightAt(centres, { x: 22, y: 44.5 })).toBeNull();
  });
});
