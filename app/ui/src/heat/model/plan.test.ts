import { describe, expect, it } from 'vitest';
import { estimateContext, estimateMin } from './estimate';
import {
  COLUMN_HEIGHT,
  DEFAULT_BLOCK_MIN,
  HOUR_PX,
  type PlanData,
  blockEndsLine,
  dragOntoColumn,
  draftKey,
  initialScrollTop,
  minutesToY,
  moveBlock,
  nowLineY,
  planHeader,
  planMyDay,
  planNext,
  planSections,
  planSubtitle,
  resizeBlock,
  snap,
  yToMinutes,
} from './plan';
import type { CalendarEvent, TaskOccurrence } from './records';
import { NY, block, habit, ids, ny, task } from './testkit';

const TODAY = '2026-10-06';
const hm = (h: number, m = 0) => h * 60 + m;

function day(over: Partial<PlanData> = {}): PlanData {
  return { tasks: [], occurrences: [], blocks: [], events: [], sessions: [], habits: [], ...over };
}

function event(start: string, end: string, over: Partial<CalendarEvent> = {}): CalendarEvent {
  return { id: `ev-${start}`, title: 'Class', start: ny(start), end: ny(end), allDay: false, ...over };
}

describe('the time column scale (3.5, from the PKM calendar)', () => {
  it('is 44 px an hour from 7 AM to midnight', () => {
    expect(HOUR_PX).toBe(44);
    expect(minutesToY(hm(7))).toBe(0);
    expect(minutesToY(hm(8))).toBe(44);
    expect(minutesToY(hm(24))).toBe(748);
    expect(COLUMN_HEIGHT).toBe(748);
    expect(yToMinutes(66)).toBe(hm(8, 30));
  });

  it('snaps to 15 minutes, to the nearest mark, and blocks default to 30 minutes', () => {
    expect(snap(hm(8, 7))).toBe(hm(8));
    expect(snap(hm(8, 8))).toBe(hm(8, 15));
    expect(DEFAULT_BLOCK_MIN).toBe(30);
  });

  it('scrolls so now sits a third of the way down', () => {
    expect(initialScrollTop(hm(14), 600)).toBe(minutesToY(hm(14)) - 200);
    expect(initialScrollTop(hm(7, 30), 600)).toBe(0);
    expect(initialScrollTop(hm(23, 30), 600)).toBe(COLUMN_HEIGHT - 600);
  });

  it('draws the red now line only inside the column', () => {
    expect(nowLineY(hm(9, 30))).toBe(110);
    expect(nowLineY(hm(6, 59))).toBeNull();
  });
});

