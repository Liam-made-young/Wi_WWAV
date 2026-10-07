// Where a body sits, at every tier (docs/SPEC.md 4.2, 4.4).
//
// "Nothing about an orbit is stored." Radius, angle and size come from a
// body's seat and its parent's seed through FNV-1a, never a random number,
// so the same catalogue lays out to the same bits on every machine. The
// numbers are iOS v5's (`ios_v4/WWAV/Views/Space/Orbits.swift`), with two
// fixes (docs/QUESTIONS.md #72): the rings' stagger never lines two rings
// up, and a gathered world gets a ring of its own.
//
// All trigonometry goes through shared/dmath, because Math.sin differs in
// the last bit between engines.

import { cos, sin, sqrt, unitJitter } from '../../shared/dmath';
import type { Galaxy, Medium, SolarSystem, World } from './catalogue';

export interface Point {
  x: number;
  y: number;
}

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

const TWO_PI = 2 * Math.PI;

// A fixed 5000 × 5000 world; only the camera moves.
export const CENTER: Point = { x: 2500, y: 2500 };

// --- The system tier: worlds around a sun ------------------------------------

// Seven to a ring, three rings deep: seven reads at a glance, and twenty-one
// is the cap, so a full project shows whole.
export const PER_RING = 7;
export const SHOWN_CAP = 21;
export const RING_RADII = [640, 1120, 1580];
// Worlds shrink outward, which gives the system depth.
export const RING_PLANET_RADII = [200, 170, 145];
// Each seat's radius varies by up to 5.5% of its ring, so periods differ.
const RING_VARIATION = 0.055;
// Rings past the third (the gathered world's, and a galaxy with more than
// twenty-one systems) keep the third ring's gap.
const OUTER_RING_GAP = 460;
export const SUN_RADIUS = 330;

const FILM_SCALE = 1.09;
// "A place you walk around", so a gallery outweighs a song.
const FASHION_SCALE = 1.35;
const OVERFLOW_SCALE = 1.14;

// The gathered world sits on the seat after the twenty-first, which is the
// first seat of a fourth ring, outside the other three.
export const OVERFLOW_SEAT = SHOWN_CAP;
export const OVERFLOW_RING_RADIUS = RING_RADII[2] + OUTER_RING_GAP;

// iOS turns each ring half a seat further than the one inside it, which
// brings ring 2 a whole seat round, in line with ring 0. A third of a seat
// per ring keeps every pair of full rings a third of a seat apart.
const STAGGER = TWO_PI / PER_RING / 3;

export function ringOf(seat: number): number {
  return Math.floor(Math.max(seat, 0) / PER_RING);
}

export function seatWithinRing(seat: number): number {
  return Math.max(seat, 0) % PER_RING;
}

// How many seats a ring holds, so a part-full ring spreads its worlds round
// the whole circle instead of leaving a gap.
export function occupancy(seat: number, total: number): number {
  return Math.max(1, Math.min(total - ringOf(seat) * PER_RING, PER_RING));
}

function ringBase(ring: number): number {
  return ring < RING_RADII.length ? RING_RADII[ring] : RING_RADII[2] + OUTER_RING_GAP * (ring - 2);
}

export function ringRadius(seat: number, seed: string): number {
  const jitter = unitJitter(String(seat), `ring|${seed}`);
  return ringBase(ringOf(seat)) * (1 + RING_VARIATION * (jitter * 2 - 1));
}

// Seats share their ring evenly, turned by a per-system rotation so two
// systems with the same population don't look stamped.
export function ringAngle(seat: number, count: number, seed: string): number {
  const rotation = unitJitter(seed, 'rotation') * TWO_PI;
  const within = seatWithinRing(seat) / occupancy(seat, Math.max(count, 1));
  return rotation + ringOf(seat) * STAGGER + within * TWO_PI;
}

export function seatPoint(seat: number, count: number, seed: string): Point {
  const r = ringRadius(seat, seed);
  const a = ringAngle(seat, count, seed);
  return { x: CENTER.x + cos(a) * r, y: CENTER.y + sin(a) * r };
}

export function worldRadius(medium: Medium, seat: number): number {
  const base = RING_PLANET_RADII[Math.min(ringOf(seat), RING_PLANET_RADII.length - 1)];
  if (medium === 'film') return base * FILM_SCALE;
  if (medium === 'fashion') return base * FASHION_SCALE;
  return base;
}

export function overflowRadius(): number {
  return worldRadius('song', OVERFLOW_SEAT) * OVERFLOW_SCALE;
}

// The first twenty-one worlds in orbit order get seats; the rest are
// gathered into one world. "Nothing is hidden, it is gathered."
export function splitWorlds(worlds: World[]): { shown: World[]; gathered: World[] } {
  const ordered = [...worlds].sort((a, b) => a.orbitIndex - b.orbitIndex || a.id - b.id);
  return { shown: ordered.slice(0, SHOWN_CAP), gathered: ordered.slice(SHOWN_CAP) };
}

export function systemSeed(system: SolarSystem): string {
  return system.colorSeed ?? system.slug;
}

