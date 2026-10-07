import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { atMinute } from '../../shared/time/zone';
import { TASK_DRAG } from '../frame';
import { $, $$, button, click, mountHeat, press, type Rig, settle, type } from '../testkit';
import { daysOf, moveSelected, monthDays, page, titleOf, weekDays } from './days';
import { dueByDay } from './items';

// docs/PLAN.md S2.4 and docs/SPEC.md 3.7. What a fail looks like: a month
// that doesn't start on Sunday with six weeks, or doesn't circle today; more
// than 3 pills a cell, or no "N more"; M W D T ← → doing nothing; a deadline
// that isn't a heat-coloured flag at its time; no all-day strip, or a
// milestone missing from it; a tray that lists a task that has a block, or
// out of heat order; "+" not due 11:59 PM on the selected day; the snapshot
// not asked for the days the view shows.

let rig: Rig;
afterEach(() => rig?.unmount());

const tab = (name: string) => button(rig, name)!;
const cell = (day: string) => $(rig, `.heat-cell[data-day="${day}"]`)!;
const within = (el: Element, selector: string) => [...el.querySelectorAll<HTMLElement>(selector)];
const pills = (day: string) => within(cell(day), '.heat-pill').map((p) => p.textContent);
const title = () => $(rig, '.heat-cal-head h1')!.textContent;
const mode = () => $(rig, '.heat-cal-modes [aria-checked="true"]')!.textContent;
const undoLabel = async () => (await rig.call<{ undo: string | null }>('history.get', {})).undo;

async function openCalendar(options: Parameters<typeof mountHeat>[0] = {}) {
  rig = await mountHeat(options);
  await click(tab('Calendar'));
}

const drop = async (el: Element, id: string, y = 0) => {
  const e = Object.assign(new Event('drop', { bubbles: true, cancelable: true }), {
    clientY: y,
    dataTransfer: { types: [TASK_DRAG], getData: (t: string) => (t === TASK_DRAG ? id : ''), dropEffect: '' },
  });
  await act(async () => {
    el.dispatchEvent(e);
    await new Promise((r) => setTimeout(r, 0));
  });
  await settle();
};

