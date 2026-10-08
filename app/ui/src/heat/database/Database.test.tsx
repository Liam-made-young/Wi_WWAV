// The Database tab (heat/database/Database.tsx), on a stand-in for the
// core's db.*. What a fail looks like: the tab reads before it is shown or
// stops keeping up; an edit, a new row, a formula, a view or an export asks
// the core for something other than what the person did; a refusal isn't
// said; the prompt box isn't told what is on screen.

import { act } from 'react';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { screen } from '../../ask/context';
import { navigate } from '../../ask/nav';
import { standIn, type StandIn } from '../../ask/testkit';
import { $, $$, button, click, mountHeat, press, type Rig, settle, type, wait } from '../testkit';
import type { Column, Query } from './api';

beforeAll(() => {
  (globalThis as { ResizeObserver?: unknown }).ResizeObserver = class {
    observe() {}
    disconnect() {}
  };
});

const col = (id: string, name: string, type: Column['type'], more: Partial<Column> = {}): Column => ({
  id,
  name,
  type,
  locked: false,
  added: false,
  ...more,
});

const TASKS: Query = {
  table: { id: 'task', name: 'Tasks', origin: 'learn', locked: false },
  columns: [
    col('id', 'ID', 'text', { locked: true, why: 'A record’s own id never changes.' }),
    col('title', 'Title', 'text'),
    col('courseId', 'Course', 'relation', { relation: 'course' }),
    col('estMin', 'Estimate (min)', 'number'),
    col('link', 'Link', 'json', { locked: true }),
    col('~heat', 'Heat', 'number', { locked: true, why: 'Learn works this out, so it can’t be typed over.' }),
  ],
  rows: [
    { id: 't1', cells: ['t1', 'Kanji quiz', { id: 'c1', label: 'JPN 101' }, 45, null, 0.8] },
    { id: 't2', cells: ['t2', 'Lab report', { id: 'c2', label: 'PHY 204' }, 120, null, 0.4] },
  ],
  total: 2,
  all: 2,
  summary: { estMin: { sum: 165, average: 82.5, count: 2 }, title: { count: 2 } },
  groups: [],
};

const COURSES: Query = {
  table: { id: 'course', name: 'Courses', origin: 'learn', locked: false },
  columns: [col('code', 'Code', 'text'), col('name', 'Name', 'text')],
  rows: [{ id: 'c1', cells: ['JPN 101', 'Elementary Japanese'] }, { id: 'c2', cells: ['PHY 204', 'Physics II'] }],
  total: 2,
  all: 2,
  summary: {},
  groups: [],
};

const MAIL: Query = {
  table: { id: 'mailThread', name: 'Mail', origin: 'learn', locked: true, why: 'Mail lists only what Claude recorded. Ask Claude to read it.' },
  columns: [col('subject', 'Subject', 'text', { locked: true })],
  rows: [],
  total: 0,
  all: 0,
  summary: {},
  groups: [],
};

let rig: Rig | null = null;
let core: StandIn;

async function mount(more: Parameters<typeof standIn>[0] = {}): Promise<Rig> {
  core = standIn({
    'db.tables': () => ({
      tables: [
        { id: 'task', name: 'Tasks', origin: 'learn', count: 2 },
        { id: 'course', name: 'Courses', origin: 'learn', count: 2 },
        { id: 'mailThread', name: 'Mail', origin: 'learn', count: 0 },
      ],
      views: [{ id: 'v1', table: 'task', name: 'Long ones', spec: { filters: [{ column: 'estMin', op: 'ge', value: '90' }], pivot: { rows: ['courseId'], values: [] } } }],
      layouts: {},
      functions: ['SUM', 'TODAY'],
    }),
    'db.query': ({ table }) => (table === 'course' ? COURSES : table === 'mailThread' ? MAIL : TASKS),
    'db.cells.set': () => ({ changed: 1, failed: [], undo: 'Undo edit task' }),
    'db.rows.add': () => ({ ids: ['t3'], failed: [], undo: 'Undo add task' }),
    'db.layout.set': () => ({}),
    'db.formula.check': ({ formula }) =>
      String(formula).includes('Nope') ? { ok: false, message: 'Tasks has no column called ‘Nope’.', at: 0 } : { ok: true, sample: [0.75, 2] },
    'db.column.add': () => ({ column: {}, undo: 'Undo add column' }),
    'db.view.save': ({ name }) => ({ view: { id: 'v2', table: 'task', name, spec: {} } }),
    'db.pivot': () => ({
      by: [{ id: 'courseId', name: 'Course' }],
      values: [],
      groups: [{ keys: ['JPN 101'], count: 1, values: [] }, { keys: ['PHY 204'], count: 1, values: [] }],
      total: { count: 2, values: [] },
    }),
    'db.csv.export': () => ({ file: 'Tasks.csv', rows: 2 }),
    ...more,
  });
  rig = await mountHeat();
  return rig;
}

