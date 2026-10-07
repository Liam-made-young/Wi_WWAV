// What moves the sky, and how much (docs/SPEC.md 4.4, 8.6). Motion comes
// from sound: the sky's clock runs on the audio engine's clock and only
// while something plays, so a silent sky holds still by construction.

import { cos, sin, sqrt } from '../../shared/dmath';
import type { Point } from './orbits';

// Pointer down stops the clock at once; resting on a body eases it to a
// stop over 240 ms, so every click hits a frozen layout.
export type Touch = 'none' | 'down' | 'resting';

export interface SkyReading {
  host: number; // the engine clock's host time, in seconds
  playing: boolean; // the engine's transport is playing
  touch: Touch;
}

export interface Sky {
  t: number; // sky seconds: what Kepler motion is a function of
  speed: number; // 0…1, eased by touch
  host: number | null;
  playing: boolean;
}

export const STILL_SECONDS = 0.24;

export function startSky(): Sky {
  return { t: 0, speed: 1, host: null, playing: false };
}

// One frame. Sky time grows by the host time that passed, scaled by the
// eased speed, and only across a frame that played from end to end, so
// stopping, starting and seeking never jump the sky.
export function tickSky(sky: Sky, reading: SkyReading): Sky {
  const dt = sky.host === null ? 0 : Math.max(0, reading.host - sky.host);
  let speed = 0;
  let travelled = 0;
  if (reading.touch !== 'down') {
    // The speed ramps linearly toward its target, a full swing taking 240 ms.
    const target = reading.touch === 'resting' ? 0 : 1;
    const ramp = Math.abs(target - sky.speed) * STILL_SECONDS;
    if (dt >= ramp) {
      speed = target;
      travelled = ((sky.speed + target) / 2) * ramp + target * (dt - ramp);
    } else {
      speed = sky.speed + Math.sign(target - sky.speed) * (dt / STILL_SECONDS);
      travelled = ((sky.speed + speed) / 2) * dt;
    }
  }
  const moved = sky.playing && reading.playing ? travelled : 0;
  return { t: sky.t + moved, speed, host: reading.host, playing: reading.playing };
}

// A playing world breathes at its tempo: once every two beats, to 1.015
// (iOS v5 and web v4). The phase follows the playhead, so the breath stays
// on the beat. Nothing breathes while paused or under Reduce Motion.
export function breathScale(playhead: number, bpm: number | null, playing: boolean, reduceMotion: boolean): number {
  if (!playing || reduceMotion) return 1;
  const period = bpm ? Math.max(0.3, 120 / bpm) : 1.6;
  return 1 + 0.015 * sin((2 * Math.PI * playhead) / period);
}

// How near a body sits, −1 (behind its sun) to 1 (in front), from where the
// camera stands. At yaw 0 the camera looks from the +y side, as iOS's
// depth (sin E) assumes.
export function depthOf(point: Point, centre: Point, yaw: number): number {
  const dx = point.x - centre.x;
  const dy = point.y - centre.y;
  const r = sqrt(dx * dx + dy * dy);
  if (r === 0) return 0;
  return (dx * sin(yaw) + dy * cos(yaw)) / r;
}

// The hub's depth dimming: far bodies at 0.45, near ones at 1.
export function depthOpacity(depth: number): number {
  return 0.45 + ((depth + 1) / 2) * 0.55;
}