describe('the month', () => {
  it('has six weeks from the Sunday before the 1st, with today in a red circle', async () => {
    await openCalendar();
    expect(title()).toBe('October 2026');
    expect($$(rig, '.heat-cell')).toHaveLength(42);
    expect($$(rig, '.heat-cell')[0].getAttribute('data-day')).toBe('2026-09-27');
    expect($$(rig, '.heat-cell')[41].getAttribute('data-day')).toBe('2026-11-07');
    expect($$(rig, '.heat-month-head').map((h) => h.textContent)).toEqual([
      'Sun',
      'Mon',
      'Tue',
      'Wed',
      'Thu',
      'Fri',
      'Sat',
    ]);
    expect($$(rig, '.heat-cell[data-today="true"]').map((c) => c.getAttribute('data-day'))).toEqual(['2026-10-07']);
    expect(cell('2026-09-27').getAttribute('data-out')).toBe('true');
    expect(cell('2026-10-07').getAttribute('data-out')).toBe('false');
    expect(mode()).toBe('Month');
  });

  it('shows a task on its due day as a pill with its heat colour, and a done task dimmed with its check', async () => {
    await openCalendar();
    expect(pills('2026-10-08')).toEqual(['Grammar quiz 4']);
    expect(within(cell('2026-10-08'), '.heat-pill')[0].getAttribute('data-level')).toBe('hot');
    expect(within(cell('2026-10-13'), '.heat-pill')[0].getAttribute('data-level')).toBe('cool');
    expect(within(cell('2026-10-10'), '.heat-pill')[0].getAttribute('data-level')).toBe('warm');
    const done = within(cell('2026-10-06'), '.heat-pill')[0];
    expect(done.textContent).toBe('✓ Read the syllabus');
    expect(done.getAttribute('data-done')).toBe('true');
  });

  it('puts a recurring task on each of its occurrences, marked ↻, and nowhere else', async () => {
    await openCalendar();
    for (const day of ['2026-10-07', '2026-10-09', '2026-10-12', '2026-10-14']) {
      expect(pills(day).some((p) => p === '↻ Listening practice')).toBe(true);
    }
    expect(pills('2026-10-08').some((p) => p!.includes('Listening'))).toBe(false);
    // Today's occurrence has the task's heat; a later one is shown without its own.
    expect(
      within(cell('2026-10-07'), '.heat-pill')
        .find((p) => p.textContent!.includes('Listening'))!
        .getAttribute('data-level'),
    ).toBe('hot');
    expect(
      within(cell('2026-10-09'), '.heat-pill')
        .find((p) => p.textContent!.includes('Listening'))!
        .getAttribute('data-level'),
    ).toBe('none');
  });

  it('draws three pills a cell and says "1 more" for a fourth, which opens that day', async () => {
    await openCalendar();
    for (const t of ['Pack the van', 'Call the venue']) {
      await rig.client.put('task', {
        spaceId: 'sp-wwav',
        title: t,
        type: 'Music',
        due: atMinute('2026-10-09', 20 * 60, 'America/New_York'),
        difficulty: 1,
        estMin: null,
        adjustMin: 0,
        notes: '',
        done: false,
        doneAt: null,
        source: 'you',
      });
    }
    await settle();
    expect(within(cell('2026-10-09'), '.heat-pill')).toHaveLength(3);
    const more = cell('2026-10-09').querySelector<HTMLElement>('.heat-more')!;
    expect(more.textContent).toBe('1 more');
    await click(more);
    expect(title()).toBe('Friday, October 9');
    expect(mode()).toBe('Day');
  });

  it('follows the space filter, but not other calendars’ events', async () => {
    await openCalendar();
    await click(button(rig, /^Personal/));
    expect(pills('2026-10-07')).toEqual([]);
    expect(pills('2026-10-12')).toEqual(['Renew passport']);
  });
});

describe('Calendar’s keys', () => {
  it('switches between Month, Week and Day on M, W and D', async () => {
    await openCalendar();
    expect(await press(rig, 'w')).toBe(true);
    expect(mode()).toBe('Week');
    expect(title()).toBe('Oct 4 – Oct 10');
    expect($$(rig, '.heat-cal-dayhead')).toHaveLength(7);
    await press(rig, 'd');
    expect(mode()).toBe('Day');
    expect(title()).toBe('Wednesday, October 7');
    expect($$(rig, '.heat-cal-dayhead')).toHaveLength(1);
    await press(rig, 'm');
    expect(mode()).toBe('Month');
  });

  it('pages on ← and →: a month to its 1st, a week, a day; and T comes back to today', async () => {
    await openCalendar();
    await press(rig, 'ArrowRight');
    expect(title()).toBe('November 2026');
    await press(rig, 'ArrowLeft');
    await press(rig, 'ArrowLeft');
    expect(title()).toBe('September 2026');
    await press(rig, 't');
    expect(title()).toBe('October 2026');
    await press(rig, 'w');
    await press(rig, 'ArrowRight');
    expect(title()).toBe('Oct 11 – Oct 17');
    await press(rig, 'd');
    await press(rig, 'ArrowLeft');
    expect(title()).toBe('Tuesday, October 13');
    await press(rig, 't');
    expect(title()).toBe('Wednesday, October 7');
  });

  it('has Today as its one secondary act, and ⇧Return runs it', async () => {
    await openCalendar();
    expect(rig.status().act).toBe('Today');
    await press(rig, 'ArrowRight');
    await act(async () => rig.view.current!.secondary());
    await settle();
    expect(title()).toBe('October 2026');
  });

  it('moves the selected day with ⇧← ⇧→ ⇧↑ ⇧↓, and the view follows it past the edge', async () => {
    await openCalendar();
    expect(cell('2026-10-07').getAttribute('data-selected')).toBe('true');
    await press(rig, 'ArrowDown', { shiftKey: true });
    expect(cell('2026-10-14').getAttribute('data-selected')).toBe('true');
    await press(rig, 'ArrowRight', { shiftKey: true });
    expect(cell('2026-10-15').getAttribute('data-selected')).toBe('true');
    for (let i = 0; i < 3; i++) await press(rig, 'ArrowDown', { shiftKey: true });
    expect(title()).toBe('November 2026');
    expect(cell('2026-11-05').getAttribute('data-selected')).toBe('true');
  });

  it('walks the tasks in view on ↑ ↓, and Get Info follows', async () => {
    await openCalendar();
    await press(rig, 'ArrowDown');
    expect($(rig, '.heat-info')!.getAttribute('aria-label')).toBe('Get Info: Read the syllabus');
    await press(rig, 'ArrowDown');
    expect($(rig, '.heat-info')!.getAttribute('aria-label')).toBe('Get Info: Kanji worksheet 7');
    expect(cell('2026-10-07').getAttribute('data-selected')).toBe('true');
  });
});

