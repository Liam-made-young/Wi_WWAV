// The Wiki tab's calls to the core (docs/ASK.md): `wiki.*`. The web view
// reaches nothing but the app, so the core does the asking and keeps what
// was read; this file is the shapes.

import { call } from '../../bridge';

/** Running text: plain words, and marks around some of them. */
export type Inline =
  | string
  | { t: 'b' | 'i' | 'sup' | 'sub' | 'code'; c: Inline[] }
  /** Another article. */
  | { t: 'a'; title: string; frag?: string; c: Inline[] }
  /** A place in this article. */
  | { t: 'j'; frag: string; c: Inline[] }
  /** The web. */
  | { t: 'x'; href: string; c: Inline[] }
  | { t: 'math'; s: string }
  | { t: 'ref'; n: string; id: string }
  | { t: 'br' };

export interface Item {
  c: Inline[];
  sub: Block[];
  term?: boolean;
}

export type Block =
  | { t: 'h'; level: number; id: string; text: string }
  | { t: 'p' | 'note'; c: Inline[] }
  | { t: 'ul' | 'ol' | 'dl'; items: Item[] }
  | { t: 'quote'; c: Block[] }
  | { t: 'pre'; text: string }
  | { t: 'math'; text: string }
  | { t: 'table'; caption?: Inline[]; box?: boolean; rows: { h?: boolean; span?: number; c: Inline[] }[][] };

export interface Article {
  title: string;
  description: string | null;
  revision: string;
  modified: string | null;
  url: string;
  sections: { id: string; title: string; level: number; number: string }[];
  blocks: Block[];
  refs: { id: string; n: string; c: Inline[] }[];
}

export interface Loaded {
  article: Article;
  fetchedAt: number;
  cached: boolean;
  /** Wikipedia couldn't be reached: this is the copy kept on this Mac. */
  offline?: boolean;
}

export interface Summary {
  title: string;
  description: string;
  extract: string;
  disambiguation: boolean;
}

export interface Suggestion {
  title: string;
  description: string;
  saved?: boolean;
}

const summaries = new Map<string, Promise<Summary>>();

export const wiki = {
  suggest: (q: string) => call<{ results: Suggestion[]; offline?: boolean }>('wiki.suggest', { q }),
  search: (q: string) => call<{ results: { title: string; snippet?: string; description?: string }[]; offline?: boolean }>('wiki.search', { q, limit: 12 }),
  article: (title: string, refresh = false) => call<Loaded>('wiki.article', { title, refresh }),
  /** A link's preview. Asked for once per title while the app is open. */
  summary(title: string): Promise<Summary> {
    let have = summaries.get(title);
    if (!have) {
      have = call<Summary>('wiki.summary', { title });
      summaries.set(title, have);
      have.catch(() => summaries.delete(title));
    }
    return have;
  },
  related: (title: string) => call<{ results: Suggestion[]; offline?: boolean }>('wiki.related', { title }),
  saved: () => call<{ articles: { title: string; fetchedAt: number }[] }>('wiki.saved'),
};
