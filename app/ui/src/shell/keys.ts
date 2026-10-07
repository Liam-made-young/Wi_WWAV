// The one keyboard router (docs/SPEC.md 2.7). The shell listens to the
// keyboard once, and every key goes through here: no component listens to
// raw keys itself, so one key never does two things in two places. The
// router only says what a key means where focus is; the shell does it.
//
// ⌘ is Command on a Mac and Ctrl elsewhere, as in the stem gesture. A text
// field keeps its typing, its own ⌘Z and Space; Esc and the ⌘ shortcuts
// reach the shell from anywhere, because Esc never discards (a sheet keeps
// its draft) and ⌘K is wanted most while typing.

import type { Stem } from '../shared/stems/stems';
import type { RoomId } from './rooms';
import { ROOMS } from './rooms';

export interface KeyLike {
  key: string;
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  repeat: boolean;
}

/** What has the keyboard: the topmost thing open over the views. */
export type Overlay = 'palette' | 'capture' | 'drawer' | 'info' | 'player' | 'settings' | 'export' | 'menu' | 'first';

export interface KeyContext {
  mac: boolean;
  /** Focus in a text field ("text") or a checkbox, select or slider ("control"). */
  field: 'text' | 'control' | null;
  /** A focused stem light. */
  stem: Stem | null;
  overlay: Overlay | null;
}

export type Command =
  | { type: 'room'; room: RoomId }
  | { type: 'playPause' }
  | { type: 'palette' }
  | { type: 'capture' }
  | { type: 'drawer' }
  | { type: 'undo' }
  | { type: 'redo' }
  | { type: 'escape' }
  | { type: 'secondary' }
  | { type: 'openInConsole' }
  | { type: 'exportEverything' }
  | { type: 'textSize'; by: 1 | -1 }
  | { type: 'getInfo' }
  | { type: 'move'; by: 1 | -1 }
  | { type: 'open'; other: boolean }
  | { type: 'save' }
  | { type: 'delete' }
  | { type: 'stemKey'; stem: Stem; key: string; shift: boolean };

const STEM_KEYS = new Set(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'm', 'M', 's', 'S', 'Enter']);

export function route(e: KeyLike, ctx: KeyContext): Command | null {
  const command = ctx.mac ? e.metaKey : e.ctrlKey;
  if (e.key === 'Escape') return { type: 'escape' };
  if (command) return commandKey(e, ctx);
  if (e.altKey || (ctx.mac && e.ctrlKey) || (!ctx.mac && e.metaKey)) return null;

  const { overlay, field } = ctx;
  const enter = e.key === 'Enter';
  if (overlay === 'palette') {
    if (e.key === 'ArrowDown') return { type: 'move', by: 1 };
    if (e.key === 'ArrowUp') return { type: 'move', by: -1 };
    if (enter && !e.shiftKey) return { type: 'open', other: false };
  }
  if (overlay === 'capture' && enter) return e.shiftKey ? null : { type: 'save' };
  if (field !== null) return null;

  if (ctx.stem && STEM_KEYS.has(e.key) && !(enter && e.shiftKey)) {
    return { type: 'stemKey', stem: ctx.stem, key: e.key, shift: e.shiftKey };
  }
  if (enter && e.shiftKey) return { type: 'secondary' };
  if (overlay === 'drawer') {
    if (e.key === 'ArrowDown') return { type: 'move', by: 1 };
    if (e.key === 'ArrowUp') return { type: 'move', by: -1 };
    if (enter) return { type: 'open', other: false };
    if (e.key === 'Backspace' || e.key === 'Delete') return { type: 'delete' };
  }
  if (e.key === ' ' && !e.shiftKey && !e.repeat) return { type: 'playPause' };
  return null;
}

function commandKey(e: KeyLike, ctx: KeyContext): Command | null {
  if (e.altKey) return null;
  const digit = /^Digit([1-3])$/.exec(e.code);
  if (digit && !e.shiftKey) return { type: 'room', room: ROOMS[Number(digit[1]) - 1] };
  if (e.key === 'Enter') return ctx.overlay === 'palette' ? { type: 'open', other: true } : null;
  if (e.repeat) return null;
  switch (e.code) {
    case 'KeyK':
      return e.shiftKey ? null : { type: 'palette' };
    case 'KeyN':
      return e.shiftKey ? { type: 'capture' } : null;
    case 'KeyL':
      return e.shiftKey ? null : { type: 'drawer' };
    case 'KeyZ':
      // A field keeps its own typing undo; the journal's undo is for
      // committed work.
      if (ctx.field === 'text') return null;
      return e.shiftKey ? { type: 'redo' } : { type: 'undo' };
    case 'KeyE':
      return e.shiftKey ? { type: 'exportEverything' } : { type: 'openInConsole' };
    case 'KeyI':
      return e.shiftKey ? null : { type: 'getInfo' };
    case 'Equal':
      return { type: 'textSize', by: 1 };
    case 'Minus':
      return e.shiftKey ? null : { type: 'textSize', by: -1 };
    default:
      return null;
  }
}

/** Every shortcut the shell answers, for Settings → Keyboard (2.13). */
export const SHORTCUTS: readonly { keys: string; does: string; where: string }[] = [
  { keys: '⌘1 · ⌘2 · ⌘3', does: 'Heat · Space · Console', where: 'everywhere' },
  { keys: 'Space', does: 'Play or pause', where: 'outside text' },
  { keys: '⌘K', does: 'Search everything', where: 'everywhere' },
  { keys: '⌘⇧N', does: 'Quick capture', where: 'everywhere' },
  { keys: '⌘L', does: 'Library drawer', where: 'everywhere' },
  { keys: '⌘Z', does: 'Undo, labelled', where: 'the view you are in' },
  { keys: '⌘⇧Z', does: 'Redo, labelled', where: 'the view you are in' },
  { keys: 'Esc', does: 'Close, collapse, deselect; never deletes', where: 'everywhere' },
  { keys: '⇧Return', does: 'The screen’s one secondary act, named in the status bar', where: 'everywhere' },
  { keys: '⌘Return', does: 'Open a result in its other view', where: 'search' },
  { keys: '↑ ↓', does: 'Move the selection', where: 'lists' },
  { keys: '← → ↑ ↓ · M · S', does: 'Level ±5% · mute · solo the focused stem', where: 'stem lights' },
  { keys: '⌘I', does: 'Get Info', where: 'the library' },
  { keys: '⌫', does: 'Delete the selection; ⌘Z brings it back', where: 'the library' },
  { keys: '⌘E', does: 'Open in the Console', where: 'a selected work' },
  { keys: '⌘⇧E', does: 'Export everything…', where: 'everywhere' },
  { keys: '⌘+ · ⌘−', does: 'Text size', where: 'everywhere' },
];
