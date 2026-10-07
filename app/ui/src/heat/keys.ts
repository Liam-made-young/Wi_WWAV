// Heat's keyboard (docs/SPEC.md 3.17), as a pure map from a key and where
// focus is to what the key means. The shell's one router hands Heat every
// key it didn't claim (⌘1-⌘4, ⌘K, ⌘⇧N, ⌘Z, Esc, ⇧Return and ⌘I are the
// shell's); nothing here listens to the keyboard.
//
//   1-6        Today, Tasks, Calendar, Grades, Habits, Mail (no field focused)
//   7 8        Database, Wiki
//   N          New item          C  make current       P  plan into the next free gap
//   ↑ ↓        move              F  focus start or pause
//   Return     edit the row under the cursor   ⇧F stop and log   I  pulled away
//   ⌘↩ · ⌫     mark done · delete (with Undo)
//   ⌥↑ ⌥↓      move the selected block 15 minutes; with ⇧, make it 15 shorter or longer
//   ⌘F · ⌥⌘R   filter the list · sync calendars
//
// Return accepts every draft while Plan my day's drafts wait, and Esc (the
// shell's) clears them. The tabs add their own through useTabKeys: M W D T
// ← → in Calendar, T in Mail.

import type { HeatKey, TabId } from './frame';
import { TAB_IDS } from './frame';

export interface HeatKeyContext {
  tab: TabId;
  /**
   * What has the keyboard: a "text" field or a select or slider (which keep
   * their keys), a "control" such as a button or a tab (which keeps Return),
   * or nothing, a list or the page.
   */
  focus: 'text' | 'control' | null;
  /** A task or a block is selected. */
  selected: boolean;
  /** Plan my day's drafts are waiting for Return. */
  drafts: boolean;
}

export type HeatCommand =
  | { type: 'tab'; tab: TabId }
  | { type: 'new' }
  | { type: 'move'; by: 1 | -1 }
  | { type: 'edit' }
  | { type: 'done' }
  | { type: 'delete' }
  | { type: 'current' }
  | { type: 'place' }
  | { type: 'focus' }
  | { type: 'stopFocus' }
  | { type: 'pulledAway' }
  | { type: 'acceptDrafts' }
  | { type: 'nudge'; by: 1 | -1; resize: boolean }
  | { type: 'filter' }
  | { type: 'sync' };

export function heatRoute(e: HeatKey, ctx: HeatKeyContext): HeatCommand | null {
  if (ctx.focus === 'text') return null;
  const { key } = e;

  if (e.command) {
    if (e.alt) return e.code === 'KeyR' && !e.shift ? { type: 'sync' } : null;
    if (e.shift) return null;
    if (key === 'Enter') return ctx.selected ? { type: 'done' } : null;
    return e.code === 'KeyF' ? { type: 'filter' } : null;
  }
  if (e.alt) {
    if (key === 'ArrowUp' || key === 'ArrowDown') {
      return ctx.selected ? { type: 'nudge', by: key === 'ArrowDown' ? 1 : -1, resize: e.shift } : null;
    }
    return null;
  }

  if (key === 'Enter') {
    if (e.shift) return null; // ⇧Return is the shell's: the tab's secondary act
    // The drafts are what Return is for while they wait, whichever button was clicked last.
    if (ctx.drafts) return { type: 'acceptDrafts' };
    return ctx.focus === null ? { type: 'edit' } : null;
  }

  if (key === 'ArrowUp') return e.shift ? null : { type: 'move', by: -1 };
  if (key === 'ArrowDown') return e.shift ? null : { type: 'move', by: 1 };
  if (e.repeat) return null;

  if ((key === 'Backspace' || key === 'Delete') && !e.shift) {
    return ctx.selected && ctx.focus === null ? { type: 'delete' } : null;
  }
  const k = key.length === 1 ? key.toLowerCase() : key;
  if (e.shift) return k === 'f' ? { type: 'stopFocus' } : null;
  if (/^[1-9]$/.test(k) && Number(k) <= TAB_IDS.length) return { type: 'tab', tab: TAB_IDS[Number(k) - 1] };
  switch (k) {
    case 'n':
      return { type: 'new' };
    case 'c':
      return ctx.selected ? { type: 'current' } : null;
    case 'p':
      return ctx.selected ? { type: 'place' } : null;
    case 'f':
      return { type: 'focus' };
    case 'i':
      return { type: 'pulledAway' };
    default:
      return null;
  }
}

/** What has the keyboard, from the focused element. */
export function focusKind(el: Element | null): HeatKeyContext['focus'] {
  if (!el || el === document.body) return null;
  const html = el as HTMLElement;
  if (html.isContentEditable || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement) return 'text';
  if (el instanceof HTMLInputElement) {
    if (el.type === 'range') return 'text';
    return ['checkbox', 'radio', 'button', 'submit'].includes(el.type) ? 'control' : 'text';
  }
  if (el.matches('button, a[href], summary, [role="tab"], [role="button"], [role="menuitem"], [role="switch"]')) {
    return 'control';
  }
  return null;
}
