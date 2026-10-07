import { afterEach, describe, expect, it } from 'vitest';
import { TAB_IDS, TAB_TABLE } from './frame';
import { $, $$, button, click, mountHeat, press, type Rig, setViewport, settle } from './testkit';

// docs/PLAN.md S2.4 and docs/SPEC.md 3.3. What a fail looks like: a tab with
// no "+" (or Mail with one) or without its one secondary act; the right
// column not folded into a 44 px strip of icons below 1240 pt, or folded at
// 1240; the spaces filter without All, each space with its open count, and
// "New space…"; Heat not refetching when the core says `heat` changed.

let rig: Rig;
afterEach(() => rig?.unmount());

const tabButton = (name: string) => button(rig, name)!;

describe('Learn’s frame', () => {
  it('holds a "+", the six tabs as one segmented control, and Sync', async () => {
    rig = await mountHeat();
    const tabs = $$(rig, '[role="tab"]').map((t) => t.textContent);
    expect(tabs).toEqual(['Today', 'Tasks', 'Calendar', 'Grades', 'Habits', 'Mail']);
    expect(tabs).toEqual(TAB_IDS.map((id) => TAB_TABLE[id].name));
    expect(button(rig, 'New task')).toBeTruthy();
    expect(button(rig, 'Sync')).toBeTruthy();
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Today');
  });

  it('gives each tab one "+" and one secondary act, from 3.3’s table', async () => {
    rig = await mountHeat();
    const plus = () => $(rig, '.heat-plus');
    const act = () => rig.status().act;
    // The tabs this build has: "+" and the secondary act the status bar names.
    await click(tabButton('Today'));
    expect(plus()!.getAttribute('aria-label')).toBe('New task');
    expect(act()).toBe('Plan my day');
    await click(tabButton('Calendar'));
    expect(plus()!.getAttribute('aria-label')).toBe('New task');
    expect(act()).toBe('Today');
    // Tasks names Triage inbox only while the inbox has items; the sample has three.
    await click(tabButton('Tasks'));
    expect(plus()!.getAttribute('aria-label')).toBe('New task');
    expect(act()).toBe('Triage inbox');
    // Grades adds a grade once a course exists, and its one secondary act is Add course.
    await click(tabButton('Grades'));
    expect(plus()!.getAttribute('aria-label')).toBe('New grade');
    expect(plus()!.hasAttribute('disabled')).toBe(false);
    expect(act()).toBe('Add course');
    // The tab still to come shows its "+" disabled and says why; Mail has none.
    await click(tabButton('Habits'));
    expect(plus()!.getAttribute('aria-label')).toBe('New habit');
    await click(tabButton('Mail'));
    expect(plus()).toBeNull();
    expect(TAB_TABLE.mail.secondary).toBe('Open in Gmail');
  });

  it('keeps every tab built, and shows one', async () => {
    rig = await mountHeat();
    expect($$(rig, '[role="tabpanel"]')).toHaveLength(6);
    await click(tabButton('Tasks'));
    const current = $$(rig, '[role="tabpanel"]').filter((p) => p.getAttribute('data-current') === 'true');
    expect(current.map((p) => p.id)).toEqual(['heat-panel-tasks']);
    // The tabs that aren't showing are out of the keyboard's way.
    expect($(rig, '#heat-panel-today')!.hasAttribute('inert')).toBe(true);
  });

  it('switches tabs on 1–6, but not while a field has the keyboard', async () => {
    rig = await mountHeat();
    expect(await press(rig, '3')).toBe(true);
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Calendar');
    expect(await press(rig, '6')).toBe(true);
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Mail');
    const field = document.createElement('input');
    rig.host.append(field);
    field.focus();
    expect(await press(rig, '1')).toBe(false);
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Mail');
    field.remove();
  });
});

