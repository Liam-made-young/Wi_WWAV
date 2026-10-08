import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { keys } from '../../shell/platform';
import type { Commitment } from '../client';
import { addPending, draftsOf, failDraft, holdReading, readyDraft } from '../fake/commitments';
import { $, $$, button, click, escape, MORNING, mountHeat, press, type Rig, settle, type } from '../testkit';

// docs/COMMITMENTS.md against the fake core. What a fail looks like: a class
// missing from a day it meets on, or drawn on a break; travel time not
// drawn; a block over a class with no flag; Today without its NEXT line or
// its free-time line; P landing in a class; a schedule that writes anything
// before Apply, or that one ⌘Z doesn't take back; a mail's change applied
// without its tap.

let rig: Rig;
afterEach(() => rig?.unmount());

const commitments = () => [...rig.fake.store.commitment.values()];
const undoLabel = async () => (await rig.call<{ undo: string | null }>('history.get', {})).undo;
const column = (day: string) => $(rig, `.heat-cal-col[data-day="${day}"]`)!;
const sheet = (name: string) => $(rig, `[role="dialog"][aria-label="${name}"]`);
/** A sheet's field, by the words of its label. */
const field = (scope: Element, label: string) =>
  [...scope.querySelectorAll('label')]
    .find((l) => l.querySelector('span')?.textContent === label)!
    .querySelector<HTMLElement>('input, select, textarea')!;

/** Something done to the core from outside the views (Claude, a test's hook), with the renders it causes. */
async function core<T>(work: () => T | Promise<T>): Promise<T> {
  let answer!: T;
  await act(async () => {
    answer = await work();
  });
  await settle();
  return answer;
}

/** JPN 101, Monday, Wednesday and Friday at 10, with 25 minutes to get there and 10 to get back. */
async function addClass(): Promise<Commitment> {
  const r = await core(() =>
    rig.client.commitments.create({
      title: 'JPN 101',
      kind: 'class',
      days: ['MO', 'WE', 'FR'],
      start: '10:00',
      end: '10:50',
      from: '2026-09-09',
      location: 'Swan Hall 201',
      bufferBefore: 25,
      bufferAfter: 10,
      course: 'JPN 101',
    }),
  );
  return r.commitment;
}

async function openWeek() {
  await click(button(rig, 'Calendar'));
  await press(rig, 'w');
}

