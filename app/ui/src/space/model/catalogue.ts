// What Space shows, tier by tier (docs/SPEC.md 4.2): everyone's galaxies
// on a spiral, a galaxy's solar systems around its bio sun, a system's
// worlds on three rings of seven, and one world's player. The shapes
// follow the iPhone's `/api/v2` payloads, which the desktop app reads.

export type Medium = 'song' | 'film' | 'writing' | 'fashion';

// The facts a card and a label read: what a work is, never how it did.
export interface WorkFacts {
  medium: Medium;
  key: string | null; // "A minor"
  bpm: number | null;
  durationSeconds?: number | null;
  words?: number | null;
  photos?: number | null;
}

export interface World extends WorkFacts {
  id: number;
  trackId: string | null;
  title: string;
  orbitIndex: number;
  // The owner's arrangement, if any: null keeps the derived orbit.
  orbitRadius: number | null;
  phaseOffset: number | null;
}

export interface SolarSystem {
  id: number;
  slug: string;
  title: string;
  colorSeed: string | null;
  orbitIndex: number;
  // The owner's position in their galaxy, both or neither.
  posX: number | null;
  posY: number | null;
  worlds: World[];
}

export interface Galaxy {
  id: number;
  slug: string;
  displayName: string;
  skySeed: string | null;
  // Where the server put this galaxy (by id, then moved by drift).
  universeX: number | null;
  universeY: number | null;
  systems: SolarSystem[];
}

// "21 worlds to a system, the length of the first record."
export const WORLD_CAP = 21;
export const WORLD_CAP_REFUSAL = 'A solar system holds 21 worlds. Start another one.';

// Why a world can't take a seat in this system, or null if it can.
export function refuseSeat(system: SolarSystem): string | null {
  return system.worlds.length >= WORLD_CAP ? WORLD_CAP_REFUSAL : null;
}

export type Tier = 'universe' | 'galaxy' | 'system' | 'planet';

export type Place =
  | { tier: 'universe' }
  | { tier: 'galaxy'; galaxyId: number }
  | { tier: 'system'; galaxyId: number; systemId: number }
  | { tier: 'planet'; galaxyId: number; systemId: number; worldId: number };

// Esc: up one tier. The universe is the end of the road.
export function up(place: Place): Place {
  switch (place.tier) {
    case 'planet':
      return { tier: 'system', galaxyId: place.galaxyId, systemId: place.systemId };
    case 'system':
      return { tier: 'galaxy', galaxyId: place.galaxyId };
    default:
      return { tier: 'universe' };
  }
}

// The breadcrumb, "Everyone › LMY › World Ending", one name per crumb.
export function crumbs(place: Place, galaxies: Galaxy[]): string[] {
  const out = ['Everyone'];
  if (place.tier === 'universe') return out;
  const galaxy = galaxies.find((g) => g.id === place.galaxyId);
  if (!galaxy) return out;
  out.push(galaxy.displayName);
  if (place.tier === 'galaxy') return out;
  const system = galaxy.systems.find((s) => s.id === place.systemId);
  if (!system) return out;
  out.push(system.title);
  if (place.tier === 'system') return out;
  const world = system.worlds.find((w) => w.id === place.worldId);
  if (world) out.push(world.title);
  return out;
}
