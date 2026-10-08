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
  /** Its own colour, when the link has one (a world on mi-wwav.com does); otherwise one comes from the link. */
  colour?: string;
}

/** You: a place, a way you face, how fast you are drifting, and how fast you are turning. */
export interface Pilot {
  at: Vector3;
  facing: Quaternion;
  velocity: Vector3;
  /** Radians a second about your own up, right and forward. */
  spin: Vector3;
}

/** What the keys ask for, each from -1 to 1, and whether you are in a hurry. */
export interface Controls {
  forward: number;
  right: number;
  up: number;
  yaw: number;
  pitch: number;
  roll: number;
  fast: boolean;
}

export const STILL: Controls = { forward: 0, right: 0, up: 0, yaw: 0, pitch: 0, roll: 0, fast: false };

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

export function pilotAt(at: Vector3, toward: Vector3): Pilot {
  const facing = new Quaternion().setFromRotationMatrix(new Matrix4().lookAt(at, toward, new Vector3(0, 1, 0)));
  return { at: at.clone(), facing, velocity: new Vector3(), spin: new Vector3() };
}

export function forwardOf(pilot: Pilot): Vector3 {
  return new Vector3(0, 0, -1).applyQuaternion(pilot.facing);
}

const half = (FOV * Math.PI) / 360;

/** How far you turn for each point the sky is dragged, so the star you took hold of stays under the pointer. */
export function turnPerPoint(view: View): number {
  return (2 * Math.tan(half)) / view.height;
}

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

/** The nearest body, and how far its surface is; never less than nothing. */
function nearest(pilot: Pilot, bodies: readonly Body[]): { body: Body; clear: number } | null {
  let best: { body: Body; clear: number } | null = null;
  for (const body of bodies) {
    const clear = Math.max(0, pilot.at.distanceTo(body.at) - body.radius);
    if (!best || clear < best.clear) best = { body, clear };
  }
  return best;
}

/**
 * How fast the keys carry you. Far from everything you cross the sky in a
 * few seconds; near a body you slow, which is its gravity felt as care; and
 * right at one you still move at half its radius a second, so the last
 * stretch is never a crawl.
 */
export function cruise(pilot: Pilot, bodies: readonly Body[]): number {
  const near = nearest(pilot, bodies);
  if (!near) return 600;
  return Math.min(9000, Math.max(near.body.radius * 0.5, near.clear * 1.4));
}

const TURN = 1.3; // radians a second at full deflection
const EASE = 0.3; // seconds for your drift to follow the keys
const SPIN_EASE = 0.11; // and for your turning to
const HURRY = 4; // how much faster Shift carries you

/** One moment of free flight. Changes `pilot`. */
export function step(pilot: Pilot, controls: Controls, bodies: readonly Body[], view: View, dt: number): void {
  const spin = new Vector3(controls.yaw, controls.pitch, controls.roll).multiplyScalar(TURN);
  pilot.spin.lerp(spin, 1 - Math.exp(-dt / SPIN_EASE));
  if (pilot.spin.lengthSq() > 1e-10) {
    const turn = new Quaternion()
      .setFromAxisAngle(new Vector3(0, 1, 0), pilot.spin.x * dt)
      .multiply(new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), pilot.spin.y * dt))
      .multiply(new Quaternion().setFromAxisAngle(new Vector3(0, 0, 1), pilot.spin.z * dt));
    pilot.facing.multiply(turn).normalize();
  }
  const want = new Vector3(controls.right, controls.up, -controls.forward);
  if (want.lengthSq() > 1) want.normalize();
  want.applyQuaternion(pilot.facing).multiplyScalar(cruise(pilot, bodies) * (controls.fast ? HURRY : 1));
  pilot.velocity.lerp(want, 1 - Math.exp(-dt / EASE));
  pilot.at.addScaledVector(pilot.velocity, dt);
  // Flying forward at a body, its gravity turns you to face it.
  if (controls.forward > 0 && !controls.yaw && !controls.pitch) {
    const body = aim(pilot, bodies, view);
    // Enough, for a big body close by, to outdo the way a body off to one side slides further aside as you near it.
    if (body) bend(pilot, body, view, controls.forward * dt * 4);
  }
}

/** A body's pull on the way you face: stronger the bigger it is and the nearer you are. `amount` is how long, or how hard, you pushed toward it. */
function bend(pilot: Pilot, body: Body, view: View, amount: number): void {
  const dock = dockDepth(body, view);
  const gap = Math.max(0, pilot.at.distanceTo(body.at) - dock);
  const near = Math.min(1, Math.max(0, 1 - gap / (10 * dock)));
  const pull = Math.min(1, near * (0.35 + 0.12 * body.magnitude) * amount);
  if (pull > 0) pilot.facing.slerp(facingTo(pilot.at, pilot.facing, body.at), pull);
}