describe('the layer in Calendar', () => {
  it('draws a Monday, Wednesday and Friday class on those days of the week, and not on a break', async () => {
    rig = await mountHeat();
    const jpn = await addClass();
    await openWeek();
    const on = (day: string) => column(day).querySelector(`[data-commitment="${jpn.id}"]`);
    expect(['2026-10-05', '2026-10-07', '2026-10-09'].every((d) => on(d) !== null)).toBe(true);
    expect(['2026-10-04', '2026-10-06', '2026-10-08', '2026-10-10'].some((d) => on(d) !== null)).toBe(false);
    // 44 px an hour from midnight: 10:00 AM is 440 px down, and 50 minutes is 36.67 px tall.
    const block = on('2026-10-05') as HTMLElement;
    expect(block.style.top).toBe('440px');
    expect(parseFloat(block.style.height)).toBeCloseTo(36.67, 1);
    expect(block.textContent).toBe('JPN 10110:00 AM to 10:50 AM');
    expect(block.getAttribute('aria-label')).toBe('JPN 101, 10:00 AM to 10:50 AM, Swan Hall 201');
    // It takes its colour from the space grouped by course, and is no task block and no other calendar's event.
    expect(block.style.getPropertyValue('--hue')).toBe('211');
    expect(block.classList.contains('heat-block') || block.classList.contains('heat-event')).toBe(false);

    await core(() => rig.client.put('termBreak', { title: 'Fall recess', from: '2026-10-09', to: '2026-10-12' }));
    expect(on('2026-10-07')).not.toBeNull();
    expect(on('2026-10-09')).toBeNull();
    // A shift is not a class: the break leaves it alone.
    expect(column('2026-10-07').querySelector('[data-commitment="cm-store"]')).not.toBeNull();
  });

  it('draws the travel time as a faint extension above and below', async () => {
    rig = await mountHeat();
    await addClass();
    await openWeek();
    const before = column('2026-10-05').querySelector<HTMLElement>('[data-buffer="before"]')!;
    const after = column('2026-10-05').querySelector<HTMLElement>('[data-buffer="after"]')!;
    // 25 minutes above from 9:35, 10 below from 10:50.
    expect(parseFloat(before.style.top)).toBeCloseTo((575 / 60) * 44, 1);
    expect(parseFloat(before.style.height)).toBeCloseTo((25 / 60) * 44, 1);
    expect(parseFloat(after.style.top)).toBeCloseTo((650 / 60) * 44, 1);
    expect(parseFloat(after.style.height)).toBeCloseTo((10 / 60) * 44, 1);
    // Today's column draws them too, from its 7 AM top.
    const today = $(rig, '.heat-column [data-buffer="before"]')!;
    expect(parseFloat(today.style.top)).toBeCloseTo(((575 - 420) / 60) * 44, 1);
  });

  it('marks a day an exception moved, where it went', async () => {
    rig = await mountHeat();
    const jpn = await addClass();
    await core(() =>
      rig.client.commitments.addException(jpn.id, { date: '2026-10-09', kind: 'move', toDate: '2026-10-08', start: 840 }),
    );
    await openWeek();
    expect(column('2026-10-09').querySelector(`[data-commitment="${jpn.id}"]`)).toBeNull();
    const moved = column('2026-10-08').querySelector<HTMLElement>(`[data-commitment="${jpn.id}"]`)!;
    expect(moved.getAttribute('data-moved')).toBe('true');
    expect(moved.querySelector('.heat-commit-moved')!.textContent).toBe('moved');
    expect(moved.getAttribute('title')).toBe('JPN 101, 2:00 PM to 2:50 PM, Swan Hall 201. Moved from Oct 9.');
    expect(await undoLabel()).toBe('Undo move class');
  });

  it('shows a month day’s commitments as dots, four at most', async () => {
    rig = await mountHeat();
    await addClass();
    for (const [title, start] of [['Lab', 780], ['Tutoring', 900], ['Commute', 1000]] as const) {
      await core(() => rig.client.commitments.create({ title, date: '2026-10-07', start, end: start + 30 }));
    }
    await click(button(rig, 'Calendar'));
    const dots = (day: string) => $(rig, `.heat-cell[data-day="${day}"] .heat-commit-dots`);
    // Wednesday the 7th: the class, three one-offs and the seeded shift.
    expect(dots('2026-10-07')!.querySelectorAll('.heat-commit-dot')).toHaveLength(4);
    expect(dots('2026-10-07')!.querySelector('.heat-commit-more')!.textContent).toBe('+1');
    expect(dots('2026-10-07')!.getAttribute('aria-label')).toContain('JPN 101 10:00 AM');
    expect(dots('2026-10-05')!.querySelectorAll('.heat-commit-dot')).toHaveLength(1);
    expect(dots('2026-10-04')).toBeNull();
  });
});

describe('a block that overlaps a commitment', () => {
  it('is flagged in the week grid with the core’s sentence, and others are not', async () => {
    rig = await mountHeat();
    await addClass();
    const { block } = await core(() =>
      rig.client.putBlock({ taskId: 't-quiz4', date: '2026-10-09', start: 615, minutes: 60 }),
    );
    await openWeek();
    const drawn = $(rig, `.heat-cal-col [data-block="${block.id}"]`)!;
    expect(drawn.getAttribute('data-conflict')).toBe('true');
    expect(drawn.querySelector('.heat-conflict')!.getAttribute('title')).toBe('Overlaps JPN 101 (10:00 AM to 10:50 AM).');
    const clear = $(rig, '.heat-cal-col [data-block="b-verse"]')!;
    expect(clear.getAttribute('data-conflict')).toBe('false');
    expect(clear.querySelector('.heat-conflict')).toBeNull();
  });

  it('is flagged in Today’s column when it only touches the travel time', async () => {
    rig = await mountHeat();
    await addClass();
    // The kanji block runs 9:00 to 9:45, and the way to class starts at 9:35.
    const kanji = $(rig, '.heat-column [data-block="b-kanji"]')!;
    expect(kanji.getAttribute('data-conflict')).toBe('true');
    expect(kanji.querySelector('.heat-conflict')!.getAttribute('title')).toBe(
      'Overlaps the travel time for JPN 101 (10:00 AM to 10:50 AM).',
    );
    expect($(rig, '.heat-column [data-block="b-pset"]')!.getAttribute('data-conflict')).toBe('false');
  });
});

