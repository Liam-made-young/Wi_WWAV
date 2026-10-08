import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { TASK_DRAG } from '../frame';
import { $, $$, button, click, escape, mountHeat, press, type Rig, settle, type, wait } from '../testkit';
import { chimeOn, setChime } from '../chime';
import { COLUMN_HEIGHT, minutesToY } from './gap';

// docs/PLAN.md S2.2 and S2.3 against the fake core (docs/SPEC.md 3.5). What
// a fail looks like: Plan my day's drafts without their reasons, or Return
// not accepting all, a click not accepting one, Esc not clearing them; a
// round or a break that starts without a press, or a timer that stops with
// the view; a block that isn't 44 px an hour, that resizes the estimate, or
// that P puts anywhere but the next free gap; ⌘↩ and ⌫ without their Undo.

let rig: Rig;
afterEach(() => rig?.unmount());

const MIN = 60_000;
const sections = () => $$(rig, '.heat-plan [role="group"]').map((g) => g.getAttribute('aria-label'));
const rowsIn = (name: string) =>
  $$(rig, `.heat-plan [role="group"][aria-label="${name}"] [role="option"]`).map((r) => r.textContent);
const blocks = () => [...rig.fake.store.timeBlock.values()];
const row = (text: string | RegExp) =>
  $$(rig, '.heat-plan [role="option"]').find((r) =>
    typeof text === 'string' ? r.textContent!.includes(text) : text.test(r.textContent!),
  )!;
const undoLabel = async () => (await rig.call<{ undo: string | null }>('history.get', {})).undo;

