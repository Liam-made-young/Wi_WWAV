// Where a link in the prompt box, the Database tab or the Wiki tab goes.
// The part of the window that shows a thing listens here for it: the Wiki
// tab for an article, the Database tab for a row or a table, the shell for a
// task or a web address. Whoever asks doesn't need to know who answers.

export type Target =
  /** A Wikipedia article, in the Wiki tab. `background` keeps what is showing in front. */
  | { what: 'wiki'; title: string; frag?: string; background?: boolean }
  /** A row of a Database table. */
  | { what: 'row'; table: string; id: string }
  | { what: 'table'; table: string; view?: string }
  /** One of Learn's tabs. */
  | { what: 'tab'; tab: string }
  /** A web address: the system browser, never a window of the app. */
  | { what: 'web'; url: string };

type Listener = (target: Target) => boolean | void;

const listeners = new Set<Listener>();

/** Hears every target until the returned function is called. Answer true when it was shown. */
export function onNavigate(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Shows `target`: every listener hears it, and the one that shows such things does. */
export function navigate(target: Target): void {
  for (const l of [...listeners]) l(target);
}

/** A `learn://table/id` or `wiki://Title` address as a target, or null for any other. */
export function targetOf(href: string): Target | null {
  const learn = /^learn:\/\/([^/]+)\/(.+)$/.exec(href);
  if (learn) return { what: 'row', table: decode(learn[1]), id: decode(learn[2]) };
  const wiki = /^wiki:\/\/(.+)$/.exec(href);
  if (wiki) {
    const [title, frag] = decode(wiki[1]).split('#');
    const clean = title.replace(/_/g, ' ').trim();
    return clean ? { what: 'wiki', title: clean, frag: frag || undefined } : null;
  }
  if (/^https?:\/\/\S+$/.test(href)) return { what: 'web', url: href };
  return null;
}

function decode(s: string): string {
  try {
    return decodeURIComponent(s);
  } catch {
    return s;
  }
}
