import { expect, test } from 'vitest';
import { GALAXIES, LINEAGE } from './catalogue';
import { LAYOUT_HASH, LAYOUT_HASH_LATER, layoutHash, layoutText } from './layoutHash';

// docs/PLAN.md S4.1: one catalogue, every position at t = 0, hashed. The
// browser check (e2e/space-layout.spec.ts) asserts the same value inside
// Chromium, and the WebKit pass inside WebKit.
test('the layout hash at t = 0 is pinned', () => {
  expect(layoutHash(GALAXIES, LINEAGE, 0)).toBe(LAYOUT_HASH);
});

// Ten minutes of play later, so Kepler motion itself is covered too.
test('the layout hash after ten minutes of play is pinned', () => {
  expect(layoutHash(GALAXIES, LINEAGE, 600.25)).toBe(LAYOUT_HASH_LATER);
});

test('the hash covers every tier', () => {
  const text = layoutText(GALAXIES, LINEAGE, 0);
  for (const tier of ['universe ', 'galaxy ', 'world ', 'gathered ', 'tree ']) {
    expect(text).toContain(tier);
  }
  // 4 galaxies; 33 systems; every shown world (48 + 44 + 45) and the one
  // gathered world; 11 tree nodes.
  const lines = text.split('\n');
  expect(lines.filter((l) => l.startsWith('universe '))).toHaveLength(4);
  expect(lines.filter((l) => l.startsWith('galaxy '))).toHaveLength(33);
  expect(lines.filter((l) => l.startsWith('world '))).toHaveLength(137);
  expect(lines.filter((l) => l.startsWith('gathered '))).toHaveLength(1);
  expect(lines.filter((l) => l.startsWith('tree '))).toHaveLength(11);
});

test('a moved world changes the hash', () => {
  const moved = structuredClone(GALAXIES);
  moved[0].systems[0].worlds[3].orbitIndex = 99;
  expect(layoutHash(moved, LINEAGE, 0)).not.toBe(LAYOUT_HASH);
});
