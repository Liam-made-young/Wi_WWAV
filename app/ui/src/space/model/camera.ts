// The camera (docs/SPEC.md 4.4). It looks at a target point on the plane
// from a height and an angle; k is a reference distance over the camera's
// distance, which at the target is the scale from world units to points.
// The renderer turns this into a three.js camera and unprojects pointers;
// everything here is in world units.

import type { Box, Point } from './orbits';

export type Scene = 'universe' | 'galaxy' | 'system';

export interface SceneLimits {
  enterK: number | null; // zoom in past this and the body underneath opens
  leaveK: number | null; // zoom out past this and the parent tier takes over
  minK: number;
  maxK: number;
}

// Enter and leave are the spec's (web v4's); min and max are web v4's,
// except the universe's floor, lowered so 2,000 galaxies spread by id
// (about 120,000 units across) still fit the window.
export const SCENES: Record<Scene, SceneLimits> = {
  system: { enterK: 1.15, leaveK: 0.075, minK: 0.05, maxK: 2.2 },
  galaxy: { enterK: 0.62, leaveK: 0.07, minK: 0.05, maxK: 1.2 },
  universe: { enterK: 0.8, leaveK: null, minK: 0.004, maxK: 1.4 },
};

export interface Camera {
  x: number; // the target on the plane
  y: number;
  k: number;
  yaw: number; // radians round the vertical; 0 looks from the +y side
  pitch: number; // radians above the plane
}

export const ZOOM_STEP = 1.55;
export const FIT = 0.8;
// ⌥-drag orbits the camera with the portfolio hub's numbers.
export const DEFAULT_PITCH = 0.62;
const PITCH_MIN = 0.35;
const PITCH_MAX = 1.15;
const YAW_PER_PT = 0.006;
const PITCH_PER_PT = 0.004;

function clampK(scene: Scene, k: number): number {
  const { minK, maxK } = SCENES[scene];
  return Math.min(maxK, Math.max(minK, k));
}

// Zoom to k2 about a world point (the one under the pointer), which stays
// where it is on screen: the target moves toward it as k grows.
export function zoomAbout(cam: Camera, scene: Scene, point: Point, k2: number): Camera {
  const k = clampK(scene, k2);
  const keep = cam.k / k;
  return { ...cam, k, x: point.x + (cam.x - point.x) * keep, y: point.y + (cam.y - point.y) * keep };
}

// The − and + buttons and ⌘− / ⌘+: whole steps of ×1.55, about the centre.
export function zoomStep(cam: Camera, scene: Scene, steps: number, about: Point = cam): Camera {
  return zoomAbout(cam, scene, about, cam.k * ZOOM_STEP ** steps);
}

// A drag keeps the world point it grabbed under the pointer.
export function panBy(cam: Camera, grabbed: Point, under: Point): Camera {
  return { ...cam, x: cam.x + grabbed.x - under.x, y: cam.y + grabbed.y - under.y };
}

export function orbitBy(cam: Camera, dx: number, dy: number): Camera {
  const pitch = Math.min(PITCH_MAX, Math.max(PITCH_MIN, cam.pitch + dy * PITCH_PER_PT));
  return { ...cam, yaw: cam.yaw + dx * YAW_PER_PT, pitch };
}

// Frame a box at 80% of the view, keeping the camera's angle.
export function fit(box: Box, view: { w: number; h: number }, scene: Scene, cam: Camera, fill = FIT): Camera {
  const k = clampK(scene, fill * Math.min(view.w / box.w, view.h / box.h));
  return { ...cam, k, x: box.x + box.w / 2, y: box.y + box.h / 2 };
}

// Judged only when a gesture ends.
export function judgeTier(scene: Scene, k: number): 'enter' | 'leave' | null {
  const { enterK, leaveK } = SCENES[scene];
  if (enterK !== null && k >= enterK) return 'enter';
  if (leaveK !== null && k <= leaveK) return 'leave';
  return null;
}

// When does a zoom gesture end? A tier changes only then, which "keeps a
// pinch from teleporting mid-gesture": when the last finger lifts after a
// pinch, 180 ms after the last wheel event (wheels have no end event), at
// a trackpad pinch's own end, or at once for a button. A one-finger pan
// never changes tier.
export const WHEEL_END_MS = 180;

export interface Gesture {
  fingers: number;
  pinched: boolean;
  lastWheel: number | null;
}

export type GestureInput =
  | { type: 'down' }
  | { type: 'up' }
  | { type: 'pinch' }
  | { type: 'pinchEnd' }
  | { type: 'button' }
  | { type: 'wheel'; time: number }
  | { type: 'tick'; time: number };

export function startGesture(): Gesture {
  return { fingers: 0, pinched: false, lastWheel: null };
}

export function endsGesture(g: Gesture, input: GestureInput): { gesture: Gesture; ended: boolean } {
  switch (input.type) {
    case 'down':
      return { gesture: { ...g, fingers: g.fingers + 1 }, ended: false };
    case 'up': {
      const fingers = Math.max(0, g.fingers - 1);
      if (fingers > 0) return { gesture: { ...g, fingers }, ended: false };
      return { gesture: { ...g, fingers, pinched: false }, ended: g.pinched };
    }
    case 'pinch':
      return { gesture: { ...g, pinched: true }, ended: false };
    case 'wheel':
      return { gesture: { ...g, lastWheel: input.time }, ended: false };
    case 'tick':
      if (g.lastWheel === null || input.time - g.lastWheel < WHEEL_END_MS) return { gesture: g, ended: false };
      return { gesture: { ...g, lastWheel: null }, ended: true };
    case 'pinchEnd':
    case 'button':
      return { gesture: g, ended: true };
  }
}
