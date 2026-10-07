import { describe, expect, test } from 'vitest';
import { ringAngle, ringOf } from './orbits';

// Adversarial review of build/spacemodel. Each test here exposes a defect
// the review found; it is skipped so the suite stays green, and runs with
// REVIEW_RUN_KNOWN=1 to watch it fail.
const env = (globalThis as { process?: { env: Record<string, string | undefined> } }).process?.env;
const known = env?.REVIEW_RUN_KNOWN ? test : test.skip;

describe('review: the stagger fix of QUESTIONS #72', () => {
  // Finding: the third-of-a-seat stagger keeps FULL rings apart, but a
  // part-full ring spreads its worlds over the whole circle, so with 10,
  // 13, 17 or 20 worlds one world sits on exactly the same ray from the sun
  // as a world on another ring (10 worlds: seat 5 on ring 0 and seat 9 on
  // ring 1). A ten-song album, the commonest size, opens with a spoke.
  known('no two worlds on different rings share a ray at t = 0, for any population', () => {
    const aligned: string[] = [];
    for (let n = 8; n <= 21; n++) {
      for (let a = 0; a < n; a++) {
        for (let b = a + 1; b < n; b++) {
          if (ringOf(a) === ringOf(b)) continue;
          let gap = Math.abs(ringAngle(a, n, 'seed') - ringAngle(b, n, 'seed')) % (2 * Math.PI);
          gap = Math.min(gap, 2 * Math.PI - gap);
          if (gap < 1e-9) aligned.push(`${n} worlds: seats ${a} and ${b}`);
        }
      }
    }
    expect(aligned).toEqual([]);
  });
});
