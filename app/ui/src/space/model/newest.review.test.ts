import { describe, expect, test } from 'vitest';
import { filterNewest, type NewestItem } from './newest';

// Adversarial review of build/spacemodel. Each test here exposed a defect
// the review found; they were skipped until the defect was fixed.

function item(id: string, createdAt: string): NewestItem {
  return { id, createdAt, title: id, artist: 'LMY', galaxyId: 1, medium: 'song', key: null, bpm: null };
}

describe('review: Newest is newest first', () => {
  // Finding: filterNewest orders createdAt as strings ("ISO times in one
  // zone compare as strings"). Valid ISO 8601 times that differ only in
  // fractional seconds or in zone sort in the wrong order, which is S4.6's
  // "any order but newest".
  test('half a second later is newer, with or without milliseconds', () => {
    const later = item('later', '2026-10-04T12:00:00.500Z');
    const earlier = item('earlier', '2026-10-04T12:00:00Z');
    expect(filterNewest([earlier, later], {}, new Set()).map((i) => i.id)).toEqual(['later', 'earlier']);
  });

  test('a time written with an offset is ordered by the instant it names', () => {
    const later = item('later', '2026-10-04T20:00:00-07:00'); // 2026-10-05T03:00Z
    const earlier = item('earlier', '2026-10-05T01:00:00Z');
    expect(filterNewest([earlier, later], {}, new Set()).map((i) => i.id)).toEqual(['later', 'earlier']);
  });

  // The server orders by numeric id DESC on a tie in time, so id 10 is
  // newer than id 9, though "9" sorts after "10" as a string.
  test('a tie in time breaks by numeric id when ids are numbers', () => {
    const at = '2026-10-04T12:00:00Z';
    expect(filterNewest([item('9', at), item('10', at)], {}, new Set()).map((i) => i.id)).toEqual(['10', '9']);
  });

  test('the same instant written two ways is a tie, broken by id', () => {
    const a = item('3', '2026-10-04T12:00:00Z');
    const b = item('4', '2026-10-04T14:00:00+02:00');
    expect(filterNewest([a, b], {}, new Set()).map((i) => i.id)).toEqual(['4', '3']);
    expect(filterNewest([b, a], {}, new Set()).map((i) => i.id)).toEqual(['4', '3']);
  });
});
