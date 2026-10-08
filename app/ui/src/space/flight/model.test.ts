import { Quaternion, Vector3 } from 'three';
import { describe, expect, it } from 'vitest';
import { provingGround, start } from './ground';
import {
  type Body,
  type Pilot,
  type View,
  STILL,
  aim,
  approach,
  cruise,
  direction,
  dockDepth,
  faceOf,
  inside,
  look,
  nudge,
  radiusOf,
  sight,
  step,
} from './model';

// Flight in first person (docs/SPACE.md 1, 3, 11). What a fail looks like: a
// page that isn't a plain rectangle the shape of the view; a page that
// doesn't exactly fill the view when you are in it; scrolling that overshoots
// a body or never arrives; a body that isn't where it was on another
// machine; sizes that jump tenfold with a power of ten; nothing slowing you
// near a body.

const VIEW: View = { width: 1280, height: 720 };

function body(over: Partial<Body> = {}): Body {
  return { id: 'b', url: 'https://example.org/', name: 'B', magnitude: 3, parent: null, at: new Vector3(0, 0, -1000), radius: 100, ...over };
}

function pilot(at = new Vector3()): Pilot {
  return { at, facing: new Quaternion(), velocity: new Vector3() };
}

describe('a body and its page', () => {
  it('grows a step with each power of ten, never tenfold', () => {
    expect(radiusOf(1) / radiusOf(0)).toBeCloseTo(1.28);
    expect(radiusOf(10) / radiusOf(0)).toBeLessThan(12);
    for (let m = 0; m < 10; m++) expect(radiusOf(m + 1)).toBeGreaterThan(radiusOf(m));
  });

  it('holds a page the shape of the view, wholly inside the sphere', () => {
    for (const view of [VIEW, { width: 900, height: 1200 }, { width: 2400, height: 600 }]) {
      const b = body();
      const face = faceOf(b, view);
      expect(face.width / face.height).toBeCloseTo(view.width / view.height);
      expect(Math.hypot(face.width / 2, face.height / 2)).toBeCloseTo(b.radius);
    }
  });

  it('is in the same direction on every machine, and not on one plane', () => {
    expect(direction('https://www.youtube.com/').toArray().map((n) => Math.round(n * 1e6))).toEqual(
      direction('https://www.youtube.com/').toArray().map((n) => Math.round(n * 1e6)),
    );
    const ys = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'].map((k) => direction(k).y);
    expect(Math.max(...ys) - Math.min(...ys)).toBeGreaterThan(0.5);
    for (const k of ['a', 'b', 'c']) expect(direction(k).length()).toBeCloseTo(1);
  });
});

describe('where a page is on the screen', () => {
  it('is a rectangle about the body, bigger the nearer you are', () => {
    const b = body();
    const far = sight(pilot(), b, VIEW)!;
    const near = sight(pilot(new Vector3(0, 0, -600)), b, VIEW)!;
    expect(far.rect.x + far.rect.width / 2).toBeCloseTo(VIEW.width / 2);
    expect(far.rect.y + far.rect.height / 2).toBeCloseTo(VIEW.height / 2);
    expect(far.rect.width / far.rect.height).toBeCloseTo(VIEW.width / VIEW.height);
    expect(near.cover).toBeCloseTo(far.cover * 2.5);
  });

  it('exactly fills the view at the docking depth, which is inside the sphere', () => {
    const b = body();
    const depth = dockDepth(b, VIEW);
    expect(depth).toBeLessThan(b.radius);
    const s = sight(pilot(new Vector3(0, 0, -1000 + depth)), b, VIEW)!;
    expect(s.cover).toBeCloseTo(1);
    expect(s.rect.x).toBeCloseTo(0);
    expect(s.rect.y).toBeCloseTo(0);
    expect(s.rect.width).toBeCloseTo(VIEW.width);
    expect(inside(s.rect, VIEW)).toBe(true);
  });

  it('is nowhere when the body is behind you, and not inside the view when it runs off an edge', () => {
    expect(sight(pilot(new Vector3(0, 0, -2000)), body(), VIEW)).toBeNull();
    const off = sight(pilot(), body({ at: new Vector3(1000, 0, -1000) }), VIEW)!;
    expect(inside(off.rect, VIEW)).toBe(false);
  });

  it('stays a level rectangle when you roll or look aside', () => {
    const me = pilot();
    look(me, 0.2, -0.1);
    const b = body();
    const s = sight(me, b, VIEW)!;
    expect(s.rect.width / s.rect.height).toBeCloseTo(VIEW.width / VIEW.height);
    expect(s.off).toBeGreaterThan(0.1);
  });
});

