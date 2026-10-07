import { describe, expect, test } from 'vitest';
import type { Galaxy, SolarSystem, World } from './catalogue';
import {
  CENTER,
  OVERFLOW_RING_RADIUS,
  RING_PLANET_RADII,
  RING_RADII,
  SUN_RADIUS,
  galaxyPosition,
  occupancy,
  placeGalaxies,
  placeSystems,
  ringAngle,
  ringOf,
  ringRadius,
  ringTurn,
  seatPoint,
  splitWorlds,
  systemBox,
  systemDots,
  universeBox,
  worldRadius,
} from './orbits';

function world(id: number, orbitIndex = id, medium: World['medium'] = 'song'): World {
  return {
    id,
    trackId: `t${id}`,
    title: `p${id}`,
    medium,
    orbitIndex,
    orbitRadius: null,
    phaseOffset: null,
    key: null,
    bpm: null,
  };
}

function system(id: number, worlds: number, extra: Partial<SolarSystem> = {}): SolarSystem {
  return {
    id,
    slug: `album-${id}`,
    title: `Album ${id}`,
    colorSeed: null,
    orbitIndex: id,
    posX: null,
    posY: null,
    worlds: Array.from({ length: worlds }, (_, i) => world(id * 100 + i, i)),
    ...extra,
  };
}

const TWO_PI = 2 * Math.PI;
const SEAT = TWO_PI / 7;

// The angular gap between two angles, folded into [0, π].
function gap(a: number, b: number): number {
  const d = (((a - b) % TWO_PI) + TWO_PI) % TWO_PI;
  return Math.min(d, TWO_PI - d);
}

describe('seats and rings', () => {
  test('seven to a ring, three rings', () => {
    expect([0, 6, 7, 13, 14, 20].map(ringOf)).toEqual([0, 0, 1, 1, 2, 2]);
  });

  test('a part-full ring spreads its worlds round the whole circle', () => {
    expect(occupancy(8, 10)).toBe(3);
    expect(occupancy(2, 10)).toBe(7);
    const angles = [7, 8, 9].map((s) => ringAngle(s, 10, 'album-1'));
    expect(gap(angles[0], angles[1])).toBeCloseTo(TWO_PI / 3, 12);
    expect(gap(angles[1], angles[2])).toBeCloseTo(TWO_PI / 3, 12);
  });

  test('radii are 640, 1120 and 1580, each within its 5.5% variation', () => {
    for (let seat = 0; seat < 21; seat++) {
      const base = RING_RADII[ringOf(seat)];
      const r = ringRadius(seat, 'album-100');
      expect(r).toBeGreaterThanOrEqual(base * 0.945);
      expect(r).toBeLessThanOrEqual(base * 1.055);
    }
    expect(RING_RADII).toEqual([640, 1120, 1580]);
  });

  test('worlds shrink outward: 200, 170, 145; films ×1.09, fashion ×1.35', () => {
    expect(RING_PLANET_RADII).toEqual([200, 170, 145]);
    expect(worldRadius('song', 0)).toBe(200);
    expect(worldRadius('writing', 7)).toBe(170);
    expect(worldRadius('film', 14)).toBeCloseTo(145 * 1.09, 12);
    expect(worldRadius('fashion', 3)).toBeCloseTo(200 * 1.35, 12);
  });

  // The fix to iOS (docs/QUESTIONS.md #72): iOS turns each ring half a
  // seat further than the last, which puts ring 2 a whole seat round and
  // lines it up with ring 0. Here ring 1 keeps iOS's half seat and ring 2
  // turns 1⅙ seats: a third of a seat from ring 1, which its worlds could
  // touch, and a sixth from ring 0, which they can only line up with.
  test('full rings turn 0, ½ and 1⅙ seats, so no two line up into spokes', () => {
    expect([0, 1, 2].map((r) => ringTurn(r, 7))).toEqual([0, 0.5, expect.closeTo(7 / 6, 12)]);
    for (const seed of ['album-100', 'album-243', 'world-ending', 'x']) {
      const angles = Array.from({ length: 21 }, (_, s) => ringAngle(s, 21, seed));
      for (let i = 0; i < 21; i++) {
        for (let j = 0; j < 21; j++) {
          if (ringOf(i) === ringOf(j)) continue;
          const neighbours = Math.abs(ringOf(i) - ringOf(j)) === 1;
          expect(gap(angles[i], angles[j])).toBeGreaterThan((neighbours ? SEAT / 3 : SEAT / 6) - 1e-9);
        }
      }
    }
  });

  // A part-full ring is spread round the whole circle, so on iOS it can
  // line up with the full rings inside it (eleven worlds put ring 1's four
  // on ring 0's rays). Here it turns to the middle of the room they leave.
  test('a part-full ring never shares a ray with the rings inside it', () => {
    expect(ringTurn(1, 4)).toBe(0.625); // iOS: ½, on ring 0's rays
    expect(ringTurn(1, 3)).toBe(0.5); // as on iOS, already between them
    let least = Infinity;
    for (let n = 8; n <= 22; n++) {
      const angles = Array.from({ length: n }, (_, s) => ringAngle(s, n, 'album-7'));
      for (let i = 0; i < n; i++) {
        for (let j = 0; j < n; j++) {
          if (ringOf(i) !== ringOf(j)) least = Math.min(least, gap(angles[i], angles[j]));
        }
      }
    }
    // The tightest is nineteen worlds: ring 2's five come within 1/30 of a
    // seat of ring 0's rays, to keep 1/15 from ring 1's.
    expect(least).toBeCloseTo(SEAT / 30, 9);
  });

  test('seats are deterministic and never overlap', () => {
    const first = Array.from({ length: 21 }, (_, s) => seatPoint(s, 21, 'album-100'));
    const second = Array.from({ length: 21 }, (_, s) => seatPoint(s, 21, 'album-100'));
    expect(second).toEqual(first);
    for (let i = 0; i < 21; i++) {
      for (let j = i + 1; j < 21; j++) {
        const d = Math.sqrt((first[i].x - first[j].x) ** 2 + (first[i].y - first[j].y) ** 2);
        expect(d).toBeGreaterThan(RING_PLANET_RADII[ringOf(i)] + RING_PLANET_RADII[ringOf(j)]);
      }
    }
  });

  test('two systems do not look stamped from one mould', () => {
    expect(seatPoint(0, 21, 'album-100')).not.toEqual(seatPoint(0, 21, 'album-243'));
  });
});

