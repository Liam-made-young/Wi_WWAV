// Where the planet, the moons and the FX moons sit (docs/SPEC.md 4.6), with
// the proportions web v4 and iOS v5 share (`MoonField.jsx`,
// `PlanetStageView.swift`, `FXMoons.swift`). On a 560 pt stage: dim 515,
// planet 88, moons 23 travelling their arms from 139 (level 0) to 242
// (level 1). "A moon's distance from the planet is its volume. There is no
// other fader."

import type { Effect, FxOwner, Mix } from './mix';
import { STEMS, type Stem } from './stems';

export interface Point {
  x: number;
  y: number;
}

export interface StageGeometry {
  centre: Point;
  dim: number;
  planetR: number;
  moonR: number;
  rMin: number; // a moon's distance at level 0
  rMax: number; // at level 1
  span: number; // rMax − rMin: the drag that takes a level from 0 to 1
}

export function stageGeometry(w: number, h: number): StageGeometry {
  const dim = Math.min(w, h) * 0.92;
  const planetR = 0.17 * dim;
  const rMin = planetR + 0.1 * dim;
  const rMax = planetR + 0.3 * dim;
  return { centre: { x: w / 2, y: h / 2 }, dim, planetR, moonR: 0.045 * dim, rMin, rMax, span: rMax - rMin };
}

// The plus sign, in screen space (y down).
export const ARMS: Record<Stem, Point> = {
  vocals: { x: 0, y: -1 },
  drums: { x: 0, y: 1 },
  bass: { x: -1, y: 0 },
  other: { x: 1, y: 0 },
};

// Across each arm: vocals and drums pan left to right, bass and other
// (whose arms already run left to right) up for left, down for right.
export const PAN_AXES: Record<Stem, Point> = {
  vocals: { x: 1, y: 0 },
  drums: { x: 1, y: 0 },
  bass: { x: 0, y: 1 },
  other: { x: 0, y: 1 },
};

export function moonCentre(g: StageGeometry, stem: Stem, level: number): Point {
  const d = g.rMin + level * g.span;
  return { x: g.centre.x + ARMS[stem].x * d, y: g.centre.y + ARMS[stem].y * d };
}

// The level a moon at this distance from the planet stands for.
export function levelAt(g: StageGeometry, distance: number): number {
  return Math.min(1, Math.max(0, (distance - g.rMin) / g.span));
}

// The diagonals, clear of the plus the stems slide along: wet and airy
// above, dirty and rhythmic below.
const D = Math.SQRT1_2;
export const FX_DIRECTIONS: Record<Effect, Point> = {
  reverb: { x: -D, y: -D },
  delay: { x: D, y: -D },
  distortion: { x: -D, y: D },
  tremolo: { x: D, y: D },
};

export interface FxReach {
  parent: Point;
  fxR: number;
  rMin: number; // dry
  rMax: number; // fully wet
}

// A stem's FX moons orbit that moon; the planet's orbit the planet and
// reach every stem. Small enough to read as belonging to their parent.
export function fxReach(g: StageGeometry, owner: FxOwner, mix: Mix): FxReach {
  const master = owner === 'master';
  const parent = master ? g.centre : moonCentre(g, owner, mix[owner].level);
  const parentR = master ? g.planetR : g.moonR;
  const fxR = g.moonR * (master ? 0.62 : 0.46);
  return { parent, fxR, rMin: parentR + fxR * 2.1, rMax: parentR + fxR * 6.2 };
}

// The planet's FX moons read as the wettest stem, so pulling one back
// always has somewhere to travel.
export function fxWet(mix: Mix, owner: FxOwner, effect: Effect): number {
  if (owner !== 'master') return mix[owner].fx[effect];
  return Math.max(mix.vocals.fx[effect], mix.drums.fx[effect], mix.other.fx[effect], mix.bass.fx[effect]);
}

export function fxCentre(g: StageGeometry, owner: FxOwner, effect: Effect, mix: Mix): Point {
  const reach = fxReach(g, owner, mix);
  const d = reach.rMin + fxWet(mix, owner, effect) * (reach.rMax - reach.rMin);
  const dir = FX_DIRECTIONS[effect];
  return { x: reach.parent.x + dir.x * d, y: reach.parent.y + dir.y * d };
}

// The Now strip (2.2) is 520 × 44 pt, or 440 pt in a window under 1180 pt
// (2.1): two lines over a 6 pt meter, a task half and a track half either
// side of a 1 px divider. The track half's line 2 starts with the four 8 pt
// lights, and each owns a 44 × 44 pt square, the strip's full height. The
// squares stand side by side, so no light's area is cut short by its
// neighbour's; they take 176 pt, and line 2 keeps the rest of the half
// (83.5 pt, or 43.5 in the narrow strip) for "1:42 / 3:58". Coordinates
// are the strip's own.
export const NOW_STRIP = { w: 520, h: 44 };
export const NOW_STRIP_NARROW_W = 440;
const METER = 6;
export const LIGHT_SIZE = 8;
export const LIGHT_HIT = 44;

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface StemLight {
  stem: Stem;
  centre: Point;
  hit: Box;
}

export function trackHalfX(stripW: number = NOW_STRIP.w): number {
  return (stripW + 1) / 2;
}

const LINE_2_Y = ((NOW_STRIP.h - METER) * 3) / 4;

export function stemLights(stripW: number = NOW_STRIP.w): StemLight[] {
  const left = trackHalfX(stripW);
  return STEMS.map((stem, i) => ({
    stem,
    centre: { x: left + LIGHT_HIT * (i + 0.5), y: LINE_2_Y },
    hit: { x: left + LIGHT_HIT * i, y: 0, w: LIGHT_HIT, h: LIGHT_HIT },
  }));
}

// Which light a point in the strip falls on, if any.
export function lightAt(p: Point, stripW: number = NOW_STRIP.w): Stem | null {
  const light = stemLights(stripW).find(
    ({ hit }) => p.x >= hit.x && p.x < hit.x + hit.w && p.y >= hit.y && p.y < hit.y + hit.h,
  );
  return light ? light.stem : null;
}
