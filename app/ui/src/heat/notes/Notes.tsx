// Notes (docs/NOTES.md): markdown notes with links between them, and the
// capture inbox. A note is a file in the notes folder and a record here;
// the core keeps the two the same, and this tab only reads the snapshot's
// `notes` and asks the core to write.
//
// A list of notes beside the one that is open (Pane.tsx). The sidebar holds
// the Inbox (captured pages waiting to be filed), All notes and today's
// note, then the courses that have notes, then the tags. The space chosen
// above them narrows the list as it does in every tab; the Inbox ignores
// it, as Tasks' does. "+" makes a note where the list is looking, Quick
// open (O, ⇧Return) finds one by its title, and the search field (⌘F) asks
// the core for the notes that hold the words. A photo or a PDF dropped or
// pasted here goes to the capture inbox, to be read and filed.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { clockAt, shortMonthDay } from '../../shared/time/format';
import { dayKey } from '../../shared/time/zone';
import { listenForDrops, pathsFromUris } from '../../shell/drop';
import type { Id, Note, NoteHit } from '../client';
import { copy, plural } from '../fmt';
import { useSheets, useSidebarSlot, useSpaceFilter, useTabActs, useTabKeys, useTabScope } from '../frame';
import { focusKind } from '../keys';
import { useHeat } from '../store';
import { GuideSheet } from './Guide';
import { Lightbox, type Shown } from './images';
import { useOpenRequests } from './open';
import { Pane, type PaneHandle } from './Pane';
import { QuickOpen } from './QuickOpen';
import './notes.css';

type View = { kind: 'all' } | { kind: 'inbox' } | { kind: 'course'; id: Id } | { kind: 'tag'; tag: string };

const SEARCH_AFTER_MS = 200;
/** What a page can be: a photo, a scan, a PDF. */
const PAGE = /\.(png|jpe?g|heic|heif|gif|webp|tiff?|bmp|pdf)$/i;
const isPage = (file: File) => file.type.startsWith('image/') || file.type === 'application/pdf' || PAGE.test(file.name);

/** A file's bytes as base64, for a view that has the file but not its path. */
function base64Of(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result).replace(/^data:[^,]*,/, ''));
    reader.onerror = () => reject(reader.error ?? new Error('That file couldn’t be read.'));
    reader.readAsDataURL(file);
  });
}

