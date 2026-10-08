// Flying through Space in first person (docs/SPACE.md 1, 3, 11): where you
// are, how keys and scrolling move you, and where a body's page sits on the
// screen. Arithmetic only, no drawing, so every rule here is tested without
// a graphics card.
//
// A body is a sphere of glass with its page at its core. The page is a flat
// rectangle that always faces you, as wide as fits inside the sphere, so
// from anywhere it is a plain rectangle on the screen: far off it is a few
// points across, and it grows as you come in until it fills the view. That
// rectangle is where the live web view is laid (space_pages.rs).

import { Matrix4, Quaternion, Vector3 } from 'three';
import { fnv1a32 } from '../../shared/dmath/fnv';

/** The camera's vertical field of view, in degrees. */
export const FOV = 60;

export interface Body {
  id: string;
  /** The link this body is. */
  url: string;
  name: string;
  /** The power of ten of the links it connects (docs/SPACE.md 3). Also its level of gravity. */
  magnitude: number;
  /** The body it is shown orbiting here, or null. */
  parent: string | null;
  at: Vector3;
  radius: number;
}

/** You: a place, a way you face, and how fast you are drifting. */
export interface Pilot {
  at: Vector3;
  facing: Quaternion;
  velocity: Vector3;
}

/** What the keys ask for, each from -1 to 1. */
export interface Controls {
  forward: number;
  right: number;
  up: number;
  yaw: number;
  pitch: number;
  roll: number;
}

export const STILL: Controls = { forward: 0, right: 0, up: 0, yaw: 0, pitch: 0, roll: 0 };

export interface View {
  width: number;
  height: number;
}

/** A rectangle on the screen, in points from the view's top left. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A body's size from its magnitude: each power of ten is a step bigger, never ten times. */
export function radiusOf(magnitude: number): number {
  return 30 * Math.pow(1.28, magnitude);
}

/**
 * A number in [0, 1) for a link and a salt, the same on every machine. FNV
 * alone keeps two links that differ in their last letter side by side, so
 * its answer is stirred (Murmur3's finalizer) until every bit of the link
 * reaches every bit of the number.
 */
export function chance(key: string, salt: string): number {
  let z = fnv1a32(`${salt}|${key}`);
  z = Math.imul(z ^ (z >>> 16), 0x85ebca6b);
  z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35);
  return ((z ^ (z >>> 16)) >>> 0) / 4294967296;
}

/** A direction for a link, anywhere on the sphere: Space is 3D, so nothing is laid on a plane. */
export function direction(key: string): Vector3 {
  const u = chance(key, 'u') * 2 - 1;
  const theta = chance(key, 't') * Math.PI * 2;
  const s = Math.sqrt(1 - u * u);
  return new Vector3(s * Math.cos(theta), u, s * Math.sin(theta));
}

export function forwardOf(pilot: Pilot): Vector3 {
  return new Vector3(0, 0, -1).applyQuaternion(pilot.facing);
}

const half = (FOV * Math.PI) / 360;

/** The page at a body's core: the widest rectangle of the view's shape that fits inside the sphere. */
export function faceOf(body: Body, view: View): { width: number; height: number } {
  const aspect = view.width / Math.max(1, view.height);
  const height = (2 * body.radius) / Math.sqrt(1 + aspect * aspect);
  return { width: height * aspect, height };
}

/** How far in front of you a body's core is when its page exactly fills the view. Inside the sphere: you are in the planet. */
export function dockDepth(body: Body, view: View): number {
  return faceOf(body, view).height / (2 * Math.tan(half));
}

/** The body's place in your own frame: x right, y up, depth ahead. */
function seen(pilot: Pilot, body: Body): { x: number; y: number; depth: number } {
  const p = body.at.clone().sub(pilot.at).applyQuaternion(pilot.facing.clone().invert());
  return { x: p.x, y: p.y, depth: -p.z };
}

export interface Sighting {
  /** Where the page is on the screen. */
  rect: Rect;
  /** How much of the view's height the page covers: 1 is filled. */
  cover: number;
  /** How far ahead the body's core is. Zero or less is behind you. */
  depth: number;
  /** The body's radius on the screen, in points. */
  radius: number;
  /** The angle between the way you face and the body, in radians. */
  off: number;
}

/** Where a body and its page are on the screen, or null when it is behind you. */
export function sight(pilot: Pilot, body: Body, view: View): Sighting | null {
  const p = seen(pilot, body);
  if (p.depth <= 1e-6) return null;
  const focal = view.height / 2 / Math.tan(half);
  const face = faceOf(body, view);
  const height = (face.height * focal) / p.depth;
  const width = (face.width * focal) / p.depth;
  const cx = view.width / 2 + (p.x * focal) / p.depth;
  const cy = view.height / 2 - (p.y * focal) / p.depth;
  return {
    rect: { x: cx - width / 2, y: cy - height / 2, width, height },
    cover: height / view.height,
    depth: p.depth,
    radius: (body.radius * focal) / p.depth,
    off: Math.atan2(Math.hypot(p.x, p.y), p.depth),
  };
}

/** Whether a rectangle is wholly inside the view. A live page can't be cut off at the edge, so it is shown only then. */
export function inside(rect: Rect, view: View, slack = 0.5): boolean {
  return (
    rect.x >= -slack &&
    rect.y >= -slack &&
    rect.x + rect.width <= view.width + slack &&
    rect.y + rect.height <= view.height + slack
  );
}

