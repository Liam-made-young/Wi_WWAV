// A space's band of light (docs/FOCUS.md). The spectrum has four anchors,
// the WWAV palette in order round the wheel: royal blue, xanadu green, blood
// orange, red. A space sits on the anchor its hue is nearest, and a space
// whose hue falls well between two sits between them, as a mix of the two.
// The answer is a CSS colour made of tokens; nothing here writes a colour.

const ANCHORS = [
  { token: '--prism-band-red', hue: 356 },
  { token: '--prism-band-orange', hue: 16 },
  { token: '--prism-band-green', hue: 136 },
  { token: '--prism-band-blue', hue: 232 },
] as const;

/** Within this many degrees of an anchor, a space takes the anchor itself. */
const SNAP = 24;

const turn = (deg: number) => ((deg % 360) + 360) % 360;
/** Degrees from `a` round to `b`, going up the wheel. */
const ahead = (a: number, b: number) => turn(b - a);

/** The band for a space of this hue (0 to 360), as a CSS colour. */
export function bandOf(hue: number | undefined | null): string {
  if (hue === undefined || hue === null || !Number.isFinite(hue)) return 'var(--prism-ink3)';
  const h = turn(hue);
  let i = 0;
  for (let k = 0; k < ANCHORS.length; k++) {
    const here = ANCHORS[k].hue;
    const next = ANCHORS[(k + 1) % ANCHORS.length].hue;
    if (ahead(here, h) <= ahead(here, next)) {
      i = k;
      break;
    }
  }
  const a = ANCHORS[i];
  const b = ANCHORS[(i + 1) % ANCHORS.length];
  const span = ahead(a.hue, b.hue);
  const into = ahead(a.hue, h);
  if (into <= SNAP && into <= span - into) return `var(${a.token})`;
  if (span - into <= SNAP) return `var(${b.token})`;
  const share = Math.round((1 - into / span) * 100);
  return `color-mix(in oklab, var(${a.token}) ${share}%, var(${b.token}))`;
}

/** How much of its band a task carries, 0 to 1: heat is light intensity. */
export function intensityOf(v: number | null | undefined): number {
  if (v === null || v === undefined) return 0;
  return Math.max(0, Math.min(1, v));
}