describe('Today’s header and plan list', () => {
  it('reads "Today, Wednesday, October 7" over the core’s subtitle', async () => {
    rig = await mountHeat();
    expect($(rig, '.heat-today-head h1')!.textContent).toBe('Today, Wednesday, October 7');
    expect($(rig, '.heat-subtitle')!.textContent).toBe('3 blocks · 3h 15m planned · 2 due today');
    expect(rig.status().count).toBe('3 blocks · 3h 15m planned · 2 due today');
  });

  it('has the plan list’s sections in order and leaves out an empty one', async () => {
    rig = await mountHeat();
    expect(sections()).toEqual(['Planned', 'Recurring today ↻', 'Hot, not planned']);
    expect(rowsIn('Planned')).toEqual([
      '9:00 AM✓ Kanji worksheet 745m',
      '1:00 PMMix the second verse1h 30m',
      '3:30 PMProblem set 51h',
    ]);
    expect(rowsIn('Hot, not planned')[0]).toContain('Grammar quiz 4');
    // Selecting the hot task by its row; nothing else is hidden by the space filter yet.
    await click(button(rig, /^WWAV/));
    // Habits belong to the person, so Recurring today keeps them whichever space is chosen.
    expect(sections()).toEqual(['Planned', 'Recurring today ↻']);
    expect(rowsIn('Planned')).toEqual(['1:00 PMMix the second verse1h 30m']);
    expect(rowsIn('Recurring today ↻')).toEqual(['Practise kanji20m']);
    expect($(rig, '.heat-subtitle')!.textContent).toBe('1 block · 1h 30m planned · 0 due today');
  });

  it('says what to do when nothing is planned', async () => {
    rig = await mountHeat({ empty: true });
    expect($(rig, '.heat-empty')!.textContent).toBe(
      'Nothing planned yet. Drag a task onto the time column, or press Plan my day.',
    );
    expect(sections()).toEqual([]);
  });

  it('keeps the daily note to one line until it is clicked, and saves it when you leave', async () => {
    rig = await mountHeat();
    expect($(rig, '.heat-note-line')!.textContent).toBe("How's the day going? Markdown + [[wikilinks]] welcome.");
    await click($(rig, '.heat-note-line'));
    const field = $(rig, '.heat-note-field') as HTMLTextAreaElement;
    expect(document.activeElement).toBe(field);
    const set = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')!.set!;
    await act(async () => {
      set.call(field, 'Mixed the bridge.\nMore tomorrow.');
      field.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await escape(rig);
    expect($(rig, '.heat-note-field')).toBeNull();
    expect($(rig, '.heat-note-line')!.textContent).toBe('Mixed the bridge.');
    // The day's note is the note titled with the day (docs/NOTES.md).
    const note = [...rig.fake.store.note.values()].find((n) => n.title === '2026-10-07')!;
    expect(note.markdown).toBe('Mixed the bridge.\nMore tomorrow.');
    expect(await undoLabel()).toBe('Undo add note');
  });
});

describe('the time column', () => {
  it('is 44 px an hour from 7 AM to midnight, with a block per planned task', async () => {
    rig = await mountHeat();
    const inner = $(rig, '[data-column]')!;
    expect(inner.style.height).toBe(`${COLUMN_HEIGHT}px`);
    expect(COLUMN_HEIGHT).toBe(17 * 44);
    expect($$(rig, '.heat-hour-label').map((l) => l.textContent)).toEqual([
      '7 AM',
      '8 AM',
      '9 AM',
      '10 AM',
      '11 AM',
      '12 PM',
      '1 PM',
      '2 PM',
      '3 PM',
      '4 PM',
      '5 PM',
      '6 PM',
      '7 PM',
      '8 PM',
      '9 PM',
      '10 PM',
      '11 PM',
    ]);
    // 9:00 AM is two hours below 7 AM, and 45 minutes is 33 px.
    const kanji = $(rig, '[data-block="b-kanji"]')!;
    expect(kanji.style.top).toBe(`${2 * 44}px`);
    expect(kanji.style.height).toBe(`${(45 / 60) * 44}px`);
    expect($$(rig, '[data-block]')).toHaveLength(3);
  });

  it('draws the 1 px red line at the current minute, a finished block dimmed with its check, and other calendars’ events grey behind', async () => {
    rig = await mountHeat();
    const line = $(rig, '[data-now-line]')!;
    expect(line.style.top).toBe(`${minutesToY(10 * 60)}px`);
    expect($(rig, '[data-block="b-kanji"]')!.getAttribute('data-finished')).toBe('true');
    expect($(rig, '[data-block="b-kanji"]')!.textContent).toContain('✓');
    expect($(rig, '[data-block="b-verse"]')!.getAttribute('data-finished')).toBe('false');
    const events = $$(rig, '[data-event]');
    expect(events.map((e) => e.textContent)).toEqual(['JPN 201 lecture', 'Studio time']);
    // An event is read-only: no pointer handling, and it sits before the blocks so they draw over it.
    const order = $$(rig, '[data-event], [data-block]').map((e) => (e.getAttribute('data-event') ? 'event' : 'block'));
    expect(order).toEqual(['event', 'event', 'block', 'block', 'block']);
  });

  it('gives the block now is in the Aqua ring', async () => {
    rig = await mountHeat({ now: Date.UTC(2026, 9, 7, 17, 15) });
    // 1:15 PM New York is inside the 1:00 PM block.
    expect($(rig, '[data-block="b-verse"]')!.getAttribute('data-current')).toBe('true');
    expect($(rig, '[data-block="b-pset"]')!.getAttribute('data-current')).toBe('false');
  });

  it('makes a block as long as the estimate, rounded up to 15 minutes, from a task dropped on it', async () => {
    rig = await mountHeat();
    const quiz = rig.seeded.records.task!.find((t) => t.id === 't-quiz4')!;
    expect(quiz.estMin).toBeNull();
    const drop = async (y: number) => {
      const inner = $(rig, '[data-column]')!;
      const e = Object.assign(new Event('drop', { bubbles: true, cancelable: true }), {
        clientY: y,
        dataTransfer: {
          types: [TASK_DRAG],
          getData: (t: string) => (t === TASK_DRAG ? 't-quiz4' : ''),
          dropEffect: '',
        },
      });
      await act(async () => {
        inner.dispatchEvent(e);
        await new Promise((r) => setTimeout(r, 0));
      });
      await settle();
    };
    // 4:00 PM is 9 hours below 7 AM: 396 px. Difficulty 3 with no average estimates 60 minutes.
    await drop(minutesToY(16 * 60) + 3);
    const made = blocks().find((b) => b.taskId === 't-quiz4')!;
    expect(made).toMatchObject({ date: '2026-10-07', start: 16 * 60, minutes: 60, origin: 'you' });
    expect(await undoLabel()).toBe('Undo add block');
  });

  it('resizes a block from its bottom edge in 15-minute steps, and never touches the estimate', async () => {
    rig = await mountHeat();
    const grip = $(rig, '[data-block="b-verse"] [data-grip]')!;
    const pointer = (type: string, y: number, target: EventTarget = window) =>
      act(async () => {
        target.dispatchEvent(new MouseEvent(type, { bubbles: true, button: 0, clientY: y }));
        await new Promise((r) => setTimeout(r, 0));
      });
    await pointer('pointerdown', 500, grip);
    // 22 px is half an hour at 44 px an hour.
    await pointer('pointermove', 522);
    expect($(rig, '[data-block="b-verse"]')!.style.height).toBe(`${(120 / 60) * 44}px`);
    await pointer('pointerup', 522);
    await settle();
    expect(rig.fake.store.timeBlock.get('b-verse')).toMatchObject({ start: 13 * 60, minutes: 120 });
    expect(rig.fake.store.task.get('t-verse')!.estMin).toBe(120);
    expect(await undoLabel()).toBe('Undo resize block');
  });

  it('selects a block on a press that doesn’t move, and opens Get Info on it', async () => {
    rig = await mountHeat();
    const block = $(rig, '[data-block="b-pset"]')!;
    await act(async () => {
      block.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0, clientY: 300 }));
      window.dispatchEvent(new MouseEvent('pointerup', { bubbles: true, clientY: 300 }));
    });
    await settle();
    expect($(rig, '[data-block="b-pset"]')!.getAttribute('data-selected')).toBe('true');
    expect($(rig, '.heat-info')).not.toBeNull();
  });

  it('puts the selected task into the next free gap after now on P, and says so', async () => {
    rig = await mountHeat();
    await click(row('Grammar quiz 4'));
    expect(await press(rig, 'p')).toBe(true);
    // 10:00 AM is the lecture's start; the first 15-minute mark after it where an hour fits is 11:15.
    const made = blocks().find((b) => b.taskId === 't-quiz4')!;
    expect(made).toMatchObject({ date: '2026-10-07', start: 11 * 60 + 15, minutes: 60 });
    expect(rig.status().count).toBe('Planned ‘Grammar quiz 4’ at 11:15 AM, 1h.');
    expect(await undoLabel()).toBe('Undo add block');
  });

  it('says why when P finds no gap', async () => {
    rig = await mountHeat({ now: Date.UTC(2026, 9, 8, 3, 50) }); // 11:50 PM
    await click(row('Grammar quiz 4'));
    await press(rig, 'p');
    expect(rig.status().count).toBe('No free gap is left today.');
  });
});

