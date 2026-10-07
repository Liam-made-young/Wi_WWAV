import { describe, expect, it } from 'vitest';
import { type KeyContext, type KeyLike, route, SHORTCUTS } from './keys';

// docs/SPEC.md 2.7's one grammar. What a fail looks like: a shortcut that
// does nothing or the wrong thing; Space playing while you type; Esc
// typed into a field instead of closing; ⌘Z eating a field's own typing
// undo; a key reaching a stem that isn't focused; ⇧Return doing anything
// but the screen's one secondary act.

const key = (k: string, mods: Partial<KeyLike> = {}): KeyLike => ({
  key: k,
  code: mods.code ?? '',
  metaKey: false,
  ctrlKey: false,
  shiftKey: false,
  altKey: false,
  repeat: false,
  ...mods,
});
const cmd = (k: string, mods: Partial<KeyLike> = {}) => key(k, { metaKey: true, ...mods });
const mac: KeyContext = { mac: true, field: null, stem: null, overlay: null };

describe('the keyboard router', () => {
  it('switches views on ⌘1–⌘3, from anywhere, even a text field, and there is no ⌘4', () => {
    expect(route(cmd('1', { code: 'Digit1' }), mac)).toEqual({ type: 'room', room: 'heat' });
    expect(route(cmd('2', { code: 'Digit2' }), mac)).toEqual({ type: 'room', room: 'space' });
    expect(route(cmd('3', { code: 'Digit3' }), { ...mac, field: 'text' })).toEqual({
      type: 'room',
      room: 'console',
    });
    // The fourth room, a shop to walk through, is cut: ⌘4 is nothing.
    expect(route(cmd('4', { code: 'Digit4' }), mac)).toBeNull();
    expect(route(cmd('5', { code: 'Digit5' }), mac)).toBeNull();
    // Plain digits are Heat's tabs (3.16), not views.
    expect(route(key('1', { code: 'Digit1' }), mac)).toBeNull();
  });

  it('takes ⌘ as Command on a Mac and Ctrl elsewhere', () => {
    const linux = { ...mac, mac: false };
    expect(route(key('k', { ctrlKey: true, code: 'KeyK' }), linux)).toEqual({ type: 'palette' });
    expect(route(key('k', { metaKey: true, code: 'KeyK' }), linux)).toBeNull();
    expect(route(key('k', { ctrlKey: true, code: 'KeyK' }), mac)).toBeNull();
  });

  it('opens the palette, capture and the drawer everywhere', () => {
    expect(route(cmd('k', { code: 'KeyK' }), { ...mac, field: 'text' })).toEqual({ type: 'palette' });
    expect(route(cmd('N', { code: 'KeyN', shiftKey: true }), mac)).toEqual({ type: 'capture' });
    expect(route(cmd('l', { code: 'KeyL' }), { ...mac, overlay: 'palette' })).toEqual({ type: 'drawer' });
  });

  it('undoes and redoes, labelled, but leaves a field its own typing undo', () => {
    expect(route(cmd('z', { code: 'KeyZ' }), mac)).toEqual({ type: 'undo' });
    expect(route(cmd('Z', { code: 'KeyZ', shiftKey: true }), mac)).toEqual({ type: 'redo' });
    expect(route(cmd('z', { code: 'KeyZ' }), { ...mac, field: 'text' })).toBeNull();
  });

  it('plays and pauses on Space outside text, and leaves Space to fields and controls', () => {
    expect(route(key(' ', { code: 'Space' }), mac)).toEqual({ type: 'playPause' });
    expect(route(key(' ', { code: 'Space' }), { ...mac, overlay: 'drawer' })).toEqual({ type: 'playPause' });
    expect(route(key(' ', { code: 'Space' }), { ...mac, field: 'text' })).toBeNull();
    expect(route(key(' ', { code: 'Space' }), { ...mac, field: 'control' })).toBeNull();
    expect(route(key(' ', { code: 'Space', repeat: true }), mac)).toBeNull();
  });

  it('closes with Esc from anywhere, a field included', () => {
    expect(route(key('Escape'), mac)).toEqual({ type: 'escape' });
    expect(route(key('Escape'), { ...mac, field: 'text', overlay: 'capture' })).toEqual({ type: 'escape' });
  });

  it("sends ⇧Return to the screen's one secondary act, never from inside a field", () => {
    expect(route(key('Enter', { shiftKey: true }), mac)).toEqual({ type: 'secondary' });
    expect(route(key('Enter', { shiftKey: true }), { ...mac, overlay: 'drawer' })).toEqual({ type: 'secondary' });
    expect(route(key('Enter', { shiftKey: true }), { ...mac, field: 'text', overlay: 'capture' })).toBeNull();
  });

  it('opens in the Console and exports everything', () => {
    expect(route(cmd('e', { code: 'KeyE' }), mac)).toEqual({ type: 'openInConsole' });
    expect(route(cmd('E', { code: 'KeyE', shiftKey: true }), mac)).toEqual({ type: 'exportEverything' });
  });

  it('moves and opens in the palette, with ⌘Return opening in the other view', () => {
    const palette = { ...mac, field: 'text' as const, overlay: 'palette' as const };
    expect(route(key('ArrowDown'), palette)).toEqual({ type: 'move', by: 1 });
    expect(route(key('ArrowUp'), palette)).toEqual({ type: 'move', by: -1 });
    expect(route(key('Enter'), palette)).toEqual({ type: 'open', other: false });
    expect(route(cmd('Enter'), palette)).toEqual({ type: 'open', other: true });
  });

  it('saves a capture on Enter and leaves ⇧Enter as a new line', () => {
    const capture = { ...mac, field: 'text' as const, overlay: 'capture' as const };
    expect(route(key('Enter'), capture)).toEqual({ type: 'save' });
    expect(route(key('Enter', { shiftKey: true }), capture)).toBeNull();
  });

  it('moves, opens, gets info and deletes in the drawer, but not while typing in its search', () => {
    const drawer = { ...mac, overlay: 'drawer' as const };
    expect(route(key('ArrowDown'), drawer)).toEqual({ type: 'move', by: 1 });
    expect(route(key('Enter'), drawer)).toEqual({ type: 'open', other: false });
    expect(route(key('Backspace'), drawer)).toEqual({ type: 'delete' });
    expect(route(cmd('i', { code: 'KeyI' }), drawer)).toEqual({ type: 'getInfo' });
    expect(route(key('Backspace'), { ...drawer, field: 'text' })).toBeNull();
    expect(route(key('ArrowDown'), { ...drawer, field: 'text' })).toBeNull();
  });

  it('sends M, S, arrows and Return to a focused stem light only', () => {
    const lit = { ...mac, stem: 'drums' as const };
    expect(route(key('m', { code: 'KeyM' }), lit)).toEqual({ type: 'stemKey', stem: 'drums', key: 'm', shift: false });
    expect(route(key('S', { code: 'KeyS', shiftKey: true }), lit)).toEqual({
      type: 'stemKey',
      stem: 'drums',
      key: 'S',
      shift: true,
    });
    expect(route(key('ArrowUp'), lit)).toEqual({ type: 'stemKey', stem: 'drums', key: 'ArrowUp', shift: false });
    expect(route(key('Enter'), lit)).toEqual({ type: 'stemKey', stem: 'drums', key: 'Enter', shift: false });
    expect(route(key('m', { code: 'KeyM' }), mac)).toBeNull();
    // ⌘ held: the app's shortcut, never the stem's (gesture.ts).
    expect(route(cmd('s', { code: 'KeyS' }), lit)).toBeNull();
  });

  it('steps the text size with ⌘+ and ⌘−', () => {
    expect(route(cmd('=', { code: 'Equal' }), mac)).toEqual({ type: 'textSize', by: 1 });
    expect(route(cmd('+', { code: 'Equal', shiftKey: true }), mac)).toEqual({ type: 'textSize', by: 1 });
    expect(route(cmd('-', { code: 'Minus' }), mac)).toEqual({ type: 'textSize', by: -1 });
  });

  it('lists every shortcut once, for the Keyboard pane', () => {
    const keys = SHORTCUTS.map((s) => s.keys);
    expect(new Set(keys).size).toBe(keys.length);
    expect(SHORTCUTS.map((s) => s.does)).toContain('Undo, labelled');
  });
});
