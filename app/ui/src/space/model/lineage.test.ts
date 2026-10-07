import { describe, expect, test } from 'vitest';
import { unitJitter } from '../../shared/dmath';
import {
  branchTo,
  buildFamilies,
  claimSentence,
  declareLink,
  layoutFamilies,
  linkShows,
  provenanceLine,
  strand,
  type LineageNode,
} from './lineage';

function node(trackId: string, parent: string | null, day: number, extra: Partial<LineageNode> = {}): LineageNode {
  return {
    trackId,
    parentTrackId: parent,
    secondaryParentTrackId: null,
    lineageRootTrackId: null,
    remixDepth: 0,
    createdAt: `2026-01-${String(day).padStart(2, '0')}T00:00:00.000Z`,
    title: trackId,
    artist: 'LMY',
    ...extra,
  };
}

// v3's own fixture (ios_v3/WWAVTests/CosmosLayoutTests.swift).
const V3 = [
  node('sun1', null, 1),
  node('moon1a', 'sun1', 2),
  node('moon1b', 'sun1', 3),
  node('moon1a1', 'moon1a', 4),
  node('sun2', null, 5),
  node('moon2a', 'sun2', 6),
];

const TWO_PI = 2 * Math.PI;
const dist = (a: { x: number; y: number }, b: { x: number; y: number }) => Math.sqrt((a.x - b.x) ** 2 + (a.y - b.y) ** 2);
const angleOf = (p: { x: number; y: number }, c: { x: number; y: number }) => Math.atan2(p.y - c.y, p.x - c.x);
function gap(a: number, b: number): number {
  const d = (((a - b) % TWO_PI) + TWO_PI) % TWO_PI;
  return Math.min(d, TWO_PI - d);
}

describe('the family tree (v3 ring rule)', () => {
  test('is the same every time', () => {
    const a = layoutFamilies(buildFamilies(['sun1', 'sun2'], V3));
    const b = layoutFamilies(buildFamilies(['sun1', 'sun2'], V3));
    expect(b).toEqual(a);
  });

  test('puts the root at the centre and generation g on the ring at 90 + (g − 1)·70', () => {
    const layout = layoutFamilies(buildFamilies(['sun1', 'sun2'], V3));
    const at = (id: string) => layout.nodes.find((n) => n.trackId === id)!;
    expect(dist(at('moon1a'), at('sun1'))).toBeGreaterThanOrEqual(90 - 8);
    expect(dist(at('moon1a'), at('sun1'))).toBeLessThanOrEqual(90 + 8);
    expect(dist(at('moon1a1'), at('sun1'))).toBeGreaterThanOrEqual(160 - 8);
    expect(dist(at('moon1a1'), at('sun1'))).toBeLessThanOrEqual(160 + 8);
    expect(at('sun1').radius).toBe(34);
    expect(at('moon1a').radius).toBe(13);
    expect(at('moon1a1').radius).toBeCloseTo(11.8, 12);
  });

  test("gives each subtree an arc in proportion to its leaves, so families stay together", () => {
    const nodes = [
      node('r', null, 1),
      node('a', 'r', 2),
      node('b', 'r', 3),
      node('a1', 'a', 4),
      node('a2', 'a', 5),
      node('a3', 'a', 6),
      node('b1', 'b', 7),
    ];
    const layout = layoutFamilies(buildFamilies(['r'], nodes));
    const at = (id: string) => layout.nodes.find((n) => n.trackId === id)!;
    const base = unitJitter('r', 'rotation') * TWO_PI;
    const jitter = Math.PI / 30; // ±6°
    // a holds three of the four leaves, so its arc is three quarters of the circle.
    expect(gap(angleOf(at('a'), at('r')), base + (3 / 8) * TWO_PI)).toBeLessThanOrEqual(jitter + 1e-9);
    expect(gap(angleOf(at('b'), at('r')), base + (7 / 8) * TWO_PI)).toBeLessThanOrEqual(jitter + 1e-9);
    ['a1', 'a2', 'a3'].forEach((id, i) => {
      expect(gap(angleOf(at(id), at('r')), base + ((2 * i + 1) / 8) * TWO_PI)).toBeLessThanOrEqual(jitter + 1e-9);
    });
    expect(gap(angleOf(at('b1'), at('r')), base + (7 / 8) * TWO_PI)).toBeLessThanOrEqual(jitter + 1e-9);
  });

  test('orders siblings by when they were made, whatever order they arrive in', () => {
    const shuffled = [V3[3], V3[0], V3[5], V3[2], V3[4], V3[1]];
    expect(layoutFamilies(buildFamilies(['sun1', 'sun2'], shuffled))).toEqual(
      layoutFamilies(buildFamilies(['sun1', 'sun2'], V3)),
    );
  });

  test('keeps families apart and every node inside the bounds', () => {
    const layout = layoutFamilies(buildFamilies(['sun1', 'sun2'], V3));
    const [f1, f2] = [layout.frames.sun1, layout.frames.sun2];
    const overlap = f1.x < f2.x + f2.w && f2.x < f1.x + f1.w && f1.y < f2.y + f2.h && f2.y < f1.y + f1.h;
    expect(overlap).toBe(false);
    for (const n of layout.nodes) {
      expect(n.x).toBeGreaterThanOrEqual(layout.bounds.x - 1);
      expect(n.x).toBeLessThanOrEqual(layout.bounds.x + layout.bounds.w + 1);
    }
  });

  test('draws fork edges solid and merges dashed, only within one family', () => {
    const nodes = [
      ...V3,
      node('mix', 'moon1b', 8, { secondaryParentTrackId: 'moon1a1' }),
      node('cross', 'moon2a', 9, { secondaryParentTrackId: 'sun1' }),
    ];
    const layout = layoutFamilies(buildFamilies(['sun1', 'sun2'], nodes));
    expect(layout.edges.filter((e) => !e.merge)).toHaveLength(6);
    const merges = layout.edges.filter((e) => e.merge);
    expect(merges.map((e) => [e.parent, e.child])).toEqual([['moon1a1', 'mix']]);
  });

  test('keeps a fork whose parent is missing, on the ring its depth claims', () => {
    const nodes = [...V3, node('lost', 'gone', 10, { lineageRootTrackId: 'sun1', remixDepth: 3 })];
    const families = buildFamilies(['sun1', 'sun2'], nodes);
    expect(families[0].generations[3].map((n) => n.trackId)).toEqual(['lost']);
    const stray = buildFamilies([], [node('alone', 'gone', 1)]);
    expect(stray.map((f) => f.id)).toEqual(['alone']);
  });

  test('marks your branch from the root down', () => {
    const [family] = buildFamilies(['sun1'], V3.slice(0, 4));
    expect([...branchTo(family, 'moon1a1')].sort()).toEqual(['moon1a', 'moon1a1', 'sun1']);
  });
});

