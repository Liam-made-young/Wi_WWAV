import { describe, expect, test } from 'vitest';
import { CENTER } from './orbits';
import { STILL_SECONDS, breathScale, depthOf, depthOpacity, startSky, tickSky, type SkyReading } from './motion';

function run(readings: SkyReading[]) {
  let sky = startSky();
  for (const r of readings) sky = tickSky(sky, r);
  return sky;
}

const playing = (host: number, touch: SkyReading['touch'] = 'none'): SkyReading => ({ host, playing: true, touch });
const quiet = (host: number): SkyReading => ({ host, playing: false, touch: 'none' });

describe("the sky's clock", () => {
  test('holds still while nothing plays', () => {
    const readings = Array.from({ length: 301 }, (_, i) => quiet(i / 60));
    expect(run(readings).t).toBe(0);
  });

  test("advances with the engine's clock while something plays", () => {
    expect(run([playing(10), playing(11), playing(12.5)]).t).toBe(2.5);
  });

  test('counts only the frames that played from end to end', () => {
    expect(run([playing(0), playing(1), quiet(2), quiet(5), playing(6), playing(7)]).t).toBe(2);
  });

  test('stops the moment a pointer goes down', () => {
    expect(run([playing(0), playing(1), playing(1.5, 'down'), playing(9, 'down')]).t).toBe(1);
  });

  test('eases to a stop over 240 ms while resting on a body', () => {
    expect(STILL_SECONDS).toBe(0.24);
    const half = run([playing(0), playing(0.12, 'resting')]);
    expect(half.speed).toBeCloseTo(0.5, 12);
    expect(half.t).toBeCloseTo(0.12 * 0.75, 12);
    const stopped = run([playing(0), playing(0.12, 'resting'), playing(5, 'resting')]);
    expect(stopped.speed).toBe(0);
    // The ease is a ramp of speed, so it covers half of its 240 ms.
    expect(stopped.t).toBeCloseTo(0.12, 12);
  });

  test('eases back over 240 ms when the pointer leaves', () => {
    const back = run([playing(0, 'down'), playing(1, 'down'), playing(1.24, 'none'), playing(2.24, 'none')]);
    expect(back.speed).toBe(1);
    expect(back.t).toBeCloseTo(0.12 + 1, 12);
  });
});

describe('breathing', () => {
  test('swells to 1.015 once per two beats while playing', () => {
    // 120 BPM: a period of 2 · 60 / 120 = 1 s.
    expect(breathScale(0.25, 120, true, false)).toBeCloseTo(1.015, 12);
    expect(breathScale(0.75, 120, true, false)).toBeCloseTo(0.985, 12);
    expect(breathScale(1.25, 120, true, false)).toBeCloseTo(1.015, 12);
  });

  test('is still while paused or under Reduce Motion', () => {
    expect(breathScale(0.25, 120, false, false)).toBe(1);
    expect(breathScale(0.25, 120, true, true)).toBe(1);
  });

  test('takes 1.6 s without a tempo, and never quicker than 0.3 s', () => {
    expect(breathScale(0.4, null, true, false)).toBeCloseTo(1.015, 12);
    expect(breathScale(0.075, 500, true, false)).toBeCloseTo(1.015, 12);
  });
});

describe('depth', () => {
  test('is +1 toward the camera and −1 away from it', () => {
    expect(depthOf({ x: CENTER.x, y: CENTER.y + 600 }, CENTER, 0)).toBeCloseTo(1, 12);
    expect(depthOf({ x: CENTER.x, y: CENTER.y - 600 }, CENTER, 0)).toBeCloseTo(-1, 12);
    expect(depthOf({ x: CENTER.x + 600, y: CENTER.y }, CENTER, 0)).toBeCloseTo(0, 12);
    expect(depthOf(CENTER, CENTER, 0)).toBe(0);
  });

  test('follows the camera round', () => {
    expect(depthOf({ x: CENTER.x + 600, y: CENTER.y }, CENTER, Math.PI / 2)).toBeCloseTo(1, 12);
  });

  test('dims far bodies to 0.45 and leaves near ones at 1', () => {
    expect(depthOpacity(-1)).toBeCloseTo(0.45, 12);
    expect(depthOpacity(1)).toBe(1);
    expect(depthOpacity(0)).toBeCloseTo(0.725, 12);
  });
});