export function Notes() {
  const { snap, idx, client, act, say, refetch, date, tz, now } = useHeat();
  const { spaceId } = useSpaceFilter();
  const { openSheet, closeSheet } = useSheets();
  const { active } = useTabScope();
  const slot = useSidebarSlot();
  const [view, setView] = useState<View>({ kind: 'all' });
  const [picked, setPicked] = useState<Id | null>(null);
  const [naming, setNaming] = useState<Id | null>(null);
  const [query, setQuery] = useState('');
  const [found, setFound] = useState<{ for: string; hits: NoteHit[] } | null>(null);
  const [quick, setQuick] = useState(false);
  const [image, setImage] = useState<Shown | null>(null);
  const pane = useRef<PaneHandle>(null);
  const search = useRef<HTMLInputElement>(null);

  const info = snap?.notes;
  const byId = useMemo(() => new Map((snap?.records.note ?? []).map((n) => [n.id, n])), [snap]);
  const index = info?.index ?? {};
  // The core gives the order; before it does, the records as they come.
  const order = info?.order ?? [...byId.keys()];

  // A note belongs to its space; one filed to a course is Classes' kind of work.
  const inSpace = (n: Note) =>
    !spaceId || n.spaceId === spaceId || (!n.spaceId && !!n.courseId && idx.space.get(spaceId)?.groupKind === 'course');
  const inView = (id: Id, v: View) => {
    const n = byId.get(id);
    if (!n) return false;
    if (v.kind === 'inbox') return info?.inbox.includes(id) ?? n.inbox === true;
    if (!inSpace(n)) return false;
    if (v.kind === 'course') return n.courseId === v.id;
    if (v.kind === 'tag') return index[id]?.tags.includes(v.tag) ?? false;
    return true;
  };
  const listed = order.filter((id) => inView(id, view));

  // The search asks the core, a moment after the typing stops; no words, the list as it was.
  const needle = query.trim();
  useEffect(() => {
    if (!needle) return setFound(null);
    let live = true;
    const t = setTimeout(() => {
      client.notes.search(needle).then(
        (r) => live && setFound({ for: needle, hits: r.hits }),
        () => live && setFound({ for: needle, hits: [] }),
      );
    }, SEARCH_AFTER_MS);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [client, needle, snap]);
  const hits = needle ? (found?.for === needle ? found.hits.filter((h) => byId.has(h.id)) : []) : null;
  const rows = hits ? hits.map((h) => h.id) : listed;
  // The open note stays open while it is there, whatever the list comes to show: a note is never taken
  // from under the caret because it was filed, or lost the tag the list is looking at. With none open,
  // the first in the list is.
  const current = (picked ? byId.get(picked) : undefined) ?? byId.get(rows[0] ?? '') ?? null;
  const currentId = current?.id ?? null;
  useEffect(() => {
    if (picked !== currentId) setPicked(currentId);
  }, [picked, currentId]);

  const open = useCallback((id: Id) => {
    setPicked(id);
    setQuick(false);
  }, []);
  useOpenRequests(open);
  /** A choice in the sidebar: the list changes, and so does the open note unless it is in the new list too. */
  const choose = (next: View) => {
    setView(next);
    setQuery('');
    if (picked && !inView(picked, next)) setPicked(null);
  };

  const add = async () => {
    const r = await act(client.notes.create(view.kind === 'course' ? { courseId: view.id } : spaceId ? { spaceId } : {}));
    if (!r) return;
    await refetch();
    open(r.note.id);
    setNaming(r.note.id);
  };

  /** Today's daily note is the note titled with the day; it is made the first time it is asked for. */
  const openDaily = async () => {
    if (info?.daily) return open(info.daily);
    const r = await act(client.notes.create({ title: date, ifMissing: true }));
    if (!r) return;
    await refetch();
    open(r.note.id);
  };

  // --- capture: a page dropped or pasted here goes to the inbox ----------------------------

  const added = async (n: number) => {
    if (n === 0) return;
    say(n === 1 ? 'Page added. It will be read and filed.' : `${n} pages added. They will be read and filed.`);
    await refetch();
  };
  const addPaths = async (paths: string[]) => {
    const pages = paths.filter((p) => PAGE.test(p));
    if (pages.length === 0) return say('Drop a photo or a PDF of a page.');
    const r = await act(client.pages.add({ paths: pages }));
    if (r) await added(r.added);
  };
  const addFiles = async (files: File[]) => {
    const pages = files.filter(isPage);
    if (pages.length === 0) return say('Drop a photo or a PDF of a page.');
    let n = 0;
    for (const file of pages) {
      // A pasted image has no name of its own: it is named for when it was pasted.
      const ext = /\.\w+$/.exec(file.name)?.[0] ?? `.${file.type.split('/')[1] || 'png'}`;
      const name = !file.name || /^image\.\w+$/i.test(file.name) ? `Pasted ${date} ${clockAt(now(), tz).replace(':', '.')}${ext}` : file.name;
      const r = await act(base64Of(file).then((base64) => client.pages.add({ name, base64 })));
      if (r) n += r.added;
    }
    await added(n);
  };
  const adding = useRef({ addPaths, addFiles });
  adding.current = { addPaths, addFiles };

  // In the app a drop's paths come from Tauri's own event, as Grades' syllabus does (shell/drop.ts).
  useEffect(
    () =>
      listenForDrops((target, paths) => {
        if ((target as string) === 'notes') void adding.current.addPaths(paths);
      }),
    [],
  );
  // A paste with no field focused is a page too; in a field it is the field's.
  useEffect(() => {
    if (!active) return;
    const onPaste = (e: ClipboardEvent) => {
      if (focusKind(document.activeElement) === 'text') return;
      const files = [...(e.clipboardData?.files ?? [])].filter(isPage);
      if (files.length === 0) return;
      e.preventDefault();
      void adding.current.addFiles(files);
    };
    document.addEventListener('paste', onPaste);
    return () => document.removeEventListener('paste', onPaste);
  }, [active]);

  useTabActs({
    plus: { run: () => void add() },
    secondary: { run: () => setQuick(true) },
    count: snap ? plural(rows.length, 'note') : null,
  });
  useTabKeys({
    move(by) {
      if (rows.length === 0) return;
      const at = current ? rows.indexOf(current.id) : -1;
      setPicked(rows[Math.min(rows.length - 1, Math.max(0, at + by))]);
    },
    edit: () => pane.current?.write(),
    escape() {
      if (image) {
        setImage(null);
        return true;
      }
      if (quick) {
        setQuick(false);
        return true;
      }
      if (pane.current?.escape()) return true;
      if (document.activeElement !== search.current) return false;
      search.current?.blur();
      return true;
    },
    filter: () => search.current?.focus(),
    key(e) {
      // ⌫ deletes nothing here: a note goes with its own Delete, whatever is selected in another tab.
      if (e.key === 'Backspace' || e.key === 'Delete') return true;
      if (e.command || e.alt || e.shift || e.repeat || e.key.toLowerCase() !== 'o') return false;
      setQuick(true);
      return true;
    },
  });

  if (!snap) return <div className="notes" />;

  const stamp = (ms: number | null | undefined) =>
    !ms ? '' : dayKey(ms, tz) === date ? clockAt(ms, tz) : shortMonthDay(dayKey(ms, tz));
  const inSpaceIds = order.filter((id) => inView(id, { kind: 'all' }));
  const courses = snap.records.course
    .map((c) => ({ course: c, count: inSpaceIds.filter((id) => byId.get(id)?.courseId === c.id).length }))
    .filter((c) => c.count > 0)
    .sort((a, b) => a.course.code.localeCompare(b.course.code));
  const capture = info?.capture;
  const waiting = capture?.waiting ?? 0;
  const empty =
    hits !== null
      ? found?.for === needle
        ? 'No note holds those words.'
        : ''
      : view.kind === 'inbox'
        ? 'Nothing is waiting to be filed.'
        : byId.size === 0
          ? 'No notes yet. Press + to write one, or drop a photo of a page here.'
          : copy.status.empty;

  return (
    <div
      className="notes"
      data-drop="notes"
      onDragOver={(e) => {
        if (e.dataTransfer.types.includes('Files')) e.preventDefault();
      }}
      onDrop={(e) => {
        // In a browser a dropped file has no path: its bytes go instead. Where there are paths, the listener above has them.
        if ('__TAURI_INTERNALS__' in window || pathsFromUris(e.dataTransfer.getData('text/uri-list')).length > 0) return;
        const files = [...e.dataTransfer.files];
        if (files.length === 0) return;
        e.preventDefault();
        void addFiles(files);
      }}
    >
      {slot &&
        createPortal(
          <>
            <h2 className="heat-side-heading" data-text="secondary">
              Notes
            </h2>
            <SideRow label="Inbox" count={info?.inbox.length ?? 0} on={view.kind === 'inbox'} onPick={() => choose({ kind: 'inbox' })} />
            <SideRow label="All notes" count={inSpaceIds.length} on={view.kind === 'all'} onPick={() => choose({ kind: 'all' })} />
            <SideRow label="Today’s note" on={false} onPick={() => void openDaily()} />
            <button
              type="button"
              className="heat-side-row heat-side-new"
              data-dense
              onClick={() => openSheet(<GuideSheet onClose={closeSheet} />)}
            >
              Send to Wi-WWAV…
            </button>
            {courses.length > 0 && (
              <>
                <h2 className="heat-side-heading" data-text="secondary">
                  Courses
                </h2>
                {courses.map(({ course, count }) => (
                  <SideRow
                    key={course.id}
                    label={course.code}
                    count={count}
                    on={view.kind === 'course' && view.id === course.id}
                    onPick={() => choose({ kind: 'course', id: course.id })}
                  />
                ))}
              </>
            )}
            {(info?.tags.length ?? 0) > 0 && (
              <>
                <h2 className="heat-side-heading" data-text="secondary">
                  Tags
                </h2>
                {info!.tags.map((t) => (
                  <SideRow
                    key={t.tag}
                    label={`#${t.tag}`}
                    count={t.count}
                    on={view.kind === 'tag' && view.tag === t.tag}
                    onPick={() => choose({ kind: 'tag', tag: t.tag })}
                  />
                ))}
              </>
            )}
          </>,
          slot,
        )}
      <header className="notes-head">
        <h1 className="heat-heading">Notes</h1>
        <div className="notes-tools">
          <button type="button" className="gel plain" onClick={() => setQuick(true)} title="Quick open (O, ⇧Return)">
            Quick open
          </button>
          <input
            ref={search}
            type="search"
            className="heat-filter"
            aria-label="Search the notes"
            placeholder="Search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </header>
      {(capture?.doing || waiting > 0) && (
        <p className="notes-capture" role="status" data-text="secondary">
          {[capture?.doing, waiting > 0 ? `${plural(waiting, 'page')} waiting to be read` : null].filter(Boolean).join(' · ')}
        </p>
      )}
      <div className="notes-body">
        <div role="listbox" aria-label="Notes" className="notes-list">
          {rows.length === 0 && <p className="heat-empty">{empty}</p>}
          {rows.map((id) => {
            const hit = hits?.find((h) => h.id === id);
            const entry = index[id];
            const note = byId.get(id)!;
            return (
              <div
                key={id}
                role="option"
                className="notes-row"
                data-dense
                aria-selected={current?.id === id}
                tabIndex={-1}
                onClick={() => setPicked(id)}
              >
                <span className="notes-row-title">{entry?.title || note.title || 'Untitled'}</span>
                <span className="notes-row-date">{stamp(entry?.updatedAt ?? note.updatedAt)}</span>
                <span className="notes-row-excerpt">{hit ? hit.snippet : (entry?.excerpt ?? '')}</span>
                {(entry?.label || note.inbox) && (
                  <span className="notes-row-chips">
                    {note.inbox && <span className="heat-tag">Inbox</span>}
                    {entry?.label && <span className="heat-tag notes-row-label">{entry.label}</span>}
                  </span>
                )}
              </div>
            );
          })}
        </div>
        {current ? (
          <Pane
            key={current.id}
            ref={pane}
            note={current}
            naming={naming === current.id}
            onNamed={() => setNaming(null)}
            onOpen={open}
            onTag={(tag) => choose({ kind: 'tag', tag })}
            onImage={setImage}
            onGone={() => setPicked(null)}
          />
        ) : (
          <p className="heat-empty notes-none">{byId.size === 0 ? 'Notes are markdown files in your library’s Notes folder.' : 'No note is open.'}</p>
        )}
      </div>
      {quick && (
        <QuickOpen
          notes={order.flatMap((id) => (byId.has(id) ? [{ id, title: index[id]?.title || byId.get(id)!.title || 'Untitled' }] : []))}
          onOpen={open}
          onClose={() => setQuick(false)}
        />
      )}
      {image && <Lightbox image={image} onClose={() => setImage(null)} />}
    </div>
  );
}

function SideRow({ label, count, on, onPick }: { label: string; count?: number; on: boolean; onPick(): void }) {
  return (
    <button type="button" className="heat-side-row" data-dense aria-current={on ? 'true' : undefined} onClick={onPick}>
      <span className="heat-side-name">{label}</span>
      {count !== undefined && (
        <span className="heat-side-count" data-text="secondary" aria-label={`${plural(count, 'note')}`}>
          {count}
        </span>
      )}
    </button>
  );
}
