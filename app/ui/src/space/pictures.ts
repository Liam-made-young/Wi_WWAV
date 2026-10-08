// The picture of a page on its body (docs/SPACE.md 11): what you see of a
// link when its page isn't live, because it is too far, half out of view,
// or not the one you are looking at. A picture is taken of the live page by
// the app (`space.page.picture`) and kept here by link, so a place you have
// been looks like itself the next time you pass. Kept in the window's own
// storage on this Mac, the newest few; for many people it is the core's to
// keep (docs/SPACE.md 12).

const KEY = 'wi.space.pictures';
/** How many pages are remembered by sight. */
const MOST = 24;

export type Pictures = Map<string, string>;

export function loadPictures(): Pictures {
  try {
    const kept = JSON.parse(localStorage.getItem(KEY) ?? '[]') as unknown;
    if (!Array.isArray(kept)) return new Map();
    return new Map(kept.filter((e): e is [string, string] => Array.isArray(e) && typeof e[0] === 'string' && typeof e[1] === 'string'));
  } catch {
    return new Map();
  }
}

/** Keeps a link's newest picture, and lets the oldest go when there are too many or the storage is full. */
export function keepPicture(pictures: Pictures, url: string, data: string): void {
  if (!data.startsWith('data:image/')) return;
  pictures.delete(url);
  pictures.set(url, data);
  while (pictures.size > MOST) pictures.delete(pictures.keys().next().value as string);
  for (;;) {
    try {
      localStorage.setItem(KEY, JSON.stringify([...pictures]));
      return;
    } catch {
      // Full: the oldest picture goes, and it is tried again.
      const oldest = pictures.keys().next().value;
      if (oldest === undefined || oldest === url) return;
      pictures.delete(oldest);
    }
  }
}
