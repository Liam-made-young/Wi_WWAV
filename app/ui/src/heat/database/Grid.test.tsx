// The Database tab's grid (Grid.tsx). What a fail looks like: a table of
// thousands of rows builds thousands of rows; a key, a paste or a fill
// changes a cell the grid was told is locked, or asks for more than one
// change where a person made one; a selection can't be made or moved from
// the keyboard.

import { act, createRef } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import type { HeatKey } from '../frame';
import type { Column, Edit, Query } from './api';
import { Grid, type GridHandle } from './Grid';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

beforeAll(() => {
  (globalThis as { ResizeObserver?: unknown }).ResizeObserver = class {
    observe() {}
    disconnect() {}
  };
});

const COLUMNS: Column[] = [
  { id: 'title', name: 'Title', type: 'text', locked: false, added: false },
  { id: 'estMin', name: 'Estimate (min)', type: 'number', locked: false, added: false },
  { id: 'done', name: 'Done', type: 'bool', locked: false, added: false },
  { id: '~heat', name: 'Heat', type: 'number', locked: true, why: 'Learn works this out.', added: false },
  { id: 'courseId', name: 'Course', type: 'relation', relation: 'course', locked: false, added: false },
];

const rowsOf = (n: number): Query['rows'] =>
  Array.from({ length: n }, (_, i) => ({
    id: `r${i + 1}`,
    cells: [`Task ${i + 1}`, (i + 1) * 10, i % 2 === 0, 0.5, i === 0 ? { id: 'c1', label: 'JPN 101' } : null],
  }));

interface Rig {
  host: HTMLElement;
  root: Root;
  grid: GridHandle;
  edits: { edits: Edit[]; label?: string }[];
  said: string[];
  links: string[];
}

let rig: Rig | null = null;

function mount(n = 4, lockedWhy: string | null = null): Rig {
  const host = document.createElement('div');
  document.body.append(host);
  const root = createRoot(host);
  const ref = createRef<GridHandle>();
  const out: Rig = { host, root, grid: null as never, edits: [], said: [], links: [] };
  act(() =>
    root.render(
      <Grid
        ref={ref}
        columns={COLUMNS}
        at={[0, 1, 2, 3, 4]}
        widths={{}}
        frozen={1}
        rows={rowsOf(n)}
        groups={[]}
        summary={{ estMin: { sum: 100, average: 25 } }}
        picked={{ estMin: 'sum' }}
        sorts={[]}
        lockedWhy={lockedWhy}
        onEdit={(edits, label) => out.edits.push({ edits, label })}
        onSay={(text) => out.said.push(text)}
        onLink={(table, id) => out.links.push(`${table}/${id}`)}
        onHeader={() => {}}
        onResize={() => {}}
        onReorder={() => {}}
        onPick={() => {}}
        newRow={null}
        onAddColumn={() => {}}
      />,
    ),
  );
  // The handle is made again on each draw: always the one of the latest.
  Object.defineProperty(out, 'grid', { get: () => ref.current! });
  rig = out;
  return out;
}

afterEach(() => {
  if (rig) {
    act(() => rig!.root.unmount());
    rig.host.remove();
    rig = null;
  }
});

const key = (k: string, over: Partial<HeatKey> = {}): HeatKey => ({
  key: k,
  code: k.length === 1 ? `Key${k.toUpperCase()}` : k,
  shift: false,
  command: false,
  alt: false,
  repeat: false,
  ...over,
});

const press = (r: Rig, k: string, over: Partial<HeatKey> = {}) => {
  let took = false;
  act(() => {
    took = r.grid.key(key(k, over));
  });
  return took;
};

const esc = (r: Rig) => {
  let took = false;
  act(() => {
    took = r.grid.escape();
  });
  return took;
};

const cell = (r: Rig, row: number, col: number) =>
  r.host.querySelectorAll<HTMLElement>(`.db-row[aria-rowindex="${row}"] .db-cell`)[col];
const down = (el: Element, init: PointerEventInit = {}) =>
  act(() => {
    el.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true, button: 0, ...init }));
  });
const active = (r: Rig) => r.host.querySelector('.db-cell[data-active]')?.textContent;
const selected = (r: Rig) => r.host.querySelectorAll('.db-cell[data-selected]').length;

function clipboard(type: 'copy' | 'paste' | 'cut', text = '') {
  const data = new Map<string, string>([['text/plain', text]]);
  const e = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperty(e, 'clipboardData', {
    value: { getData: (k: string) => data.get(k) ?? '', setData: (k: string, v: string) => data.set(k, v) },
  });
  act(() => {
    rig!.host.querySelector('.db-grid')!.dispatchEvent(e);
  });
  return data.get('text/plain') ?? '';
}

