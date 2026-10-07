import { describe, expect, test } from 'vitest';
import type { Galaxy, SolarSystem, World } from './catalogue';
import { crumbs, refuseSeat, up } from './catalogue';
import { cardFacts, verbFor } from './facts';
import { artistLine, labelTier, nextInOrbit, orbitOrder, worldLabel, gatheredLabel } from './labels';
import { placeSystem, systemOrbits } from './system';

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

describe('labels follow k', () => {
  test('dots under 0.35, names to 0.9, titles above, the artist line from 1.6', () => {
    expect(labelTier(0.349)).toEqual({ show: 'dots', artistLine: false });
    expect(labelTier(0.35)).toEqual({ show: 'names', artistLine: false });
    expect(labelTier(0.899)).toEqual({ show: 'names', artistLine: false });
    expect(labelTier(0.9)).toEqual({ show: 'titles', artistLine: false });
    expect(labelTier(1.599)).toEqual({ show: 'titles', artistLine: false });
    expect(labelTier(1.6)).toEqual({ show: 'titles', artistLine: true });
  });

  test('the artist line names the maker and the generation', () => {
    expect(artistLine('LMY', 2)).toBe('LMY · gen 2');
    expect(artistLine('LMY', 0)).toBe('LMY · gen 0');
  });
});

describe('what a work is', () => {
  test('one fact line and one verb per medium', () => {
    expect(cardFacts({ medium: 'song', key: 'A minor', bpm: 128 })).toBe('Song · A minor · 128 BPM');
    expect(cardFacts({ medium: 'writing', key: null, bpm: null, words: 1200 })).toBe('Writing · 1,200 words');
    expect(cardFacts({ medium: 'fashion', key: null, bpm: null, photos: 14 })).toBe('Fashion · 14 photos');
    expect(cardFacts({ medium: 'film', key: null, bpm: null, durationSeconds: 238 })).toBe('Film · 3:58');
    expect(cardFacts({ medium: 'song', key: null, bpm: 127.6 })).toBe('Song · 128 BPM');
    expect(cardFacts({ medium: 'fashion', key: null, bpm: null, photos: 1 })).toBe('Fashion · 1 photo');
    expect(['song', 'film', 'writing', 'fashion'].map((m) => verbFor(m as World['medium']))).toEqual([
      'Play',
      'Watch',
      'Read',
      'Look',
    ]);
  });
});

describe('the accessibility tree', () => {
  test('reads a world as one sentence', () => {
    const w = world(3, { title: 'Low Tide', key: 'A minor', bpm: 86 });
    expect(worldLabel(w, 2, 12, 'World Ending')).toBe('Low Tide. Song, A minor, 86 BPM. World 3 of 12 in World Ending.');
  });

  test('reads the gathered world without hiding what it holds', () => {
    expect(gatheredLabel(4, 'World Ending')).toBe('4 more worlds, gathered. In World Ending.');
  });

  test('walks the bodies in orbit order: the sun, the worlds by seat, then the gathered world', () => {
    const s: SolarSystem = {
      id: 9,
      slug: 's',
      title: 'S',
      colorSeed: null,
      orbitIndex: 0,
      posX: null,
      posY: null,
      worlds: Array.from({ length: 23 }, (_, i) => world(100 + i, { orbitIndex: 22 - i })),
    };
    const order = orbitOrder(placeSystem(systemOrbits(s), 0));
    expect(order[0]).toBe('sun');
    expect(order[1]).toBe('world:122');
    expect(order[21]).toBe('world:102');
    expect(order[22]).toBe('gathered');
    expect(order).toHaveLength(23);
  });

  test('Tab leaves at the ends; the arrows go round', () => {
    const order = ['sun', 'world:1', 'world:2'];
    expect(nextInOrbit(order, 'world:1', 1, false)).toBe('world:2');
    expect(nextInOrbit(order, 'world:2', 1, false)).toBeNull();
    expect(nextInOrbit(order, 'sun', -1, false)).toBeNull();
    expect(nextInOrbit(order, 'world:2', 1, true)).toBe('sun');
    expect(nextInOrbit(order, 'sun', -1, true)).toBe('world:2');
    expect(nextInOrbit(order, null, 1, false)).toBe('sun');
  });
});

describe('places', () => {
  const galaxies: Galaxy[] = [
    {
      id: 1,
      slug: 'lmy',
      displayName: 'LMY',
      skySeed: null,
      universeX: null,
      universeY: null,
      systems: [
        {
          id: 2,
          slug: 'world-ending',
          title: 'World Ending',
          colorSeed: null,
          orbitIndex: 0,
          posX: null,
          posY: null,
          worlds: [world(3, { title: 'Low Tide' })],
        },
      ],
    },
  ];

  test('the breadcrumb names each tier', () => {
    expect(crumbs({ tier: 'system', galaxyId: 1, systemId: 2 }, galaxies).join(' › ')).toBe('Everyone › LMY › World Ending');
    expect(crumbs({ tier: 'universe' }, galaxies)).toEqual(['Everyone']);
    expect(crumbs({ tier: 'planet', galaxyId: 1, systemId: 2, worldId: 3 }, galaxies)).toEqual([
      'Everyone',
      'LMY',
      'World Ending',
      'Low Tide',
    ]);
  });

  test('Esc goes up one tier, and the universe is the end of the road', () => {
    expect(up({ tier: 'planet', galaxyId: 1, systemId: 2, worldId: 3 })).toEqual({ tier: 'system', galaxyId: 1, systemId: 2 });
    expect(up({ tier: 'system', galaxyId: 1, systemId: 2 })).toEqual({ tier: 'galaxy', galaxyId: 1 });
    expect(up({ tier: 'galaxy', galaxyId: 1 })).toEqual({ tier: 'universe' });
    expect(up({ tier: 'universe' })).toEqual({ tier: 'universe' });
  });

  test('a 22nd world is refused in the sentence the server uses', () => {
    const full = { ...galaxies[0].systems[0], worlds: Array.from({ length: 21 }, (_, i) => world(i)) };
    expect(refuseSeat(full)).toBe('A solar system holds 21 worlds. Start another one.');
    expect(refuseSeat({ ...full, worlds: full.worlds.slice(1) })).toBeNull();
  });
});