/** Looking around by dragging: `dx` and `dy` in radians. */
export function look(pilot: Pilot, dx: number, dy: number): void {
  const turn = new Quaternion()
    .setFromAxisAngle(new Vector3(0, 1, 0), dx)
    .multiply(new Quaternion().setFromAxisAngle(new Vector3(1, 0, 0), dy));
  pilot.facing.multiply(turn).normalize();
}

/** The facing that looks from `from` at `to`, keeping the up of `facing` as near as it can. */
function facingTo(from: Vector3, facing: Quaternion, to: Vector3): Quaternion {
  const up = new Vector3(0, 1, 0).applyQuaternion(facing);
  return new Quaternion().setFromRotationMatrix(new Matrix4().lookAt(from, to, up));
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
  if (amount > 0) bend(pilot, body, view, amount / 90);
  pilot.velocity.set(0, 0, 0);
}

/** Scrolling you have done that hasn't moved you yet. A wheel turns in steps; you shouldn't. */
export interface Glide {
  push: number;
}

const GLIDE = 0.085; // seconds for a push to be mostly spent

/** Spends some of what was scrolled, a frame's worth. Changes `pilot` and `glide`. */
export function coast(pilot: Pilot, bodies: readonly Body[], view: View, glide: Glide, dt: number): void {
  if (Math.abs(glide.push) < 0.01) {
    glide.push = 0;
    return;
  }
  const now = glide.push * (1 - Math.exp(-dt / GLIDE));
  glide.push -= now;
  nudge(pilot, bodies, view, now);
}

/** A flight you are being taken on: to a body, until its page covers so much of the view. */
export interface Course {
  id: string;
  /** How much of the view's height the page covers at the end: 1 is in it. */
  cover: number;
  /** Whether you go into the page on arriving. */
  enter: boolean;
  /** Where you were when it began. */
  from: { at: Vector3; facing: Quaternion };
  /** The way from the body to where you were, which is the line you fly along. */
  along: Vector3;
  elapsed: number;
  seconds: number;
}

const smooth = (t: number) => {
  const x = Math.min(1, Math.max(0, t));
  return x * x * x * (x * (x * 6 - 15) + 10);
};

/** Plans a flight to a body. Longer for a longer way, but a hundred times the distance is not a hundred times the wait. */
export function plot(pilot: Pilot, body: Body, view: View, cover: number, enter: boolean): Course {
  const away = pilot.at.clone().sub(body.at);
  const distance = away.length();
  const along = distance > 1e-9 ? away.divideScalar(distance) : forwardOf(pilot).negate();
  const end = dockDepth(body, view) / cover;
  const stretch = Math.abs(Math.log(Math.max(distance, 1e-9) / end));
  return {
    id: body.id,
    cover,
    enter,
    from: { at: pilot.at.clone(), facing: pilot.facing.clone() },
    along,
    elapsed: 0,
    seconds: Math.min(2.6, Math.max(0.65, 0.55 + 0.42 * stretch)),
  };
}

/**
 * A moment of that flight. You turn to face the body first and then close
 * the distance by the same share each moment, easing off at both ends, so a
 * far body rushes up and the last of the way is slow. Changes `pilot` and
 * `course`; answers true once you are there.
 */
export function follow(pilot: Pilot, course: Course, body: Body, view: View, dt: number): boolean {
  course.elapsed += dt;
  const t = Math.min(1, course.elapsed / course.seconds);
  const start = Math.max(course.from.at.distanceTo(body.at), 1e-9);
  const end = dockDepth(body, view) / course.cover;
  const distance = Math.exp(Math.log(start) + (Math.log(end) - Math.log(start)) * smooth(t));
  pilot.at.copy(body.at).addScaledVector(course.along, distance);
  const facing = facingTo(pilot.at, course.from.facing, body.at);
  pilot.facing.copy(course.from.facing).slerp(facing, smooth(t / 0.6)).normalize();
  pilot.velocity.set(0, 0, 0);
  pilot.spin.set(0, 0, 0);
  return t >= 1;
}

/** A page is live from this much of the view's height, and stays so until it falls under the second. */
export const LIVE_FROM = 0.2;
export const LIVE_UNTIL = 0.14;
/** Flying free, you are taken the rest of the way in once the page covers this much and is near the middle. */
export const PULLED_IN = 0.94;