describe('the grid', () => {
  it('draws only the rows in view, however many there are', () => {
    const r = mount(5000);
    expect(r.host.querySelector('.db-grid')!.getAttribute('aria-rowcount')).toBe('5000');
    expect(r.host.querySelectorAll('.db-row').length).toBeLessThan(60);
    // The canvas is as tall as every row, so the scrollbar is the table's.
    expect(Number.parseInt(r.host.querySelector<HTMLElement>('.db-canvas')!.style.height, 10)).toBeGreaterThan(5000 * 28);
  });

  it('moves and grows the selection with the keyboard', () => {
    const r = mount();
    // With nothing selected, a digit is the frame's (it opens a tab), and an arrow starts at the first cell.
    expect(press(r, '2')).toBe(false);
    expect(press(r, 'ArrowDown')).toBe(true);
    expect(active(r)).toBe('Task 1');
    press(r, 'ArrowDown');
    press(r, 'ArrowRight');
    expect(active(r)).toBe('20');
    press(r, 'Tab');
    press(r, 'Tab', { shift: true });
    expect(active(r)).toBe('20');
    press(r, 'ArrowDown', { shift: true });
    press(r, 'ArrowRight', { shift: true });
    expect(selected(r)).toBe(4);
    expect(r.grid.selected()).toEqual({ rows: ['r2', 'r3'], columns: ['estMin', 'done'] });
    press(r, 'a', { command: true });
    expect(selected(r)).toBe(20);
    press(r, 'ArrowUp', { command: true });
    expect(active(r)).toBe('JPN 101');
    // Esc drops the selection, and then has nothing left to do.
    expect(esc(r)).toBe(true);
    expect(selected(r)).toBe(0);
    expect(esc(r)).toBe(false);
  });

  it('types into a cell and hands the change up once', () => {
    const r = mount();
    down(cell(r, 2, 1));
    expect(press(r, '4')).toBe(true);
    const editor = r.host.querySelector<HTMLInputElement>('.db-editor')!;
    expect(editor.value).toBe('4');
    const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
    act(() => {
      set.call(editor, '45');
      editor.dispatchEvent(new Event('input', { bubbles: true }));
    });
    // While a cell is being typed in, the grid takes no keys of its own.
    expect(press(r, 'ArrowDown')).toBe(false);
    act(() => {
      editor.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    });
    expect(r.edits).toEqual([{ edits: [{ row: 'r2', column: 'estMin', value: '45' }], label: undefined }]);
    expect(active(r)).toBe('30');
    // Return opens the cell with what it holds; leaving it unchanged asks for nothing.
    press(r, 'Enter');
    expect(r.host.querySelector<HTMLInputElement>('.db-editor')!.value).toBe('30');
    expect(esc(r)).toBe(true);
    expect(r.edits).toHaveLength(1);
  });

  it('flips a checkbox, follows a link, and says why a locked cell stays', () => {
    const r = mount();
    down(cell(r, 1, 2));
    press(r, 'Enter');
    expect(r.edits.pop()).toEqual({ edits: [{ row: 'r1', column: 'done', value: false }], label: undefined });
    act(() => r.host.querySelector<HTMLElement>('.db-link')!.click());
    expect(r.links).toEqual(['course/c1']);
    down(cell(r, 1, 3));
    press(r, 'Enter');
    press(r, '9');
    expect(r.said).toEqual(['Learn works this out.', 'Learn works this out.']);
    expect(r.edits).toHaveLength(0);
    expect(cell(r, 1, 3).hasAttribute('data-locked')).toBe(true);
  });

  it('copies a rectangle as tab-separated text and pastes one as a single change', () => {
    const r = mount();
    down(cell(r, 1, 0));
    down(cell(r, 2, 1), { shiftKey: true });
    expect(clipboard('copy')).toBe('Task 1\t10\nTask 2\t20');
    // A block pasted at a cell lands from there, skipping what is locked and what is off the table.
    down(cell(r, 3, 1));
    clipboard('paste', '15\tyes\t7\n25\tno\t8\n35\tno\t9\n');
    expect(r.edits).toEqual([
      {
        label: 'paste',
        edits: [
          { row: 'r3', column: 'estMin', value: '15' },
          { row: 'r3', column: 'done', value: 'yes' },
          { row: 'r4', column: 'estMin', value: '25' },
          { row: 'r4', column: 'done', value: 'no' },
        ],
      },
    ]);
    // One value pasted over a selection fills it.
    r.edits.length = 0;
    down(cell(r, 1, 1));
    down(cell(r, 3, 1), { shiftKey: true });
    clipboard('paste', '60');
    expect(r.edits[0].edits.map((e) => [e.row, e.value])).toEqual([['r1', '60'], ['r2', '60'], ['r3', '60']]);
    // Cutting copies, then clears what can be cleared.
    r.edits.length = 0;
    expect(clipboard('cut')).toBe('10\n20\n30');
    expect(r.edits[0]).toEqual({
      label: 'clear cells',
      edits: [
        { row: 'r1', column: 'estMin', value: '' },
        { row: 'r2', column: 'estMin', value: '' },
        { row: 'r3', column: 'estMin', value: '' },
      ],
    });
  });

  it('fills down and clears, each as one change', () => {
    const r = mount();
    down(cell(r, 1, 0));
    down(cell(r, 3, 4), { shiftKey: true });
    press(r, 'd', { command: true });
    expect(r.edits[0].label).toBe('fill down');
    // The top row's values go down each column; the locked column is passed over; a link goes by its id.
    expect(r.edits[0].edits.filter((e) => e.row === 'r2')).toEqual([
      { row: 'r2', column: 'title', value: 'Task 1' },
      { row: 'r2', column: 'estMin', value: '10' },
      { row: 'r2', column: 'done', value: true },
      { row: 'r2', column: 'courseId', value: 'c1' },
    ]);
    expect(r.edits[0].edits).toHaveLength(8);
    r.edits.length = 0;
    down(cell(r, 4, 1));
    press(r, 'Backspace');
    expect(r.edits).toEqual([{ edits: [{ row: 'r4', column: 'estMin', value: '' }], label: undefined }]);
  });

  it('changes nothing in a table nobody edits', () => {
    const r = mount(3, 'Mail lists only what Claude recorded.');
    down(cell(r, 1, 0));
    press(r, 'x');
    press(r, 'Backspace');
    clipboard('paste', 'new');
    expect(r.edits).toHaveLength(0);
    expect(r.said[0]).toBe('Mail lists only what Claude recorded.');
  });

  it('shows the total picked for a column', () => {
    const r = mount();
    expect(r.host.querySelector('.db-summary')!.textContent).toContain('Sum 100');
  });
});
