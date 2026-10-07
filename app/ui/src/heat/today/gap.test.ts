import { describe, expect, it } from 'vitest';
import type { CalendarEvent, TimeBlock } from '../client';
import { epochOf } from '../../shared/time/zone';
import {
  blockLength,
  busySpans,
  COLUMN_HEIGHT,
  dropStart,
  firstGap,
  initialScroll,
  minutesToY,
  movedStart,
  nextGap,
  nowLineY,
  resizedMinutes,
  snap,
  yToMinutes,
} from './gap';

// docs/SPEC.md 3.5. What a fail looks like: a column that isn't 44 px an hour
// from 7 AM to midnight; a block that isn't snapped to 15 minutes; P that
// lands on a block or an event, before now, or in a gap too short; a drop,
// move or resize that leaves the column.

const NY = 'America/New_York';
const block = (start: number, minutes: number, date = '2026-10-07'): TimeBlock => ({
  id: `${date}-${start}`,
  taskId: 't',
  date,
  start,
  minutes,
  origin: 'you',
});
const at = (h: number, m = 0, day = 7) => epochOf({ year: 2026, month: 10, day, hour: h, minute: m }, NY);
const event = (from: number, to: number, allDay = false): CalendarEvent => ({
  id: `e${from}`,
  title: 'x',
  start: from,
  end: to,
  allDay,
});

describe('the column’s scale', () => {
  it('is 44 px an hour from 7 AM to midnight', () => {
    expect(COLUMN_HEIGHT).toBe(748);
    expect(minutesToY(7 * 60)).toBe(0);
    expect(minutesToY(8 * 60)).toBe(44);
    expect(minutesToY(24 * 60)).toBe(748);
    expect(yToMinutes(44)).toBe(8 * 60);
  });

  it('snaps to the nearest quarter hour', () => {
    expect([snap(607), snap(608), snap(622), snap(623)]).toEqual([600, 615, 615, 630]);
  });

  it('rounds a block up to 15 minutes, with a floor of 15', () => {
    expect([
      blockLength(1),
      blockLength(15),
      blockLength(16),
      blockLength(45),
      blockLength(50),
      blockLength(0),
    ]).toEqual([15, 15, 30, 45, 60, 15]);
  });

  it('puts the now line in the column only, and scrolls so now sits a third of the way down', () => {
    expect(nowLineY(6 * 60)).toBeNull();
    expect(nowLineY(7 * 60)).toBe(0);
    expect(nowLineY(23 * 60 + 59)).toBeCloseTo(minutesToY(23 * 60 + 59));
    expect(nowLineY(24 * 60)).toBeNull();
    expect(initialScroll(14 * 60, 300)).toBe(minutesToY(14 * 60) - 100);
    expect(initialScroll(7 * 60, 300)).toBe(0);
    expect(initialScroll(23 * 60, 300)).toBe(COLUMN_HEIGHT - 300);
  });
});

describe('P, the next free gap', () => {
  it('is now, rounded up to the grid, when nothing is in the way', () => {
    expect(nextGap([], [], '2026-10-07', NY, 10 * 60 + 2, 60)).toBe(10 * 60 + 15);
    expect(nextGap([], [], '2026-10-07', NY, 10 * 60, 60)).toBe(10 * 60);
  });

  it('never starts before 7 AM, whatever now says', () => {
    expect(nextGap([], [], '2026-10-07', NY, 3 * 60, 30)).toBe(7 * 60);
  });

  it('steps past a block, and past an event, and fits a task between them only if it is short enough', () => {
    const blocks = [block(9 * 60, 60), block(11 * 60, 60)];
    expect(nextGap(blocks, [], '2026-10-07', NY, 9 * 60, 30)).toBe(10 * 60);
    expect(nextGap(blocks, [], '2026-10-07', NY, 9 * 60, 60)).toBe(10 * 60);
    expect(nextGap(blocks, [], '2026-10-07', NY, 9 * 60, 75)).toBe(12 * 60);
    const events = [event(at(10), at(11, 15))];
    expect(nextGap([], events, '2026-10-07', NY, 10 * 60, 60)).toBe(11 * 60 + 15);
  });

  it('ignores another day’s blocks and an all-day event', () => {
    expect(
      nextGap([block(10 * 60, 60, '2026-10-08')], [event(at(0), at(23), true)], '2026-10-07', NY, 10 * 60, 60),
    ).toBe(10 * 60);
  });

  it('finds nothing when the day is out of room, and a block that ends at midnight fits', () => {
    expect(nextGap([], [], '2026-10-07', NY, 23 * 60 + 30, 60)).toBeNull();
    expect(nextGap([], [], '2026-10-07', NY, 23 * 60, 60)).toBe(23 * 60);
  });

  it('counts an event that began yesterday or ends tomorrow for the part that is today', () => {
    const spans = busySpans([], [event(at(22, 0, 6), at(8, 30))], '2026-10-07', NY);
    expect(spans).toEqual([[0, 8 * 60 + 30]]);
    expect(busySpans([], [event(at(22), at(2, 0, 8))], '2026-10-07', NY)).toEqual([[22 * 60, 1440]]);
  });

  it('finds the first mark in a list of spans', () => {
    expect(
      firstGap(
        [
          [0, 600],
          [630, 700],
        ],
        480,
        1440,
        30,
      ),
    ).toBe(600);
    expect(
      firstGap(
        [
          [0, 600],
          [630, 700],
        ],
        480,
        1440,
        45,
      ),
    ).toBe(705); // the grid is 15 minutes: 700 is between marks
  });
});

describe('dropping, moving and resizing', () => {
  it('starts a dropped block on the mark nearest the pointer, inside the column', () => {
    expect(dropStart(minutesToY(16 * 60) + 3, 60)).toBe(16 * 60);
    expect(dropStart(-50, 60)).toBe(7 * 60);
    expect(dropStart(COLUMN_HEIGHT + 100, 60)).toBe(23 * 60);
  });

  it('resizes by the bottom edge in 15-minute steps, never under 15 or past midnight', () => {
    expect(resizedMinutes(13 * 60, minutesToY(14 * 60 + 30))).toBe(90);
    expect(resizedMinutes(13 * 60, minutesToY(13 * 60 + 2))).toBe(15);
    expect(resizedMinutes(13 * 60, minutesToY(12 * 60))).toBe(15);
    expect(resizedMinutes(23 * 60, COLUMN_HEIGHT + 400)).toBe(60);
  });

  it('moves by the top edge, keeping its length inside the column', () => {
    expect(movedStart(60, minutesToY(9 * 60 + 8))).toBe(9 * 60 + 15);
    expect(movedStart(60, -300)).toBe(7 * 60);
    expect(movedStart(90, COLUMN_HEIGHT)).toBe(24 * 60 - 90);
  });
});
