// A work's colour is its key colour (docs/SPEC.md 8.3): the same function as
// crates/wwav-tokens and v4's keyColor.js, so a planet is one colour in the
// web UI, the compositor and on the web. Also WCAG 2's contrast ratio. Its
// one import is the token file, so it runs under vitest or plain node alike.

import tokens from '../../../../design/tokens.json' with { type: 'json' };

const KEY = tokens.key;

/** A key: its tonic as a pitch class with A = 0, as v4's analysis counts. */
export type Key = { pc: number; minor: boolean };

const NATURALS = new Map([['A', 0], ['B', 2], ['C', 3], ['D', 5], ['E', 7], ['F', 8], ['G', 10]]);
const ACCIDENTALS = new Map([['', 0], ['#', 1], ['♯', 1], ['b', -1], ['♭', -1]]);

/**
 * Reads a key the way .wwav writes it (6.1's wmet.key): a note A to G with at
 * most one sharp or flat, then major or minor, as in "A minor", "F# major",
 * "Bb minor". Anything else is null, a key not known yet.
 */
export function parseKey(name: string | null | undefined): Key | null {
  const words = (name ?? '').trim().split(/\s+/);
  if (words.length !== 2) return null;
  const [note, mode] = words;
  const natural = NATURALS.get(note[0].toUpperCase());
  const shift = ACCIDENTALS.get(note.slice(1));
  const minor = mode.toLowerCase() === 'minor';
  if (natural === undefined || shift === undefined) return null;
  if (!minor && mode.toLowerCase() !== 'major') return null;
  return { pc: (natural + shift + 12) % 12, minor };
}

/**
 * The hue in degrees: ((pc * 7) mod 12) * 30 walks the circle of fifths, so a
 * key a fifth away is the neighbouring shade. An unknown key takes night
 * indigo, the sky's own hue: "still condensing".
 */
export function keyHue(key: Key | null): number {
  return key ? ((key.pc * 7) % 12) * 30 : KEY.unknownHue;
}

// An unknown key wears the minor look, as in v4.
const look = (key: Key | null) => (key && !key.minor ? KEY.major : KEY.minor);

/** A work's colour as #rrggbb. It is a fill, never a ground for text. */
export function keyColor(key: Key | null): string {
  const { saturation, lightness } = look(key);
  return hsl(keyHue(key), saturation, lightness);
}

/** The tone an orb's rim and the sky's glow take (8.2), as v4's glowRgb. */
export function glowTone(key: Key | null): string {
  return hsl(keyHue(key), look(key).saturation, KEY.glowLightness);
}

// hsl(h, s%, l%) as #rrggbb, worked out and rounded as v4's hslToRgb does.
function hsl(h: number, s: number, l: number): string {
  s /= 100;
  l /= 100;
  const a = s * Math.min(l, 1 - l);
  const channel = (n: number) => {
    const k = (n + h / 30) % 12;
    const v = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(v * 255).toString(16).padStart(2, '0');
  };
  return `#${channel(0)}${channel(8)}${channel(4)}`;
}

const linear = (c: number) => {
  c /= 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};
const luminance = ([r, g, b]: number[]) => 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
const channels = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

/**
 * WCAG 2's contrast ratio of an ink on an opaque ground, both #rrggbb, from 1
 * to 21. An ink at an opacity under 1 (the night's 70% and 50%) is laid over
 * the ground first, as the screen shows it.
 */
export function contrast(ink: string, ground: string, alpha = 1): number {
  const g = channels(ground);
  const shown = channels(ink).map((c, i) => c * alpha + g[i] * (1 - alpha));
  const [x, y] = [luminance(shown), luminance(g)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
