// What differs between a Mac and anything else, for the keyboard: ⌘ is
// Command on a Mac and Ctrl elsewhere (2.7), and shortcuts are written to
// match.

export const IS_MAC =
  typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** A shortcut as this machine's keyboard prints it: "⌘K" on a Mac, "Ctrl+K" elsewhere. */
export function keys(mac: string): string {
  return IS_MAC ? mac : mac.replace(/⌘/g, 'Ctrl+').replace(/⇧/g, 'Shift+');
}