describe('the spaces filter', () => {
  it('opens with All, then each space with its hue dot and open count, then "New space…"', async () => {
    rig = await mountHeat();
    const rows = $$(rig, '.heat-spaces .heat-side-row').map((r) => r.textContent);
    expect(rows).toEqual(['All11', 'Classes4', 'WWAV5', 'Personal2', 'New space…']);
    expect($$(rig, '.heat-spaces .heat-dot')).toHaveLength(3);
    expect($(rig, '.heat-spaces [aria-current="true"]')!.textContent).toContain('All');
    await click(button(rig, /^WWAV/));
    expect($(rig, '.heat-spaces [aria-current="true"]')!.textContent).toContain('WWAV');
  });

  it('sends "New space…" to Settings, where spaces are edited', async () => {
    rig = await mountHeat();
    await click(button(rig, 'New space…'));
    expect(rig.opens.settings).toBe(1);
  });

  it('says Grades and Habits ignore it', async () => {
    rig = await mountHeat();
    await click(tabButton('Grades'));
    expect($(rig, '.heat-side-why')!.textContent).toBe('Grades ignore spaces.');
    await click(tabButton('Today'));
    expect($(rig, '.heat-side-why')).toBeNull();
  });
});

describe('the right column', () => {
  const widgets = () => $$(rig, '.heat-widget-title').map((t) => t.textContent);

  it('is five metal panels at 1240 pt and wider, with Grades once a course exists', async () => {
    rig = await mountHeat({ width: 1240 });
    expect($(rig, '.heat')!.getAttribute('data-folded')).toBe('false');
    expect(widgets()).toEqual(['Now', 'Habits', 'Hot tasks', 'Mail', 'Grades']);
    expect($(rig, '.heat-strip')).toBeNull();
  });

  it('folds into a 44 px strip of icons below 1240 pt, each opening its widget as a popover', async () => {
    rig = await mountHeat({ width: 1239 });
    expect($(rig, '.heat')!.getAttribute('data-folded')).toBe('true');
    const icons = $$(rig, '.heat-widget-icon');
    expect(icons.map((i) => i.getAttribute('aria-label'))).toEqual(['Now', 'Habits', 'Hot tasks', 'Mail', 'Grades']);
    expect($$(rig, '.heat-widget-well')).toHaveLength(0);
    await click(icons[2]);
    expect($(rig, '.heat-popover')!.getAttribute('aria-label')).toBe('Hot tasks');
    expect($(rig, '.heat-popover')!.textContent).toContain('Grammar quiz 4');
    await click(icons[2]);
    expect($(rig, '.heat-popover')).toBeNull();
  });

  it('folds and unfolds as the window is resized', async () => {
    rig = await mountHeat({ width: 1280 });
    expect($(rig, '.heat')!.getAttribute('data-folded')).toBe('false');
    const { act } = await import('react');
    act(() => setViewport(1100));
    await settle();
    expect($(rig, '.heat')!.getAttribute('data-folded')).toBe('true');
    act(() => setViewport(1300));
    await settle();
    expect($(rig, '.heat')!.getAttribute('data-folded')).toBe('false');
  });

  it('hides Grades until a course exists', async () => {
    rig = await mountHeat({ empty: true });
    expect(widgets()).toEqual(['Now', 'Habits', 'Hot tasks', 'Mail']);
  });
});

describe('the snapshot', () => {
  it('is fetched again when the core says heat changed', async () => {
    rig = await mountHeat();
    const before = $$(rig, '.heat-spaces .heat-side-count')[0].textContent;
    await rig.client.put('task', {
      spaceId: 'sp-classes',
      title: 'A new one',
      type: 'Quiz',
      due: null,
      difficulty: 3,
      estMin: null,
      adjustMin: 0,
      notes: '',
      done: false,
      doneAt: null,
      source: 'you',
    });
    await settle();
    expect(Number($$(rig, '.heat-spaces .heat-side-count')[0].textContent)).toBe(Number(before) + 1);
  });

  it('tells the status bar the save line', async () => {
    rig = await mountHeat();
    expect(rig.status().save).toBe('Saved on this Mac');
  });
});
