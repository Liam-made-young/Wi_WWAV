// The key colour and contrast ratio in the UI (docs/SPEC.md 8.2, 8.3), held to
// the same values as crates/wwav-tokens/tests/key_color.rs.
//
// Fails if: any of the 24 keys or "unknown" gives a colour other than v4's; a
// sharp and its flat name different keys; anything but "<note> major|minor" parses;
// a glow tone is under 4:1 on the night; a known contrast ratio comes out
// wrong.
//
// It uses only the part of assert that chai and node:assert share (assert(),
// strictEqual, deepStrictEqual), so with 'vitest' swapped for node:test and
// node:assert it runs under plain node too.

import { assert, describe, test } from 'vitest';
import tokens from '../../../../design/tokens.json' with { type: 'json' };
import { contrast, glowTone, keyColor, keyHue, parseKey } from './keyColor';

// v4's colorsForKey() for every key: planet body, glow.
const V4: [string, string, string][] = [
  ['A major', '#e14747', '#e66565'],
  ['A♯ major', '#4794e1', '#65a6e6'],
  ['B major', '#e1e147', '#e6e665'],
  ['C major', '#9447e1', '#a665e6'],
  ['C♯ major', '#47e147', '#65e665'],
  ['D major', '#e14794', '#e665a6'],
  ['D♯ major', '#47e1e1', '#65e6e6'],
  ['E major', '#e19447', '#e6a665'],
  ['F major', '#4747e1', '#6565e6'],
  ['F♯ major', '#94e147', '#a6e665'],
  ['G major', '#e147e1', '#e665e6'],
  ['G♯ major', '#47e194', '#65e6a6'],
  ['A minor', '#a92d2d', '#da7272'],
  ['A♯ minor', '#2d6ba9', '#72a6da'],
  ['B minor', '#a9a92d', '#dada72'],
  ['C minor', '#6b2da9', '#a672da'],
  ['C♯ minor', '#2da92d', '#72da72'],
  ['D minor', '#a92d6b', '#da72a6'],
  ['D♯ minor', '#2da9a9', '#72dada'],
  ['E minor', '#a96b2d', '#daa672'],
  ['F minor', '#2d2da9', '#7272da'],
  ['F♯ minor', '#6ba92d', '#a6da72'],
  ['G minor', '#a92da9', '#da72da'],
  ['G♯ minor', '#2da96b', '#72daa6'],
  ['unknown', '#2d3ea9', '#7280da'],
];

describe('key colour', () => {
  test('every key and unknown match v4', () => {
    for (const [name, body, glow] of V4) {
      const key = parseKey(name);
      assert.strictEqual(key === null, name === 'unknown', name);
      assert.strictEqual(keyColor(key), body, `${name} body`);
      assert.strictEqual(glowTone(key), glow, `${name} glow`);
    }
  });

  test('hue walks the circle of fifths from A', () => {
    const hue = (name: string) => keyHue(parseKey(name));
    assert.strictEqual(hue('A minor'), 0);
    assert.strictEqual(hue('E major'), 30); // a fifth up is the next hue
    assert.strictEqual(hue('B major'), 60);
    assert.strictEqual(hue('D major'), 330); // a fifth down is the previous one
    assert.strictEqual(hue('C major'), 270);
    assert.strictEqual(keyHue(null), 232); // night indigo
  });

  test('key names parse with sharps, flats and either case', () => {
    const k = (pc: number, minor: boolean) => ({ pc, minor });
    assert.deepStrictEqual(parseKey('A minor'), k(0, true));
    assert.deepStrictEqual(parseKey('F# major'), k(9, false));
    assert.deepStrictEqual(parseKey('F♯ major'), k(9, false));
    assert.deepStrictEqual(parseKey('Gb major'), k(9, false));
    assert.deepStrictEqual(parseKey('G♭ major'), k(9, false));
    assert.deepStrictEqual(parseKey('Bb minor'), k(1, true));
    assert.deepStrictEqual(parseKey('A# minor'), k(1, true));
    assert.deepStrictEqual(parseKey('Ab minor'), k(11, true)); // wraps below A
    assert.deepStrictEqual(parseKey('Cb major'), k(2, false)); // is B
    assert.deepStrictEqual(parseKey('E# minor'), k(8, true)); // is F
    assert.deepStrictEqual(parseKey('c minor'), k(3, true));
    assert.deepStrictEqual(parseKey('C MAJOR'), k(3, false));
    assert.deepStrictEqual(parseKey('  D   minor '), k(5, true));
  });

  test('anything else is unknown', () => {
    const names = ['', 'C', 'minor', 'H major', 'C## major', 'Cbb minor', 'C dorian', 'Am', 'C major 7'];
    for (const name of [...names, '♯C major', 'C♯♭ major', 'CtoString major', null, undefined]) {
      assert.strictEqual(parseKey(name), null, String(name));
    }
  });

  test('every glow tone holds four to one on the night', () => {
    // 8.2: "so no planet disappears in F minor". The closest is F major.
    let lowest = { ratio: Infinity, name: '' };
    for (const [name] of V4) {
      const ratio = contrast(glowTone(parseKey(name)), tokens.night.ground);
      if (ratio < lowest.ratio) lowest = { ratio, name };
    }
    assert(lowest.ratio >= 4, `${lowest.name} is ${lowest.ratio.toFixed(2)}:1`);
    assert.strictEqual(lowest.name, 'F major');
  });
});

describe('contrast', () => {
  const near = (got: number, want: number) => assert(Math.abs(got - want) < 0.005, `${got}, not ${want}`);

  test('runs from 1 to 21 either way round', () => {
    assert.strictEqual(contrast('#000000', '#ffffff'), 21);
    assert.strictEqual(contrast('#ffffff', '#000000'), 21);
    assert.strictEqual(contrast('#ffffff', '#ffffff'), 1);
  });

  test('reproduces known ratios', () => {
    near(contrast('#767676', '#ffffff'), 4.54); // WCAG's AA line
    near(contrast('#ffffff', '#1B4C8C'), 8.54); // 8.2's selected row
  });

  test('lays a translucent ink over its ground first', () => {
    // The night's 70% ink (8.2: 8.5:1).
    near(contrast(tokens.night.ink, tokens.night.ground, 0.7), 8.55);
    assert.strictEqual(contrast('#ffffff', '#000000', 0), 1);
  });
});
