import { describe, expect, it } from 'vitest';
import type { HeatKey } from './frame';
import { focusKind, type HeatKeyContext, heatRoute } from './keys';

// docs/SPEC.md 3.17. What a fail looks like: a key that does nothing or the
// wrong thing; a letter typed into a field taken as a command; Return
// accepting drafts only when no button has the keyboard; ⌫ deleting while a
// control has it; a key held down repeating a command.

const key = (k: string, mods: Partial<HeatKey> = {}): HeatKey => ({
  key: k,
  code: k.length === 1 ? `Key${k.toUpperCase()}` : k,
  shift: false,
  command: false,
  alt: false,
  repeat: false,
  ...mods,
});
const ctx = (over: Partial<HeatKeyContext> = {}): HeatKeyContext => ({
  tab: 'today',
  focus: null,
  selected: true,
  drafts: false,
  ...over,
});

describe('Heat’s keyboard', () => {
  it('opens the tabs on 1–6', () => {
    expect([1, 2, 3, 4, 5, 6].map((n) => heatRoute(key(String(n)), ctx()))).toEqual([
      { type: 'tab', tab: 'today' },
      { type: 'tab', tab: 'tasks' },
      { type: 'tab', tab: 'calendar' },
      { type: 'tab', tab: 'grades' },
      { type: 'tab', tab: 'habits' },
      { type: 'tab', tab: 'mail' },
    ]);
    expect(heatRoute(key('7'), ctx())).toBeNull();
  });

  it('takes N, C, P, F, ⇧F and I, but C and P only with something selected', () => {
    expect(heatRoute(key('n'), ctx({ selected: false }))).toEqual({ type: 'new' });
    expect(heatRoute(key('N'), ctx())).toEqual({ type: 'new' });
    expect(heatRoute(key('c'), ctx())).toEqual({ type: 'current' });
    expect(heatRoute(key('c'), ctx({ selected: false }))).toBeNull();
    expect(heatRoute(key('p'), ctx())).toEqual({ type: 'place' });
    expect(heatRoute(key('p'), ctx({ selected: false }))).toBeNull();
    expect(heatRoute(key('f'), ctx({ selected: false }))).toEqual({ type: 'focus' });
    expect(heatRoute(key('F', { shift: true }), ctx())).toEqual({ type: 'stopFocus' });
    expect(heatRoute(key('i'), ctx())).toEqual({ type: 'pulledAway' });
  });

  it('moves on ↑ ↓ and edits on Return, from a list, not from a button', () => {
    expect(heatRoute(key('ArrowUp'), ctx())).toEqual({ type: 'move', by: -1 });
    expect(heatRoute(key('ArrowDown'), ctx({ focus: 'control' }))).toEqual({ type: 'move', by: 1 });
    expect(heatRoute(key('Enter'), ctx())).toEqual({ type: 'edit' });
    expect(heatRoute(key('Enter'), ctx({ focus: 'control' }))).toBeNull();
    // ⇧Return is the shell's: the tab's secondary act.
    expect(heatRoute(key('Enter', { shift: true }), ctx())).toBeNull();
  });

  it('accepts all the drafts on Return, whichever button last had the keyboard', () => {
    expect(heatRoute(key('Enter'), ctx({ drafts: true }))).toEqual({ type: 'acceptDrafts' });
    expect(heatRoute(key('Enter'), ctx({ drafts: true, focus: 'control' }))).toEqual({ type: 'acceptDrafts' });
    expect(heatRoute(key('Enter'), ctx({ drafts: true, focus: 'text' }))).toBeNull();
  });

  it('marks done on ⌘↩, and deletes on ⌫ only when nothing but the list has the keyboard', () => {
    expect(heatRoute(key('Enter', { command: true }), ctx())).toEqual({ type: 'done' });
    expect(heatRoute(key('Enter', { command: true }), ctx({ selected: false }))).toBeNull();
    expect(heatRoute(key('Enter', { command: true }), ctx({ focus: 'control' }))).toEqual({ type: 'done' });
    expect(heatRoute(key('Backspace'), ctx())).toEqual({ type: 'delete' });
    expect(heatRoute(key('Delete'), ctx())).toEqual({ type: 'delete' });
    expect(heatRoute(key('Backspace'), ctx({ focus: 'control' }))).toBeNull();
    expect(heatRoute(key('Backspace'), ctx({ selected: false }))).toBeNull();
  });

  it('filters on ⌘F and syncs on ⌥⌘R', () => {
    expect(heatRoute(key('f', { command: true }), ctx())).toEqual({ type: 'filter' });
    expect(heatRoute(key('®', { command: true, alt: true, code: 'KeyR' }), ctx())).toEqual({ type: 'sync' });
    expect(heatRoute(key('r', { command: true }), ctx())).toBeNull();
  });

  it('nudges a selected block on ⌥↑ ⌥↓, and resizes it with ⇧', () => {
    expect(heatRoute(key('ArrowDown', { alt: true }), ctx())).toEqual({ type: 'nudge', by: 1, resize: false });
    expect(heatRoute(key('ArrowUp', { alt: true }), ctx())).toEqual({ type: 'nudge', by: -1, resize: false });
    expect(heatRoute(key('ArrowDown', { alt: true, shift: true }), ctx())).toEqual({
      type: 'nudge',
      by: 1,
      resize: true,
    });
    expect(heatRoute(key('ArrowDown', { alt: true }), ctx({ selected: false }))).toBeNull();
  });

  it('leaves every key to a field that has the keyboard', () => {
    for (const k of ['n', 'c', 'p', 'f', 'i', '1', 'Enter', 'Backspace', 'ArrowDown']) {
      expect(heatRoute(key(k), ctx({ focus: 'text', drafts: true }))).toBeNull();
    }
    expect(heatRoute(key('Enter', { command: true }), ctx({ focus: 'text' }))).toBeNull();
  });

  it('doesn’t repeat a command while a key is held, but ↑ ↓ keep moving', () => {
    expect(heatRoute(key('f', { repeat: true }), ctx())).toBeNull();
    expect(heatRoute(key('Backspace', { repeat: true }), ctx())).toBeNull();
    expect(heatRoute(key('ArrowDown', { repeat: true }), ctx())).toEqual({ type: 'move', by: 1 });
  });

  it('leaves the keys of the shell and the tabs alone', () => {
    expect(heatRoute(key('k', { command: true }), ctx())).toBeNull();
    expect(heatRoute(key('z', { command: true }), ctx())).toBeNull();
    expect(heatRoute(key('m'), ctx({ tab: 'calendar' }))).toBeNull();
    expect(heatRoute(key('t'), ctx({ tab: 'mail' }))).toBeNull();
    expect(heatRoute(key('ArrowLeft'), ctx({ tab: 'calendar' }))).toBeNull();
    expect(heatRoute(key('Escape'), ctx())).toBeNull();
  });
});

describe('what has the keyboard', () => {
  const html = (markup: string) => {
    const host = document.createElement('div');
    host.innerHTML = markup;
    document.body.append(host);
    return host.firstElementChild as HTMLElement;
  };

  it('tells a field from a control from a list', () => {
    expect(focusKind(null)).toBeNull();
    expect(focusKind(html('<input>'))).toBe('text');
    expect(focusKind(html('<textarea></textarea>'))).toBe('text');
    expect(focusKind(html('<select><option>a</option></select>'))).toBe('text');
    expect(focusKind(html('<input type="range">'))).toBe('text');
    expect(focusKind(html('<input type="checkbox">'))).toBe('control');
    expect(focusKind(html('<button>Go</button>'))).toBe('control');
    expect(focusKind(html('<div role="tab" tabindex="0">Tasks</div>'))).toBe('control');
    expect(focusKind(html('<div role="listbox" tabindex="0"></div>'))).toBeNull();
    expect(focusKind(document.body)).toBeNull();
  });
});