describe('"+" on Today', () => {
  it('adds a task straight into the plan, in the next free gap, as a block as long as its estimate', async () => {
    rig = await mountHeat();
    await click($(rig, '.heat-plus'));
    const sheet = $(rig, '[role="dialog"][aria-label="New task"]')!;
    await type(sheet.querySelector('input'), 'Write the liner notes');
    await click([...sheet.querySelectorAll('button')].find((b) => b.textContent === 'Add task'));
    const made = [...rig.fake.store.task.values()].find((t) => t.title === 'Write the liner notes')!;
    expect(made).toMatchObject({ spaceId: 'sp-classes', due: null });
    // No type's word is in the title, so it starts with the catch-all's 45 minutes; the lecture holds the morning until 11:15.
    expect(blocks().find((b) => b.taskId === made.id)).toMatchObject({
      date: '2026-10-07',
      start: 11 * 60 + 15,
      minutes: 45,
    });
    expect(rowsIn('Planned').some((r) => r!.includes('Write the liner notes'))).toBe(true);
    expect(rig.status().count).toBe('Planned ‘Write the liner notes’ at 11:15 AM, 45m.');
  });

  it('asks for a name first, and N opens the same sheet', async () => {
    rig = await mountHeat();
    expect(await press(rig, 'n')).toBe(true);
    await click($$(rig, '[aria-label="New task"] button').find((b) => b.textContent === 'Add task'));
    expect($(rig, '#heat-new-why')!.textContent).toBe('Give the task a name first.');
    expect(rig.fake.store.task.size).toBe(12);
  });
});

