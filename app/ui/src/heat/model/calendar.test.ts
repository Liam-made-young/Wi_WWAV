import { describe, expect, it } from 'vitest';
import {
  type CalendarData,
  GRID_HOUR_PX,
  calendarTitle,
  dayLayout,
  dueAtEndOfDay,
  monthGrid,
  page,
  unscheduledTray,
  weekDays,
} from './calendar';
import type { CalendarEvent, Milestone } from './records';
import { NY, block, ny, task } from './testkit';

const now = ny('2026-10-06 08:40');

function data(over: Partial<CalendarData> = {}): CalendarData {
  return { tasks: [], occurrences: [], blocks: [], events: [], milestones: [], ...over };
}

describe('the month grid (3.1)', () => {
  it('runs six weeks from the Sunday before the 1st, and circles today', () => {
    const cells = monthGrid('2026-10-15', data(), now, NY);
    expect(cells).toHaveLength(42);
    expect(cells[0].date).toBe('2026-09-27');
    expect(cells[41].date).toBe('2026-11-07');
    expect(cells.filter((c) => c.inMonth)).toHaveLength(31);
    expect(cells.filter((c) => c.isToday).map((c) => c.date)).toEqual(['2026-10-06']);
  });

  it('shows 3 pills a cell in heat order with their heat colour, then "N more"', () => {
    const due = (h: number, difficulty: number, title: string) =>
      task({ title, difficulty, due: ny(`2026-10-08 ${String(h).padStart(2, '0')}:00`) });
    const tasks = [due(9, 1, 'a'), due(10, 5, 'b'), due(11, 2, 'c'), due(12, 1, 'd'), due(13, 3, 'e')];
    const cell = monthGrid('2026-10-01', data({ tasks }), now, NY).find((c) => c.date === '2026-10-08')!;
    expect(cell.pills.map((p) => [p.title, p.level, p.colour])).toEqual([
      ['b', 'Hot', '#e0402c'],
      ['e', 'Warm', '#efa431'],
      ['c', 'Warm', '#efa431'],
    ]);
    expect(cell.more).toBe('2 more');
    expect(monthGrid('2026-10-01', data({ tasks: tasks.slice(0, 3) }), now, NY)[11].more).toBeNull();
  });

  it('puts a recurring task on each of its days, and shows done tasks last without a colour', () => {
    const weekly = task({ title: 'Weekly quiz', due: ny('2026-10-02 23:59'), rrule: 'FREQ=WEEKLY' });
    const shut = task({ title: 'Shipped', done: true, doneAt: now, due: ny('2026-10-09 12:00') });
    const cells = monthGrid('2026-10-01', data({ tasks: [weekly, shut] }), now, NY);
    const fridays = cells.filter((c) => c.pills.some((p) => p.title === 'Weekly quiz')).map((c) => c.date);
    expect(fridays).toEqual(['2026-10-02', '2026-10-09', '2026-10-16', '2026-10-23', '2026-10-30', '2026-11-06']);
    const ninth = cells.find((c) => c.date === '2026-10-09')!;
    expect(ninth.pills.map((p) => [p.title, p.colour])).toEqual([
      ['Weekly quiz', '#efa431'],
      ['Shipped', null],
    ]);
  });
});

