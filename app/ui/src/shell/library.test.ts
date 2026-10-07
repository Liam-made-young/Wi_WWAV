import { describe, expect, it } from 'vitest';
import { addToSelection, clickSelect, moveCursor, noSelection, tagName } from './library';

// 2.5 and 2.7's library screen. What a fail looks like: a tag shown with a
// capital; a ⌘-click that drops the rest of the selection; a ⇧-click (the
// screen's secondary act) that doesn't add its row; ⇧Return doing anything but adding the row
// under the cursor; arrows that drop a selection or run off either end.

const order = ['a', 'b', 'c', 'd', 'e'];

describe('tags', () => {
  it('are lowercase, trimmed, with spaces as hyphens', () => {
    expect(tagName('  Live Drums ')).toBe('live-drums');
    expect(tagName('LoFi')).toBe('lofi');
    expect(tagName('   ')).toBe('');
  });
});

describe('selecting clips', () => {
  it('selects one on a click, toggles on ⌘-click, and adds on ⇧-click', () => {
    let s = clickSelect(noSelection(), 'b', 'only', order);
    expect([...s.ids]).toEqual(['b']);
    s = clickSelect(s, 'd', 'toggle', order);
    expect([...s.ids].sort()).toEqual(['b', 'd']);
    s = clickSelect(s, 'b', 'toggle', order);
    expect([...s.ids]).toEqual(['d']);
    s = clickSelect(s, 'a', 'add', order);
    expect([...s.ids].sort()).toEqual(['a', 'd']);
    expect(s.cursor).toBe('a');
  });

  it('moves the cursor with the arrows, leaving the selection alone, and stops at the ends', () => {
    let s = moveCursor(noSelection(), 1, order);
    expect(s.cursor).toBe('a');
    expect(s.ids.size).toBe(0);
    s = moveCursor(moveCursor(clickSelect(noSelection(), 'b', 'only', order), 1, order), 1, order);
    expect(s.cursor).toBe('d');
    expect([...s.ids]).toEqual(['b']);
    expect(moveCursor(clickSelect(noSelection(), 'e', 'only', order), 1, order).cursor).toBe('e');
    expect(moveCursor(clickSelect(noSelection(), 'a', 'only', order), -1, order).cursor).toBe('a');
  });

  it('adds the row under the cursor on ⇧Return, keeping the rest', () => {
    let s = clickSelect(noSelection(), 'a', 'only', order);
    s = moveCursor(moveCursor(s, 1, order), 1, order);
    s = addToSelection(s);
    expect([...s.ids].sort()).toEqual(['a', 'c']);
    expect(addToSelection(noSelection()).ids.size).toBe(0);
  });
});
