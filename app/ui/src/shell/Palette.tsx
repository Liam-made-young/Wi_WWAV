// ⌘K (docs/SPEC.md 2.7): one palette over tasks, clips, settings and
// actions. Local matches show at once; the library's search answers after
// 200 ms, and an older answer never replaces a newer one. Filters: tag:,
// key:, bpm:, is:hot, is:remix and @name. Return opens a result; ⌘Return
// opens it in its other view. Its keys come through the shell's router.

import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';
import { call } from '../bridge';
import { IS_MAC, keys } from './platform';
import { latestOnly, parseQuery, type Query, rank } from './palette';
import type { Clip } from './LibraryDrawer';

export interface Item {
  label: string;
  hint?: string;
  run(): void;
}

export interface PaletteTask {
  id: string;
  title: string;
  level: string;
}

interface Props {
  shown: boolean;
  actions: Item[];
  settings: Item[];
  tasks: PaletteTask[];
  onTask(id: string): void;
  onClip(clip: Clip, other: boolean): void;
  onClose(): void;
}

export interface PaletteHandle {
  move(by: 1 | -1): void;
  open(other: boolean): void;
}

interface Result {
  id: string;
  group: string;
  label: string;
  hint?: string;
  run(other: boolean): void;
}

// The core's own search reads tag:, key: and bpm: (library.search).
function coreQuery(q: Query): string {
  const parts = [q.words, ...q.tags.map((t) => `tag:${t}`)];
  if (q.key) parts.push(`key:${q.key.replace(/ /g, '-')}`);
  if (q.bpm) parts.push(`bpm:${q.bpm[0]}-${q.bpm[1]}`);
  return parts.filter(Boolean).join(' ');
}

export const Palette = forwardRef<PaletteHandle, Props>(function Palette(
  { shown, actions, settings, tasks, onTask, onClip, onClose },
  ref,
) {
  const [text, setText] = useState('');
  const [clips, setClips] = useState<Clip[]>([]);
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const query = useMemo(() => parseQuery(text), [text]);
  const filtered = query.tags.length > 0 || query.key !== null || query.bpm !== null;

  const search = useMemo(
    () =>
      latestOnly(
        (q) => call<{ clips: Clip[] }>('library.search', { q, limit: 20 }).then((r) => r.clips),
        (_q, found) => setClips(found),
      ),
    [],
  );

  useEffect(() => {
    if (!shown) {
      search.cancel();
      return;
    }
    input.current?.focus();
    input.current?.select();
  }, [shown, search]);

  useEffect(() => {
    setAt(0);
    setClips([]);
    const q = coreQuery(query);
    if (shown && q && !query.remix && !query.hot && query.person === null) search.ask(q);
    else search.cancel();
  }, [query, shown, search]);

  const results = useMemo<Result[]>(() => {
    const out: Result[] = [];
    const plain = !filtered && !query.hot && !query.remix && query.person === null;
    if (plain) {
      for (const a of rank(query.words, actions, (a) => a.label)) {
        out.push({ id: `a:${a.label}`, group: 'Actions', label: a.label, hint: a.hint, run: () => a.run() });
      }
    }
    if (!filtered && !query.remix && query.person === null) {
      const pool = query.hot ? tasks.filter((t) => t.level === 'Hot' || t.level === 'Overdue') : tasks;
      for (const t of rank(query.words, pool, (t) => t.title).slice(0, 8)) {
        out.push({ id: `t:${t.id}`, group: 'Tasks', label: t.title, hint: t.level, run: () => onTask(t.id) });
      }
    }
    if (plain && query.words) {
      for (const s of rank(query.words, settings, (s) => s.label)) {
        out.push({ id: `s:${s.label}`, group: 'Settings', label: s.label, run: () => s.run() });
      }
    }
    for (const c of clips) {
      const hint = [c.kind === 'wwav' ? 'song' : c.kind, c.key, c.bpm ? `${c.bpm} BPM` : null]
        .filter(Boolean)
        .join(' · ');
      out.push({ id: `c:${c.id}`, group: 'Clips', label: c.title, hint, run: (other) => onClip(c, other) });
    }
    return out;
  }, [query, filtered, actions, settings, tasks, clips, onTask, onClip]);

  useImperativeHandle(ref, () => ({
    move(by) {
      setAt((i) => Math.min(Math.max(0, i + by), Math.max(0, results.length - 1)));
    },
    open(other) {
      const r = results[at];
      if (!r) return;
      onClose();
      r.run(other);
    },
  }));

  const why = query.remix
    ? 'is:remix finds nothing yet: the library doesn’t record which songs are remixes.'
    : query.person !== null
      ? '@name finds people in Space, which isn’t in this build yet.'
      : null;

  let group = '';
  return (
    <div className="sheet palette" role="dialog" aria-label="Search" hidden={!shown}>
      <input
        ref={input}
        className="palette-input"
        type="search"
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-results"
        aria-activedescendant={results[at] ? `palette-${at}` : undefined}
        placeholder="Tasks, clips, settings and actions"
        value={text}
        onChange={(e) => setText(e.target.value)}
      />
      <div id="palette-results" className="palette-results" role="listbox" aria-label="Results">
        {results.map((r, i) => {
          const heading = r.group !== group ? r.group : null;
          group = r.group;
          return (
            <div key={r.id}>
              {heading && (
                <div className="palette-group" role="presentation" data-text="secondary">
                  {heading}
                </div>
              )}
              <div
                id={`palette-${i}`}
                role="option"
                aria-selected={i === at}
                className="palette-row"
                onPointerEnter={() => setAt(i)}
                onClick={(e) => {
                  onClose();
                  r.run(IS_MAC ? e.metaKey : e.ctrlKey);
                }}
              >
                <span className="palette-label">{r.label}</span>
                {r.hint && (
                  <span className="palette-hint" data-text="secondary">
                    {r.hint}
                  </span>
                )}
              </div>
            </div>
          );
        })}
        {results.length === 0 && <p className="palette-empty">{why ?? 'Nothing here right now.'}</p>}
      </div>
      <p className="palette-foot" data-text="secondary">
        Return opens · {keys('⌘')}Return opens in its other view · Esc closes
      </p>
    </div>
  );
});
