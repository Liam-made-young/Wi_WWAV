// The open note (docs/NOTES.md): its title, where it is filed, its text
// (Editor.tsx), what Claude suggested for it, and the notes that link to
// it. The title is the note's name everywhere, so a new one is saved when
// the field is left and every link to the note follows it. A checkbox is a
// real one: ticking it is the core's `toggleBox`, "Make task" makes its line
// a task, and from then on the box shows the task's own state.

import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';
import { clockAt, shortMonthDay } from '../../shared/time/format';
import { dayKey } from '../../shared/time/zone';
import type { Id, Note, NotePlace } from '../client';
import { courseLabel, dayText, plural } from '../fmt';
import { useSelection, useTabs } from '../frame';
import { useHeat } from '../store';
import { Editor, type EditorHandle } from './Editor';
import { NoteImage, type Shown } from './images';
import type { Hooks, Linked } from './markdown';
import { MARKER } from './text';

export interface PaneHandle {
  /** Esc: leaves the text or the title. Says whether it did anything. */
  escape(): boolean;
  /** Puts the caret at the end of the note's text. */
  write(): void;
  /** Sends what is typed now, before something else is done to the note. */
  flush(): Promise<void>;
}

interface Props {
  note: Note;
  /** A new note: the caret starts in its title, with the name it was given selected. */
  naming: boolean;
  onNamed(): void;
  onOpen(id: Id): void;
  onTag(tag: string): void;
  onImage(image: Shown): void;
  /** The note was deleted. */
  onGone(): void;
}

/** "today", "tomorrow", "Oct 9": a day after the word Due. */
const dueWord = (day: string, today: string) => dayText(day, today).replace(/^(Today|Tomorrow|Yesterday)$/, (w) => w.toLowerCase());
const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();
const squashed = (text: string) => text.replace(/\s+/g, '').toLowerCase();

