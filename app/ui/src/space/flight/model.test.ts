import { Matrix4, Quaternion, Vector3 } from 'three';
import { describe, expect, it } from 'vitest';
import { provingGround, sky, start } from './ground';
import { type Home, homeBodies, homeStart } from './home';
import {
  type Body,
  type Pilot,
  type View,
  STILL,
  aim,
  coast,
  cruise,
  direction,
  dockDepth,
  faceOf,
  follow,
  inside,
  look,
  nudge,
  plot,
  radiusOf,
  sight,
  step,
  turnPerPoint,
} from './model';

// Flight in first person (docs/SPACE.md 1, 3, 11). What a fail looks like: a
// page that isn't a plain rectangle the shape of the view; a page that
// doesn't exactly fill the view when you are in it; scrolling that overshoots
// a body or never arrives; a body that isn't where it was on another
// machine; sizes that jump tenfold with a power of ten; nothing slowing you
// near a body, or the last stretch to one a crawl; a wheel's click moving you
// in one jerk; a flight that takes a minute because the body is far; home's
// worlds on top of each other, or all on one plane.

const VIEW: View = { width: 1280, height: 720 };

function body(over: Partial<Body> = {}): Body {
  return { id: 'b', url: 'https://example.org/', name: 'B', magnitude: 3, parent: null, at: new Vector3(0, 0, -1000), radius: 100, ...over };
}