describe('gathering', () => {
  test('twenty-one show whole; the rest are gathered, in orbit order', () => {
    expect(splitWorlds(system(1, 21).worlds).gathered).toEqual([]);
    const scrambled = [world(9, 9), world(0, 0), world(4, 4)];
    expect(splitWorlds(scrambled).shown.map((w) => w.orbitIndex)).toEqual([0, 4, 9]);
    const big = splitWorlds(system(1, 25).worlds);
    expect(big.shown).toHaveLength(21);
    expect(big.gathered.map((w) => w.orbitIndex)).toEqual([21, 22, 23, 24]);
  });

  test('a tie in orbit index breaks by id, so the order never depends on input order', () => {
    const a = splitWorlds([world(5, 0), world(3, 0)]).shown.map((w) => w.id);
    const b = splitWorlds([world(3, 0), world(5, 0)]).shown.map((w) => w.id);
    expect(a).toEqual([3, 5]);
    expect(b).toEqual([3, 5]);
  });

  // As iOS checks its own rings: the gap between two rings is wider than
  // the two worlds that have to pass through it.
  test('the overflow ring sits outside ring 3, and no ring crowds the next', () => {
    const rings = [...RING_RADII, OVERFLOW_RING_RADIUS];
    const sizes = [...RING_PLANET_RADII, RING_PLANET_RADII[2] * 1.14];
    for (let r = 0; r + 1 < rings.length; r++) {
      expect(rings[r] + sizes[r]).toBeLessThan(rings[r + 1] - sizes[r + 1]);
    }
  });
});