describe('Plan my day', () => {
  const plan = async () => {
    await click(button(rig, 'Plan my day'));
  };

  it('fills the day with dashed drafts, each with its reason, and writes nothing yet', async () => {
    rig = await mountHeat();
    const before = blocks().length;
    await plan();
    const drafts = $$(rig, '[data-draft]');
    expect(drafts.length).toBeGreaterThan(2);
    for (const d of drafts)
      expect(d.querySelector('.heat-row-why')!.textContent).toMatch(/^(Due |No due date|.* overdue)/);
    expect(drafts[0].querySelector('.heat-row-why')!.textContent).toMatch(
      /^Due (today|tomorrow) .*, (Hot|Warm|Cool)\./,
    );
    expect($$(rig, '[data-draft-block]')).toHaveLength(drafts.length);
    expect(blocks()).toHaveLength(before);
    expect(rig.status().act).toBe('Plan my day');
  });

  it('accepts every draft on Return, as one undo step', async () => {
    rig = await mountHeat();
    await plan();
    const n = $$(rig, '[data-draft]').length;
    const before = blocks().length;
    expect(await press(rig, 'Enter')).toBe(true);
    expect(blocks()).toHaveLength(before + n);
    expect($$(rig, '[data-draft]')).toHaveLength(0);
    expect(blocks().filter((b) => b.origin === 'plan').length).toBeGreaterThanOrEqual(n);
    expect(await undoLabel()).toBe('Undo plan my day');
  });

  it('accepts one draft on a click, and leaves the rest waiting', async () => {
    rig = await mountHeat();
    await plan();
    const all = $$(rig, '[data-draft]').length;
    const before = blocks().length;
    await click($$(rig, '[data-draft]')[0]);
    expect(blocks()).toHaveLength(before + 1);
    expect($$(rig, '[data-draft]')).toHaveLength(all - 1);
    expect(await undoLabel()).toBe('Undo accept draft');
  });

  it('clears the drafts on Esc, and nothing is saved', async () => {
    rig = await mountHeat();
    await plan();
    const before = blocks().length;
    await escape(rig);
    expect($$(rig, '[data-draft]')).toHaveLength(0);
    expect(blocks()).toHaveLength(before);
  });

  it('runs on ⇧Return, the tab’s secondary act', async () => {
    rig = await mountHeat();
    await act(async () => rig.view.current!.secondary());
    await settle();
    expect($$(rig, '[data-draft]').length).toBeGreaterThan(0);
  });

  it('runs on a ⇧-click in the main view, the other way to ⇧Return, and a plain click still selects', async () => {
    rig = await mountHeat();
    const target = row('Grammar quiz 4');
    await act(async () => {
      target.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, shiftKey: true }));
    });
    await settle();
    expect($$(rig, '[data-draft]').length).toBeGreaterThan(0);
    expect($(rig, '.heat-info')).toBeNull();
    await escape(rig);
    await click(row('Grammar quiz 4'));
    expect($(rig, '.heat-info')).not.toBeNull();
  });

  it('puts Return to work even when the Plan my day button still has the keyboard', async () => {
    rig = await mountHeat();
    await plan();
    button(rig, 'Plan my day')!.focus();
    const n = $$(rig, '[data-draft]').length;
    expect(await press(rig, 'Enter')).toBe(true);
    expect($$(rig, '[data-draft]')).toHaveLength(0);
    expect(blocks().filter((b) => b.origin === 'plan').length).toBeGreaterThanOrEqual(n);
  });
});