/** The body you are looking at: the one nearest the middle of the view, a big one winning a near tie. */
export function aim(pilot: Pilot, bodies: readonly Body[], view: View): Body | null {
  let best: Body | null = null;
  let score = 0.6; // about 35°: beyond that you are not looking at it
  for (const body of bodies) {
    const s = sight(pilot, body, view);
    if (!s) continue;
    const size = Math.atan2(body.radius, s.depth);
    const here = s.off - size * 0.8;
    if (here < score) {
      best = body;
      score = here;
    }
  }
  return best;
}

/** The distance from you to the nearest body's surface; never less than a step. */
export function clearance(pilot: Pilot, bodies: readonly Body[]): number {
  let least = Infinity;
  for (const body of bodies) least = Math.min(least, pilot.at.distanceTo(body.at) - body.radius);
  return Math.max(least, 0);
}

/** How fast the keys carry you: slower the nearer you are to something, so a body's gravity is felt as care. */
export function cruise(pilot: Pilot, bodies: readonly Body[]): number {
  if (bodies.length === 0) return 400;
  return Math.min(6000, Math.max(8, clearance(pilot, bodies) * 0.9));
}

const TURN = 1.1; // radians a second at full deflection
const EASE = 0.35; // seconds for your drift to follow the keys

/** One moment of free flight. Changes `pilot`. */
export function step(pilot: Pilot, controls: Controls, bodies: readonly Body[], dt: number): void {
  if (controls.yaw || controls.pitch || controls.roll) {
    const turn = new Quaternion()
      .setFromAxisAngle(new Vector3(0, 1, 0), controls.yaw * TURN * dt)
      .multiply(new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), controls.pitch * TURN * dt))
      .multiply(new Quaternion().setFromAxisAngle(new Vector3(0, 0, 1), controls.roll * TURN * dt));
    pilot.facing.multiply(turn).normalize();
  }
  const want = new Vector3(controls.right, controls.up, -controls.forward);
  if (want.lengthSq() > 1) want.normalize();
  want.applyQuaternion(pilot.facing).multiplyScalar(cruise(pilot, bodies));
  pilot.velocity.lerp(want, 1 - Math.exp(-dt / EASE));
  pilot.at.addScaledVector(pilot.velocity, dt);
}

/** Looking around by dragging: `dx` and `dy` in radians. */
export function look(pilot: Pilot, dx: number, dy: number): void {
  const turn = new Quaternion()
    .setFromAxisAngle(new Vector3(0, 1, 0), dx)
    .multiply(new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), dy));
  pilot.facing.multiply(turn).normalize();
}

/** The facing that looks from `from` at `to`, keeping your own up as near as it can. */
function facingTo(pilot: Pilot, to: Vector3): Quaternion {
  const up = new Vector3(0, 1, 0).applyQuaternion(pilot.facing);
  return new Quaternion().setFromRotationMatrix(new Matrix4().lookAt(pilot.at, to, up));
}

const NUDGE = 0.0022; // how much of the way one point of scrolling takes you

/**
 * Scrolling or pinching: `amount` in points, forward when positive. Toward
 * the body you are looking at, each point takes a share of what is left, so
 * you close in quickly from far off and gently at the end, and the body's
 * gravity turns you to face it as you come. With nothing in view you simply
 * move.
 */
export function nudge(pilot: Pilot, bodies: readonly Body[], view: View, amount: number): void {
  const body = aim(pilot, bodies, view);
  if (!body) {
    pilot.at.addScaledVector(forwardOf(pilot), amount * cruise(pilot, bodies) * 0.004);
    return;
  }
  const to = body.at.clone().sub(pilot.at);
  const distance = to.length();
  const dock = dockDepth(body, view);
  const gap = Math.max(distance - dock, 0);
  // Backing away from the very middle still has to get you out.
  const from = amount > 0 ? gap : Math.max(gap, dock * 0.02);
  const next = from * Math.exp(-NUDGE * amount);
  if (distance > 1e-9) pilot.at.addScaledVector(to.divideScalar(distance), gap - next);
  if (amount > 0) {
    const near = Math.min(1, Math.max(0, 1 - next / (8 * dock)));
    const pull = Math.min(1, near * (0.05 + 0.03 * body.magnitude) * (amount / 30));
    pilot.facing.slerp(facingTo(pilot, body.at), Math.min(1, Math.max(0, pull)));
  }
  pilot.velocity.set(0, 0, 0);
}

/**
 * Being flown to a body until its page covers `cover` of the view (1 is in
 * it). Changes `pilot`, and answers true once you are there.
 */
export function approach(pilot: Pilot, body: Body, view: View, cover: number, dt: number): boolean {
  const depth = dockDepth(body, view) / cover;
  const away = pilot.at.clone().sub(body.at);
  if (away.lengthSq() < 1e-12) away.copy(forwardOf(pilot)).negate();
  const target = body.at.clone().addScaledVector(away.normalize(), depth);
  const facing = facingTo({ ...pilot, at: target }, body.at);
  const k = 1 - Math.exp(-dt * 3.4);
  pilot.at.lerp(target, k);
  pilot.facing.slerp(facing, k).normalize();
  pilot.velocity.set(0, 0, 0);
  const there = pilot.at.distanceTo(target) < depth * 0.004 && pilot.facing.angleTo(facing) < 0.004;
  if (there) {
    pilot.at.copy(target);
    pilot.facing.copy(facing);
  }
  return there;
}

/** A page is live from this much of the view's height, and stays so until it falls under the second. */
export const LIVE_FROM = 0.2;
export const LIVE_UNTIL = 0.14;
/** Flying free, you are taken the rest of the way in once the page covers this much and is near the middle. */
export const PULLED_IN = 0.94;
