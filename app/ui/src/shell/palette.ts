// ⌘K's query, matching and ordering (docs/SPEC.md 2.7). Local results are
// matched here at once; the core's search answers after 200 ms, as the
// PKM's palette does, but only the newest answer is ever shown.

export interface Query {
  /** What is left after the filters: the words to match. */
  words: string;
  tags: string[];
  /** "A minor": a key as .wwav writes it. */
  key: string | null;
  bpm: [number, number] | null;
  hot: boolean;
  remix: boolean;
  /** @name: a person. */
  person: string | null;
}

/** Reads 2.7's filters out of a query: tag:, key:, bpm:, is:hot, is:remix and @name. */
export function parseQuery(q: string): Query {
  const out: Query = { words: '', tags: [], key: null, bpm: null, hot: false, remix: false, person: null };
  const words: string[] = [];
  for (const w of q.trim().split(/\s+/).filter(Boolean)) {
    const [field, value] = splitOnce(w, ':');
    const range = field === 'bpm' && value ? /^(\d+(?:\.\d+)?)(?:-(\d+(?:\.\d+)?))?$/.exec(value) : null;
    if (field === 'tag' && value) out.tags.push(value.toLowerCase());
    else if (field === 'key' && value) out.key = value.replace(/-/g, ' ');
    else if (range) {
      const lo = Number(range[1]);
      const hi = range[2] === undefined ? lo : Number(range[2]);
      out.bpm = [Math.min(lo, hi), Math.max(lo, hi)];
    } else if (w === 'is:hot') out.hot = true;
    else if (w === 'is:remix') out.remix = true;
    else if (w.startsWith('@') && w.length > 1) out.person = w.slice(1);
    else words.push(w);
  }
  out.words = words.join(' ');
  return out;
}

function splitOnce(s: string, sep: string): [string, string | null] {
  const i = s.indexOf(sep);
  return i < 0 ? [s, null] : [s.slice(0, i).toLowerCase(), s.slice(i + 1)];
}

/**
 * How well `query` matches `text`, higher is better, or null when its
 * letters don't appear in order. A match at the start beats one at a
 * word's start, which beats letters scattered through the text.
 */
export function fuzzy(query: string, text: string): number | null {
  const q = query.trim().toLowerCase();
  const t = text.toLowerCase();
  if (!q) return 0;
  if (t.startsWith(q)) return 3000 - t.length;
  const word = t.search(new RegExp(`[^a-z0-9]${escape(q)}`));
  if (word >= 0) return 2000 - word;
  let at = -1;
  let first = -1;
  for (const c of q) {
    at = t.indexOf(c, at + 1);
    if (at < 0) return null;
    if (first < 0) first = at;
  }
  return 1000 - (at - first);
}

function escape(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** The items that match, best first; ties keep their order. */
export function rank<T>(query: string, items: readonly T[], text: (item: T) => string): T[] {
  return items
    .map((item, i) => ({ item, i, score: fuzzy(query, text(item)) }))
    .filter((x) => x.score !== null)
    .sort((a, b) => b.score! - a.score! || a.i - b.i)
    .map((x) => x.item);
}

/**
 * Asks `run` once the keys have rested `delay` ms, and delivers only the
 * answer to the newest question: an older one that comes back later is
 * dropped, and so is any answer after cancel().
 */
export function latestOnly<T>(run: (q: string) => Promise<T>, deliver: (q: string, answer: T) => void, delay = 200) {
  let newest = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  return {
    ask(q: string) {
      const mine = ++newest;
      clearTimeout(timer);
      timer = setTimeout(() => {
        run(q).then(
          (answer) => {
            if (mine === newest) deliver(q, answer);
          },
          () => {},
        );
      }, delay);
    },
    cancel() {
      newest++;
      clearTimeout(timer);
    },
  };
}
