import { describe, expect, test } from 'vitest';
import {
  DEFAULT_PITCH,
  SCENES,
  ZOOM_STEP,
  endsGesture,
  fit,
  judgeTier,
  orbitBy,
  panBy,
  startGesture,
  zoomAbout,
  zoomStep,
  type Camera,
} from './camera';

const VIEW = { w: 1280, h: 726 };

// A top-down orthographic view: enough to check that a zoom keeps the
// point under the pointer where it was.
function screen(cam: Camera, p: { x: number; y: number }) {
  return { x: (p.x - cam.x) * cam.k + VIEW.w / 2, y: (p.y - cam.y) * cam.k + VIEW.h / 2 };
}

const cam: Camera = { x: 2500, y: 2500, k: 0.3, yaw: 0, pitch: DEFAULT_PITCH };

describe('zoom', () => {
  test('keeps the point under the pointer still', () => {
    const p = { x: 2900, y: 2300 };
    const before = screen(cam, p);
    const after = screen(zoomAbout(cam, 'system', p, 0.9), p);
    expect(after.x).toBeCloseTo(before.x, 9);
    expect(after.y).toBeCloseTo(before.y, 9);
  });

  test('steps ×1.55 and stays inside the scene', () => {
    expect(ZOOM_STEP).toBe(1.55);
    expect(zoomStep(cam, 'system', 1).k).toBeCloseTo(0.3 * 1.55, 12);
    expect(zoomStep(cam, 'system', -2).k).toBeCloseTo(0.3 / 1.55 / 1.55, 12);
    expect(zoomStep({ ...cam, k: 2 }, 'system', 1).k).toBe(SCENES.system.maxK);
    expect(zoomStep({ ...cam, k: 0.05 }, 'system', -1).k).toBe(SCENES.system.minK);
  });
});

describe('tiers', () => {
  test("enter and leave at each scene's k", () => {
    expect(judgeTier('system', 1.15)).toBe('enter');
    expect(judgeTier('system', 1.149)).toBeNull();
    expect(judgeTier('system', 0.075)).toBe('leave');
    expect(judgeTier('system', 0.0751)).toBeNull();
    expect(judgeTier('galaxy', 0.62)).toBe('enter');
    expect(judgeTier('galaxy', 0.07)).toBe('leave');
    expect(judgeTier('universe', 0.8)).toBe('enter');
    // "The end of the road."
    expect(judgeTier('universe', 0.000001)).toBeNull();
  });

  test('change only when the last finger lifts after a pinch', () => {
    let g = startGesture();
    let ended: boolean;
    ({ gesture: g, ended } = endsGesture(g, { type: 'down' }));
    ({ gesture: g, ended } = endsGesture(g, { type: 'down' }));
    ({ gesture: g, ended } = endsGesture(g, { type: 'pinch' }));
    ({ gesture: g, ended } = endsGesture(g, { type: 'up' }));
    expect(ended).toBe(false);
    ({ gesture: g, ended } = endsGesture(g, { type: 'up' }));
    expect(ended).toBe(true);
  });

  test('never change on a one-finger pan', () => {
    let g = startGesture();
    let ended: boolean;
    ({ gesture: g, ended } = endsGesture(g, { type: 'down' }));
    ({ gesture: g, ended } = endsGesture(g, { type: 'up' }));
    expect(ended).toBe(false);
  });

  test('change 180 ms after the last wheel event', () => {
    let g = startGesture();
    let ended: boolean;
    for (const time of [0, 50, 100]) ({ gesture: g, ended } = endsGesture(g, { type: 'wheel', time }));
    ({ gesture: g, ended } = endsGesture(g, { type: 'tick', time: 279 }));
    expect(ended).toBe(false);
    ({ gesture: g, ended } = endsGesture(g, { type: 'tick', time: 280 }));
    expect(ended).toBe(true);
    ({ gesture: g, ended } = endsGesture(g, { type: 'tick', time: 400 }));
    expect(ended).toBe(false);
  });

  test('change at once for a button press and at the end of a trackpad pinch', () => {
    expect(endsGesture(startGesture(), { type: 'button' }).ended).toBe(true);
    expect(endsGesture(startGesture(), { type: 'pinchEnd' }).ended).toBe(true);
  });
});

describe('⌥-drag orbit', () => {
  test('turns 0.006 rad per pt across and tips 0.004 per pt down, pitch 0.35–1.15', () => {
    expect(DEFAULT_PITCH).toBe(0.62);
    const across = orbitBy(cam, 100, 0);
    expect(across.yaw).toBeCloseTo(0.6, 12);
    expect(across.pitch).toBe(0.62);
    expect(orbitBy(cam, 0, 50).pitch).toBeCloseTo(0.82, 12);
    expect(orbitBy(cam, 0, 1000).pitch).toBe(1.15);
    expect(orbitBy(cam, 0, -1000).pitch).toBe(0.35);
  });
});

describe('framing', () => {
  test('fits a box to 80% of the view, centred', () => {
    const framed = fit({ x: 2000, y: 2200, w: 1000, h: 500 }, VIEW, 'system', cam);
    expect(framed.k).toBeCloseTo(0.8 * Math.min(1280 / 1000, 726 / 500), 12);
    expect(framed).toMatchObject({ x: 2500, y: 2450, yaw: 0, pitch: DEFAULT_PITCH });
  });

  test('a pan keeps the grabbed point under the pointer', () => {
    const moved = panBy(cam, { x: 2600, y: 2500 }, { x: 2650, y: 2480 });
    expect(moved).toMatchObject({ x: 2450, y: 2520, k: 0.3 });
  });
});
