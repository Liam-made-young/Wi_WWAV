// The library drawer's rules on the UI side (docs/SPEC.md 2.5, 2.7): how
// a tag is written, and how clips are selected. The core keeps the rest:
// it lowercases tags too, refuses a 13th and a fifth pin, and deletes rows,
// never files.

/** A tag as the library writes it: lowercase, with spaces as hyphens ("live-drums"). */
export function tagName(typed: string): string {
  return typed.trim().toLowerCase().replace(/\s+/g, '-');
}

// The cursor is the keyboard's row (↑ ↓ move it, Return opens it, Space
// plays it); the selection is what bulk edits act on. A click selects one
// row; ⇧-click and ⇧Return, the library's one secondary act, add a row.
export interface Selection {
  ids: Set<string>;
  cursor: string | null;
}

export function noSelection(): Selection {
  return { ids: new Set(), cursor: null };
}

/** A click selects one row, ⌘-click toggles one, ⇧-click adds one. */
export function clickSelect(s: Selection, id: string, mode: 'only' | 'toggle' | 'add', order: string[]): Selection {
  if (!order.includes(id)) return s;
  if (mode === 'only') return { ids: new Set([id]), cursor: id };
  const ids = new Set(s.ids);
  if (mode === 'toggle' && ids.has(id)) ids.delete(id);
  else ids.add(id);
  return { ids, cursor: id };
}

/** ↑ ↓ move the cursor and leave the selection as it is; the ends hold. */
export function moveCursor(s: Selection, by: 1 | -1, order: string[]): Selection {
  if (order.length === 0) return s;
  const at = s.cursor === null ? -1 : order.indexOf(s.cursor);
  const next = at < 0 ? (by > 0 ? 0 : order.length - 1) : Math.min(order.length - 1, Math.max(0, at + by));
  return { ...s, cursor: order[next] };
}

/** ⇧Return: add the row under the cursor. */
export function addToSelection(s: Selection): Selection {
  if (s.cursor === null) return s;
  return { ...s, ids: new Set([...s.ids, s.cursor]) };
}
