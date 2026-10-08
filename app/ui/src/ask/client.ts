// The prompt box's calls to the core (docs/ASK.md): `ask.*`.

import { call } from '../bridge';

/** One instant result: a record in Learn, by the words it holds. */
export interface Found {
  kind: string;
  id: string;
  title: string;
  hint: string;
  /** "Task", "Note", "Row": the word shown beside it. */
  word: string;
  /** The Database table it is a row of. */
  table: string;
}

/** What Claude would change, waiting for a click. */
export interface Change {
  /** "4 tasks moved, 1 task created" */
  summary: string;
  lines: string[];
  count: number;
}

export interface Answer {
  id: string;
  /** Markdown. A Learn record is a `learn://table/id` link, an article a `wiki://Title` one. */
  answer: string;
  wiki: string[];
  opens: { what: string; title?: string; table?: string; id?: string; tab?: string }[];
  change: Change | null;
  /** Changes that leave this Mac: each waits for its own answer. */
  outward: { index: number; line: string }[];
  tools: string[];
  seconds: number;
}

export interface Applied {
  applied: number;
  failed: { line: string; message: string }[];
  summary: string;
  undo: string | null;
  steps: number;
}

export const ask = {
  search: (q: string, limit = 12) => call<{ results: Found[] }>('ask.search', { q, limit }).then((r) => r.results),
  status: () => call<{ available: boolean; reason?: string }>('ask.status'),
  send: (prompt: string, context: unknown, history: unknown) =>
    call<Answer>('ask.send', { prompt, context: context ?? undefined, history }),
  apply: (id: string) => call<Applied>('ask.apply', { id }),
  applyOutward: (id: string, index: number) =>
    call<{ done: boolean; line: string; undo: string | null; sentence?: string | null }>('ask.applyOutward', {
      id,
      index,
    }),
  discard: (id: string) => call('ask.discard', { id }),
  cancel: (id: string) => call('ask.cancel', { id }),
};