describe('Today’s lines', () => {
  it('says what is next with when to leave, and what the day has left', async () => {
    rig = await mountHeat();
    // Before the class is added the evening shift is next.
    expect($(rig, '.heat-next-line')!.textContent).toBe('NEXT Campus store 6:00 · LEAVE 5:40');
    await addClass();
    expect($(rig, '.heat-next-line')!.textContent).toBe('NEXT JPN 101 10:00 · LEAVE 9:35');
    // From 10:00: free 11:15 to 2:00, 3:00 to 5:40 and 9:20 to 11:00; the 1:00 and 3:30 blocks are planned.
    const free = $(rig, '.heat-free-line')!;
    expect(free.textContent).toBe('7h 5m free today. Planned 2h 30m.');
    expect(free.getAttribute('data-over')).toBe('false');
    // The subtitle is still the core's own.
    expect($(rig, '.heat-subtitle')!.textContent).toBe('3 blocks · 3h 15m planned · 2 due today');
  });

  it('stands out quietly when more is planned than the day has left, and has no NEXT once nothing is', async () => {
    rig = await mountHeat({ now: MORNING + 11 * 3_600_000 }); // 9:00 PM
    expect($(rig, '.heat-next-line')).toBeNull();
    expect($(rig, '.heat-free-line')!.textContent).toBe('1h 40m free today. Planned 0m.');
    await core(() => rig.client.putBlock({ taskId: 't-quiz4', date: '2026-10-07', start: 21 * 60 + 30, minutes: 120 }));
    const free = $(rig, '.heat-free-line')!;
    expect(free.textContent).toBe('1h 40m free today. Planned 2h. Move 20m?');
    expect(free.getAttribute('data-over')).toBe('true');
  });

  it('puts P’s block after a commitment and its travel, never in them', async () => {
    rig = await mountHeat();
    // The lecture holds the morning until 11:15; a lab then holds 11:15 to 11:45, and ten minutes to get back.
    await core(() => rig.client.commitments.create({ title: 'Lab', date: '2026-10-07', start: 675, end: 705, bufferAfter: 10 }));
    await click($$(rig, '.heat-plan [role="option"]').find((r) => r.textContent!.includes('Grammar quiz 4')));
    await press(rig, 'p');
    const made = [...rig.fake.store.timeBlock.values()].find((b) => b.taskId === 't-quiz4')!;
    expect(made).toMatchObject({ date: '2026-10-07', start: 12 * 60, minutes: 60 });
    expect($(rig, `.heat-column [data-block="${made.id}"]`)!.getAttribute('data-conflict')).toBe('false');
  });
});