describe('"+" and the selected day', () => {
  it('adds a task due 11:59 PM on the selected day', async () => {
    await openCalendar();
    await click(cell('2026-10-14'));
    expect(cell('2026-10-14').getAttribute('data-selected')).toBe('true');
    await click($(rig, '.heat-plus'));
    const sheet = $(rig, '[role="dialog"][aria-label="New task"]')!;
    expect((sheet.querySelector('input[type="date"]') as HTMLInputElement).value).toBe('2026-10-14');
    expect((sheet.querySelector('input[type="time"]') as HTMLInputElement).value).toBe('23:59');
    await type(sheet.querySelector('input'), 'Send the stems');
    await click(within(sheet, 'button').find((b) => b.textContent === 'Add task'));
    const made = [...rig.fake.store.task.values()].find((t) => t.title === 'Send the stems')!;
    expect(made.due).toBe(atMinute('2026-10-14', 23 * 60 + 59, 'America/New_York'));
    expect(pills('2026-10-14').some((p) => p === 'Send the stems')).toBe(true);
    expect(await undoLabel()).toBe('Undo add task');
  });
});

describe('Week and Day', () => {
  const week = async () => {
    await openCalendar();
    await press(rig, 'w');
  };

  it('puts a deadline at its time as a heat-coloured flag, "due 4:00 PM"', async () => {
    await week();
    const flag = $(rig, '.heat-cal-col[data-day="2026-10-07"] [data-flag="t-kanji7"]')!;
    expect(flag.textContent).toBe('due 4:00 PM');
    expect(flag.getAttribute('data-level')).toBe('hot');
    expect(flag.style.top).toBe(`${16 * 44}px`);
    expect($(rig, '[data-flag="t-verse"]')!.getAttribute('data-level')).toBe('warm');
    expect($(rig, '[data-flag="t-syllabus"]')).toBeNull();
  });

  it('draws blocks over other calendars’ events on a 44 px grid, with the now line today', async () => {
    await week();
    const col = $(rig, '.heat-cal-col[data-day="2026-10-07"]')!;
    expect(within(col, '[data-block]').map((b) => b.getAttribute('data-block'))).toEqual([
      'b-kanji',
      'b-verse',
      'b-pset',
    ]);
    expect(within(col, '[data-event]').map((e) => e.textContent)).toEqual(['JPN 201 lecture', 'Studio time']);
    expect((col.querySelector('[data-block="b-kanji"]') as HTMLElement).style.top).toBe(`${9 * 44}px`);
    expect((col.querySelector('[data-now-line]') as HTMLElement).style.top).toBe(`${10 * 44}px`);
    expect(within($(rig, '#heat-panel-calendar')!, '[data-now-line]')).toHaveLength(1);
    expect($(rig, '.heat-cal-col[data-day="2026-10-08"] [data-event]')!.textContent).toBe('Office hours');
    expect($(rig, '.heat-cal-grid')!.style.height).toBe(`${24 * 44}px`);
  });

  it('holds due pills and milestone beads in the all-day strip', async () => {
    await week();
    expect($$(rig, '.heat-cal-strip[data-day="2026-10-08"] .heat-pill').map((p) => p.textContent)).toEqual([
      'Grammar quiz 4',
    ]);
    await press(rig, 'ArrowRight');
    await press(rig, 'ArrowRight');
    // The week of Oct 18: EP v1 mixed on the 21st, and Fall break, a grey all-day event.
    const strip = $(rig, '.heat-cal-strip[data-day="2026-10-21"]')!;
    expect(strip.querySelector('.heat-milestone')!.textContent).toBe('EP v1 mixed');
    await press(rig, 'ArrowLeft');
    expect($(rig, '.heat-cal-strip[data-day="2026-10-15"] .heat-allday-event')!.textContent).toBe('Fall break');
  });

  it('lists this week’s open tasks with no block in the tray, in heat order, and nothing already planned', async () => {
    await week();
    const tray = $$(rig, '.heat-tray [role="option"]').map((r) => r.querySelector('.heat-tray-title')!.textContent);
    expect(tray).toEqual(['Listening practice', 'Grammar quiz 4']);
    // Planning one takes it out of the tray.
    await rig.client.putBlock({ taskId: 't-quiz4', date: '2026-10-09', start: 600, minutes: 60 });
    await settle();
    expect($$(rig, '.heat-tray [role="option"]').map((r) => r.querySelector('.heat-tray-title')!.textContent)).toEqual([
      'Listening practice',
    ]);
  });

  it('turns a tray task dropped on a column into a block as long as its estimate', async () => {
    await week();
    await drop($(rig, '.heat-cal-col[data-day="2026-10-08"]')!, 't-quiz4', 9 * 44 + 2);
    const made = [...rig.fake.store.timeBlock.values()].find((b) => b.taskId === 't-quiz4')!;
    expect(made).toMatchObject({ date: '2026-10-08', start: 9 * 60, minutes: 60, origin: 'you' });
    expect(await undoLabel()).toBe('Undo add block');
    expect($$(rig, '.heat-tray [role="option"]').map((r) => r.querySelector('.heat-tray-title')!.textContent)).toEqual([
      'Listening practice',
    ]);
  });

  it('schedules a task dropped on a day’s all-day strip, and on a month cell', async () => {
    await week();
    await drop($(rig, '.heat-cal-strip[data-day="2026-10-09"]')!, 't-quiz4');
    expect(rig.fake.store.task.get('t-quiz4')!.scheduledDate).toBe('2026-10-09');
    expect(rig.status().count).toBe('Scheduled ‘Grammar quiz 4’ for Oct 9.');
    await press(rig, 'm');
    await drop(cell('2026-10-14'), 't-quiz4');
    expect(rig.fake.store.task.get('t-quiz4')!.scheduledDate).toBe('2026-10-14');
    expect(await undoLabel()).toBe('Undo edit task');
  });

  it('shows one day with the same grid, and the tray beside it', async () => {
    await openCalendar();
    await press(rig, 'd');
    expect($$(rig, '.heat-cal-col')).toHaveLength(1);
    expect($$(rig, '.heat-tray [role="option"]')).toHaveLength(2);
    expect($$(rig, '.heat-cal-col [data-block]')).toHaveLength(3);
  });

  it('selects a block, and moves it 15 minutes on ⌥↓', async () => {
    await week();
    await click(within($(rig, '#heat-panel-calendar')!, '[data-block="b-pset"]')[0]);
    expect($(rig, '.heat-info')!.getAttribute('aria-label')).toBe('Get Info: Problem set 5');
    await press(rig, 'ArrowDown', { altKey: true });
    expect(rig.fake.store.timeBlock.get('b-pset')!.start).toBe(15 * 60 + 45);
  });
});

