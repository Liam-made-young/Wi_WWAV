// One fixed catalogue for the layout hash (docs/PLAN.md S4.1): every tier
// and every rule that places something. Galaxies placed by the server and
// galaxies still to be placed; a system of each size, a gathered one, an
// owner-arranged one, an empty one; songs, films, pages and galleries; and
// a few families with a merge and an orphan.

import type { Galaxy, Medium, SolarSystem, World } from '../catalogue';
import type { LineageNode } from '../lineage';

const KEYS = ['A minor', 'C major', 'E minor', 'F♯ major', 'D minor', null];
const MEDIA: Medium[] = ['song', 'song', 'song', 'film', 'song', 'writing', 'song', 'fashion'];

function worlds(systemId: number, count: number): World[] {
  return Array.from({ length: count }, (_, i) => {
    const medium = MEDIA[i % MEDIA.length];
    const song = medium === 'song';
    return {
      id: systemId * 100 + i,
      // Films and galleries have no track id; their orbits go by planet id.
      trackId: song ? `track_${systemId}_${i}` : null,
      title: `World ${systemId}.${i}`,
      medium,
      orbitIndex: i,
      orbitRadius: null,
      phaseOffset: null,
      key: song ? KEYS[i % KEYS.length] : null,
      bpm: song ? 80 + ((i * 7) % 60) : null,
    };
  });
}

function system(id: number, slug: string, count: number, extra: Partial<SolarSystem> = {}): SolarSystem {
  return {
    id,
    slug,
    title: slug,
    colorSeed: null,
    orbitIndex: id,
    posX: null,
    posY: null,
    worlds: worlds(id, count),
    ...extra,
  };
}

const arranged = system(14, 'arranged', 4);
arranged.worlds[0] = { ...arranged.worlds[0], orbitRadius: 950, phaseOffset: 1.2 };
arranged.worlds[1] = { ...arranged.worlds[1], orbitRadius: 1400, phaseOffset: null };
arranged.worlds[2] = { ...arranged.worlds[2], orbitRadius: null, phaseOffset: 4.4 };

export const GALAXIES: Galaxy[] = [
  {
    id: 1,
    slug: 'lmy',
    displayName: 'LMY',
    skySeed: 'galaxy-1',
    universeX: null,
    universeY: null,
    systems: [
      system(11, 'world-ending', 12),
      system(12, 'glass', 25, { colorSeed: 'album-243' }),
      system(13, 'one', 1),
      arranged,
      system(15, 'empty', 0),
      system(16, 'placed', 3, { posX: 900, posY: 4100 }),
      system(17, 'seven', 7, { orbitIndex: 0 }),
    ],
  },
  {
    id: 2,
    slug: 'ana',
    displayName: 'Ana',
    skySeed: 'galaxy-2',
    universeX: 4180.5,
    universeY: 1311.25,
    systems: [system(21, 'full', 21), system(22, 'eight', 8), system(23, 'fifteen', 15)],
  },
  {
    id: 42,
    slug: 'no-seed',
    displayName: 'No Seed',
    skySeed: null,
    universeX: null,
    universeY: null,
    systems: Array.from({ length: 23 }, (_, i) => system(4200 + i, `small-${i}`, 1 + (i % 3))),
  },
  { id: 1000, slug: 'rim', displayName: 'Rim', skySeed: '', universeX: null, universeY: null, systems: [] },
];

function node(trackId: string, parent: string | null, day: number, extra: Partial<LineageNode> = {}): LineageNode {
  return {
    trackId,
    parentTrackId: parent,
    secondaryParentTrackId: null,
    lineageRootTrackId: null,
    remixDepth: 0,
    createdAt: `2026-01-${String(day).padStart(2, '0')}T00:00:00.000Z`,
    title: trackId,
    artist: 'LMY',
    ...extra,
  };
}

export const LINEAGE = {
  roots: ['sun1', 'sun2'],
  nodes: [
    node('sun1', null, 1),
    node('moon1a', 'sun1', 2),
    node('moon1b', 'sun1', 3),
    node('moon1a1', 'moon1a', 4),
    node('moon1a2', 'moon1a', 5),
    node('mix', 'moon1b', 6, { secondaryParentTrackId: 'moon1a1' }),
    node('sun2', null, 7),
    node('moon2a', 'sun2', 8),
    node('lost', 'gone', 9, { lineageRootTrackId: 'sun1', remixDepth: 3 }),
    node('alone', 'gone', 10),
    node('sun3', null, 11),
  ],
};
