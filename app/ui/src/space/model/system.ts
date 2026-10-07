// One solar system at one moment of sky time: its worlds on their orbits,
// and the gathered world outside them when there are more than twenty-one.

import { atan2, sqrt } from '../../shared/dmath';
import type { SolarSystem, World } from './catalogue';
import { orbitForSeat, placeOrbit, separationOffsets, type Orbit, type Point3 } from './kepler';
import {
  CENTER,
  OVERFLOW_SEAT,
  overflowRadius,
  splitWorlds,
  systemSeed,
  worldRadius,
  type Point,
} from './orbits';

export interface SeatedWorld {
  world: World;
  seat: number;
  orbit: Orbit;
  radius: number;
}

export interface SystemOrbits {
  shown: SeatedWorld[];
  gathered: World[];
  overflow: { orbit: Orbit; radius: number } | null;
}

// A world's orbit is named by its track id, or "planet-<id>" when it has
// none (films and galleries), as on iOS.
export function orbitId(world: World): string {
  return world.trackId ?? `planet-${world.id}`;
}

// The fixed shape of every orbit in a system: worked out once per system,
// then placed at any time.
export function systemOrbits(system: SolarSystem): SystemOrbits {
  const seed = systemSeed(system);
  const { shown, gathered } = splitWorlds(system.worlds);
  const count = shown.length;
  return {
    shown: shown.map((world, seat) => ({
      world,
      seat,
      radius: worldRadius(world.medium, seat),
      orbit: orbitForSeat({
        seat,
        count,
        id: orbitId(world),
        seed,
        radius: world.orbitRadius,
        phase: world.phaseOffset,
      }),
    })),
    gathered,
    overflow:
      gathered.length === 0
        ? null
        : {
            radius: overflowRadius(),
            orbit: orbitForSeat({ seat: OVERFLOW_SEAT, count: OVERFLOW_SEAT + 1, id: `overflow-${system.id}`, seed }),
          },
  };
}

export interface PlacedWorld extends Point3 {
  id: number;
  seat: number;
  radius: number;
}

export interface PlacedOverflow extends Point3 {
  worlds: World[];
  radius: number;
}

export interface PlacedSystem {
  worlds: PlacedWorld[];
  overflow: PlacedOverflow | null;
}

// Every world at sky time t, turned by the hand-turned rotation. While the
// owner arranges (always at t = 0) the anti-collision nudge is skipped:
// they are stating positions, and a nudge would argue with them.
export function placeSystem(orbits: SystemOrbits, t: number, turn = 0, arranging = false): PlacedSystem {
  const bodies = orbits.shown.map((s) => ({ orbit: s.orbit, radius: s.radius }));
  if (orbits.overflow) bodies.push(orbits.overflow);
  let points = bodies.map((b) => placeOrbit(b.orbit, t, turn));
  if (!arranging) {
    const offsets = separationOffsets(
      points,
      bodies.map((b) => b.radius),
    );
    points = points.map((p, i) => (offsets[i] === 0 ? p : placeOrbit(bodies[i].orbit, t, turn + offsets[i])));
  }
  const worlds = orbits.shown.map((s, i) => ({ id: s.world.id, seat: s.seat, radius: s.radius, ...points[i] }));
  const overflow = orbits.overflow
    ? { worlds: orbits.gathered, radius: orbits.overflow.radius, ...points[points.length - 1] }
    : null;
  return { worlds, overflow };
}

// Where an owner dropped a world, as the orbit the server stores: the
// circle through that point (400–1800, the server's limits) and the angle
// on it, in [0, 2π).
export function arrangedOrbit(point: Point): { orbitRadius: number; phaseOffset: number } {
  const dx = point.x - CENTER.x;
  const dy = point.y - CENTER.y;
  const radius = Math.min(Math.max(sqrt(dx * dx + dy * dy), 400), 1800);
  let phase = atan2(dy, dx);
  if (phase < 0) phase += 2 * Math.PI;
  return { orbitRadius: radius, phaseOffset: phase };
}

// A drag at the system tier turns the system about its sun, as on iOS: the
// turn grows by the angle the pointer swept round the centre.
export function turnBy(turn: number, from: Point, to: Point): number {
  return turn + atan2(to.y - CENTER.y, to.x - CENTER.x) - atan2(from.y - CENTER.y, from.x - CENTER.x);
}