describe('Plan my day (3.5)', () => {
  const now = ny('2026-10-06 08:41');

  it('places unplanned open tasks in heat order, each in the first gap that fits', () => {
    const hot = task({ title: 'hot', difficulty: 3, due: ny('2026-10-06 16:00'), estMin: 45 });
    const warm = task({ title: 'warm', difficulty: 3, due: ny('2026-10-08 23:59'), estMin: 10 });
    const cool = task({ title: 'cool', difficulty: 1, due: ny('2026-10-20 23:59'), estMin: 130 });
    const undated = task({ title: 'undated', estMin: 60 });
    const drafts = planMyDay(
      day({
        tasks: [undated, cool, warm, hot],
        events: [event('2026-10-06 09:00', '2026-10-06 10:00')],
        blocks: [block({ date: TODAY, start: hm(13), minutes: 60 })],
      }),
      now,
      NY,
    );
    expect(drafts.map((d) => [d.taskId, d.start, d.minutes, d.leftMin])).toEqual([
      // 8:45 to 9:00 is too short for 45m; 10:00 is the first gap that fits.
      [hot.id, hm(10), 45, 0],
      // 10m rounds up to 15, and 8:45 to 9:00 fits it.
      [warm.id, hm(8, 45), 15, 0],
      // 130m rounds up to 135, capped at 90: "45m left to plan".
      [cool.id, hm(10, 45), 90, 45],
      // 12:15 to 13:00 is too short for 60m; the block ends at 14:00.
      [undated.id, hm(14), 60, 0],
    ]);
    expect(drafts.every((d) => d.date === TODAY)).toBe(true);
    expect(drafts[2].leftLine).toBe('45m left to plan');
    expect(drafts[0].leftLine).toBeNull();
  });

  it('gives each draft its reason', () => {
    const quiz = task({ title: 'Grammar quiz 4', difficulty: 2, due: ny('2026-10-07 23:59'), estMin: 45 });
    const hot = task({ difficulty: 2, due: ny('2026-10-07 09:00'), estMin: 15 });
    const thursday = task({ difficulty: 3, due: ny('2026-10-08 23:59'), estMin: 15 });
    const late = task({ due: ny('2026-10-04 23:59'), estMin: 15 });
    const undated = task({ estMin: 15 });
    const reasons = Object.fromEntries(
      planMyDay(day({ tasks: [quiz, hot, thursday, late, undated] }), now, NY).map((d) => [d.taskId, d.reason]),
    );
    // 1.6: "Due tomorrow 11:59 PM, Warm."
    expect(reasons[quiz.id]).toBe('Due tomorrow 11:59 PM, Warm.');
    // 3.5: "Due tomorrow 11:59 PM, Hot." has the same shape.
    expect(reasons[hot.id]).toBe('Due tomorrow 9:00 AM, Hot.');
    expect(reasons[thursday.id]).toBe('Due Thursday 11:59 PM, Warm.');
    expect(reasons[late.id]).toBe('1d overdue.');
    expect(reasons[undated.id]).toBe('No due date.');
  });

  it('rounds each estimate up to 15 minutes, never less than 15', () => {
    const tasks = [46, 15, 1].map((estMin, i) => task({ estMin, due: ny('2026-10-06 20:00') + i }));
    expect(planMyDay(day({ tasks }), now, NY).map((d) => d.minutes)).toEqual([60, 15, 15]);
  });

  it('uses the estimate chain when a task has no estMin', () => {
    const t = task({ difficulty: 2, estMin: null, due: ny('2026-10-07 23:59') });
    expect(planMyDay(day({ tasks: [t] }), now, NY)[0].minutes).toBe(45); // 40m rounds up to 45
  });

  it('leaves out done tasks, tasks already planned today, and parents of open subtasks', () => {
    const done = task({ done: true, doneAt: now, estMin: 15 });
    const plannedToday = task({ estMin: 15 });
    const plannedTomorrow = task({ estMin: 15 });
    const parent = task({ estMin: 15 });
    const child = task({ parentTaskId: parent.id, estMin: 15 });
    const drafts = planMyDay(
      day({
        tasks: [done, plannedToday, plannedTomorrow, parent, child],
        blocks: [
          block({ taskId: plannedToday.id, date: TODAY, start: hm(20) }),
          block({ taskId: plannedTomorrow.id, date: '2026-10-07', start: hm(20) }),
        ],
      }),
      now,
      NY,
    );
    expect(drafts.map((d) => d.taskId).sort()).toEqual([plannedTomorrow.id, child.id].sort());
  });

  it('stops at "Day ends at", 11 PM by default', () => {
    const late = ny('2026-10-06 22:20');
    const long = task({ estMin: 45, due: ny('2026-10-07 09:00') });
    const short = task({ estMin: 30, due: ny('2026-10-07 10:00') });
    expect(planMyDay(day({ tasks: [long, short] }), late, NY).map((d) => [d.taskId, d.start])).toEqual([
      [short.id, hm(22, 30)],
    ]);
    expect(planMyDay(day({ tasks: [long, short] }), now, NY, { dayEndsAt: hm(9, 30) }).map((d) => d.taskId)).toEqual([
      long.id,
    ]);
    expect(planMyDay(day({ tasks: [short] }), ny('2026-10-06 23:10'), NY)).toEqual([]);
  });

  it('works around timed events, including one from yesterday, but not all-day ones', () => {
    const t = task({ estMin: 60, due: ny('2026-10-07 09:00') });
    const drafts = planMyDay(
      day({
        tasks: [t],
        events: [
          event('2026-10-05 22:00', '2026-10-06 10:00', { title: 'Overnight' }),
          event('2026-10-06 00:00', '2026-10-07 00:00', { allDay: true }),
        ],
      }),
      now,
      NY,
    );
    expect(drafts[0].start).toBe(hm(10));
  });

  it('plans only the selected space’s tasks, but steers round every block', () => {
    const mine = task({ spaceId: 'wwav', estMin: 30, due: ny('2026-10-07 09:00') });
    const other = task({ spaceId: 'classes', estMin: 30, due: ny('2026-10-06 12:00') });
    const drafts = planMyDay(
      day({ tasks: [mine, other], blocks: [block({ taskId: other.id, date: TODAY, start: hm(8, 45), minutes: 60 })] }),
      now,
      NY,
      { spaceId: 'wwav' },
    );
    expect(drafts.map((d) => [d.taskId, d.start])).toEqual([[mine.id, hm(9, 45)]]);
  });

  it('accepts all on Return, clears on Esc, and ignores other keys', () => {
    const tasks = [task({ estMin: 30, due: ny('2026-10-07 09:00') }), task({ estMin: 15, due: ny('2026-10-07 10:00') })];
    const drafts = planMyDay(day({ tasks }), now, NY);
    const accepted = draftKey(drafts, 'Enter', ids('blk'))!;
    expect(accepted.drafts).toEqual([]);
    expect(accepted.blocks).toEqual([
      { id: 'blk-1', taskId: tasks[0].id, date: TODAY, start: hm(8, 45), minutes: 30, origin: 'plan' },
      { id: 'blk-2', taskId: tasks[1].id, date: TODAY, start: hm(9, 15), minutes: 15, origin: 'plan' },
    ]);
    expect(draftKey(drafts, 'Escape', ids())).toEqual({ drafts: [], blocks: [] });
    expect(draftKey(drafts, 'p', ids())).toBeNull();
    expect(draftKey([], 'Enter', ids())).toBeNull();
  });
});