describe('framing', () => {
  test('a system box grows with its rings and stays centred on the sun', () => {
    const small = systemBox(2, 's', false);
    const full = systemBox(21, 's', false);
    const gathered = systemBox(21, 's', true);
    expect(full.w).toBeGreaterThan(small.w);
    expect(gathered.w).toBeGreaterThan(full.w);
    expect(full.x + full.w / 2).toBeCloseTo(CENTER.x, 9);
    expect(systemBox(0, 's', false).w).toBeGreaterThan(SUN_RADIUS * 2);
  });

  test('the universe box floors at five glows, so one galaxy does not fill the screen', () => {
    const box = universeBox([{ id: 1, x: 100, y: 100, radius: 430 }]);
    expect(box.w).toBe(430 * 5);
    expect(box.h).toBe(430 * 5);
  });
});

describe('the galaxy tier', () => {
  const galaxy: Galaxy = {
    id: 7,
    slug: 'lmy',
    displayName: 'LMY',
    skySeed: 'galaxy-7',
    universeX: null,
    universeY: null,
    systems: [system(2, 3), system(1, 12), system(3, 0, { posX: 900, posY: 4100 })],
  };

  test('systems take seats by orbit index, on the galaxy seed', () => {
    const placed = placeSystems(galaxy);
    expect(placed.map((s) => s.id)).toEqual([1, 2, 3]);
    expect({ x: placed[0].x, y: placed[0].y }).toEqual(seatPoint(0, 3, 'galaxy-7'));
    expect({ x: placed[1].x, y: placed[1].y }).toEqual(seatPoint(1, 3, 'galaxy-7'));
  });

  test("an owner's position wins over the seat", () => {
    const placed = placeSystems(galaxy);
    expect(placed[2]).toMatchObject({ x: 900, y: 4100 });
  });

  test('a disc shows at most twelve dots', () => {
    expect(systemDots(30, 'album-1')).toHaveLength(12);
    expect(systemDots(0, 'album-1')).toHaveLength(0);
    for (const dot of systemDots(12, 'album-1')) {
      expect(Math.sqrt(dot.x * dot.x + dot.y * dot.y)).toBeLessThanOrEqual(300 * 0.78 + 1e-9);
    }
  });

  test('more than twenty-one systems keep finding new rings instead of stacking', () => {
    const points = Array.from({ length: 30 }, (_, s) => seatPoint(s, 30, 'galaxy-7'));
    for (let i = 0; i < 30; i++) {
      for (let j = i + 1; j < 30; j++) {
        const d = Math.sqrt((points[i].x - points[j].x) ** 2 + (points[i].y - points[j].y) ** 2);
        expect(d).toBeGreaterThan(100);
      }
    }
  });
});

describe('the universe tier', () => {
  // The server's formula (server/utils/universeMath.js), which persists
  // universeX/Y by galaxy id: same inputs, so within a rounding.
  test('places a galaxy by its id as the server does', () => {
    // A missing sky seed falls back to "galaxy-<id>", as on the server;
    // unitJitter('galaxy-1', 'sky') is one of the pinned parity vectors.
    const a = 1 * 2.399963 + 0.22785640804777063 * 0.6;
    const got = galaxyPosition(1, null);
    expect(got.x).toBeCloseTo(2500 + Math.cos(a) * 1320, 9);
    expect(got.y).toBeCloseTo(2500 + Math.sin(a) * 1320, 9);
    const p = galaxyPosition(42, 'galaxy-42');
    expect(Math.sqrt((p.x - 2500) ** 2 + (p.y - 2500) ** 2)).toBeCloseTo(1320 * Math.sqrt(42), 9);
  });

  test('newcomers land on the rim', () => {
    const radii = [1, 2, 10, 100, 2000].map((id) => {
      const p = galaxyPosition(id, null);
      return Math.sqrt((p.x - 2500) ** 2 + (p.y - 2500) ** 2);
    });
    expect([...radii].sort((a, b) => a - b)).toEqual(radii);
  });

  test("a stored position (the server's, after drift) wins", () => {
    const g: Galaxy = { id: 3, slug: 'a', displayName: 'A', skySeed: null, universeX: 10, universeY: 20, systems: [] };
    expect(placeGalaxies([g])[0]).toMatchObject({ id: 3, x: 10, y: 20, radius: 430 });
  });
});