describe('the days asked of the core', () => {
  const snapshots = () => rig.calls.filter((c) => c.cmd === 'heat.snapshot').map((c) => [c.args.from, c.args.to]);

  it('asks for the days the month shows, and always takes in today', async () => {
    await openCalendar();
    expect(snapshots().at(-1)).toEqual(['2026-09-27', '2026-11-07']);
    await press(rig, 'ArrowRight');
    expect(snapshots().at(-1)).toEqual(['2026-10-07', '2026-12-12']);
    await press(rig, 'ArrowLeft');
    await press(rig, 'ArrowLeft');
    expect(snapshots().at(-1)).toEqual(['2026-08-30', '2026-10-10']);
  });

  it('asks for the whole week behind a day, which the tray reads', async () => {
    await openCalendar();
    await press(rig, 'd');
    expect(snapshots().at(-1)).toEqual(['2026-10-04', '2026-10-10']);
  });
});

describe('the days', () => {
  it('lays out the month, the week and the day', () => {
    expect(monthDays('2026-10-20')[0]).toBe('2026-09-27');
    expect(monthDays('2026-01-10')[0]).toBe('2025-12-28');
    expect(weekDays('2026-10-07')).toEqual([
      '2026-10-04',
      '2026-10-05',
      '2026-10-06',
      '2026-10-07',
      '2026-10-08',
      '2026-10-09',
      '2026-10-10',
    ]);
    expect(daysOf('day', '2026-10-07')).toEqual(['2026-10-07']);
    expect(daysOf('month', '2026-10-07')).toHaveLength(42);
  });

  it('pages a month to its 1st across a year’s end, a week and a day', () => {
    expect(page('month', '2026-12-15', 1)).toBe('2027-01-01');
    expect(page('month', '2026-01-15', -1)).toBe('2025-12-01');
    expect(page('week', '2026-10-07', 1)).toBe('2026-10-14');
    expect(page('day', '2026-10-01', -1)).toBe('2026-09-30');
  });

  it('titles each view', () => {
    expect(titleOf('month', '2026-10-07')).toBe('October 2026');
    expect(titleOf('week', '2026-10-07')).toBe('Oct 4 – Oct 10');
    expect(titleOf('day', '2026-10-06')).toBe('Tuesday, October 6');
  });

  it('keeps the selected day in view as it moves', () => {
    expect(moveSelected('month', '2026-10-01', '2026-10-31', 1)).toEqual({
      selected: '2026-11-01',
      anchor: '2026-11-01',
    });
    expect(moveSelected('month', '2026-10-01', '2026-10-30', 1)).toEqual({
      selected: '2026-10-31',
      anchor: '2026-10-01',
    });
    expect(moveSelected('week', '2026-10-07', '2026-10-10', 1)).toEqual({
      selected: '2026-10-11',
      anchor: '2026-10-11',
    });
    expect(moveSelected('day', '2026-10-07', '2026-10-07', -7)).toEqual({
      selected: '2026-09-30',
      anchor: '2026-09-30',
    });
  });

  it('files each task by its due day, a recurring one once per occurrence, open ones in heat order', async () => {
    rig = await mountHeat();
    const snap = await rig.client.snapshot('2026-10-07');
    const days = monthDays('2026-10-07');
    const by = dueByDay(snap, days);
    expect(by.get('2026-10-07')!.map((i) => i.title)).toEqual(['Kanji worksheet 7', 'Listening practice']);
    expect(by.get('2026-10-06')!.map((i) => [i.title, i.done])).toEqual([['Read the syllabus', true]]);
    expect(
      by
        .get('2026-10-12')!
        .map((i) => i.title)
        .sort(),
    ).toEqual(['Listening practice', 'Renew passport']);
    expect(by.get('2026-10-08')![0]).toMatchObject({
      taskId: 't-quiz4',
      minute: 23 * 60 + 59,
      level: 'Hot',
      repeats: false,
    });
    // Without the snapshot's occurrences only the next one is known.
    delete snap.derived.occurrences;
    const next = dueByDay(snap, days);
    expect(next.get('2026-10-07')!.some((i) => i.repeats)).toBe(true);
    expect(next.get('2026-10-12')!.some((i) => i.repeats)).toBe(false);
  });
});
