// Hit testing in world space, against the layout frozen at pointer down
// (docs/SPEC.md 4.5). The renderer casts the pointer's ray from the camera;
// the model says which body it meets. Worlds ride tilted orbits, so a ray
// is tested against where a world is, not where its shadow falls.

import { sqrt } from '../../shared/dmath';
import type { Point3 } from './kepler';
import type { Point } from './orbits';

export interface Body<T> extends Point3 {
  id: T;
  radius: number;
}

export interface Ray {
  origin: Point3;
  dir: Point3;
}

// The body whose sphere (plus `slop`, in world units) the ray passes
// through, the nearest along the ray when it passes several.
export function hitRay<T>(bodies: Body<T>[], ray: Ray, slop: number): T | null {
  const { origin: o, dir: d } = ray;
  const dd = d.x * d.x + d.y * d.y + d.z * d.z;
  let best: T | null = null;
  let bestT = Infinity;
  for (const b of bodies) {
    const vx = b.x - o.x;
    const vy = b.y - o.y;
    const vz = b.z - o.z;
    const t = (vx * d.x + vy * d.y + vz * d.z) / dd;
    if (t < 0) continue; // behind the camera
    const qx = vx - d.x * t;
    const qy = vy - d.y * t;
    const qz = vz - d.z * t;
    if (sqrt(qx * qx + qy * qy + qz * qz) - b.radius > slop) continue;
    if (t < bestT) {
      bestT = t;
      best = b.id;
    }
  }
  return best;
}

// The body a pinch ended over, measured on the plane to each body's edge
// so a big world is easier to land on. Bounded, so a pinch in empty space
// doesn't fall into whatever is least far away.
export function pickNearest<T>(bodies: (Point & { id: T; radius: number })[], point: Point, tolerance = 420): T | null {
  let best: T | null = null;
  let bestReach = Infinity;
  for (const b of bodies) {
    const dx = b.x - point.x;
    const dy = b.y - point.y;
    const reach = Math.max(0, sqrt(dx * dx + dy * dy) - b.radius);
    if (reach < bestReach) {
      bestReach = reach;
      best = b.id;
    }
  }
  return bestReach <= tolerance ? best : null;
}