describe('the week and day views (3.7)', () => {
  it('gives the week from Sunday', () => {
    expect(weekDays('2026-10-08')).toEqual([
      '2026-10-04',
      '2026-10-05',
      '2026-10-06',
      '2026-10-07',
      '2026-10-08',
      '2026-10-09',
      '2026-10-10',
    ]);
  });

  it('lays out events behind blocks, and a heat-coloured due flag at its time', () => {
    const quiz = task({ title: 'Grammar quiz 4', difficulty: 2, due: ny('2026-10-07 23:59') });
    const lecture: CalendarEvent = { id: 'e1', title: 'JPN 201', start: ny('2026-10-07 09:00'), end: ny('2026-10-07 10:15'), allDay: false };
    const overnight: CalendarEvent = { id: 'e2', title: 'Flight', start: ny('2026-10-06 22:00'), end: ny('2026-10-07 01:00'), allDay: false };
    const holiday: CalendarEvent = { id: 'e3', title: 'Fall break', start: ny('2026-10-07 00:00'), end: ny('2026-10-08 00:00'), allDay: true };
    const bead: Milestone = { id: 'm1', spaceId: 'wwav', title: 'Enclosure v2', date: '2026-10-07', done: false, order: 1 };
    const b = block({ taskId: quiz.id, date: '2026-10-07', start: 9 * 60 + 30, minutes: 45 });
    const layout = dayLayout('2026-10-07', data({ tasks: [quiz], events: [lecture, overnight, holiday], blocks: [b], milestones: [bead] }), now, NY);
    expect(layout.items.map((i) => [i.kind, i.title, i.top, i.height, i.colour])).toEqual([
      ['event', 'Flight', 0, GRID_HOUR_PX, null],
      ['event', 'JPN 201', 9 * GRID_HOUR_PX, 1.25 * GRID_HOUR_PX, null],
      ['block', 'Grammar quiz 4', 9.5 * GRID_HOUR_PX, 0.75 * GRID_HOUR_PX, '#efa431'],
    ]);
    expect(layout.dueFlags).toEqual([
      { taskId: quiz.id, title: 'Grammar quiz 4', top: ((23 * 60 + 59) / 60) * GRID_HOUR_PX, label: 'due 11:59 PM', colour: '#efa431' },
    ]);
    expect(layout.allDay).toEqual({
      due: [{ taskId: quiz.id, title: 'Grammar quiz 4', colour: '#efa431' }],
      beads: [bead],
      events: [holiday],
    });
  });

  it('never draws a Brightspace item as a grey event', () => {
    const synced = task({ source: 'ical', due: ny('2026-10-07 23:59') });
    const layout = dayLayout('2026-10-07', data({ tasks: [synced] }), now, NY);
    expect(layout.items).toEqual([]);
    expect(layout.dueFlags).toHaveLength(1);
  });

  it('keeps a short block tall enough to read', () => {
    const b = block({ date: '2026-10-07', start: 9 * 60, minutes: 15, taskId: 'x' });
    const layout = dayLayout('2026-10-07', data({ tasks: [task({ id: 'x' })], blocks: [b] }), now, NY);
    expect(layout.items[0].height).toBe(16);
  });
});

describe('the unscheduled tray (3.7)', () => {
  it('lists this week’s open tasks with no block, in heat order', () => {
    const blocked = task({ title: 'blocked', due: ny('2026-10-08 23:59') });
    const hot = task({ title: 'hot', difficulty: 3, due: ny('2026-10-07 12:00') });
    const warm = task({ title: 'warm', difficulty: 3, due: ny('2026-10-10 12:00') });
    const scheduled = task({ title: 'scheduled', scheduledDate: '2026-10-09' });
    const next = task({ title: 'next week', due: ny('2026-10-12 12:00') });
    const done = task({ title: 'done', done: true, doneAt: now, due: ny('2026-10-08 12:00') });
    const tray = unscheduledTray(
      '2026-10-06',
      data({ tasks: [blocked, warm, scheduled, next, done, hot], blocks: [block({ taskId: blocked.id, date: '2026-10-05' })] }),
      now,
      NY,
    );
    expect(tray.map((t) => t.title)).toEqual(['hot', 'warm', 'scheduled']);
  });
});

describe('moving around', () => {
  it('pages a month, a week or a day', () => {
    expect(page('month', '2026-10-31', 1)).toBe('2026-11-01');
    expect(page('month', '2026-01-15', -1)).toBe('2025-12-01');
    expect(page('week', '2026-10-06', 1)).toBe('2026-10-13');
    expect(page('day', '2026-10-06', -1)).toBe('2026-10-05');
  });

  it('titles each view', () => {
    expect(calendarTitle('month', '2026-10-06')).toBe('October 2026');
    expect(calendarTitle('week', '2026-10-06')).toBe('Oct 4 – Oct 10');
    expect(calendarTitle('day', '2026-10-06')).toBe('Tuesday, October 6');
  });

  it('"+" adds a task due 11:59 PM on the selected day', () => {
    expect(dueAtEndOfDay('2026-11-01', NY)).toBe(ny('2026-11-01 23:59'));
  });
});
