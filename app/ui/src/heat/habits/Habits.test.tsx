import { afterEach, describe, expect, it } from 'vitest';
import { $, $$, button, byRole, click, mountHeat, press, type Rig, type } from '../testkit';

// docs/SPEC.md 3.9 and docs/PLAN.md S2.4. What a fail looks like: a 7th habit
// that fits, or a limit with no "Habit limit reached"; the streak counter
// showing by default; a record that isn't "Done N days since …"; a year that
// isn't 53 × 7; a habit with a length that doesn't tick itself when a focus
// session on it reaches that length.

let rig: Rig;
afterEach(() => rig?.unmount());

async function open() {
  rig = await mountHeat();
  await click(button(rig, 'Habits'));
}
const text = (el: Element | null | undefined) => (el?.textContent ?? '').replace(/\s+/g, ' ').trim();
const card = (title: string) => $(rig, `.heat-habit[aria-label="${title}"]`)!;

describe('Habits', () => {
  it('gives each habit an orb, its record, and a 14-day grid', async () => {
    await open();
    expect($$(rig, '.heat-habit').map((c) => c.getAttribute('aria-label'))).toEqual([
      'Practise kanji',
      'Stretch',
      'Read ten pages',
    ]);
    // The record only grows: "Done 9 days since September 28", never a counter by default.
    expect(text($(card('Practise kanji'), '.heat-habit-record'))).toBe('Done 9 days since September 28');
    expect(text($(card('Practise kanji'), '.heat-habit-title'))).toBe('Practise kanji, 20m');
    expect($$(card('Practise kanji'), '.habit-cell')).toHaveLength(14);
    expect(
      $$(rig, '.heat-habit-record')
        .map((r) => r.textContent)
        .join(),
    ).not.toMatch(/streak/);
    expect(rig.status().count).toBe('1 of 3 done');
    // Today's square is open until it is ticked, because today counts until midnight.
    const today = $$(card('Stretch'), '.habit-cell').at(-1)!;
    expect(today.getAttribute('data-today')).toBe('true');
    expect(today.getAttribute('aria-label')).toContain('today');
  });

  it('ticks today with the orb, and a past day with its square, each as its own undo step', async () => {
    await open();
    await click($(card('Stretch'), '.heat-habit-orb'));
    expect(rig.fake.store.habit.get('h-stretch')!.log['2026-10-07']).toBe(true);
    expect((await rig.call<{ undo: string }>('history.get', {})).undo).toBe('Undo tick habit');
    // Two days back was missed: its square ticks it.
    await click($$(card('Stretch'), '.habit-cell').at(-3)!);
    expect(rig.fake.store.habit.get('h-stretch')!.log['2026-10-05']).toBe(true);
    await click($(card('Stretch'), '.heat-habit-orb'));
    expect((await rig.call<{ undo: string }>('history.get', {})).undo).toBe('Undo untick habit');
  });

  it('shows the year as 53 weeks of 7 days, under each habit', async () => {
    await open();
    expect($(rig, '.heat-year')).toBeNull();
    await click(button(rig, 'Show the year'));
    expect($$(rig, '.heat-year')).toHaveLength(3);
    expect($$(card('Practise kanji'), '.heat-year-cell')).toHaveLength(53 * 7);
    expect($$(card('Practise kanji'), '.heat-year-cell[data-done="true"]')).toHaveLength(9);
    // Days still to come are left out of the picture.
    expect($$(card('Practise kanji'), '.heat-year-cell[data-future="true"]')).toHaveLength(3);
    expect(rig.status().act).toBe('Show the year');
    await click(button(rig, 'Hide the year'));
    expect($(rig, '.heat-year')).toBeNull();
  });
});

