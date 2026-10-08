import { describe, expect, it } from 'vitest';
import { atMinute } from '../../shared/time/zone';
import type { CommitmentOccurrence, CommitmentsSnapshot } from '../client';
import { nextGap } from '../today/gap';
import { busyEvents, sleepSpans } from './busy';

// docs/COMMITMENTS.md, "Planning". What a fail looks like: P landing in a
// class, in the travel before or after it, or in the night; sleep that runs
// over midnight read as one stretch; a day with no commitments losing time.

const NY = 'America/New_York';
const DAY = '2026-10-07';

const occurrence = (over: Partial<CommitmentOccurrence>): CommitmentOccurrence => ({
  commitmentId: 'c-1',
  date: DAY,
  start: 600,
  end: 650,
  title: 'JPN 101',
  kind: 'class',
  location: 'Swan Hall 201',
  bufferBefore: 0,
  bufferAfter: 0,
  hardness: 'fixed',
  hue: 210,
  label: null,
  ...over,
});

const commitments = (over: Partial<CommitmentsSnapshot>): CommitmentsSnapshot => ({
  days: {},
  list: [],
  next: null,
  free: { date: DAY, freeMin: 0, plannedMin: 0, overMin: 0, spans: [], line: '' },
  conflicts: [],
  sleep: { from: 0, to: 0 },
  term: { start: null, end: null },
  drafts: [],
  pending: [],
  feeds: [],
  ...over,
});

const minutes = (events: { start: number; end: number }[]) =>
  events.map((e) => [(e.start - atMinute(DAY, 0, NY)) / 60_000, (e.end - atMinute(DAY, 0, NY)) / 60_000]);

describe('sleep', () => {
  it('is two stretches of a day when bed is before midnight, and one when it is after', () => {
    expect(sleepSpans({ from: 1380, to: 420 })).toEqual([
      [0, 420],
      [1380, 1440],
    ]);
    expect(sleepSpans({ from: 60, to: 540 })).toEqual([[60, 540]]);
    // To bed at midnight sharp: nothing before it, the morning after it.
    expect(sleepSpans({ from: 0, to: 420 })).toEqual([[0, 420]]);
    expect(sleepSpans({ from: 420, to: 420 })).toEqual([]);
    expect(sleepSpans(undefined)).toEqual([]);
  });
});

describe('what P keeps clear of', () => {
  it('is nothing without the snapshot’s commitments', () => {
    expect(busyEvents(undefined, DAY, NY)).toEqual([]);
    expect(busyEvents(commitments({}), DAY, NY)).toEqual([]);
  });

  it('holds a commitment from the start of its travel to the end of it, on its own day only', () => {
    const c = commitments({
      days: {
        [DAY]: [occurrence({ bufferBefore: 25, bufferAfter: 10 })],
        '2026-10-08': [occurrence({ date: '2026-10-08', start: 840, end: 900 })],
      },
    });
    const events = busyEvents(c, DAY, NY);
    expect(minutes(events)).toEqual([[575, 660]]);
    expect(events[0]).toMatchObject({ title: 'JPN 101', allDay: false });
    expect(minutes(busyEvents(c, '2026-10-09', NY))).toEqual([]);
  });

  it('never reaches past the day’s two midnights', () => {
    const c = commitments({
      days: {
        [DAY]: [
          occurrence({ start: 10, end: 60, bufferBefore: 30 }),
          occurrence({ commitmentId: 'c-2', start: 1380, end: 1430, bufferAfter: 30 }),
        ],
      },
    });
    expect(minutes(busyEvents(c, DAY, NY))).toEqual([
      [0, 60],
      [1380, 1440],
    ]);
  });

  it('adds the minutes asleep', () => {
    const c = commitments({ sleep: { from: 1380, to: 420 } });
    expect(minutes(busyEvents(c, DAY, NY))).toEqual([
      [0, 420],
      [1380, 1440],
    ]);
  });

  it('moves the next free gap past a class, its travel and the night', () => {
    const c = commitments({
      days: { [DAY]: [occurrence({ bufferBefore: 25, bufferAfter: 10 })] },
      sleep: { from: 1380, to: 420 },
    });
    const busy = busyEvents(c, DAY, NY);
    // 9:30 AM, an hour to place: the travel starts at 9:35, so the first gap is 11:00, after the travel back.
    expect(nextGap([], [], DAY, NY, 570, 60)).toBe(570);
    expect(nextGap([], busy, DAY, NY, 570, 60)).toBe(660);
    // 10:15 PM: an hour would run into sleep at 11.
    expect(nextGap([], [], DAY, NY, 1335, 60)).toBe(1335);
    expect(nextGap([], busy, DAY, NY, 1335, 60)).toBeNull();
  });
});
