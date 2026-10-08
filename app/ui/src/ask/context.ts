// What is on screen, for the prompt box to send with a request, so "move
// these to Friday" and "what's due in this class" mean something. Each part
// of the window says what it is showing; the box reads all of it when
// Return is pressed.

const parts = new Map<string, unknown>();

/**
 * Says what one part of the window shows now; null when it shows nothing. A
 * function is asked when the box sends, for what changes too often to keep
 * saying (a grid's selection).
 */
export function setScreen(part: string, value: unknown | (() => unknown) | null): void {
  if (value === null || value === undefined) parts.delete(part);
  else parts.set(part, value);
}

/** Everything on screen, as one object, or null when nothing has said anything. */
export function screen(): Record<string, unknown> | null {
  if (parts.size === 0) return null;
  const out: Record<string, unknown> = {};
  for (const [k, said] of parts) {
    const v = typeof said === 'function' ? (said as () => unknown)() : said;
    if (v === null || v === undefined) continue;
    // A part may hold a whole object to spread at the top (the tab and the selection).
    if (k === 'learn' && v && typeof v === 'object') Object.assign(out, v);
    else out[k] = v;
  }
  return out;
}