describe('making and changing blocks by hand', () => {
  const now = ny('2026-10-06 08:41');

  it('P puts a task in the next free gap after now, as long as its estimate rounded up to 15', () => {
    const t = task({ estMin: 50 });
    const data = day({ tasks: [t], events: [event('2026-10-06 09:00', '2026-10-06 10:00')] });
    expect(planNext({ taskId: t.id }, data, now, NY, ids('blk'))).toEqual({
      id: 'blk-1',
      taskId: t.id,
      date: TODAY,
      start: hm(10),
      minutes: 60,
      origin: 'you',
    });
  });

  it('P blocks a habit for its length, or 30 minutes without one', () => {
    const short = habit({ minutes: 15 });
    const plain = habit();
    const data = day({ habits: [short, plain], events: [event('2026-10-06 09:00', '2026-10-06 10:00')] });
    expect(planNext({ habitId: short.id }, data, now, NY, ids())?.start).toBe(hm(8, 45));
    expect(planNext({ habitId: plain.id }, data, now, NY, ids())).toMatchObject({ start: hm(10), minutes: 30 });
  });

  it('P finds nothing when the day is full', () => {
    const t = task({ estMin: 90 });
    expect(planNext({ taskId: t.id }, day({ tasks: [t] }), ny('2026-10-06 23:00'), NY, ids())).toBeNull();
  });

  it('dragging a task onto the column makes a block as long as its estimate, rounded up to 15', () => {
    const t = task({ estMin: 50 });
    expect(dragOntoColumn({ taskId: t.id }, minutesToY(hm(9, 7)), TODAY, day({ tasks: [t] }), ids('blk'))).toEqual({
      id: 'blk-1',
      taskId: t.id,
      date: TODAY,
      start: hm(9),
      minutes: 60,
      origin: 'you',
    });
    // Dropped too low, it ends at midnight rather than past it.
    expect(dragOntoColumn({ taskId: t.id }, minutesToY(hm(23, 30)), TODAY, day({ tasks: [t] }), ids())?.start).toBe(hm(23));
  });

  it('resizing changes the block, never the estimate', () => {
    const t = task({ estMin: 30 });
    const b = block({ taskId: t.id, start: hm(9), minutes: 30 });
    const before = estimateMin(t, estimateContext([t], []));
    expect(resizeBlock(b, minutesToY(hm(10, 10)))).toEqual({ ...b, minutes: 75 });
    expect(resizeBlock(b, minutesToY(hm(9, 2)))).toEqual({ ...b, minutes: 15 });
    expect(resizeBlock({ ...b, start: hm(23) }, minutesToY(hm(24)) + 200)).toEqual({ ...b, start: hm(23), minutes: 60 });
    expect(t.estMin).toBe(30);
    expect(estimateMin(t, estimateContext([t], []))).toBe(before);
  });

  it('moving a block snaps its start and keeps it inside the column', () => {
    const b = block({ start: hm(9), minutes: 60 });
    expect(moveBlock(b, minutesToY(hm(13, 5)))).toEqual({ ...b, start: hm(13) });
    expect(moveBlock(b, -50)).toEqual({ ...b, start: hm(7) });
    expect(moveBlock(b, COLUMN_HEIGHT)).toEqual({ ...b, start: hm(23) });
  });

  it('says when the current block ends', () => {
    expect(blockEndsLine(block({ start: hm(14), minutes: 90 }))).toBe('Block ends 3:30 PM');
  });
});