describe('the Pomodoro timer', () => {
  const lcd = () => ({
    digits: $(rig, '.heat-lcd-digits')!.textContent,
    line: $(rig, '.heat-lcd-line')!.textContent,
    note: $(rig, '.heat-lcd-note')!.textContent,
  });

  it('starts nothing on its own: a current task, a clock that moves on and a break all wait for a press', async () => {
    rig = await mountHeat();
    expect(lcd()).toMatchObject({ digits: '25:00', line: 'Focus 1 of 4 · Mix the second verse' });
    rig.fake.now += 3 * 60 * MIN;
    rig.fake.emit([]);
    await settle();
    expect(rig.fake.state.timer.phase).toBe('idle');
    expect(rig.fake.state.timer.endsAt).toBeNull();
    expect(lcd().digits).toBe('25:00');
    expect([...rig.fake.store.focusSession.values()].length).toBe(2);
  });

  it('starts on F, counts down, pauses on F and resumes on F', async () => {
    rig = await mountHeat();
    expect(await press(rig, 'f')).toBe(true);
    expect(rig.fake.state.timer).toMatchObject({ phase: 'focus', running: true, taskId: 't-verse' });
    expect(lcd().digits).toBe('25:00');
    rig.fake.now += 61_000;
    rig.fake.emit([]);
    await settle();
    expect(lcd().digits).toBe('23:59');
    await press(rig, 'f');
    expect(rig.fake.state.timer).toMatchObject({ phase: 'focus', running: false });
    rig.fake.now += 10 * MIN; // a paused round doesn't move
    rig.fake.emit([]);
    await settle();
    expect(lcd().digits).toBe('23:59');
    expect(button(rig, 'Resume')).toBeTruthy();
    await press(rig, 'f');
    expect(rig.fake.state.timer.running).toBe(true);
  });

  it('logs the minutes to the current task on ⇧F, and adds them to its time', async () => {
    rig = await mountHeat();
    await press(rig, 'f');
    rig.fake.now += 12 * MIN;
    rig.fake.emit([]);
    await settle();
    expect(await press(rig, 'F', { shiftKey: true })).toBe(true);
    const logged = [...rig.fake.store.focusSession.values()].find((s) => s.taskId === 't-verse')!;
    expect(logged.focusMin).toBe(12);
    expect(rig.fake.state.timer.phase).toBe('idle');
    expect(lcd().note).toBe('Focus stopped. 12m logged to Mix the second verse.');
    expect((await rig.client.snapshot('2026-10-07')).derived.tasks['t-verse'].actualMin).toBe(12);
  });

  it('marks "Pulled away" on I: it pauses and counts the interruption', async () => {
    rig = await mountHeat();
    await press(rig, 'f');
    expect(await press(rig, 'i')).toBe(true);
    expect(rig.fake.state.timer.running).toBe(false);
    await press(rig, 'f');
    rig.fake.now += 5 * MIN;
    rig.fake.emit([]);
    await settle();
    await press(rig, 'F', { shiftKey: true });
    const s = [...rig.fake.store.focusSession.values()].find((x) => x.taskId === 't-verse')!;
    expect(s.interruptions).toBe(1);
  });

  it('ends a round at its time by itself, logs it, and waits for a press to start the break', async () => {
    rig = await mountHeat();
    await press(rig, 'f');
    rig.fake.now += 26 * MIN;
    rig.fake.emit([]);
    await wait(700);
    await settle();
    expect(rig.fake.state.timer).toMatchObject({ phase: 'break', running: false, endsAt: null });
    expect(lcd()).toMatchObject({
      digits: '5:00',
      line: 'Break 5:00. Press F to start it.',
      note: 'Focus done. 25m logged to Mix the second verse.',
    });
    // Hours pass: the break never starts itself, and the next round never starts after it.
    rig.fake.now += 3 * 60 * MIN;
    rig.fake.emit([]);
    await wait(700);
    expect(rig.fake.state.timer).toMatchObject({ phase: 'break', running: false });
    await press(rig, 'f');
    expect(rig.fake.state.timer).toMatchObject({ phase: 'break', running: true });
  });

  it('keeps counting when Learn’s tab changes, and reads "focus 18:42 left" for the Now strip', async () => {
    const { stripText } = await import('../timer');
    rig = await mountHeat();
    await press(rig, 'f');
    rig.fake.now += 6 * MIN + 18_000;
    await press(rig, '2');
    const snap = await rig.client.snapshot('2026-10-07');
    expect(stripText(snap.heatState.timer, rig.fake.now)).toBe('focus 18:42 left');
    expect(rig.fake.state.timer.running).toBe(true);
  });

  it('does nothing on F with no current task, and the LCD says what to do', async () => {
    rig = await mountHeat({ empty: true });
    await press(rig, 'f');
    expect(rig.fake.state.timer.phase).toBe('idle');
    expect($(rig, '.heat-lcd-line')!.textContent).toBe(
      'Nothing is current. Pick a task and press C, or drag one here.',
    );
    expect(button(rig, 'Start focus')!.hasAttribute('disabled')).toBe(true);
  });

  it('starts on a selected task, and makes it current', async () => {
    rig = await mountHeat();
    await click(row('Grammar quiz 4'));
    await press(rig, 'f');
    expect(rig.fake.state).toMatchObject({ currentTaskId: 't-quiz4' });
    expect(rig.fake.state.timer.taskId).toBe('t-quiz4');
  });

  it('takes the length chosen: 25, 50, or a custom 10–90', async () => {
    rig = await mountHeat();
    await click(button(rig, '50'));
    expect($(rig, '.heat-lcd-digits')!.textContent).toBe('50:00');
    await press(rig, 'f');
    expect(rig.fake.state.timer.focusMin).toBe(50);
  });
});