describe('the Schedule section and the New commitment sheet', () => {
  it('lists each commitment with when it meets, and opens it on a click', async () => {
    rig = await mountHeat();
    const jpn = await addClass();
    // The section belongs to Calendar: Today's sidebar has none.
    expect($(rig, '.heat-schedule-side')).toBeNull();
    await click(button(rig, 'Calendar'));
    const row = $(rig, `[data-commitment-row="${jpn.id}"]`)!;
    expect(row.textContent).toBe('JPN 101MWF 10:00 AM to 10:50 AMFrom Sep 9');
    expect($$(rig, '[data-commitment-row]')).toHaveLength(3);
    await click(row);
    const open = sheet('Edit JPN 101')!;
    expect((field(open, 'Name') as HTMLInputElement).value).toBe('JPN 101');
    expect([...open.querySelectorAll('.heat-day-toggle[aria-pressed="true"]')].map((b) => b.getAttribute('aria-label'))).toEqual([
      'Monday',
      'Wednesday',
      'Friday',
    ]);

    // Only what changed is sent.
    await type(field(open, 'Travel before, in minutes'), '30');
    await click(button(open, 'Save'));
    expect(rig.calls.find((c) => c.cmd === 'heat.commitment.update')!.args).toEqual({ id: jpn.id, set: { bufferBefore: 30 } });
    expect(rig.fake.store.commitment.get(jpn.id)).toMatchObject({ bufferBefore: 30, rrule: 'FREQ=WEEKLY;BYDAY=MO,WE,FR' });
    expect(sheet('Edit JPN 101')).toBeNull();
    expect(await undoLabel()).toBe('Undo edit commitment');
  });

  it('saves a new class with its days, times, travel and a new course’s code, as one undo step', async () => {
    rig = await mountHeat();
    await click(button(rig, 'Calendar'));
    await click(button(rig, 'New commitment…'));
    const open = sheet('New commitment')!;
    await click(button(open, 'Add commitment'));
    expect(open.querySelector('#heat-commit-why')!.textContent).toBe('Give the commitment a name first.');
    expect(commitments()).toHaveLength(2);

    await type(field(open, 'Name'), 'EGR 101');
    // No day picked yet: it would happen once, on the selected day.
    expect((field(open, 'Day') as HTMLInputElement).value).toBe('2026-10-07');
    await click(button(open, 'Tuesday'));
    await click(button(open, 'Thursday'));
    await type(field(open, 'From'), '2026-09-10');
    await type(field(open, 'Starts'), '14:00');
    await type(field(open, 'Ends'), '15:15');
    await type(field(open, 'Where'), 'Bliss Hall 190');
    await type(field(open, 'Travel before, in minutes'), '15');
    await type(field(open, 'Course'), 'new');
    await type(field(open, 'New course’s code'), 'EGR 101');
    await click(button(open, 'Add commitment'));

    const made = commitments().find((c) => c.title === 'EGR 101')!;
    expect(made).toMatchObject({
      kind: 'class',
      rrule: 'FREQ=WEEKLY;BYDAY=TU,TH',
      from: '2026-09-10',
      start: 840,
      end: 915,
      location: 'Bliss Hall 190',
      bufferBefore: 15,
      bufferAfter: 0,
      hardness: 'fixed',
      source: 'you',
    });
    expect(made.until).toBeUndefined();
    // A code Learn doesn't hold makes a stub course in the same entry.
    const course = rig.fake.store.course.get(made.courseId!)!;
    expect(course).toMatchObject({ code: 'EGR 101', status: 'stub' });
    expect(sheet('New commitment')).toBeNull();
    expect(rig.status().count).toBe('Added EGR 101, TTh 2:00 PM to 3:15 PM.');
    expect(await undoLabel()).toBe('Undo add commitment');
    await core(() => rig.call('history.undo', { room: 'heat' }));
    expect(commitments()).toHaveLength(2);
    expect(rig.fake.store.course.has(course.id)).toBe(false);
  });

  it('skips a day from the sheet, lists it, and restores it', async () => {
    rig = await mountHeat();
    const jpn = await addClass();
    await click(button(rig, 'Calendar'));
    await click($(rig, `[data-commitment-row="${jpn.id}"]`));
    const open = sheet('Edit JPN 101')!;
    await click(button(open, 'Skip a day'));
    // A Thursday isn't a day it meets on: the core says so, and nothing is written.
    await type(field(open, 'The day'), '2026-10-08');
    await click(button(open, 'Skip it'));
    expect(rig.status().count).toBe('JPN 101 doesn’t meet on Thursday, October 8.');
    expect(rig.fake.store.commitment.get(jpn.id)!.exceptions).toEqual([]);

    await type(field(open, 'The day'), '2026-10-09');
    await click(button(open, 'Skip it'));
    expect(rig.fake.store.commitment.get(jpn.id)!.exceptions).toEqual([{ date: '2026-10-09', kind: 'skip', source: 'you' }]);
    expect(open.querySelector('[data-exception="2026-10-09"]')!.textContent).toContain('Oct 9Skipped');
    expect(await undoLabel()).toBe('Undo skip class');
    await click(button(open, 'Restore Oct 9'));
    expect(rig.fake.store.commitment.get(jpn.id)!.exceptions).toEqual([]);
    expect(open.querySelector('[data-exception]')).toBeNull();
  });

  it('sets when the person sleeps from the sidebar', async () => {
    rig = await mountHeat();
    await click(button(rig, 'Calendar'));
    const side = $(rig, '.heat-schedule-side')!;
    expect((field(side, 'To bed') as HTMLInputElement).value).toBe('23:00');
    expect((field(side, 'Up') as HTMLInputElement).value).toBe('07:00');
    await type(field(side, 'To bed'), '00:30');
    await settle();
    expect(rig.calls.some((c) => c.cmd === 'heat.sleep.set' && c.args.from === 30 && c.args.to === 420)).toBe(true);
    expect((await rig.client.snapshot('2026-10-07')).commitments!.sleep).toEqual({ from: 30, to: 420 });
  });
});