export const Pane = forwardRef<PaneHandle, Props>(function Pane({ note, naming, onNamed, onOpen, onTag, onImage, onGone }, ref) {
  const { snap, idx, client, act, say, refetch, date, tz } = useHeat();
  const { setTab } = useTabs();
  const { selectTask } = useSelection();
  const editor = useRef<EditorHandle>(null);
  const titleField = useRef<HTMLInputElement>(null);
  const [title, setTitle] = useState(note.title ?? '');
  useEffect(() => setTitle(note.title ?? ''), [note.title]);
  useEffect(() => {
    if (!naming) return;
    titleField.current?.focus();
    titleField.current?.select();
    onNamed();
  }, [naming, onNamed]);

  const flush = () => editor.current?.flush() ?? Promise.resolve();
  useImperativeHandle(ref, () => ({
    escape() {
      if (editor.current?.escape()) return true;
      if (document.activeElement !== titleField.current) return false;
      titleField.current?.blur();
      return true;
    },
    write: () => editor.current?.focus('end'),
    flush,
  }));

  const entry = snap?.notes?.index[note.id];

  const rename = async () => {
    const next = title.trim();
    const was = note.title ?? '';
    if (next === was) return setTitle(was);
    await flush();
    const r = await act(client.notes.save(note.id, { title: next }));
    // A name another note has is refused in the core's sentence, and the note keeps its own.
    if (!r) return setTitle(was);
    if (r.renamed > 0) say(`Renamed. Links in ${plural(r.renamed, 'other note')} follow it.`);
  };

  // Each handler is read when it runs, so the drawn blocks needn't be drawn again for a new one.
  const does = useRef({ onOpen, onTag, onImage });
  does.current = { onOpen, onTag, onImage };

  const hooks = useMemo<Hooks>(() => {
    const titles = new Map((snap?.records.note ?? []).flatMap((n) => (n.title ? [[n.title.trim().toLowerCase(), n.id] as const] : [])));
    const codes = new Map((snap?.records.course ?? []).map((c) => [squashed(c.code), c.id] as const));
    const open = (snap?.derived.lists.allOpen ?? []).flatMap((id) => {
      const task = idx.task.get(id);
      return task ? [[task.title.trim().toLowerCase(), id] as const] : [];
    });
    const tasks = new Map(open);
    /** What a link finds: the index's answer, or the same rule here while the index catches up. */
    const resolve = (target: string): Linked => {
      const known = entry?.links.find((l) => same(l.target, target));
      if (known && known.kind !== 'missing') return { kind: known.kind, id: known.id };
      const key = target.trim().toLowerCase();
      if (titles.has(key)) return { kind: 'note', id: titles.get(key)! };
      if (codes.has(squashed(target))) return { kind: 'course', id: codes.get(squashed(target))! };
      if (tasks.has(key)) return { kind: 'task', id: tasks.get(key)! };
      return { kind: 'missing', id: null };
    };

    const makeTask = async (line: number) => {
      await editor.current?.flush();
      if (await act(client.notes.taskFromLine(note.id, line))) say('Task added.');
    };

    return {
      resolve,
      async onLink(target, to) {
        if (to.kind === 'note' && to.id) return does.current.onOpen(to.id);
        if (to.kind === 'course') return setTab('grades');
        if (to.kind === 'task' && to.id) {
          selectTask(to.id);
          return setTab('tasks');
        }
        // A link to nothing yet: the note is made, and opened.
        const r = await act(client.notes.create({ title: target, ifMissing: true }));
        if (!r) return;
        await refetch();
        does.current.onOpen(r.note.id);
      },
      onTag: (tag) => does.current.onTag(tag),
      box(line, written, text) {
        const marker = MARKER.exec(text)?.[1] ?? null;
        if (!marker) {
          return {
            beside: (
              <button type="button" className="notes-make-task" data-dense onClick={() => void makeTask(line)}>
                Make task
              </button>
            ),
          };
        }
        // A box that is a task shows the task's own state.
        const held = entry?.boxes.find((b) => b.taskId === marker);
        const task = idx.task.get(marker);
        if (!held && !task) return undefined;
        return {
          done: held?.done ?? task?.done ?? written,
          beside: (
            <button type="button" className="heat-tag notes-task-chip" data-dense title="Select the task" onClick={() => selectTask(marker)}>
              task
            </button>
          ),
        };
      },
      async onBox(line, done) {
        await editor.current?.flush();
        await act(client.notes.toggleBox(note.id, line, done));
      },
      image: (src, alt) => <NoteImage src={src} alt={alt} onOpen={(image) => does.current.onImage(image)} />,
    };
  }, [snap, idx, entry, note.id, client, act, say, refetch, setTab, selectTask]);

  const file = async (value: string) => {
    const place: NotePlace =
      value === 'none' ? { none: true } : value.startsWith('course:') ? { courseId: value.slice(7) } : { spaceId: value.slice(6) };
    await flush();
    const r = await act(client.notes.file(note.id, place));
    if (r) say(r.line);
  };

  const remove = async () => {
    await flush();
    if (!(await act(client.notes.remove(note.id)))) return;
    say(`Deleted ‘${note.title ?? 'Untitled'}’.`);
    onGone();
  };

  if (!snap) return null;
  const courses = [...snap.records.course].sort((a, b) => a.code.localeCompare(b.code));
  const filed = note.inbox ? '' : note.courseId ? `course:${note.courseId}` : note.spaceId ? `space:${note.spaceId}` : 'none';
  const stamp = (ms: number) => (dayKey(ms, tz) === date ? `today ${clockAt(ms, tz)}` : shortMonthDay(dayKey(ms, tz)));
  const suggestion = snap.notes?.suggestions[note.id];
  const backlinks = entry?.backlinks ?? [];

  return (
    <section className="notes-pane" aria-label={note.title || 'Untitled'}>
      <div className="notes-page">
        <input
          ref={titleField}
          className="notes-title"
          aria-label="Title"
          placeholder="Untitled"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={() => void rename()}
          onKeyDown={(e) => {
            if (e.key !== 'Enter' || e.nativeEvent.isComposing) return;
            e.preventDefault();
            // Leaving the field saves the name; the caret goes on to the text.
            editor.current?.focus('start');
          }}
        />
        <div className="notes-meta" data-inbox={note.inbox ? '' : undefined}>
          <label className="notes-file">
            <span data-text="secondary">{note.inbox ? 'In the Notes inbox' : 'Filed to'}</span>
            <select aria-label={note.inbox ? 'File to' : 'Filed to'} value={filed} onChange={(e) => void file(e.target.value)}>
              {note.inbox && (
                <option value="" disabled>
                  File to…
                </option>
              )}
              {courses.length > 0 && (
                <optgroup label="Courses">
                  {courses.map((c) => (
                    <option key={c.id} value={`course:${c.id}`}>
                      {snap.derived.courses[c.id]?.label ?? courseLabel(c)}
                    </option>
                  ))}
                </optgroup>
              )}
              <optgroup label="Spaces">
                {snap.records.space.map((s) => (
                  <option key={s.id} value={`space:${s.id}`}>
                    {s.name}
                  </option>
                ))}
              </optgroup>
              <option value="none">No course or space</option>
            </select>
          </label>
          <span className="notes-when" data-text="secondary">
            {note.capturedAt ? `Captured ${stamp(note.capturedAt)}` : note.updatedAt ? `Edited ${stamp(note.updatedAt)}` : ''}
          </span>
          <span className="notes-acts">
            <button type="button" className="heat-link" data-dense onClick={() => void act(client.notes.reveal(note.id))}>
              Show in Finder
            </button>
            <button type="button" className="heat-link" data-dense onClick={() => void remove()}>
              Delete
            </button>
          </span>
        </div>

        <Editor ref={editor} note={note} hooks={hooks} />

        {suggestion && (
          <section className="notes-suggest" aria-label="Claude suggests">
            <h2 className="notes-section-title" data-text="secondary">
              Claude suggests
            </h2>
            {suggestion.tasks.length > 0 && (
              <ul className="notes-suggest-tasks">
                {suggestion.tasks.map((t, n) => (
                  <li key={n} className="notes-suggest-task">
                    <span className="notes-suggest-title">{t.title}</span>
                    {t.due && (
                      <span className="notes-suggest-due" data-text="secondary">
                        Due {dueWord(t.due, date)}
                      </span>
                    )}
                    {t.taskId ? (
                      <span className="heat-tag">Added</span>
                    ) : (
                      <button type="button" className="gel plain" onClick={() => void act(client.notes.acceptSuggestion(note.id, n)).then(() => refetch())}>
                        Add task
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            )}
            {suggestion.terms.length > 0 && (
              <p className="notes-terms">
                <span data-text="secondary">Key terms</span>
                {suggestion.terms.map((term) => (
                  <span key={term} className="heat-tag">
                    {term}
                  </span>
                ))}
              </p>
            )}
            <button type="button" className="heat-link" data-dense onClick={() => void act(client.notes.dismissSuggestions(note.id)).then(() => refetch())}>
              Dismiss
            </button>
          </section>
        )}

        <section className="notes-backlinks" aria-label="Backlinks">
          <h2 className="notes-section-title" data-text="secondary">
            Backlinks
          </h2>
          {backlinks.length === 0 ? (
            <p className="why" data-text="secondary">
              No notes link here yet.
            </p>
          ) : (
            <ul className="notes-backlink-list">
              {backlinks.map((b, n) => (
                <li key={`${b.id}-${n}`}>
                  <button type="button" className="notes-backlink" data-dense onClick={() => onOpen(b.id)}>
                    <span className="notes-backlink-title">{b.title || 'Untitled'}</span>
                    <span className="notes-backlink-line" data-text="secondary">
                      {b.line}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </section>
  );
});