function pilot(at = new Vector3()): Pilot {
  return { at, facing: new Quaternion(), velocity: new Vector3(), spin: new Vector3() };
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
    for (let i = 0; i < 60; i++) step(me, { ...STILL, forward: 1 }, [], VIEW, 1 / 60);
    expect(me.at.z).toBeLessThan(-100);
    expect(Math.abs(me.at.x)).toBeLessThan(1e-6);
    const speed = me.velocity.length();
    for (let i = 0; i < 120; i++) step(me, STILL, [], VIEW, 1 / 60);
    expect(me.velocity.length()).toBeLessThan(speed * 0.01);
  });

  it('is slower the nearer you are to a body, and never a crawl at one', () => {
    const b = body();
    expect(cruise(pilot(new Vector3(0, 0, -850)), [b])).toBeLessThan(cruise(pilot(), [b]) / 10);
    // Inside the sphere, the page a few steps off: still half a radius a second.
    expect(cruise(pilot(new Vector3(0, 0, -1000)), [b])).toBe(b.radius * 0.5);
  });

  it('goes four times as fast with Shift', () => {
    const slow = pilot();
    const fast = pilot();
    for (let i = 0; i < 90; i++) {
      step(slow, { ...STILL, forward: 1 }, [], VIEW, 1 / 60);
      step(fast, { ...STILL, forward: 1, fast: true }, [], VIEW, 1 / 60);
    }
    expect(fast.at.z / slow.at.z).toBeCloseTo(4, 1);
  });

  it('turns with the arrow keys without moving you, easing in and out', () => {
    const me = pilot();
    step(me, { ...STILL, yaw: 1 }, [], VIEW, 1 / 60);
    const first = me.facing.angleTo(new Quaternion());
    for (let i = 0; i < 59; i++) step(me, { ...STILL, yaw: 1 }, [], VIEW, 1 / 60);
    const turned = me.facing.angleTo(new Quaternion());
    expect(me.at.length()).toBe(0);
    expect(first).toBeLessThan(turned / 120);
    expect(turned).toBeGreaterThan(1);
    for (let i = 0; i < 60; i++) step(me, STILL, [], VIEW, 1 / 60);
    expect(me.spin.length()).toBeLessThan(0.001);
  });

  it('is turned toward a body it flies at a little to one side of', () => {
    const b = body({ at: new Vector3(120, 0, -1000), magnitude: 8 });
    const me = pilot();
    const before = sight(me, b, VIEW)!.off;
    const straight = pilot();
    for (let i = 0; i < 90; i++) {
      step(me, { ...STILL, forward: 1 }, [b], VIEW, 1 / 60);
      // The same flight with nothing pulling: the body slides further aside.
      step(straight, { ...STILL, forward: 1 }, [body({ at: new Vector3(120, 0, -1000), magnitude: 8, radius: 100 })], VIEW, 1 / 60);
      straight.facing.identity();
    }
    expect(sight(me, b, VIEW)!.off).toBeLessThan(before);
    expect(sight(straight, b, VIEW)!.off).toBeGreaterThan(before);
  });

  it('keeps the star you took hold of under the pointer when you drag', () => {
    const me = pilot();
    const b = body({ at: new Vector3(0, 0, -1000) });
    const turn = turnPerPoint(VIEW);
    // Dragging 100 points to the right near the middle moves the body about 100 points right.
    look(me, 100 * turn, 0);
    const s = sight(me, b, VIEW)!;
    expect(s.rect.x + s.rect.width / 2 - VIEW.width / 2).toBeGreaterThan(95);
    expect(s.rect.x + s.rect.width / 2 - VIEW.width / 2).toBeLessThan(105);
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

describe('a wheel turned in steps', () => {
  it('moves you over several frames, and all the way it was turned', () => {
    const b = body();
    const once = pilot();
    nudge(once, [b], VIEW, 120);
    const smooth = pilot();
    const glide = { push: 120 };
    coast(smooth, [b], VIEW, glide, 1 / 60);
    const first = smooth.at.length();
    expect(first).toBeGreaterThan(0);
    expect(first).toBeLessThan(once.at.length() * 0.3);
    for (let i = 0; i < 60; i++) coast(smooth, [b], VIEW, glide, 1 / 60);
    expect(glide.push).toBe(0);
    expect(smooth.at.length()).toBeCloseTo(once.at.length(), 0);
  });
});

describe('being flown to a body', () => {
  function fly(me: Pilot, b: Body, cover: number): number {
    const course = plot(me, b, VIEW, cover, false);
    let frames = 0;
    while (!follow(me, course, b, VIEW, 1 / 60) && frames < 2000) frames++;
    return frames / 60;
  }

  it('arrives with the page covering what was asked, squarely in the middle', () => {
    for (const cover of [0.55, 1]) {
      const b = body({ at: new Vector3(400, -300, -1000) });
      const me = pilot();
      fly(me, b, cover);
      const s = sight(me, b, VIEW)!;
      expect(s.cover).toBeCloseTo(cover, 3);
      expect(s.off).toBeLessThan(0.001);
      expect(inside(s.rect, VIEW, 1)).toBe(true);
    }
  });

  it('takes a moment for a near body and under three seconds for one a thousand times further', () => {
    const near = fly(pilot(new Vector3(0, 0, -700)), body(), 1);
    const far = fly(pilot(new Vector3(0, 0, 300000)), body(), 1);
    expect(near).toBeGreaterThan(0.6);
    expect(near).toBeLessThan(1.5);
    expect(far).toBeGreaterThan(near);
    expect(far).toBeLessThan(2.7);
  });

  it('starts gently and ends gently, never passing the page', () => {
    const b = body();
    const me = pilot();
    const course = plot(me, b, VIEW, 1, true);
    const gaps: number[] = [];
    for (let i = 0; i < 400; i++) {
      const there = follow(me, course, b, VIEW, 1 / 60);
      gaps.push(me.at.distanceTo(b.at) - dockDepth(b, VIEW));
      if (there) break;
    }
    for (let i = 1; i < gaps.length; i++) expect(gaps[i]).toBeLessThanOrEqual(gaps[i - 1] + 1e-9);
    expect(gaps.at(-1)).toBeCloseTo(0, 6);
    // The first frame and the last move you far less than one in the middle.
    const steps = gaps.slice(1).map((g, i) => gaps[i] - g);
    const most = Math.max(...steps);
    expect(steps[0]).toBeLessThan(most / 20);
    expect(steps.at(-1)!).toBeLessThan(most / 20);
  });

  it('backs you out of a page to where the page covers half the view', () => {
    const b = body();
    const me = pilot(new Vector3(0, 0, -1000 + dockDepth(b, VIEW)));
    fly(me, b, 0.5);
    expect(sight(me, b, VIEW)!.cover).toBeCloseTo(0.5, 3);
  });
});

const HOME: Home = {
  slug: 'someone',
  url: 'https://www.mi-wwav.com/summer_26/g/someone',
  name: 'someone',
  systems: [
    {
      url: 'https://www.mi-wwav.com/summer_26/g/someone/s/first',
      name: 'First',
      x: 2932,
      y: 3018,
      worlds: Array.from({ length: 24 }, (_, i) => ({
        url: `https://www.mi-wwav.com/summer_26/play/track_${i}`,
        name: `Song ${i}`,
        kind: 'song',
        order: i,
        // Some where the server put two on one spot, some it has no orbit for.
        radius: i % 5 === 4 ? null : 640 + 480 * (i % 3),
        phase: i % 5 === 4 ? null : (i % 4) * 1.5,
        palette: i % 2 ? { base: '#b51a00' } : null,
      })),
    },
    {
      url: 'https://www.mi-wwav.com/summer_26/g/someone/s/second',
      name: 'Second',
      x: 2715,
      y: 1750,
      worlds: [{ url: 'https://www.mi-wwav.com/summer_26/watch/67', name: 'A film', kind: 'film', order: 0, radius: 1120, phase: 2.2, palette: null }],
    },
  ],
};

describe('home', () => {
  it('is the galaxy, its solar systems and their worlds, each a link, sized by the links it connects', () => {
    const bodies = homeBodies(HOME);
    expect(bodies).toHaveLength(1 + 2 + 25);
    expect(new Set(bodies.map((b) => b.url)).size).toBe(bodies.length);
    const [sun, first] = bodies;
    expect(sun.at.length()).toBe(0);
    expect(sun.magnitude).toBe(1);
    expect(first.magnitude).toBe(1);
    expect(bodies.find((b) => b.name === 'Second')!.magnitude).toBe(0);
    expect(bodies.find((b) => b.name === 'Song 3')!.magnitude).toBe(0);
    expect(bodies.find((b) => b.name === 'Song 3')!.parent).toBe(first.id);
    expect(bodies.find((b) => b.name === 'Song 3')!.colour).toBe('#b51a00');
  });

  it('puts no two bodies on top of each other, and not all on one plane', () => {
    const bodies = homeBodies(HOME);
    for (const a of bodies) {
      for (const b of bodies) {
        if (a !== b) expect(a.at.distanceTo(b.at)).toBeGreaterThan(a.radius + b.radius);
      }
    }
    const first = bodies[1];
    const worlds = bodies.filter((b) => b.parent === first.id);
    const heights = worlds.map((w) => w.at.y - first.at.y);
    expect(Math.max(...heights) - Math.min(...heights)).toBeGreaterThan(40);
    // A world goes round its own system, nearer to it than to the other.
    const second = bodies.find((b) => b.name === 'Second')!;
    for (const w of worlds) expect(w.at.distanceTo(first.at)).toBeLessThan(w.at.distanceTo(second.at));
  });

  it('is laid out the same way every time', () => {
    expect(homeBodies(HOME).map((b) => b.at.toArray())).toEqual(homeBodies(HOME).map((b) => b.at.toArray()));
  });

  it('starts you where all of it is in view, facing its sun', () => {
    const bodies = homeBodies(HOME);
    const { at, toward } = homeStart(bodies);
    const me: Pilot = { at, facing: new Quaternion(), velocity: new Vector3(), spin: new Vector3() };
    me.facing.setFromRotationMatrix(new Matrix4().lookAt(at, toward, new Vector3(0, 1, 0)));
    for (const b of bodies) {
      const s = sight(me, b, VIEW)!;
      expect(s).not.toBeNull();
      expect(inside(s.rect, VIEW)).toBe(true);
    }
  });

  it('stands the sites well clear of it, and nothing in the whole sky overlaps', () => {
    const { bodies } = sky(HOME);
    const mine = homeBodies(HOME);
    const reach = Math.max(...mine.map((b) => b.at.length() + b.radius));
    for (const site of bodies.filter((b) => !mine.some((m) => m.id === b.id) && b.parent === null)) {
      expect(site.at.length() - site.radius).toBeGreaterThan(reach * 1.5);
    }
    for (const a of bodies) {
      for (const b of bodies) {
        if (a !== b) expect(a.at.distanceTo(b.at)).toBeGreaterThan(a.radius + b.radius);
      }
    }
    expect(sky(null).bodies).toHaveLength(provingGround().length);
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