describe('P, ⌘↩ and ⌫', () => {
  it('marks a task done on ⌘↩ and asks "Time it took" when no time was logged', async () => {
    rig = await mountHeat();
    await click(row('Grammar quiz 4'));
    const cmd = { metaKey: true, ctrlKey: true };
    expect(await press(rig, 'Enter', cmd)).toBe(true);
    const sheet = $(rig, '[aria-label="Time it took"]')!;
    expect(sheet.textContent).toContain('This trains your time averages for this type of task.');
    expect(rig.fake.store.task.get('t-quiz4')!.done).toBe(false);
    const field = sheet.querySelector('input')!;
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
    await act(async () => {
      set.call(field, '40');
      field.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await click(sheet.querySelector('button[type="submit"]'));
    expect(rig.fake.store.task.get('t-quiz4')).toMatchObject({ done: true, adjustMin: 40 });
    expect(rig.status().count).toBe('Done. Took 40m.');
    expect(await undoLabel()).toBe('Undo mark done');
  });

  it('checks a task with logged time off at once, and says what it took', async () => {
    rig = await mountHeat();
    for (const [id, min] of [
      ['s-a', 30],
      ['s-b', 45],
    ] as const) {
      rig.fake.store.focusSession.set(id, {
        id,
        taskId: 't-quiz4',
        startedAt: 0,
        endedAt: 1,
        focusMin: min,
        interruptions: 0,
        source: 'timer',
        public: false,
      });
    }
    rig.fake.emit(['focusSession']);
    await settle();
    await click(row('Grammar quiz 4'));
    await press(rig, 'Enter', { metaKey: true, ctrlKey: true });
    expect($(rig, '[aria-label="Time it took"]')).toBeNull();
    expect(rig.fake.store.task.get('t-quiz4')!.done).toBe(true);
    expect(rig.status().count).toBe('Done. Took 1h 15m across 2 focus sessions.');
    expect(await undoLabel()).toBe('Undo mark done');
  });

  it('puts the task back on ⌘Z after ⌘↩', async () => {
    rig = await mountHeat();
    rig.fake.store.focusSession.set('s-a', {
      id: 's-a',
      taskId: 't-quiz4',
      startedAt: 0,
      endedAt: 1,
      focusMin: 20,
      interruptions: 0,
      source: 'timer',
      public: false,
    });
    rig.fake.emit(['focusSession']);
    await settle();
    await click(row('Grammar quiz 4'));
    await press(rig, 'Enter', { metaKey: true, ctrlKey: true });
    expect(rig.fake.store.task.get('t-quiz4')!.done).toBe(true);
    await rig.call('history.undo', { room: 'heat' });
    await settle();
    expect(rig.fake.store.task.get('t-quiz4')!.done).toBe(false);
  });

  it('deletes the selection on ⌫ and brings it back on ⌘Z', async () => {
    rig = await mountHeat();
    await click(row('Grammar quiz 4'));
    expect(await press(rig, 'Backspace')).toBe(true);
    expect(rig.fake.store.task.has('t-quiz4')).toBe(false);
    expect(rig.status().count).toBe('Deleted ‘Grammar quiz 4’.');
    expect($(rig, '.heat-info')).toBeNull();
    expect(await undoLabel()).toBe('Undo delete task');
    await rig.call('history.undo', { room: 'heat' });
    await settle();
    expect(rig.fake.store.task.has('t-quiz4')).toBe(true);
  });

  it('removes a selected block on ⌫, as "remove block"', async () => {
    rig = await mountHeat();
    await click(row('Problem set 5'));
    await press(rig, 'Backspace');
    expect(rig.fake.store.timeBlock.has('b-pset')).toBe(false);
    expect(rig.fake.store.task.has('t-pset5')).toBe(true);
    expect(await undoLabel()).toBe('Undo remove block');
  });

  it('makes the selected task current on C', async () => {
    rig = await mountHeat();
    await click(row('Grammar quiz 4'));
    await press(rig, 'c');
    expect(rig.fake.state.currentTaskId).toBe('t-quiz4');
  });

  it('walks the plan list on ↑ ↓, Esc clears the selection, and never deletes', async () => {
    rig = await mountHeat();
    await press(rig, 'ArrowDown');
    expect($(rig, '.heat-row[aria-selected="true"]')!.textContent).toContain('Kanji worksheet 7');
    await press(rig, 'ArrowDown');
    await press(rig, 'ArrowDown');
    await press(rig, 'ArrowDown');
    expect($(rig, '.heat-row[aria-selected="true"]')!.textContent).toContain('Listening practice');
    await press(rig, 'ArrowUp');
    expect($(rig, '.heat-row[aria-selected="true"]')!.textContent).toContain('Problem set 5');
    await escape(rig);
    expect($(rig, '.heat-row[aria-selected="true"]')).toBeNull();
    expect(rig.fake.store.task.size).toBe(12);
  });

  it('moves a selected block 15 minutes on ⌥↓, and lengthens it on ⇧⌥↓', async () => {
    rig = await mountHeat();
    await click(row('Problem set 5'));
    await press(rig, 'ArrowDown', { altKey: true });
    expect(rig.fake.store.timeBlock.get('b-pset')).toMatchObject({ start: 15 * 60 + 45, minutes: 60 });
    expect(await undoLabel()).toBe('Undo move block');
    await press(rig, 'ArrowDown', { altKey: true, shiftKey: true });
    expect(rig.fake.store.timeBlock.get('b-pset')).toMatchObject({ start: 15 * 60 + 45, minutes: 75 });
    expect(await undoLabel()).toBe('Undo resize block');
  });
});

describe('Learn’s chime', () => {
  const rings: number[] = [];
  const stub = class {
    constructor() {
      rings.push(Date.now());
    }
    currentTime = 0;
    destination = {};
    createGain() {
      const gain = { value: 0, setValueAtTime() {}, exponentialRampToValueAtTime() {} };
      return { gain, connect: (n: unknown) => n };
    }
    createOscillator() {
      return { type: '', frequency: { value: 0 }, connect: (n: unknown) => n, start() {}, stop() {} };
    }
    close() {
      return Promise.resolve();
    }
  };

  const round = async () => {
    await press(rig, 'f');
    rig.fake.now += 26 * MIN;
    rig.fake.emit([]);
    await wait(700);
    await settle();
  };

  it('is off until it is turned on, and a round that ends makes no sound', async () => {
    rings.length = 0;
    (window as unknown as { AudioContext: unknown }).AudioContext = stub;
    localStorage.removeItem('wi.heat.chime');
    rig = await mountHeat();
    expect(chimeOn()).toBe(false);
    expect(($(rig, '.heat-chime input') as HTMLInputElement).checked).toBe(false);
    await round();
    expect(rig.fake.state.timer.phase).toBe('break');
    expect(rings).toEqual([]);
  });

  it('rings once when a round ends, if its switch is on, and never for a break', async () => {
    rings.length = 0;
    (window as unknown as { AudioContext: unknown }).AudioContext = stub;
    rig = await mountHeat();
    await click($(rig, '.heat-chime input'));
    expect(chimeOn()).toBe(true);
    await round();
    expect(rings).toHaveLength(1);
    // The break waits for a press, and its end calls nobody back.
    await press(rig, 'f');
    rig.fake.now += 6 * MIN;
    rig.fake.emit([]);
    await wait(700);
    expect(rings).toHaveLength(1);
    setChime(false);
  });
});
