import { describe, expect, test } from 'vitest';
import { hitRay, pickNearest } from './hit';

const bodies = [
  { id: 'a', x: 1000, y: 1000, z: 0, radius: 100 },
  { id: 'b', x: 1000, y: 1600, z: 0, radius: 100 },
  { id: 'raised', x: 2000, y: 1000, z: 150, radius: 60 },
];

// Straight down from above, as a top-down camera casts.
const down = (x: number, y: number) => ({ origin: { x, y, z: 5000 }, dir: { x: 0, y: 0, z: -1 } });

describe('hit testing in world space', () => {
  test('hits the body the ray passes through', () => {
    expect(hitRay(bodies, down(1050, 1020), 0)).toBe('a');
    expect(hitRay(bodies, down(1000, 1690), 0)).toBe('b');
  });

  test('misses beyond the radius plus the slop', () => {
    expect(hitRay(bodies, down(1000, 1350), 0)).toBeNull();
    expect(hitRay(bodies, down(1000, 1112), 14)).toBe('a');
    expect(hitRay(bodies, down(1000, 1115), 14)).toBeNull();
  });

  test('takes the nearer of two bodies on one ray', () => {
    const along = { origin: { x: 1000, y: 0, z: 0 }, dir: { x: 0, y: 1, z: 0 } };
    expect(hitRay(bodies, along, 0)).toBe('a');
    const back = { origin: { x: 1000, y: 3000, z: 0 }, dir: { x: 0, y: -2, z: 0 } };
    expect(hitRay(bodies, back, 0)).toBe('b');
  });

  test('hits a raised world where it is drawn, not where its shadow falls', () => {
    const slanted = { origin: { x: 2000, y: 1000 + 1000, z: 150 + 1000 }, dir: { x: 0, y: -1, z: -1 } };
    expect(hitRay(bodies, slanted, 0)).toBe('raised');
  });

  test('ignores bodies behind the camera', () => {
    const away = { origin: { x: 1000, y: 1300, z: 0 }, dir: { x: 0, y: 1, z: 0 } };
    expect(hitRay(bodies, away, 0)).toBe('b');
  });
});

describe('the body under a pinch', () => {
  test('is the nearest by edge, within 420', () => {
    expect(pickNearest(bodies, { x: 1000, y: 1250 })).toBe('a');
    expect(pickNearest(bodies, { x: 1000, y: 1350 })).toBe('b');
    expect(pickNearest(bodies, { x: 4000, y: 4000 })).toBeNull();
  });
});
