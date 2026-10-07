// ⌘L, the library drawer (docs/SPEC.md 2.5): a 280 pt source list that
// slides over any view. Sources on top, the clips below with each one's
// verdict, bulk tag, colour and delete for a selection, and import by
// dropping files or folders, which first says what they hold ("214 files:
// 38 .wwav, 12 .swav, 160 plain audio, 4 other. Plain audio comes in as
// master only."). A delete removes rows, never files, and ⌘Z brings them
// back. Its keys come through the shell's router.

import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useState } from 'react';
import { call, CoreError } from '../bridge';
import { useCoreEvent } from './hooks';
import { addToSelection, clickSelect, moveCursor, noSelection, type Selection, tagName } from './library';
import { IS_MAC } from './platform';
import type { ScreenStatus } from './StatusBar';

export interface Clip {
  id: string;
  kind: string;
  title: string;
  artist: string;
  bpm: number | null;
  key: string | null;
  duration: number;
  colour: string | null;
  tags: string[];
  pinned: number | null;
  verdict: string;
  published: 'up' | 'queued' | null;
  sha256: string;
  bytes: number;
  created: number;
}

export const COLOURS = ['red', 'orange', 'yellow', 'green', 'blue', 'purple'] as const;
const PINS = 4;

type Source =
  | { kind: 'all' }
  | { kind: 'clips'; value: 'wwav' | 'swav' | 'audio'; label: string }
  | { kind: 'tag'; value: string }
  | { kind: 'colour'; value: string }
  | { kind: 'takes' };

const LIBRARY: { source: Source; label: string }[] = [
  { source: { kind: 'all' }, label: 'All clips' },
  { source: { kind: 'clips', value: 'wwav', label: 'Songs' }, label: 'Songs' },
  { source: { kind: 'clips', value: 'swav', label: 'Films' }, label: 'Films' },
  { source: { kind: 'clips', value: 'audio', label: 'Plain audio' }, label: 'Plain audio' },
  { source: { kind: 'takes' }, label: 'Takes' },
];

// What an empty source says, and why (8.10).
const EMPTY: Partial<Record<Source['kind'], string>> = {
  takes: 'Takes land here when you record in the Console.',
};

const same = (a: Source, b: Source) => JSON.stringify(a) === JSON.stringify(b);

function filterOf(s: Source): Record<string, unknown> | null {
  switch (s.kind) {
    case 'all':
      return {};
    case 'clips':
      return { kind: s.value };
    case 'tag':
      return { tags: [s.value] };
    case 'colour':
      return { colour: s.value };
    default:
      return null;
  }
}

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

interface Props {
  shown: boolean;
  onStatus(status: ScreenStatus): void;
  onInfo(id: string): void;
  onError(message: string): void;
}

export interface DrawerHandle {
  move(by: 1 | -1): void;
  open(): void;
  addToSelection(): void;
  remove(): void;
  /** Esc: clears a selection, and says whether there was one to clear. */
  deselect(): boolean;
  drop(paths: string[]): void;
  /** The clip under the cursor, for Space, ⌘E and ⌘I. */
  current(): Clip | null;
}

