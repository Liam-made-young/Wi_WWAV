// Home (docs/SPACE.md 1): the founder's galaxy from mi-wwav.com, laid out in
// the sky. The core reads it as links (`space.home`); this puts each one
// somewhere. A solar system stands where its maker put it on the galaxy's
// own map, lifted off that map by its link's hash, and its worlds go round
// it on the orbits the server keeps, in a plane tilted by the system's hash:
// "3D space, not 2D space".
//
// Sizes follow the one rule (3.4): a body is as big as the links it
// connects, by powers of ten. A world connects none, a solar system of 24
// connects about ten. So home is small beside YouTube, which is the point.

import { Vector3 } from 'three';
import { type Body, chance, direction, radiusOf } from './model';

export interface HomeWorld {
  url: string;
  name: string;
  kind: string;
  order: number | null;
  radius: number | null;
  phase: number | null;
  palette: { base?: string } | null;
}

export interface HomeSystem {
  url: string;
  name: string;
  x: number | null;
  y: number | null;
  worlds: HomeWorld[];
}

export interface Home {
  slug: string;
  url: string;
  name: string;
  systems: HomeSystem[];
}

/** The galaxy's own map is 5,000 across, and so is a solar system's; in one sky the systems have to stand further apart than their worlds reach. */
const MAP = 5000;
const SYSTEMS_APART = 3.2;
const ORBITS = 0.22;
const LIFT = 700;
/** How far a system's orbits lean from the galaxy's map: 0 is flat on it. */
const LEAN = 0.4;

const magnitudeOf = (links: number) => (links < 1 ? 0 : Math.floor(Math.log10(links)));

/** Two directions square to `normal` and to each other: the plane worlds go round in. */
function plane(normal: Vector3): [Vector3, Vector3] {
  const aside = Math.abs(normal.y) < 0.9 ? new Vector3(0, 1, 0) : new Vector3(1, 0, 0);
  const u = new Vector3().crossVectors(normal, aside).normalize();
  return [u, new Vector3().crossVectors(normal, u).normalize()];
}

/** Home's bodies: the galaxy's sun at the middle, its solar systems about it, their worlds about them. */
export function homeBodies(home: Home): Body[] {
  const bodies: Body[] = [];
  const links = home.systems.reduce((n, s) => n + 1 + s.worlds.length, 0);
  const sun: Body = {
    id: home.url,
    url: home.url,
    name: home.name,
    magnitude: magnitudeOf(links),
    parent: null,
    at: new Vector3(0, 0, 0),
    radius: radiusOf(magnitudeOf(links)),
    colour: '#f0a23c',
  };
  bodies.push(sun);

  for (const system of home.systems) {
    const magnitude = magnitudeOf(system.worlds.length);
    const fallback = direction(system.url).multiplyScalar(MAP * 0.2);
    const x = system.x === null ? fallback.x : system.x - MAP / 2;
    const z = system.y === null ? fallback.z : system.y - MAP / 2;
    const at = new Vector3(x * SYSTEMS_APART, (chance(system.url, 'lift') * 2 - 1) * LIFT, z * SYSTEMS_APART);
    const centre: Body = {
      id: system.url,
      url: system.url,
      name: system.name,
      magnitude,
      parent: sun.id,
      at,
      radius: radiusOf(magnitude),
      colour: '#ffd89a',
    };
    bodies.push(centre);

    const normal = new Vector3(0, 1, 0).lerp(direction(`${system.url} plane`), LEAN).normalize();
    const [u, v] = plane(normal);
    const size = radiusOf(0);
    const placed: Vector3[] = [];
    const worlds = [...system.worlds].sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
    worlds.forEach((world, index) => {
      // A world the server has no orbit for takes the next ring out, at a place of its link's own.
      const reach = (world.radius ?? 700 + 110 * index) * ORBITS + centre.radius;
      let phase = world.phase ?? chance(world.url, 'phase') * Math.PI * 2;
      let place = new Vector3();
      // Two worlds the server put on top of each other are moved apart along the orbit.
      for (let tries = 0; tries < 80; tries++) {
        place = at
          .clone()
          .addScaledVector(u, Math.cos(phase) * reach)
          .addScaledVector(v, Math.sin(phase) * reach);
        if (placed.every((p) => p.distanceTo(place) > size * 2.6)) break;
        phase += 0.14;
      }
      placed.push(place);
      bodies.push({
        id: world.url,
        url: world.url,
        name: world.name,
        magnitude: 0,
        parent: centre.id,
        at: place,
        radius: size,
        colour: world.palette?.base,
      });
    });
  }
  return bodies;
}

/** Where you start at home: above and back from the galaxy, far enough to see all of it, facing its sun. */
export function homeStart(bodies: readonly Body[]): { at: Vector3; toward: Vector3 } {
  const reach = bodies.reduce((far, b) => Math.max(far, b.at.length() + b.radius), 600);
  return { at: new Vector3(0, reach * 0.55, reach * 1.25), toward: new Vector3(0, 0, 0) };
}