describe('links and consent', () => {
  const planet = (id: number) => ({ type: 'planet' as const, id });

  test('a link touching someone else’s work waits for them, and shows only once accepted', () => {
    const made = declareLink({ from: planet(1), to: planet(2), kind: 'influence', note: '' }, { from: 7, to: 9 }, 7);
    expect(made).toMatchObject({ link: { status: 'pending' } });
    if (!('link' in made)) throw new Error('refused');
    expect(linkShows(made.link)).toBe(false);
    expect(linkShows({ ...made.link, status: 'accepted' })).toBe(true);
  });

  test('a link inside your own galaxy accepts itself', () => {
    const made = declareLink({ from: planet(1), to: planet(3), kind: 'sample', note: '' }, { from: 7, to: 7 }, 7);
    expect(made).toMatchObject({ link: { status: 'accepted', kind: 'sample' } });
  });

  test('is refused in the server’s words', () => {
    expect(declareLink({ from: planet(1), to: planet(2), kind: 'collab', note: '' }, { from: 8, to: 9 }, 7)).toEqual({
      refused: 'A link has to touch something of yours',
    });
    expect(declareLink({ from: planet(1), to: planet(1), kind: 'collab', note: '' }, { from: 7, to: 7 }, 7)).toEqual({
      refused: 'A thing cannot descend from itself',
    });
  });

  test('keeps one of five kinds and a note of 280 characters', () => {
    const made = declareLink({ from: planet(1), to: planet(2), kind: 'remix' as never, note: 'x'.repeat(400) }, { from: 7, to: 9 }, 7);
    expect(made).toMatchObject({ link: { kind: 'influence' } });
    if ('link' in made) expect(made.link.note).toHaveLength(280);
  });

  test('reads as a claim the other owner can agree to', () => {
    expect(claimSentence('LMY', { from: planet(1), to: planet(2), kind: 'influence' })).toBe(
      'LMY says their planet is an influence of your planet.',
    );
    expect(claimSentence('Ana', { from: planet(1), to: { type: 'galaxy', id: 3 }, kind: 'cover' })).toBe(
      'Ana says their planet is a cover of your galaxy.',
    );
  });
});

describe('constellations', () => {
  test('a strand grows thicker the more work has passed between two galaxies, up to a limit', () => {
    const a = { x: 0, y: 0 };
    const b = { x: 1000, y: 0 };
    const widths = [1, 2, 5, 11, 50].map((weight) => strand({ a: 1, b: 2, weight }, a, b).width);
    expect(widths[0]).toBeCloseTo(1.4 + 3.2 * (Math.log(2) / Math.log(12)), 12);
    expect(widths[1]).toBeGreaterThan(widths[0]);
    expect(widths[2]).toBeGreaterThan(widths[1]);
    expect(widths[3]).toBeCloseTo(4.6, 12);
    expect(widths[4]).toBe(widths[3]);
  });

  test('bows the same way on every machine, by 13% of its span', () => {
    const s = strand({ a: 1, b: 2, weight: 3 }, { x: 0, y: 0 }, { x: 1000, y: 0 });
    expect(Math.abs(s.control.y)).toBeCloseTo(130, 9);
    expect(s.control.x).toBeCloseTo(500, 9);
    expect(strand({ a: 1, b: 2, weight: 3 }, { x: 0, y: 0 }, { x: 1000, y: 0 })).toEqual(s);
  });
});

describe('provenance', () => {
  test('names the generation and the parent', () => {
    expect(provenanceLine(2, [{ title: 'World Ending', artist: 'LMY' }])).toBe('gen 2 · remix of World Ending by LMY');
    expect(
      provenanceLine(1, [
        { title: 'World Ending', artist: 'LMY' },
        { title: 'Low Tide', artist: 'Ana' },
      ]),
    ).toBe('gen 1 · remix of World Ending by LMY and Low Tide by Ana');
    expect(provenanceLine(0, [])).toBe('gen 0 · original');
  });
});