export const LibraryDrawer = forwardRef<DrawerHandle, Props>(function LibraryDrawer(
  { shown, onStatus, onInfo, onError },
  ref,
) {
  const [source, setSource] = useState<Source>({ kind: 'all' });
  const [filter, setFilter] = useState('');
  const [clips, setClips] = useState<Clip[]>([]);
  const [everything, setEverything] = useState<Clip[]>([]);
  const [selection, setSelection] = useState<Selection>(noSelection);
  const [bulkTag, setBulkTag] = useState('');
  const [importing, setImporting] = useState<{ paths: string[]; lines: string[]; busy: boolean } | null>(null);
  const [said, setSaid] = useState<string | null>(null);

  const order = useMemo(() => clips.map((c) => c.id), [clips]);

  const refresh = useCallback(() => {
    const listed = filter.trim()
      ? call<{ clips: Clip[] }>('library.search', { q: filter.trim(), limit: 500 })
      : filterOf(source)
        ? call<{ clips: Clip[] }>('library.list', { filter: filterOf(source), limit: 500 })
        : Promise.resolve({ clips: [] });
    listed.then(
      (r) => setClips(r.clips),
      () => {},
    );
    call<{ clips: Clip[] }>('library.list', { limit: 10_000 }).then(
      (r) => setEverything(r.clips),
      () => {},
    );
  }, [filter, source]);

  useEffect(() => {
    if (shown) refresh();
  }, [shown, refresh]);
  useCoreEvent('library', () => {
    if (shown) refresh();
  });
  useCoreEvent<{ done: number; total: number }>('library.import', ({ done, total }) => {
    setSaid(`Bringing in ${done} of ${total}…`);
  });

  useEffect(() => {
    if (!shown) return;
    const picked = selection.ids.size;
    onStatus({
      count: picked > 1 ? `${picked} of ${plural(clips.length, 'clip')} selected` : plural(clips.length, 'clip'),
      act: 'Add to selection',
    });
  }, [shown, clips.length, selection.ids.size, onStatus]);

  const tags = useMemo(() => [...new Set(everything.flatMap((c) => c.tags))].sort(), [everything]);
  const pins = useMemo(() => {
    const slots: (Clip | null)[] = Array(PINS).fill(null);
    for (const c of everything) if (c.pinned !== null && c.pinned < PINS) slots[c.pinned] = c;
    return slots;
  }, [everything]);
  const selected = clips.filter((c) => selection.ids.has(c.id));

  const run = <T,>(p: Promise<T>) =>
    p.catch((e: unknown) => {
      onError(e instanceof CoreError || e instanceof Error ? e.message : String(e));
      return null;
    });

  // ⌫ deletes the selection, or the row under the cursor when nothing is selected.
  const removeSelected = () => {
    const ids = selected.length ? selected.map((c) => c.id) : selection.cursor ? [selection.cursor] : [];
    if (!ids.length) return;
    const label = ids.length === 1 ? 'delete clip' : `delete ${ids.length} clips`;
    void run(call('library.delete', { ids, label })).then(() => setSelection(noSelection()));
  };

  useImperativeHandle(ref, () => ({
    move: (by) => setSelection((s) => moveCursor(s, by, order)),
    open() {
      if (selection.cursor) onInfo(selection.cursor);
    },
    addToSelection: () => setSelection(addToSelection),
    remove: removeSelected,
    deselect() {
      if (selection.ids.size === 0) return false;
      setSelection(noSelection());
      return true;
    },
    drop(paths) {
      Promise.all(paths.map((path) => call<{ summary: string }>('library.inspect', { path }).then((r) => r.summary)))
        .then((lines) => setImporting({ paths, lines, busy: false }))
        .catch((e: Error) => onError(e.message));
    },
    current: () => clips.find((c) => c.id === selection.cursor) ?? null,
  }));

  const bringIn = () => {
    if (!importing) return;
    setImporting({ ...importing, busy: true });
    void run(
      call<{ clips: Clip[]; skipped: { why: string }[] }>('library.import', {
        paths: importing.paths,
        label: 'import',
      }),
    ).then((r) => {
      setImporting(null);
      if (!r) return setSaid(null);
      const skipped = r.skipped.length ? ` ${plural(r.skipped.length, 'file')} stayed out: ${r.skipped[0].why}` : '';
      setSaid(`Brought in ${plural(r.clips.length, 'file')}.${skipped}`);
    });
  };

  const pick = (s: Source) => {
    setSource(s);
    setFilter('');
    setSelection(noSelection());
  };

  return (
    <aside className="drawer register-desk" aria-label="Library" hidden={!shown} data-drop="drawer">
      <input
        className="drawer-filter"
        type="search"
        aria-label="Filter the library"
        placeholder="Filter: words, tag:, key:, bpm:"
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
      />
      <nav className="sources" aria-label="Sources">
        <h2 className="source-heading">Library</h2>
        {LIBRARY.map(({ source: s, label }) => (
          <SourceRow key={label} label={label} on={same(s, source)} onPick={() => pick(s)} />
        ))}
        <h2 className="source-heading">Pins</h2>
        <div className="pins">
          {pins.map((c, i) => (
            <button
              key={i}
              type="button"
              className="pin"
              aria-label={c ? `Pin ${i + 1}: ${c.title}` : `Pin ${i + 1}, empty`}
              disabled={!c}
              title={c ? c.title : 'Pin a clip from Get Info.'}
              onClick={() => c && onInfo(c.id)}
            >
              {c ? c.title.slice(0, 2) : ''}
            </button>
          ))}
        </div>
        {tags.length > 0 && <h2 className="source-heading">Tags</h2>}
        {tags.map((t) => (
          <SourceRow
            key={t}
            label={t}
            on={same({ kind: 'tag', value: t }, source)}
            onPick={() => pick({ kind: 'tag', value: t })}
          />
        ))}
        <h2 className="source-heading">Colour labels</h2>
        <div className="swatches">
          {COLOURS.map((c) => (
            <button
              key={c}
              type="button"
              className="swatch"
              data-colour={c}
              data-dense
              aria-label={`${c[0].toUpperCase()}${c.slice(1)} label`}
              aria-pressed={same({ kind: 'colour', value: c }, source)}
              onClick={() => pick({ kind: 'colour', value: c })}
            />
          ))}
        </div>
      </nav>
      {importing && (
        <div className="drawer-note" role="status">
          {importing.lines.map((l) => (
            <p key={l}>{l}</p>
          ))}
          <div className="note-actions">
            <button type="button" className="gel" disabled={importing.busy} onClick={bringIn}>
              Bring them in
            </button>
            <button type="button" className="gel plain" onClick={() => setImporting(null)}>
              Not now
            </button>
          </div>
        </div>
      )}
      {said && !importing && (
        <p className="drawer-said" role="status">
          {said}
        </p>
      )}
      <div className="clips" role="listbox" aria-label="Clips" aria-multiselectable="true">
        {clips.map((c) => (
          <div
            key={c.id}
            role="option"
            className="clip-row"
            data-dense
            aria-selected={selection.ids.has(c.id)}
            data-cursor={selection.cursor === c.id}
            onClick={(e) => {
              const mode = e.shiftKey ? 'add' : (IS_MAC ? e.metaKey : e.ctrlKey) ? 'toggle' : 'only';
              setSelection((s) => clickSelect(s, c.id, mode, order));
            }}
            onDoubleClick={() => onInfo(c.id)}
          >
            {c.colour && <span className="clip-colour" data-colour={c.colour} aria-label={`${c.colour} label`} />}
            <span className="clip-title">{c.title}</span>
            <span className="clip-verdict" data-text="secondary">
              {c.verdict}
            </span>
          </div>
        ))}
        {clips.length === 0 && <p className="clips-empty">{EMPTY[source.kind] ?? 'Nothing here right now.'}</p>}
      </div>
      {selected.length > 0 && (
        <div className="bulk" aria-label="Selection">
          <form
            onSubmit={(e) => {
              e.preventDefault();
              const tag = tagName(bulkTag);
              if (!tag) return;
              const label = `tag ${selected.length === 1 ? 'clip' : 'clips'}`;
              void run(call('library.tag', { ids: selected.map((c) => c.id), add: [tag], label })).then((r) => {
                if (r) setBulkTag('');
              });
            }}
          >
            <input
              className="bulk-tag"
              aria-label="Add a tag to the selection"
              placeholder="Add a tag"
              value={bulkTag}
              onChange={(e) => setBulkTag(e.target.value.toLowerCase())}
            />
          </form>
          <div className="swatches">
            {COLOURS.map((c) => (
              <button
                key={c}
                type="button"
                className="swatch"
                data-colour={c}
                data-dense
                aria-label={`Label ${c}`}
                onClick={() =>
                  void run(call('library.colour', { ids: selected.map((x) => x.id), colour: c, label: 'colour clips' }))
                }
              />
            ))}
          </div>
          <button type="button" className="gel" onClick={removeSelected}>
            Delete
          </button>
        </div>
      )}
    </aside>
  );
});

function SourceRow({ label, on, onPick }: { label: string; on: boolean; onPick(): void }) {
  return (
    <button type="button" className="source-row" data-dense aria-current={on ? 'true' : undefined} onClick={onPick}>
      {label}
    </button>
  );
}