describe('importing a schedule', () => {
  const paste = async (text: string) => {
    await click(button(rig, 'Calendar'));
    await click(button(rig, 'Import schedule…'));
    const open = sheet('Import schedule')!;
    await type(open.querySelector('textarea'), text);
    await click($$(open, 'fieldset')[0].querySelector('button'));
    return draftsOf(rig.fake).at(-1)!;
  };

  it('previews a pasted timetable, writes nothing until Apply, and one undo takes it back', async () => {
    rig = await mountHeat();
    await addClass();
    holdReading(rig.fake);
    const before = commitments().length;
    const draft = await paste('JPN 101 MWF 10-10:50 Swan 201\nEGR 101 TTh 2-3:15 Bliss 190\nCHM 101 MW 1-1:50');
    expect(rig.calls.find((c) => c.cmd === 'heat.commitment.importText')!.args).toMatchObject({ mode: 'schedule' });
    expect(sheet('Import schedule')!.getAttribute('data-state')).toBe('reading');
    expect(sheet('Import schedule')!.textContent).toContain('Reading the schedule…');

    await core(() =>
      readyDraft(rig.fake, draft.id, [
        { title: 'JPN 101', days: ['MO', 'WE', 'FR'], start: 600, end: 650, course: 'JPN 101' },
        { title: 'EGR 101', days: ['TU', 'TH'], start: '2pm', end: '3:15pm', location: 'Bliss Hall 190', course: 'EGR 101' },
        { title: 'CHM 101', days: ['MO', 'WE'], start: '13:00', end: '13:50', course: 'CHM 101' },
        { title: 'Office hours', start: 'afternoons', end: '' },
      ]),
    );
    const open = sheet('Import schedule')!;
    expect(open.getAttribute('data-state')).toBe('ready');
    expect(open.querySelector('.heat-syllabus-line')!.textContent).toBe('2 classes. 2 new courses. 1 already there.');
    const rows = $$(open, '[data-item]');
    expect(rows.map((r) => r.querySelector('th')!.textContent)).toEqual(['JPN 101', 'EGR 101', 'CHM 101']);
    expect(rows[1].textContent).toBe('Include EGR 101EGR 101TTh 2:00 PM to 3:15 PMEGR 101 · Bliss Hall 190new course');
    // The class that is there already is greyed, says so, and can't be ticked.
    expect(rows[0].getAttribute('data-new')).toBe('false');
    expect(rows[0].textContent).toContain('already there');
    expect((rows[0].querySelector('input') as HTMLInputElement).disabled).toBe(true);
    expect(open.textContent).toContain('1 line couldn’t be read and is left out.');
    // Nothing is written yet.
    expect(commitments()).toHaveLength(before);
    expect(await undoLabel()).toBe('Undo add commitment');

    // Leave chemistry out, and apply.
    await click(rows[2].querySelector('input'));
    await click(button(open, 'Apply'));
    expect(rig.calls.find((c) => c.cmd === 'heat.commitment.draft.accept')!.args).toEqual({ draftId: draft.id, skip: [2] });
    expect(commitments().map((c) => c.title).sort()).toEqual(['Campus store', 'EGR 101', 'JPN 101', 'MTH 142']);
    expect(commitments().find((c) => c.title === 'EGR 101')).toMatchObject({ rrule: 'FREQ=WEEKLY;BYDAY=TU,TH', source: 'paste' });
    expect([...rig.fake.store.course.values()].some((c) => c.code === 'EGR 101' && c.status === 'stub')).toBe(true);
    expect(sheet('Import schedule')).toBeNull();
    expect(draftsOf(rig.fake)).toHaveLength(0);
    expect(rig.status().count).toBe(`1 commitment added, 1 new course. ${keys('⌘Z')} undoes it.`);

    // One undo takes all of it back: the class and the course it made.
    expect(await undoLabel()).toBe('Undo import schedule');
    await core(() => rig.call('history.undo', { room: 'heat' }));
    expect(commitments()).toHaveLength(before);
    expect([...rig.fake.store.course.values()].some((c) => c.code === 'EGR 101')).toBe(false);
  });

  it('keeps a schedule that was closed waiting in the sidebar, and Discard drops it', async () => {
    rig = await mountHeat();
    holdReading(rig.fake);
    const draft = await paste('Tue 5-9, Sat 10-4');
    // Esc closes the sheet; the reading goes on, and the sidebar says so.
    await escape(rig);
    expect(sheet('Import schedule')).toBeNull();
    expect($(rig, '.heat-schedule-side')!.textContent).toContain('Reading 1 schedule…');
    await core(() => readyDraft(rig.fake, draft.id));
    await click(button(rig, '1 schedule to review'));
    const open = sheet('Import schedule')!;
    expect($$(open, '[data-item]').map((r) => r.querySelector('th')!.textContent)).toEqual(['JPN 101', 'EGR 101']);
    await click(button(open, 'Discard'));
    expect(sheet('Import schedule')).toBeNull();
    expect(draftsOf(rig.fake)).toHaveLength(0);
    expect(commitments()).toHaveLength(2);
    expect(rig.status().count).toBe('Schedule discarded. Nothing changed.');
    expect(button(rig, '1 schedule to review')).toBeUndefined();
  });

  it('says why a schedule couldn’t be read', async () => {
    rig = await mountHeat();
    holdReading(rig.fake);
    const draft = await paste('see attached');
    await core(() => failDraft(rig.fake, draft.id, 'There are no times in that text.'));
    const open = sheet('Import schedule')!;
    expect(open.querySelector('[role="alert"]')!.textContent).toBe('There are no times in that text.');
    expect(button(open, 'Apply')).toBeUndefined();
  });

  it('replaces only that week’s shifts with this week’s', async () => {
    rig = await mountHeat();
    holdReading(rig.fake);
    await click(button(rig, 'Calendar'));
    await click(button(rig, 'Import schedule…'));
    let open = sheet('Import schedule')!;
    await click($$(open, 'input[type="radio"]')[1]);
    // The week picker starts on this week's Monday.
    expect((field(open, 'Week of') as HTMLInputElement).value).toBe('2026-10-05');
    const shifts = async (lines: { date: string; start: number; end: number }[]) => {
      await type(open.querySelector('textarea'), 'this week');
      await click($$(open, 'fieldset')[0].querySelector('button'));
      const draft = draftsOf(rig.fake).at(-1)!;
      expect(draft).toMatchObject({ mode: 'week', weekOf: '2026-10-05' });
      await core(() => readyDraft(rig.fake, draft.id, lines.map((l) => ({ title: 'Diner', kind: 'work' as const, ...l }))));
      await click(button(sheet('Import schedule'), 'Apply'));
    };
    await shifts([
      { date: '2026-10-06', start: 1020, end: 1260 },
      { date: '2026-10-10', start: 600, end: 960 },
    ]);
    const diner = () => commitments().filter((c) => c.title === 'Diner');
    expect(diner().map((c) => [c.from, c.weekOf, c.rrule])).toEqual([
      ['2026-10-06', '2026-10-05', undefined],
      ['2026-10-10', '2026-10-05', undefined],
    ]);

    // The rota changes: read again, the week's two shifts go and the new one comes, in one entry.
    await click(button(rig, 'Import schedule…'));
    open = sheet('Import schedule')!;
    await click($$(open, 'input[type="radio"]')[1]);
    await shifts([{ date: '2026-10-08', start: 1020, end: 1260 }]);
    expect(diner().map((c) => c.from)).toEqual(['2026-10-08']);
    // The seeded shift has no week of its own, and stays.
    expect(commitments().some((c) => c.id === 'cm-store')).toBe(true);
    expect(await undoLabel()).toBe("Undo this week's shifts");
    await core(() => rig.call('history.undo', { room: 'heat' }));
    expect(diner().map((c) => c.from).sort()).toEqual(['2026-10-06', '2026-10-10']);
  });

  it('reads a photo dropped on the sheet by its path, and an .ics file in the core', async () => {
    rig = await mountHeat();
    holdReading(rig.fake);
    await click(button(rig, 'Calendar'));
    await click(button(rig, 'Import schedule…'));
    const dropFile = (uri: string) =>
      core(() => {
        const e = Object.assign(new Event('drop', { bubbles: true, cancelable: true }), {
          dataTransfer: { getData: (t: string) => (t === 'text/uri-list' ? uri : '') },
        });
        sheet('Import schedule')!.querySelector('.heat-schedule-drop')!.dispatchEvent(e);
      });
    // Something that is neither is refused in a line, and nothing is asked of the core.
    await dropFile('file:///Users/me/Desktop/notes.txt');
    expect(sheet('Import schedule')!.querySelector('[role="status"]')!.textContent).toBe(
      'Drop a photo, a screenshot or an .ics file.',
    );
    expect(draftsOf(rig.fake)).toHaveLength(0);

    await dropFile('file:///Users/me/Desktop/Fall%20timetable.png');
    expect(rig.calls.find((c) => c.cmd === 'heat.commitment.importImage')!.args).toMatchObject({
      path: '/Users/me/Desktop/Fall timetable.png',
      mode: 'schedule',
    });
    expect(sheet('Import schedule')!.textContent).toContain('Reading Fall timetable.png…');
    await click(button(sheet('Import schedule'), 'Discard'));

    // A calendar file needs no model: its preview is there at once.
    await click(button(rig, 'Import schedule…'));
    await dropFile('file:///Users/me/Downloads/rota.ics');
    expect(rig.calls.find((c) => c.cmd === 'heat.commitment.importIcs')!.args).toMatchObject({
      path: '/Users/me/Downloads/rota.ics',
    });
    expect(sheet('Import schedule')!.getAttribute('data-state')).toBe('ready');
    expect($$(sheet('Import schedule'), '[data-item]')).toHaveLength(1);
    expect(commitments()).toHaveLength(2);
  });

  it('keeps a calendar’s address in step, lists it in the sidebar, and removes it leaving what it made', async () => {
    rig = await mountHeat();
    await click(button(rig, 'Calendar'));
    await click(button(rig, 'Import schedule…'));
    const open = sheet('Import schedule')!;
    await click(button(open, 'Read it'));
    expect(open.querySelector(':scope > [role="status"]')!.textContent).toBe('Paste the schedule first.');
    await click($$(open, 'fieldset')[2].querySelector('button'));
    expect(open.querySelector(':scope > [role="status"]')!.textContent).toBe('Type the calendar’s address first.');
    await type(open.querySelector('input[type="url"]'), 'https://work.example/rota.ics');
    await click($$(open, 'fieldset')[2].querySelector('input[type="checkbox"]'));
    await click(button(open, 'Subscribe'));
    expect(rig.calls.find((c) => c.cmd === 'heat.commitment.importIcs')!.args).toEqual({
      url: 'https://work.example/rota.ics',
      mode: 'schedule',
      subscribe: true,
    });
    // Its commitments are written at once, as one entry, and no draft waits.
    expect(sheet('Import schedule')).toBeNull();
    expect(draftsOf(rig.fake)).toHaveLength(0);
    expect(commitments().filter((c) => c.feedId)).toHaveLength(1);
    expect(await undoLabel()).toBe('Undo schedule sync');
    expect(rig.status().count).toBe(`Subscribed to work.example. 1 commitment added. ${keys('⌘Z')} undoes it.`);

    const feed = $(rig, '.heat-schedule-feed')!;
    expect(feed.textContent).toContain('work.example');
    await click(button(feed, 'Remove work.example'));
    expect($(rig, '.heat-schedule-feed')).toBeNull();
    expect(commitments().filter((c) => c.feedId)).toHaveLength(1);
  });

  it('adds a break by hand in the academic calendar, and opens the import on breaks', async () => {
    rig = await mountHeat();
    await click(button(rig, 'Calendar'));
    await click(button(rig, 'Academic calendar…'));
    let open = sheet('Academic calendar')!;
    expect(open.querySelector('.heat-term-dates')!.textContent).toBe('The term has no dates yet. Set in Settings → Learn.');
    await type(field(open, 'Break'), 'Thanksgiving recess');
    await type(field(open, 'First day'), '2026-11-25');
    await type(field(open, 'Last day'), '2026-11-29');
    await click(button(open, 'Add break'));
    expect([...rig.fake.store.termBreak.values()]).toMatchObject([
      { title: 'Thanksgiving recess', from: '2026-11-25', to: '2026-11-29', source: 'you' },
    ]);
    open = sheet('Academic calendar')!;
    expect(open.querySelector('[data-break]')!.textContent).toContain('Thanksgiving recessNov 25 to Nov 29');
    await click(button(open, 'Delete Thanksgiving recess'));
    expect(rig.fake.store.termBreak.size).toBe(0);

    await click(button(sheet('Academic calendar'), 'Import a calendar…'));
    const radios = $$(sheet('Import schedule'), 'input[type="radio"]') as HTMLInputElement[];
    expect(radios.map((r) => r.checked)).toEqual([false, false, true]);
  });
});

