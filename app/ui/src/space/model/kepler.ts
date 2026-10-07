// Real orbital mechanics, slowed to the speed of weather (docs/SPEC.md 4.4;
// iOS v5's `KeplerMotion.swift`). An ellipse with the sun at a focus, a
// sweep that quickens at perihelion, and a period that grows with radius:
// the base ring comes round in 74 s.
//
// Time here is the sky's clock (motion.ts), which only runs while
// something plays, so a silent sky holds still by construction.

import { atan2, cos, sin, sqrt, unitJitter } from '../../shared/dmath';
import { CENTER, RING_RADII, ringAngle, ringRadius, type Point } from './orbits';

export interface Orbit {
  a: number; // semi-major axis, world units
  e: number; // eccentricity
  inclination: number; // tilt of the orbit's plane, radians
  phase: number; // mean anomaly at t = 0
}

// A point in the world: x and y on the system's plane (iOS's frame, y
// toward the viewer at yaw 0), z the height above it.
export interface Point3 extends Point {
  z: number;
}

const TWO_PI = 2 * Math.PI;
const BASE_PERIOD = 74;
const GRAVITATIONAL = (TWO_PI / BASE_PERIOD) * (RING_RADII[0] * sqrt(RING_RADII[0]));

// Radians of mean anomaly per second, by Kepler's third law.
export function meanMotion(a: number): number {
  return GRAVITATIONAL / (a * sqrt(a));
}

export function periodOf(a: number): number {
  return TWO_PI / meanMotion(a);
}

export interface SeatOrbit {
  seat: number;
  count: number;
  id: string;
  seed: string;
  radius?: number | null;
  phase?: number | null;
}

// Radius and phase come from the seat and the system's seed; eccentricity
// and tilt from the world's own id, each under its own salt so an
// eccentric world isn't also a steep one. An owner's arrangement is a flat
// circle, so where they dropped it and where it rides are the same place.
export function orbitForSeat({ seat, count, id, seed, radius = null, phase = null }: SeatOrbit): Orbit {
  const arranged = radius !== null || phase !== null;
  const a = radius ?? ringRadius(seat, seed);
  const e = arranged ? 0 : 0.025 + unitJitter(id, 'ecc') * 0.055;
  const sign = unitJitter(id, 'tiltsign') < 0.5 ? -1 : 1;
  const inclination = arranged ? 0 : sign * (0.05 + unitJitter(id, 'tilt') * 0.07);
  return { a, e, inclination, phase: phase ?? ringAngle(seat, count, seed) };
}

// Kepler's equation M = E − e·sin E by Newton's method; at these
// eccentricities it settles in two or three steps.
export function eccentricAnomaly(M: number, e: number): number {
  let E = M;
  for (let i = 0; i < 5; i++) {
    const delta = (E - e * sin(E) - M) / (1 - e * cos(E));
    E -= delta;
    if (Math.abs(delta) < 1e-6) break;
  }
  return E;
}

export function placeOrbit(orbit: Orbit, t: number, phaseOffset = 0): Point3 {
  const M = orbit.phase + phaseOffset + meanMotion(orbit.a) * t;
  const E = eccentricAnomaly(M, orbit.e);
  const x = orbit.a * (cos(E) - orbit.e);
  const planeY = orbit.a * sqrt(Math.max(0, 1 - orbit.e * orbit.e)) * sin(E);
  // The plane tips about its x axis by the inclination, so no two rings
  // lie quite parallel. The camera, not the orbit, supplies the view tilt.
  return {
    x: CENTER.x + x,
    y: CENTER.y + planeY * cos(orbit.inclination),
    z: planeY * sin(orbit.inclination),
  };
}

// Two worlds that come too close are each nudged along their own orbit,
// never off it, by at most 0.55 rad a second. Computed afresh each frame
// from where the worlds are, so it holds no state.
export const SEPARATION_RATE = 0.55;

// iOS draws every orbit squashed to 0.88 of its height and judges
// closeness there, on its screen. The model keeps the plane unsquashed
// (the camera supplies the tilt), so it squashes y the same way here, and
// the iPhone and the Mac nudge the same worlds by the same amounts.
export const VIEW_TILT = 0.88;

export function separationOffsets(points: Point[], radii: number[], dt = 1 / 30): number[] {
  const offsets = points.map(() => 0);
  const screen = points.map((p) => ({ x: p.x, y: CENTER.y + (p.y - CENTER.y) * VIEW_TILT }));
  for (let i = 0; i < points.length; i++) {
    for (let j = i + 1; j < points.length; j++) {
      const pi = screen[i];
      const pj = screen[j];
      const dx = pj.x - pi.x;
      const dy = pj.y - pi.y;
      const distance = sqrt(dx * dx + dy * dy);
      const wanted = (radii[i] + radii[j]) * 1.18;
      if (!(distance < wanted && distance > 0.0001)) continue;
      const push = SEPARATION_RATE * ((wanted - distance) / wanted) * dt;
      const toward = atan2(dy, dx);
      const iTangent = atan2(pi.y - CENTER.y, pi.x - CENTER.x) + Math.PI / 2;
      const jTangent = atan2(pj.y - CENTER.y, pj.x - CENTER.x) + Math.PI / 2;
      offsets[i] -= push * cos(toward - iTangent);
      offsets[j] += push * cos(toward - jTangent);
    }
  }
  return offsets;
}
