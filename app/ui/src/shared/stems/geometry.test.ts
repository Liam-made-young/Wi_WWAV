import { describe, expect, test } from 'vitest';
import { freshMix } from './mix';
import {
  ARMS,
  FX_DIRECTIONS,
  LIGHT_HIT,
  LIGHT_SIZE,
  NOW_STRIP,
  NOW_STRIP_NARROW_W,
  fxCentre,
  fxReach,
  levelAt,
  lightAt,
  moonCentre,
  stageGeometry,
  stemLights,
  trackHalfX,
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
  for (const stripW of [NOW_STRIP.w, NOW_STRIP_NARROW_W]) {
    const lights = stemLights(stripW);

    test(`sit in the track half of the ${stripW} × 44 pt strip, in file order, each with a 44 × 44 pt hit area`, () => {
      expect(NOW_STRIP).toEqual({ w: 520, h: 44 });
      expect(LIGHT_HIT).toBe(44);
      expect(lights.map((l) => l.stem)).toEqual(['vocals', 'drums', 'other', 'bass']);
      for (const l of lights) {
        expect([l.hit.w, l.hit.h]).toEqual([44, 44]);
        expect(l.hit.x).toBeGreaterThanOrEqual(trackHalfX(stripW));
        expect(l.hit.x + l.hit.w).toBeLessThanOrEqual(stripW);
        expect([l.hit.y, l.hit.y + l.hit.h]).toEqual([0, NOW_STRIP.h]);
        // The 8 pt light sits inside its own square, on line 2.
        expect(l.centre.x - LIGHT_SIZE / 2).toBeGreaterThanOrEqual(l.hit.x);
        expect(l.centre.x + LIGHT_SIZE / 2).toBeLessThanOrEqual(l.hit.x + l.hit.w);
        expect(l.centre.y).toBeGreaterThan(NOW_STRIP.h / 2 - 6);
        expect(l.centre.y + LIGHT_SIZE / 2).toBeLessThanOrEqual(NOW_STRIP.h - 6);
      }
      // Centres a whole hit area apart, so no light's area is cut short.
      for (let i = 1; i < lights.length; i++) expect(lights[i].centre.x - lights[i - 1].centre.x).toBeGreaterThanOrEqual(LIGHT_HIT);
    });

    // The review: lightAt split overlapping squares by the nearer centre, so
    // a strip that drew its lights closer than 44 pt gave each less than 44 × 44.
    test(`every point of each 44 × 44 pt square answers to its own light, and nowhere else does (${stripW} pt)`, () => {
      for (const l of lights) {
        for (let dx = 0; dx < 44; dx += 1) {
          for (let dy = 0; dy < 44; dy += 1) {
            expect(lightAt({ x: l.hit.x + dx + 0.5, y: l.hit.y + dy + 0.5 }, stripW)).toBe(l.stem);
          }
        }
      }
      const first = lights[0].hit;
      const last = lights[3].hit;
      expect(lightAt({ x: first.x - 0.5, y: 22 }, stripW)).toBeNull();
      expect(lightAt({ x: last.x + last.w + 0.5, y: 22 }, stripW)).toBeNull();
      expect(lightAt({ x: first.x + 10, y: -0.5 }, stripW)).toBeNull();
      expect(lightAt({ x: first.x + 10, y: 44.5 }, stripW)).toBeNull();
    });
  }

  test('leave line 2 the rest of the track half for "1:42 / 3:58"', () => {
    const room = (w: number) => w - (stemLights(w)[3].hit.x + LIGHT_HIT);
    expect(room(NOW_STRIP.w)).toBe(83.5);
    expect(room(NOW_STRIP_NARROW_W)).toBe(43.5);
  });
});