describe('the plan list (3.5)', () => {
  const now = ny('2026-10-06 10:15');

  it('has four sections, Planned in time order with start times, and hides the empty ones', () => {
    const a = task({ title: 'Read chapter 3', estMin: 45, due: ny('2026-10-09 23:59') });
    const b = task({ title: 'Mix the second verse', spaceId: 'wwav' });
    const dueA = task({ title: 'Problem set', difficulty: 2, due: ny('2026-10-06 16:00') });
    const dueB = task({ title: 'Lab report', difficulty: 2, due: ny('2026-10-06 23:59') });
    const doneToday = task({ title: 'Shipped', done: true, doneAt: now, due: ny('2026-10-06 12:00') });
    const scales = task({ title: 'Scales', rrule: 'FREQ=DAILY', due: ny('2026-10-01 07:00') });
    const kanji = habit({ title: 'Practise kanji', minutes: 20 });
    const stretch = habit({ title: 'Stretch' });
    const hotOnes = [1, 2, 3, 4, 5, 6].map((i) =>
      task({ title: `hot ${i}`, difficulty: 5, due: ny('2026-10-07 12:00') + i * 60_000 }),
    );
    const occ: TaskOccurrence = { id: 'o', taskId: scales.id, date: TODAY, doneAt: ny('2026-10-06 07:30') };
    const sections = planSections(
      day({
        tasks: [a, b, dueA, dueB, doneToday, scales, ...hotOnes],
        habits: [kanji, stretch],
        occurrences: [occ],
        blocks: [
          block({ taskId: b.id, date: TODAY, start: hm(14), minutes: 90 }),
          block({ taskId: a.id, date: TODAY, start: hm(9, 45), minutes: 45 }),
          block({ taskId: a.id, date: '2026-10-07', start: hm(9) }),
        ],
      }),
      now,
      NY,
    );
    expect(sections.map((s) => s.title)).toEqual([
      'Planned',
      'Due today, not planned',
      'Recurring today ↻',
      'Hot, not planned',
    ]);
    const [planned, due, recurring, hot] = sections;
    expect(planned.kind === 'planned' && planned.items.map((r) => [r.title, r.time, r.length, r.current, r.finished])).toEqual([
      ['Read chapter 3', '9:45 AM', '45m', true, false],
      ['Mix the second verse', '2:00 PM', '1h 30m', false, false],
    ]);
    expect(due.kind === 'dueToday' && due.items.map((t) => t.title)).toEqual(['Problem set', 'Lab report']);
    expect(recurring.kind === 'recurring' && recurring.items).toEqual([
      { kind: 'task', id: scales.id, title: 'Scales', done: true },
      { kind: 'habit', id: kanji.id, title: 'Practise kanji', done: false },
    ]);
    // Up to 5, and not the ones already listed as due today.
    expect(hot.kind === 'hot' && hot.items.map((t) => t.title)).toEqual(['hot 1', 'hot 2', 'hot 3', 'hot 4', 'hot 5']);
  });

  it('shows nothing at all for an empty day', () => {
    expect(planSections(day(), now, NY)).toEqual([]);
  });

  it('applies the space filter to tasks, not to habits', () => {
    const mine = task({ spaceId: 'wwav', due: ny('2026-10-06 16:00') });
    const other = task({ spaceId: 'classes', due: ny('2026-10-06 16:00') });
    const kanji = habit({ minutes: 20 });
    const sections = planSections(day({ tasks: [mine, other], habits: [kanji] }), now, NY, 'wwav');
    expect(sections.map((s) => s.items.map((i) => ('id' in i ? i.id : '')))).toEqual([[mine.id], [kanji.id]]);
  });
});

describe('the header and subtitle', () => {
  it('reads "Today, Tuesday, October 6"', () => {
    expect(planHeader(ny('2026-10-06 08:40'), NY)).toBe('Today, Tuesday, October 6');
  });

  it('reads "4 blocks · 3h 10m planned · 2 due today" (1.6)', () => {
    const now = ny('2026-10-06 08:41');
    const quiz = task({ due: ny('2026-10-06 16:00') });
    const lab = task({ due: ny('2026-10-06 23:59') });
    const shut = task({ due: ny('2026-10-06 12:00'), done: true, doneAt: now });
    const mix = task({ title: 'Mix the second verse' });
    const kanji = habit({ minutes: 25 });
    const blocks = [
      block({ taskId: quiz.id, date: TODAY, start: hm(9), minutes: 45 }),
      block({ taskId: lab.id, date: TODAY, start: hm(11), minutes: 30 }),
      block({ habitId: kanji.id, date: TODAY, start: hm(13), minutes: 25 }),
      block({ taskId: mix.id, date: TODAY, start: hm(14), minutes: 90 }),
    ];
    const tomorrow = block({ taskId: mix.id, date: '2026-10-07' });
    const data = day({ tasks: [quiz, lab, shut, mix], habits: [kanji], blocks: [...blocks, tomorrow] });
    expect(planSubtitle(data, now, NY)).toBe('4 blocks · 3h 10m planned · 2 due today');
    expect(planSubtitle(day({ tasks: [mix], blocks: [blocks[3]] }), now, NY)).toBe('1 block · 1h 30m planned · 0 due today');
  });
});
