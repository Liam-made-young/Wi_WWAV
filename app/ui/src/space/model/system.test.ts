import { describe, expect, test } from 'vitest';
import type { Medium, SolarSystem, World } from './catalogue';
import { CENTER, OVERFLOW_RING_RADIUS, RING_PLANET_RADII, ringOf, ringRadius } from './orbits';
import { arrangedOrbit, placeSystem, systemOrbits, turnBy } from './system';

function world(id: number, extra: Partial<World> = {}): World {
  return {
    id,
    trackId: `t${id}`,
    title: `p${id}`,
    medium: 'song',
    orbitIndex: id,
    orbitRadius: null,
    phaseOffset: null,
    key: null,
    bpm: null,
    ...extra,
  };
}

function system(worlds: World[]): SolarSystem {
  return { id: 5, slug: 'world-ending', title: 'World Ending', colorSeed: null, orbitIndex: 0, posX: null, posY: null, worlds };
}

const distance = (p: { x: number; y: number }) => Math.sqrt((p.x - CENTER.x) ** 2 + (p.y - CENTER.y) ** 2);

describe('a system', () => {
  test('places every shown world on its seat with its own size', () => {
    const ws = [world(0), world(1, { medium: 'film' }), world(2, { medium: 'fashion' })];
    const placed = placeSystem(systemOrbits(system(ws)), 0);
    expect(placed.worlds.map((w) => [w.id, w.seat])).toEqual([
      [0, 0],
      [1, 1],
      [2, 2],
    ]);
    expect(placed.worlds.map((w) => w.radius)).toEqual([200, 200 * 1.09, 200 * 1.35]);
    expect(placed.overflow).toBeNull();
  });

  test('uses the colour seed when there is one, and the slug otherwise', () => {
    const plain = systemOrbits(system([world(0)]));
    const seeded = systemOrbits({ ...system([world(0)]), colorSeed: 'sky-9' });
    expect(plain.shown[0].orbit.a).toBe(ringRadius(0, 'world-ending'));
    expect(seeded.shown[0].orbit.a).toBe(ringRadius(0, 'sky-9'));
  });

  test("names a world's orbit by its track id, or planet-<id> when it has none", () => {
    const a = systemOrbits(system([world(0, { trackId: null })])).shown[0].orbit;
    const b = systemOrbits(system([world(1, { trackId: 'planet-0', orbitIndex: 0 })])).shown[0].orbit;
    expect(a.e).toBe(b.e);
  });

  test('gathers the 22nd world on onward into one world outside ring 3', () => {
    const ws = Array.from({ length: 25 }, (_, i) => world(i));
    const placed = placeSystem(systemOrbits(system(ws)), 0);
    expect(placed.worlds).toHaveLength(21);
    expect(placed.overflow?.worlds.map((w) => w.id)).toEqual([21, 22, 23, 24]);
    expect(placed.overflow?.radius).toBeCloseTo(RING_PLANET_RADII[2] * 1.14, 12);
    // On its own ring, not on the 21st world's seat (iOS's choice).
    const r = distance(placed.overflow!);
    expect(r).toBeGreaterThan(OVERFLOW_RING_RADIUS * 0.945 * (1 - 0.08) - 1e-9);
    const seat20 = placed.worlds[20];
    expect(Math.sqrt((seat20.x - placed.overflow!.x) ** 2 + (seat20.y - placed.overflow!.y) ** 2)).toBeGreaterThan(300);
  });

  test("an owner's orbit is a flat circle through the drop point", () => {
    const drop = { x: CENTER.x + 600, y: CENTER.y - 800 };
    const arranged = arrangedOrbit(drop);
    expect(arranged.orbitRadius).toBeCloseTo(1000, 9);
    expect(arranged.phaseOffset).toBeGreaterThanOrEqual(0);
    const placed = placeSystem(systemOrbits(system([world(0, arranged)])), 0, 0, true);
    expect(placed.worlds[0].x).toBeCloseTo(drop.x, 6);
    expect(placed.worlds[0].y).toBeCloseTo(drop.y, 6);
    expect(Math.abs(placed.worlds[0].z)).toBe(0);
  });

  test('keeps an arranged radius within 400–1800, as the server does', () => {
    expect(arrangedOrbit({ x: CENTER.x + 10, y: CENTER.y }).orbitRadius).toBe(400);
    expect(arrangedOrbit({ x: CENTER.x + 5000, y: CENTER.y }).orbitRadius).toBe(1800);
  });

  test('turns about its sun when dragged', () => {
    const from = { x: CENTER.x + 100, y: CENTER.y };
    const to = { x: CENTER.x, y: CENTER.y + 100 };
    expect(turnBy(0.5, from, to)).toBeCloseTo(0.5 + Math.PI / 2, 12);
    const ws = [world(0)];
    const still = placeSystem(systemOrbits(system(ws)), 0, 0);
    const turned = placeSystem(systemOrbits(system(ws)), 0, Math.PI);
    expect(distance(turned.worlds[0])).toBeGreaterThan(0);
    expect(turned.worlds[0].x).not.toBeCloseTo(still.worlds[0].x, 3);
  });

  test('holds still while the sky clock holds still', () => {
    const orbits = systemOrbits(system(Array.from({ length: 21 }, (_, i) => world(i))));
    expect(placeSystem(orbits, 30)).toEqual(placeSystem(orbits, 30));
  });
});

// The review of #72's fix: turning each ring a third of a seat brought
// neighbouring full rings closer than iOS's half seat, and some
// mixed-media albums of 14 and 21 opened with two worlds overlapping.
// Full rings keep iOS's room from the ring just inside.
test('full rings of mixed media open with no two worlds on neighbouring rings overlapping', () => {
  const media: Medium[] = ['song', 'film', 'writing', 'fashion'];
  const crowded: string[] = [];
  for (const n of [14, 21]) {
    for (let k = 0; k < 250; k++) {
      const ws = Array.from({ length: n }, (_, i) => world(i, { trackId: `t${k}-${i}`, medium: media[(i * 7 + k * 3) % 4] }));
      const placed = placeSystem(systemOrbits({ ...system(ws), slug: `album-${k}` }), 0).worlds;
      for (const a of placed) {
        for (const b of placed) {
          if (ringOf(b.seat) !== ringOf(a.seat) + 1) continue;
          if (Math.hypot(a.x - b.x, a.y - b.y) < a.radius + b.radius) crowded.push(`${n} worlds, album-${k}: seats ${a.seat} and ${b.seat}`);
        }
      }
    }
  }
  expect(crowded).toEqual([]);
});