// The box a system occupies, so the camera frames a three-world project
// tight and a full one wide.
export function systemBox(shownCount: number, seed: string, gathered: boolean): Box {
  let pad = SUN_RADIUS * 2.4;
  if (shownCount > 0) {
    let outer = 0;
    for (let seat = 0; seat < shownCount; seat++) {
      outer = Math.max(outer, ringRadius(seat, seed) + worldRadius('song', seat));
    }
    if (gathered) outer = Math.max(outer, ringRadius(OVERFLOW_SEAT, seed) + overflowRadius());
    pad = outer + 150;
  }
  return { x: CENTER.x - pad, y: CENTER.y - pad, w: pad * 2, h: pad * 2 };
}

// The box around one body, for flying into it.
export function bodyBox(point: Point, radius: number): Box {
  const r = radius * 2.6;
  return { x: point.x - r, y: point.y - r, w: r * 2, h: r * 2 };
}

// --- The galaxy tier: systems around a bio sun -------------------------------

export const SYSTEM_DISC_RADIUS = 300;
const SYSTEM_DOTS_MAX = 12;
const GOLDEN = Math.PI * (3 - Math.sqrt(5));

export interface SystemDisc extends Point {
  id: number;
  radius: number;
  // A disc shows its worlds as dots, offsets from its centre.
  dots: Point[];
}

export function galaxySeed(galaxy: Galaxy): string {
  return galaxy.skySeed ?? galaxy.slug;
}

export function systemDots(worldCount: number, seed: string): Point[] {
  const n = Math.min(Math.max(worldCount, 0), SYSTEM_DOTS_MAX);
  const turn = unitJitter(seed, 'dots') * TWO_PI;
  return Array.from({ length: n }, (_, i) => {
    const angle = i * GOLDEN + turn;
    const ring = 0.42 + (i % 3) * 0.18;
    return { x: cos(angle) * SYSTEM_DISC_RADIUS * ring, y: sin(angle) * SYSTEM_DISC_RADIUS * ring };
  });
}

// Systems take seats by orbit index, as worlds do, but don't orbit. An
// owner's position wins over the seat.
export function placeSystems(galaxy: Galaxy): SystemDisc[] {
  const seed = galaxySeed(galaxy);
  const ordered = [...galaxy.systems].sort((a, b) => a.orbitIndex - b.orbitIndex || a.id - b.id);
  return ordered.map((system, seat) => {
    const owned = system.posX !== null && system.posY !== null;
    const at = owned ? { x: system.posX!, y: system.posY! } : seatPoint(seat, ordered.length, seed);
    return {
      id: system.id,
      x: at.x,
      y: at.y,
      radius: SYSTEM_DISC_RADIUS,
      dots: systemDots(system.worlds.length, system.slug),
    };
  });
}

// --- The universe tier: everyone ---------------------------------------------

export const GALAXY_GLOW = 430;
export const GALAXY_SUN_FRACTION = 0.33;
const UNIVERSE_SPREAD = 1320;
const PHYLLO_ANGLE = 2.399963;

export interface PlacedGalaxy extends Point {
  id: number;
  radius: number;
}

// Phyllotaxis by galaxy id, as the server computes it: ids only grow, so a
// galaxy keeps its place forever and "newcomers land on the rim".
export function galaxyPosition(id: number, skySeed: string | null): Point {
  const angle = id * PHYLLO_ANGLE + unitJitter(skySeed || `galaxy-${id}`, 'sky') * 0.6;
  const r = UNIVERSE_SPREAD * sqrt(id);
  return { x: CENTER.x + cos(angle) * r, y: CENTER.y + sin(angle) * r };
}

// The server stores each galaxy's place (and moves it by drift); a galaxy
// it hasn't placed yet is derived the same way the server would.
export function placeGalaxies(galaxies: Galaxy[]): PlacedGalaxy[] {
  return [...galaxies]
    .sort((a, b) => a.id - b.id)
    .map((g) => {
      const stored = g.universeX !== null && g.universeY !== null;
      const at = stored ? { x: g.universeX!, y: g.universeY! } : galaxyPosition(g.id, g.skySeed);
      return { id: g.id, x: at.x, y: at.y, radius: GALAXY_GLOW };
    });
}

// Framed from where the galaxies are, with a floor so a lone galaxy
// doesn't fill the screen.
export function universeBox(placed: PlacedGalaxy[]): Box {
  if (placed.length === 0) {
    return { x: CENTER.x - UNIVERSE_SPREAD, y: CENTER.y - UNIVERSE_SPREAD, w: UNIVERSE_SPREAD * 2, h: UNIVERSE_SPREAD * 2 };
  }
  const margin = GALAXY_GLOW + GALAXY_GLOW * 0.6;
  const minX = Math.min(...placed.map((g) => g.x)) - margin;
  const minY = Math.min(...placed.map((g) => g.y)) - margin;
  const w = Math.max(...placed.map((g) => g.x)) + margin - minX;
  const h = Math.max(...placed.map((g) => g.y)) + margin - minY;
  const floor = GALAXY_GLOW * 5;
  if (w >= floor && h >= floor) return { x: minX, y: minY, w, h };
  const side = Math.max(w, h, floor);
  return { x: minX + w / 2 - side / 2, y: minY + h / 2 - side / 2, w: side, h: side };
}