describe('what a mail asked for', () => {
  it('waits as one quiet row, and one tap confirms it', async () => {
    rig = await mountHeat();
    const jpn = await addClass();
    const pending = await core(() => addPending(rig.fake, { commitmentId: jpn.id, date: '2026-10-09' }));
    const row = $(rig, `.heat-today [data-pending="${pending.id}"]`)!;
    expect(row.querySelector('.heat-pending-line')!.textContent).toBe('JPN 101 is canceled Friday, Oct 9. Skip it?');
    // Nothing is applied without the tap.
    expect(rig.fake.store.commitment.get(jpn.id)!.exceptions).toEqual([]);
    // Calendar shows the same row at its top.
    expect($(rig, `.heat-cal [data-pending="${pending.id}"]`)).not.toBeNull();

    await click(button(row, 'Skip it'));
    expect(rig.calls.filter((c) => c.cmd.startsWith('heat.commitment.exception'))).toEqual([
      { cmd: 'heat.commitment.exception.confirm', args: { id: pending.id } },
    ]);
    expect(rig.fake.store.commitment.get(jpn.id)!.exceptions).toEqual([{ date: '2026-10-09', kind: 'skip', source: 'mail' }]);
    expect($(rig, '[data-pending]')).toBeNull();
    expect(rig.status().count).toBe('JPN 101 is skipped Friday, Oct 9.');
    expect(await undoLabel()).toBe('Undo skip class');
  });

  it('goes away on Not now, and nothing changes', async () => {
    rig = await mountHeat();
    const jpn = await addClass();
    const pending = await core(() => addPending(rig.fake, { commitmentId: jpn.id, date: '2026-10-09' }));
    await click(button($(rig, `[data-pending="${pending.id}"]`), 'Not now'));
    expect($(rig, '[data-pending]')).toBeNull();
    expect(rig.fake.store.commitment.get(jpn.id)!.exceptions).toEqual([]);
    expect((await rig.client.snapshot('2026-10-07')).commitments!.pending).toEqual([]);
  });
});