afterEach(() => {
  rig?.unmount();
  rig = null;
});

const show = async () => {
  await click(button(rig, 'Database'));
  await settle(6);
};
const heads = () => $$(rig, '.db-th .db-th-name').map((h) => h.textContent);
const cell = (row: number, c: number) => $$(rig, `.db-row[aria-rowindex="${row}"] .db-cell`)[c];
const down = (el: Element) =>
  act(async () => {
    el.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0 }));
  });
const sideRows = () => $$(document.body, '.db-side .heat-side-row').map((r) => r.textContent);
const bar = (name: string | RegExp) => $$(rig, '.db-bar button').find((b) => (typeof name === 'string' ? b.textContent === name : name.test(b.textContent ?? '')));

describe('the Database tab', () => {
  it('reads nothing until it is shown, then lists every table and shows the first', async () => {
    await mount();
    expect(core.calls).toHaveLength(0);
    await show();
    expect(sideRows()).toEqual(['Tasks2', 'Long ones', 'Courses2', 'Mail0', 'New table…', 'Import CSV…']);
    // What a row is called comes first; the id and the list are there under Columns, not in the way.
    expect(heads()).toEqual(['Title', 'Course', 'Estimate (min)', 'Heat']);
    expect($$(rig, '.db-row[aria-rowindex]').map((r) => r.textContent)).toEqual(['1Kanji quizJPN 101450.8', '2Lab reportPHY 2041200.4']);
    expect(rig!.status().count).toBe('2 rows');
    expect($(rig, '.db-summary')!.textContent).toContain('Sum 165');
    expect($(rig, '.db-summary')!.textContent).toContain('Count 2');
  });

  it('hands an edit to the core as the cell it was made in, and reads again', async () => {
    await mount();
    await show();
    await down(cell(1, 2));
    await press(rig!, '9');
    const editor = $(rig, '.db-editor') as HTMLInputElement;
    await type(editor, '90');
    await act(async () => {
      editor.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    await settle(6);
    expect(core.asked('db.cells.set')).toEqual([{ table: 'task', edits: [{ row: 't1', column: 'estMin', value: '90' }] }]);
    expect(core.asked('db.query').length).toBeGreaterThan(1);
  });

  it('says what the core refused, in its sentence', async () => {
    await mount({
      'db.cells.set': () => ({ changed: 0, failed: [{ row: 't1', column: 'estMin', message: '‘soon’ isn’t a number.' }], undo: null }),
    });
    await show();
    await down(cell(1, 2));
    await press(rig!, 's');
    const editor = $(rig, '.db-editor') as HTMLInputElement;
    await type(editor, 'soon');
    await act(async () => {
      editor.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    await settle(6);
    expect(rig!.status().count).toBe('‘soon’ isn’t a number.');
    // A locked cell says why without asking the core at all.
    await down(cell(1, 3));
    await press(rig!, '1');
    expect(rig!.status().count).toBe('Learn works this out, so it can’t be typed over.');
    expect(core.asked('db.cells.set')).toHaveLength(1);
  });

  it('asks the core for the rows a search, a sort and a group leave', async () => {
    await mount();
    await show();
    await type($(rig, '.db-search'), 'kanji');
    await settle(6);
    expect(core.asked('db.query').at(-1)!.spec).toMatchObject({ search: 'kanji' });
    await click($$(rig, '.db-th').find((h) => h.textContent?.includes('Estimate')));
    await click($$(rig, '.db-menu-item').find((m) => m.textContent === 'Sort descending'));
    await settle(6);
    expect(core.asked('db.query').at(-1)!.spec).toMatchObject({ search: 'kanji', sorts: [{ column: 'estMin', dir: 'desc' }] });
    await click($$(rig, '.db-th').find((h) => h.textContent?.includes('Course')));
    await click($$(rig, '.db-menu-item').find((m) => m.textContent === 'Group by this column'));
    await settle(6);
    expect(core.asked('db.query').at(-1)!.spec).toMatchObject({ group: 'courseId' });
    // Hiding a column asks the core nothing: it is the grid's own business.
    const before = core.asked('db.query').length;
    await click($$(rig, '.db-th').find((h) => h.textContent?.includes('Heat')));
    await click($$(rig, '.db-menu-item').find((m) => m.textContent === 'Hide column'));
    expect(heads()).toEqual(['Title', 'Course', 'Estimate (min)']);
    expect(core.asked('db.query')).toHaveLength(before);
    // The layout is remembered for the table, outside the journal.
    await wait(450);
    expect(core.asked('db.layout.set').at(-1)).toMatchObject({ table: 'task', spec: { hidden: expect.arrayContaining(['~heat', 'id']) } });
  });

  it('adds a row by what it is called', async () => {
    await mount();
    await show();
    const line = $(rig, '.db-new-input') as HTMLInputElement;
    expect(line.placeholder).toBe('New title, then Return');
    await type(line, 'Vocabulary cards');
    await act(async () => {
      line.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    await settle(6);
    expect(core.asked('db.rows.add')).toEqual([{ table: 'task', rows: [{ title: 'Vocabulary cards' }] }]);
  });

  it('checks a formula as it is typed, and adds the column when it reads', async () => {
    await mount();
    await show();
    await click($(rig, '.db-add-column'));
    // On one of Learn's own tables a new column is a formula.
    expect($$(rig, '.db-form select option').map((o) => o.textContent)).toContain('Formula');
    expect($(rig, '.db-form')!.textContent).toContain('worked out by a formula');
    await type($(rig, '.db-form input'), 'Hours');
    await type($(rig, '.db-formula'), '[Nope] / 60');
    await wait(260);
    await settle();
    expect($(rig, '.db-form-note[role="status"]')!.textContent).toBe('Tasks has no column called ‘Nope’.');
    expect(button(rig, 'Add column')!.hasAttribute('disabled')).toBe(true);
    await type($(rig, '.db-formula'), '[Estimate (min)] / 60');
    await wait(260);
    await settle();
    expect($(rig, '.db-form-note[role="status"]')!.textContent).toBe('First rows: 0.75 · 2');
    await click(button(rig, 'Add column'));
    expect(core.asked('db.column.add')).toEqual([{ table: 'task', name: 'Hours', type: 'formula', formula: '[Estimate (min)] / 60' }]);
  });

  it('opens a saved view with its filters and its pivot, and saves a new one', async () => {
    await mount();
    await show();
    await click($$(document.body, '.db-side-view')[0]);
    await settle(6);
    expect($(rig, '.db-title')!.textContent).toBe('Tasks · Long ones');
    expect(core.asked('db.query').at(-1)!.spec).toMatchObject({ filters: [{ column: 'estMin', op: 'ge', value: '90' }] });
    expect($(rig, '.db-pivot')!.textContent).toContain('JPN 1011');
    expect(core.asked('db.pivot').at(-1)).toMatchObject({ table: 'task', rows: ['courseId'] });
    await click(bar(/^Save view/));
    await type($(rig, '.db-form input'), 'Long, by course');
    await click(button(rig, 'Save as new'));
    expect(core.asked('db.view.save').at(-1)).toMatchObject({ table: 'task', name: 'Long, by course', spec: { pivot: { rows: ['courseId'] } } });
    expect(rig!.status().count).toBe('Saved the view ‘Long, by course’.');
    await click(bar('Export CSV'));
    expect(core.asked('db.csv.export').at(-1)).toMatchObject({ table: 'task', to: 'downloads' });
    expect(rig!.status().count).toBe('Saved Tasks.csv to Downloads: 2 rows.');
  });

  it('follows a link to a row in another table, from a cell or from anywhere', async () => {
    await mount();
    await show();
    await click($(rig, '.db-link'));
    await wait(60);
    expect($(rig, '.db-title')!.textContent).toBe('Courses');
    expect($$(rig, '.db-gutter[data-selected]').map((g) => g.textContent)).toEqual(['1']);
    // From another tab: the prompt box's link to a record.
    await click(button(rig, 'Today'));
    await act(async () => navigate({ what: 'row', table: 'task', id: 't2' }));
    await wait(60);
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Database');
    expect($(rig, '.db-title')!.textContent).toBe('Tasks');
    expect($$(rig, '.db-gutter[data-selected]').map((g) => g.textContent)).toEqual(['2']);
  });

  it('shows a table nobody edits as read-only, with why', async () => {
    await mount();
    await show();
    await click($$(document.body, '.db-side .heat-side-row').find((r) => r.textContent === 'Mail0'));
    await settle(6);
    expect($(rig, '.db-locked')!.textContent).toBe('Read-only. Mail lists only what Claude recorded. Ask Claude to read it.');
    expect($(rig, '.db-new-input')).toBeNull();
  });

  it('keeps up when Learn changes, and tells the prompt box what is showing', async () => {
    await mount();
    await show();
    const before = core.asked('db.query').length;
    await act(async () => core.emit('heat', { kinds: ['task'] }));
    await wait(120);
    expect(core.asked('db.query').length).toBe(before + 1);
    await down(cell(2, 0));
    expect(screen()!.database).toMatchObject({
      table: 'Tasks',
      tableId: 'task',
      columns: ['Title', 'Course', 'Estimate (min)', 'Heat'],
      rowsShown: 2,
      selectedRows: ['t2'],
      selectedColumns: ['Title'],
    });
    // Away from the tab it reads nothing, and says nothing is on screen.
    await click(button(rig, 'Today'));
    await act(async () => core.emit('heat', { kinds: ['task'] }));
    await wait(120);
    expect(core.asked('db.query').length).toBe(before + 1);
    expect(screen()!.database).toBeUndefined();
  });
});