describe('the limit of 6', () => {
  it('says "Habit limit reached" at 6, on "+" and on N, and the core refuses a 7th', async () => {
    await open();
    for (const title of ['Walk', 'Floss', 'Journal'])
      await rig.client.put('habit', { title, log: {}, showCounter: false });
    await click(button(rig, 'Habits'));
    expect(rig.fake.store.habit.size).toBe(6);
    const plus = $(rig, '.heat-plus')!;
    expect(plus.getAttribute('aria-label')).toBe('New habit');
    expect(plus.hasAttribute('disabled')).toBe(true);
    expect(text($(rig, '.heat-why'))).toBe('Habit limit reached');
    expect(await press(rig, 'n')).toBe(true);
    expect(rig.status().count).toBe('Habit limit reached');
    expect(byRole(rig, 'dialog', 'New habit')).toBeUndefined();
    await expect(rig.client.put('habit', { title: 'A seventh', log: {}, showCounter: false })).rejects.toThrow(
      'Habit limit reached',
    );
    expect(rig.fake.store.habit.size).toBe(6);
    // Deleting one makes room.
    await click(button(rig, 'Edit habit…'));
    await click(button(rig, 'Delete habit'));
    expect($(rig, '.heat-plus')!.hasAttribute('disabled')).toBe(false);
  });
});

describe('a habit and its settings', () => {
  it('"+" adds one, with a length that puts it under Recurring in Today, and the counter off', async () => {
    await open();
    await click(button(rig, 'New habit'));
    const sheet = byRole(rig, 'dialog', 'New habit')!;
    await type($(sheet, 'input'), 'Scales');
    await type($(sheet, 'input[type="number"]'), '15');
    expect(($(sheet, 'input[type="checkbox"]') as HTMLInputElement).checked).toBe(false);
    await click($(sheet, 'button[type="submit"]'));
    const made = [...rig.fake.store.habit.values()].find((h) => h.title === 'Scales')!;
    expect(made).toMatchObject({ minutes: 15, showCounter: false, public: false, log: {} });
    expect(text($(card('Scales'), '.heat-habit-record'))).toBe('Not done yet');
    const snap = await rig.client.snapshot('2026-10-07');
    expect(snap.derived.today.recurringToday).toContainEqual({ kind: 'habit', id: made.id });
  });

  it('asks for a name first, in one line', async () => {
    await open();
    await click(button(rig, 'New habit'));
    await click($(byRole(rig, 'dialog', 'New habit'), 'button[type="submit"]'));
    expect(text($(rig, '.heat-sheet-why'))).toBe('Give the habit a name first.');
  });

  it('shows a streak counter only for the habit that switched it on', async () => {
    await open();
    await click($$(rig, 'button').find((b) => b.textContent === 'Edit habit…' && card('Practise kanji').contains(b)));
    await click($(byRole(rig, 'dialog', 'Edit habit Practise kanji'), 'input[type="checkbox"]'));
    await click($(byRole(rig, 'dialog', 'Edit habit Practise kanji'), 'button[type="submit"]'));
    expect(text($(card('Practise kanji'), '.heat-habit-record'))).toBe('9-day streak');
    expect(text($(card('Stretch'), '.heat-habit-record'))).toMatch(/^Done 2 days since/);
  });

  it('starts private: every habit’s Public switch is off, with what it would show', async () => {
    await open();
    const switches = $$(rig, '.heat-habit input[role="switch"]') as HTMLInputElement[];
    expect(switches.map((s) => s.checked)).toEqual([false, false, false]);
    expect(text($(card('Stretch'), '.heat-switch-hint'))).toContain('A streak or running count never leaves your Mac.');
  });
});

describe('a habit with a length', () => {
  it('ticks itself when a focus session on it reaches that length, and logs the session', async () => {
    await open();
    expect(rig.fake.store.habit.get('h-kanji')!.log['2026-10-07']).toBeUndefined();
    await click(button(card('Practise kanji'), 'Start focus'));
    expect((await rig.client.snapshot('2026-10-07')).heatState.timer).toMatchObject({
      phase: 'focus',
      habitId: 'h-kanji',
    });
    // 19 minutes in, it hasn't ticked; at 20 it does, without being asked.
    rig.fake.now += 19 * 60_000;
    await rig.client.focus('finish');
    expect(rig.fake.store.habit.get('h-kanji')!.log['2026-10-07']).toBeUndefined();
    rig.fake.now += 60_000;
    await rig.client.focus('finish');
    expect(rig.fake.store.habit.get('h-kanji')!.log['2026-10-07']).toBe(true);
    await rig.client.focus('stop');
    const session = [...rig.fake.store.focusSession.values()].find((s) => s.habitId === 'h-kanji')!;
    expect(session.focusMin).toBe(20);
    expect(session.taskId).toBeUndefined();
  });
});
