// The proving ground (docs/SPACE.md 10, step 1): six real sites and four
// links on them, written by hand, so there is a sky to fly through and real
// pages to go into before search fills it. Home (the founder's galaxy) and
// search replace this. Magnitudes are the hand-written table of 3.2, rough
// on purpose.

import { Vector3 } from 'three';
import { type Body, direction, radiusOf } from './model';

interface Entry {
  id: string;
  url: string;
  name: string;
  magnitude: number;
  parent: string | null;
}

const ENTRIES: Entry[] = [
  { id: 'youtube', url: 'https://www.youtube.com/', name: 'YouTube', magnitude: 10, parent: null },
  { id: 'spotify', url: 'https://open.spotify.com/', name: 'Spotify', magnitude: 8, parent: null },
  { id: 'apple-music', url: 'https://music.apple.com/', name: 'Apple Music', magnitude: 8, parent: null },
  { id: 'wikipedia', url: 'https://en.wikipedia.org/', name: 'Wikipedia', magnitude: 8, parent: null },
  { id: 'duckduckgo', url: 'https://duckduckgo.com/', name: 'DuckDuckGo', magnitude: 9, parent: null },
  { id: 'x', url: 'https://x.com/', name: 'X', magnitude: 9, parent: null },
  {
    id: 'youtube-zoo',
    url: 'https://www.youtube.com/watch?v=jNQXAC9IVRw',
    name: 'Me at the zoo',
    magnitude: 1,
    parent: 'youtube',
  },
  {
    id: 'spotify-song',
    url: 'https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT',
    name: 'Never Gonna Give You Up',
    magnitude: 1,
    parent: 'spotify',
  },
  {
    id: 'apple-song',
    url: 'https://music.apple.com/us/album/never-gonna-give-you-up-escape-to-new-york-mix/1612648318?i=1612648440',
    name: 'Never Gonna Give You Up (Escape to New York Mix)',
    magnitude: 1,
    parent: 'apple-music',
  },
  { id: 'wikipedia-saturn', url: 'https://en.wikipedia.org/wiki/Saturn', name: 'Saturn', magnitude: 3, parent: 'wikipedia' },
];

/** How far from you the sites stand, and how far out a link orbits, in its parent's radii. */
const SHELL = 5200;
const ORBIT = 3.4;

/** The proving ground's bodies, each where its link's hash puts it: the same on every machine. */
export function provingGround(): Body[] {
  const bodies = new Map<string, Body>();
  for (const e of ENTRIES) {
    const radius = radiusOf(e.magnitude);
    const parent = e.parent ? bodies.get(e.parent) : undefined;
    const at = parent
      ? parent.at.clone().addScaledVector(direction(`${parent.url} ${e.url}`), parent.radius * ORBIT)
      : direction(e.url).multiplyScalar(SHELL * (0.75 + 0.5 * fraction(e.url)));
    bodies.set(e.id, { ...e, at, radius });
  }
  return [...bodies.values()];
}

function fraction(key: string): number {
  // A second, independent number for the link: how far out it stands.
  return direction(`far ${key}`).y * 0.5 + 0.5;
}

/** Where you start: at the middle, facing the biggest body. */
export function start(bodies: readonly Body[]): { at: Vector3; toward: Vector3 } {
  const biggest = [...bodies].sort((a, b) => b.magnitude - a.magnitude)[0];
  return { at: new Vector3(0, 0, 0), toward: biggest ? biggest.at.clone() : new Vector3(0, 0, -1) };
}
