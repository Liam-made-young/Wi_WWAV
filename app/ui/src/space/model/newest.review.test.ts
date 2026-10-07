import { describe, expect, test } from 'vitest';
import { filterNewest, type NewestItem } from './newest';

// Adversarial review of build/spacemodel. Each test here exposes a defect
// the review found; they are skipped so the suite stays green, and run
// with REVIEW_RUN_KNOWN=1 to watch them fail.
const env = (globalThis as { process?: { env: Record<string, string | undefined> } }).process?.env;
const known = env?.REVIEW_RUN_KNOWN ? test : test.skip;

function item(id: string, createdAt: string): NewestItem {
  return { id, createdAt, title: id, artist: 'LMY', galaxyId: 1, medium: 'song', key: null, bpm: null };
}

describe('review: Newest is newest first', () => {
  // Finding: filterNewest orders createdAt as strings ("ISO times in one
  // zone compare as strings"). Valid ISO 8601 times that differ only in
  // fractional seconds or in zone sort in the wrong order, which is S4.6's
  // "any order but newest".
  known('half a second later is newer, with or without milliseconds', () => {
    const later = item('later', '2026-10-04T12:00:00.500Z');
    const earlier = item('earlier', '2026-10-04T12:00:00Z');
    expect(filterNewest([earlier, later], {}, new Set()).map((i) => i.id)).toEqual(['later', 'earlier']);
  });

  known('a time written with an offset is ordered by the instant it names', () => {
    const later = item('later', '2026-10-04T20:00:00-07:00'); // 2026-10-05T03:00Z
    const earlier = item('earlier', '2026-10-05T01:00:00Z');
    expect(filterNewest([earlier, later], {}, new Set()).map((i) => i.id)).toEqual(['later', 'earlier']);
  });
});
