import { describe, expect, test } from 'vitest';
import { sin } from '../../shared/dmath';
import { CENTER, ringAngle, ringRadius } from './orbits';
import {
  SEPARATION_RATE,
  eccentricAnomaly,
  orbitForSeat,
  periodOf,
  placeOrbit,
  separationOffsets,
} from './kepler';

describe('the elements', () => {
  test('are the same every time for one identity', () => {
    const a = orbitForSeat({ seat: 2, count: 7, id: 'track_abc', seed: 'album-100' });
    const b = orbitForSeat({ seat: 2, count: 7, id: 'track_abc', seed: 'album-100' });
    expect(b).toEqual(a);
  });

  test('come from the seat and the system seed, eccentricity and tilt from the id', () => {
    const o = orbitForSeat({ seat: 9, count: 12, id: 'track_abc', seed: 'album-100' });
    expect(o.a).toBe(ringRadius(9, 'album-100'));
    expect(o.phase).toBe(ringAngle(9, 12, 'album-100'));
    const other = orbitForSeat({ seat: 9, count: 12, id: 'track_xyz', seed: 'album-100' });
    expect(other.a).toBe(o.a);
    expect(other.e).not.toBe(o.e);
  });

  // iOS v5's ranges (KeplerMotion.swift), gentler than the hub's 0.04–0.15.
  test('keep eccentricity in 0.025–0.08 and tilt in 0.05–0.12 rad', () => {
    for (let i = 0; i < 200; i++) {
      const o = orbitForSeat({ seat: i % 21, count: 21, id: `track_${i}`, seed: 's' });
      expect(o.e).toBeGreaterThanOrEqual(0.025);
      expect(o.e).toBeLessThanOrEqual(0.08);
      expect(Math.abs(o.inclination)).toBeGreaterThanOrEqual(0.05);
      expect(Math.abs(o.inclination)).toBeLessThanOrEqual(0.12);
    }
  });

  test('an arranged orbit is a flat circle where the owner put it', () => {
    const o = orbitForSeat({ seat: 3, count: 7, id: 't', seed: 's', radius: 950, phase: 1.2 });
    expect(o).toEqual({ a: 950, e: 0, inclination: 0, phase: 1.2 });
    const p = placeOrbit({ a: 1000, e: 0, inclination: 0, phase: Math.PI / 3 }, 0);
    expect(Math.atan2(p.y - CENTER.y, p.x - CENTER.x)).toBeCloseTo(Math.PI / 3, 12);
    expect(Math.sqrt((p.x - CENTER.x) ** 2 + (p.y - CENTER.y) ** 2)).toBeCloseTo(1000, 9);
    expect(p.z).toBe(0);
  });
});

describe('the motion', () => {
  test('takes 74 s round the base ring', () => {
    expect(periodOf(640)).toBeCloseTo(74, 9);
  });

  test('is weather: every seat takes between 45 s and 400 s', () => {
    for (let seat = 0; seat < 21; seat++) {
      const o = orbitForSeat({ seat, count: 21, id: `track_${seat}`, seed: 's' });
      expect(periodOf(o.a)).toBeGreaterThan(45);
      expect(periodOf(o.a)).toBeLessThan(400);
    }
  });

  test("solves Kepler's equation", () => {
    for (let M = 0; M < 6.2; M += 0.7) {
      for (const e of [0, 0.05, 0.15]) {
        const E = eccentricAnomaly(M, e);
        expect(Math.abs(E - e * sin(E) - M)).toBeLessThan(1e-5);
      }
    }
  });

  test('comes back to its start after one period', () => {
    const o = orbitForSeat({ seat: 0, count: 7, id: 't', seed: 's' });
    const start = placeOrbit(o, 0);
    const round = placeOrbit(o, periodOf(o.a));
    expect(round.x).toBeCloseTo(start.x, 6);
    expect(round.y).toBeCloseTo(start.y, 6);
  });

  test('sweeps faster at perihelion', () => {
    const o = { a: 1000, e: 0.15, inclination: 0.1, phase: 0 };
    const speed = (t: number) => {
      const p0 = placeOrbit(o, t);
      const p1 = placeOrbit(o, t + 0.1);
      return Math.sqrt((p1.x - p0.x) ** 2 + (p1.y - p0.y) ** 2 + (p1.z - p0.z) ** 2);
    };
    expect(speed(0)).toBeGreaterThan(speed(periodOf(1000) / 2));
  });

  test('puts the sun at a focus and tilts the plane by the inclination', () => {
    const o = { a: 1000, e: 0.08, inclination: 0.1, phase: Math.PI / 2 };
    const p = placeOrbit(o, 0);
    expect(p.z / (p.y - CENTER.y)).toBeCloseTo(Math.tan(0.1), 9);
    const peri = placeOrbit({ ...o, phase: 0 }, 0);
    expect(peri.x - CENTER.x).toBeCloseTo(1000 * (1 - 0.08), 9);
  });

  test('stands still while sky time stands still', () => {
    const o = orbitForSeat({ seat: 4, count: 7, id: 't', seed: 's' });
    expect(placeOrbit(o, 12.5)).toEqual(placeOrbit(o, 12.5));
  });
});

describe('keeping worlds apart', () => {
  test('pushes overlapping bodies opposite ways along their orbits', () => {
    const off = separationOffsets(
      [
        { x: 2500, y: 1500 },
        { x: 2560, y: 1500 },
      ],
      [260, 260],
    );
    expect(off[0]).not.toBe(0);
    expect(off[1]).not.toBe(0);
    expect(off[0] > 0).not.toBe(off[1] > 0);
  });

  test('leaves well-separated bodies alone', () => {
    const off = separationOffsets(
      [
        { x: 1500, y: 2500 },
        { x: 3500, y: 2500 },
      ],
      [260, 260],
    );
    expect(off).toEqual([0, 0]);
  });

  test('is bounded so the nudge stays invisible', () => {
    const same = { x: 2500, y: 1500 };
    for (const o of separationOffsets([same, same, { x: 2501, y: 1500 }], [260, 260, 260])) {
      expect(Math.abs(o)).toBeLessThanOrEqual(2 * SEPARATION_RATE * (1 / 30) + 1e-12);
    }
  });
});