describe('what you are looking at', () => {
  it('is the body nearest the middle of the view, and nothing when you face empty sky', () => {
    const ahead = body({ id: 'ahead' });
    const aside = body({ id: 'aside', at: new Vector3(700, 0, -1000) });
    expect(aim(pilot(), [aside, ahead], VIEW)?.id).toBe('ahead');
    expect(aim(pilot(), [body({ at: new Vector3(0, 0, 1000) })], VIEW)).toBeNull();
    expect(aim(pilot(), [body({ at: new Vector3(3000, 0, -1000) })], VIEW)).toBeNull();
  });
});

describe('moving', () => {
  it('goes the way the keys say and drifts to a stop when they are let go', () => {
    const me = pilot();
    for (let i = 0; i < 60; i++) step(me, { ...STILL, forward: 1 }, [], 1 / 60);
    expect(me.at.z).toBeLessThan(-100);
    expect(Math.abs(me.at.x)).toBeLessThan(1e-6);
    const speed = me.velocity.length();
    for (let i = 0; i < 120; i++) step(me, STILL, [], 1 / 60);
    expect(me.velocity.length()).toBeLessThan(speed * 0.01);
  });

  it('is slower the nearer you are to a body', () => {
    const b = body();
    expect(cruise(pilot(new Vector3(0, 0, -850)), [b])).toBeLessThan(cruise(pilot(), [b]) / 10);
    expect(cruise(pilot(new Vector3(0, 0, -1000)), [b])).toBeGreaterThan(0);
  });

  it('turns with the arrow keys without moving you', () => {
    const me = pilot();
    step(me, { ...STILL, yaw: 1 }, [], 0.5);
    expect(me.at.length()).toBe(0);
    expect(me.facing.angleTo(new Quaternion())).toBeCloseTo(0.55);
  });
});

describe('scrolling toward what you face', () => {
  it('closes a share of the distance each time, never passing the page', () => {
    const b = body();
    const me = pilot();
    const dock = dockDepth(b, VIEW);
    let gap = me.at.distanceTo(b.at) - dock;
    for (let i = 0; i < 400; i++) {
      nudge(me, [b], VIEW, 30);
      const now = me.at.distanceTo(b.at) - dock;
      expect(now).toBeLessThanOrEqual(gap);
      expect(now).toBeGreaterThanOrEqual(0);
      gap = now;
    }
    expect(sight(me, b, VIEW)!.cover).toBeGreaterThan(0.99);
  });

  it('backs you out again, even from the very middle', () => {
    const b = body();
    const me = pilot(new Vector3(0, 0, -1000 + dockDepth(b, VIEW)));
    for (let i = 0; i < 200; i++) nudge(me, [b], VIEW, -30);
    expect(sight(me, b, VIEW)!.cover).toBeLessThan(0.2);
  });

  it('turns you to face a body you come at from the side', () => {
    const b = body({ at: new Vector3(260, 0, -1000) });
    const me = pilot();
    const before = sight(me, b, VIEW)!.off;
    for (let i = 0; i < 300; i++) nudge(me, [b], VIEW, 30);
    expect(sight(me, b, VIEW)!.off).toBeLessThan(before / 10);
  });

  it('simply moves you when nothing is ahead', () => {
    const me = pilot();
    nudge(me, [], VIEW, 100);
    expect(me.at.z).toBeLessThan(0);
  });
});

describe('being flown to a body', () => {
  it('arrives with the page covering what was asked, squarely in the middle', () => {
    for (const cover of [0.55, 1]) {
      const b = body({ at: new Vector3(400, -300, -1000) });
      const me = pilot();
      let there = false;
      for (let i = 0; i < 600 && !there; i++) there = approach(me, b, VIEW, cover, 1 / 60);
      expect(there).toBe(true);
      const s = sight(me, b, VIEW)!;
      expect(s.cover).toBeCloseTo(cover, 2);
      expect(s.off).toBeLessThan(0.01);
      expect(inside(s.rect, VIEW, 2)).toBe(true);
    }
  });
});

describe('the proving ground', () => {
  it('puts every link in one place, apart from the others, with a link beside its site', () => {
    const a = provingGround();
    const b = provingGround();
    expect(a.map((x) => x.at.toArray())).toEqual(b.map((x) => x.at.toArray()));
    for (const x of a) {
      for (const y of a) {
        if (x !== y) expect(x.at.distanceTo(y.at)).toBeGreaterThan(x.radius + y.radius);
      }
    }
    const zoo = a.find((x) => x.id === 'youtube-zoo')!;
    const youtube = a.find((x) => x.id === 'youtube')!;
    const others = a.filter((x) => x.parent === null && x !== youtube);
    for (const o of others) expect(zoo.at.distanceTo(youtube.at)).toBeLessThan(zoo.at.distanceTo(o.at));
    expect(youtube.radius).toBeGreaterThan(zoo.radius);
  });

  it('starts you facing the biggest body', () => {
    const bodies = provingGround();
    const { toward } = start(bodies);
    expect(toward.toArray()).toEqual(bodies.find((x) => x.id === 'youtube')!.at.toArray());
  });
});
