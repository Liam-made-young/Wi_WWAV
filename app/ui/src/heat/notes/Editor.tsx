// A note's text, read and written in one place (docs/NOTES.md). The note is
// its blocks (notes/text.ts), and every block is drawn as formatted
// markdown except the one the caret is in, which shows as it is written, in
// a field as tall as its text. A click on a block writes in it; ↑ on its
// first line and ↓ on its last go to the block before and after; ⌫ at its
// start joins it to the one before; a blank line at its end starts the next
// one; Esc leaves. In a list, Return goes on with the list (and ends it on
// an empty item) and Tab and ⇧Tab move an item in and out. `[[` opens the
// list of what a link can name.
//
// What is typed is the truth while it is being typed. It is saved 600 ms
// after the typing stops and when the block is left, and a snapshot that
// arrives in between never replaces it: the text from the core is taken
// only when nothing typed is still waiting to be saved.

import {
  forwardRef,
  type KeyboardEvent,
  memo,
  type MouseEvent,
  type RefObject,
  useCallback,
  useEffect,
  useId,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { Id, Note } from '../client';
import { useHeat } from '../store';
import { caretPoint } from './caret';
import { type Hooks, renderMarkdown } from './markdown';
import { type Block, canon, firstLines, inOpenFence, joinBlocks, LIST_ITEM, splitBlocks, tailOf } from './text';

const SAVE_AFTER_MS = 600;
/** How long a snapshot that says what was sent a moment ago is taken for an echo of it. */
const ECHO_MS = 2000;
const INDENT = '    ';

interface Doc extends Block {
  /** Its own for as long as the note is open, so a block keeps its field while others come and go. */
  key: number;
}

export interface EditorHandle {
  /** Esc: closes the list of links, else leaves the block being written. Says whether it did anything. */
  escape(): boolean;
  /** Sends what is typed now, and waits for the core's answer. */
  flush(): Promise<void>;
  /** Puts the caret in the note: at its end, or its start. */
  focus(where?: 'start' | 'end'): void;
}

// --- what a key does to a list ----------------------------------------------------

interface Edit {
  value: string;
  pos: number;
}

const lineAt = (value: string, from: number, to = from) => {
  const start = value.lastIndexOf('\n', from - 1) + 1;
  const end = value.indexOf('\n', to);
  return { start, end: end < 0 ? value.length : end };
};

/** Return in a list item: the next item, or the list's end when the item is empty. Null where Return is only a new line. */
export function continueList(value: string, from: number, to: number): Edit | null {
  if (value.slice(from, to).includes('\n')) return null;
  const { start, end } = lineAt(value, from, to);
  const line = value.slice(start, end);
  const m = LIST_ITEM.exec(line);
  if (!m) return null;
  const [, indent, mark, space, box, rest] = m;
  if (from - start < line.length - rest.length) return null;
  if (rest.trim() === '') {
    // An empty item ends the list: its marker goes, and at the list's end a blank line starts the next block.
    const before = value.slice(0, start);
    const after = value.slice(end);
    if (after === '' && before !== '') return { value: `${before}\n`, pos: before.length + 1 };
    return { value: before + after, pos: start };
  }
  const number = /^(\d+)(.)$/.exec(mark);
  const next = number ? `${Number(number[1]) + 1}${number[2]}` : mark;
  const head = `\n${indent}${next}${space}${box !== undefined ? '[ ] ' : ''}`;
  return { value: value.slice(0, from) + head + value.slice(to), pos: from + head.length };
}

/** Tab and ⇧Tab on a list item: in or out one step. Null on any other line. */
export function indentItem(value: string, at: number, out: boolean): Edit | null {
  const { start, end } = lineAt(value, at);
  const line = value.slice(start, end);
  if (!LIST_ITEM.test(line)) return null;
  if (!out) return { value: value.slice(0, start) + INDENT + value.slice(start), pos: at + INDENT.length };
  const lead = /^(\t| {1,4})/.exec(line)?.[0] ?? '';
  return { value: value.slice(0, start) + value.slice(start + lead.length), pos: Math.max(start, at - lead.length) };
}

// --- where a click falls in a block's text ----------------------------------------------

function pointAt(x: number, y: number): { node: Node; offset: number } | null {
  const d = document as Document & {
    caretPositionFromPoint?(x: number, y: number): { offsetNode: Node; offset: number } | null;
    caretRangeFromPoint?(x: number, y: number): Range | null;
  };
  const position = d.caretPositionFromPoint?.(x, y);
  if (position) return { node: position.offsetNode, offset: position.offset };
  const range = d.caretRangeFromPoint?.(x, y);
  return range ? { node: range.startContainer, offset: range.startOffset } : null;
}

/** The place in a block's text nearest a click on it as drawn: the words clicked, found again as written, else the end of their line. */
function clickedOffset(text: string, e: { target: EventTarget; clientX: number; clientY: number }): number {
  const drawn = (e.target as HTMLElement).closest<HTMLElement>('[data-line]');
  const rows = text.split('\n');
  const line = drawn ? Number(drawn.dataset.line) : Number.NaN;
  if (!drawn || !Number.isInteger(line) || line >= rows.length) return text.length;
  const start = rows.slice(0, line).reduce((n, r) => n + r.length + 1, 0);
  const point = pointAt(e.clientX, e.clientY);
  if (point && point.node.nodeType === Node.TEXT_NODE && drawn.contains(point.node)) {
    const words = point.node.textContent ?? '';
    const found = words ? text.indexOf(words, start) : -1;
    if (found >= 0) return found + Math.min(point.offset, words.length);
  }
  return start + rows[line].length;
}

// --- what `[[` offers ---------------------------------------------------------------------

interface Offer {
  kind: 'note' | 'course' | 'task' | 'new';
  name: string;
}

interface Menu {
  /** Where the name being typed starts, just after `[[`. */
  from: number;
  query: string;
  row: number;
  left: number;
  top: number;
}

const OFFERED = 8;

function useOffers(noteId: Id, query: string | null): Offer[] {
  const { snap, idx } = useHeat();
  return useMemo(() => {
    if (query === null || !snap) return [];
    const index = snap.notes?.index ?? {};
    const all: Offer[] = [
      ...(snap.notes?.order ?? [])
        .filter((id) => id !== noteId && index[id]?.title)
        .map((id): Offer => ({ kind: 'note', name: index[id].title })),
      ...snap.records.course.map((c): Offer => ({ kind: 'course', name: c.code })),
      ...snap.derived.lists.allOpen.flatMap((id): Offer[] => {
        const task = idx.task.get(id);
        return task ? [{ kind: 'task', name: task.title }] : [];
      }),
    ];
    const q = query.trim().toLowerCase();
    const has = all.filter((o) => o.name.toLowerCase().includes(q));
    // What starts with the words typed comes first; within that, notes, then courses, then tasks, as listed.
    const rows = [...has.filter((o) => o.name.toLowerCase().startsWith(q)), ...has.filter((o) => !o.name.toLowerCase().startsWith(q))];
    const exact = all.some((o) => o.name.toLowerCase() === q);
    return [...rows.slice(0, OFFERED), ...(q && !exact ? [{ kind: 'new' as const, name: query.trim() }] : [])];
  }, [snap, idx, noteId, query]);
}

// --- the editor ---------------------------------------------------------------------------

/** A block as formatted markdown. Drawn again only when its own text, its place or what a link finds changes. */
const Drawn = memo(function Drawn({ text, base, hooks }: { text: string; base: number; hooks: Hooks }) {
  return <>{renderMarkdown(text, hooks, base)}</>;
});

/** The block being written: a field as tall as its text. */
function Field({ field, value, ...rest }: { field: RefObject<HTMLTextAreaElement | null>; value: string } & React.ComponentProps<'textarea'>) {
  useLayoutEffect(() => {
    const el = field.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }, [field, value]);
  return <textarea ref={field} rows={1} value={value} {...rest} />;
}

const trimmed = (text: string) => text.replace(/\n+$/, '');

export const Editor = forwardRef<EditorHandle, { note: Note; hooks: Hooks }>(function Editor({ note, hooks }, ref) {
  const { client, act } = useHeat();
  const keys = useRef(0);
  const fresh = (b: Block): Doc => ({ key: ++keys.current, text: b.text, gap: b.gap });
  const [blocks, setBlocks] = useState<Doc[]>(() => splitBlocks(note.markdown).map(fresh));
  const [editing, setEditing] = useState<number | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);

  // What the handlers read: the same as the state, there before the next draw.
  const doc = useRef(blocks);
  const at = useRef<number | null>(null);
  const tail = useRef(tailOf(note.markdown));
  const field = useRef<HTMLTextAreaElement>(null);
  /** Where the caret goes once the block's field is drawn. */
  const want = useRef<{ key: number; pos: number } | null>(null);
  const listId = useId();

  const put = (next: Doc[], edit: number | null = at.current, pos?: number) => {
    doc.current = next;
    at.current = edit;
    if (edit !== null && pos !== undefined) want.current = { key: edit, pos };
    setBlocks(next);
    setEditing(edit);
  };
  useLayoutEffect(() => {
    const w = want.current;
    const el = field.current;
    if (!w || !el || at.current !== w.key) return;
    want.current = null;
    el.focus();
    el.setSelectionRange(w.pos, w.pos);
  });

  // --- saving ------------------------------------------------------------------------

  /** What the core holds, as far as this editor knows. */
  const base = useRef(canon(note.markdown));
  const sent = useRef<{ text: string; at: number }[]>([]);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const again = useRef<ReturnType<typeof setTimeout>>(undefined);
  const saving = useRef<Promise<void>>(Promise.resolve());
  const noteId = note.id;

  const flush = useCallback(() => {
    clearTimeout(timer.current);
    // One save at a time, in the order they were typed.
    saving.current = saving.current.then(async () => {
      const text = joinBlocks(doc.current, tail.current);
      if (text === base.current) return;
      sent.current = [...sent.current.slice(-7), { text, at: Date.now() }];
      if (await act(client.notes.save(noteId, { markdown: text }))) base.current = text;
    });
    return saving.current;
  }, [act, client, noteId]);
  const schedule = () => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => void flush(), SAVE_AFTER_MS);
  };
  useEffect(
    () => () => {
      clearTimeout(again.current);
      void flush();
    },
    [flush],
  );

  // The core's text is taken when it isn't this editor's own and nothing typed waits to be saved.
  const theirs = useRef(note.markdown);
  theirs.current = note.markdown;
  const sync = useCallback(() => {
    const incoming = canon(theirs.current);
    if (incoming === base.current) return;
    const mine = joinBlocks(doc.current, tail.current);
    if (incoming === mine) {
      base.current = incoming;
      return;
    }
    // An older save of this editor's, arriving late: not news. Look again once it has settled.
    if (sent.current.some((s) => s.text === incoming && Date.now() - s.at < ECHO_MS)) {
      clearTimeout(again.current);
      again.current = setTimeout(sync, ECHO_MS);
      return;
    }
    if (mine !== base.current) return;
    const was = doc.current;
    const next = splitBlocks(theirs.current).map((b, i): Doc => ({ key: was[i]?.key ?? ++keys.current, text: b.text, gap: b.gap }));
    base.current = incoming;
    tail.current = tailOf(theirs.current);
    doc.current = next;
    if (at.current !== null && !next.some((b) => b.key === at.current)) at.current = null;
    setBlocks(next);
    setEditing(at.current);
  }, []);
  useEffect(sync, [note.markdown, sync]);

  // --- moving between blocks -----------------------------------------------------------

  /** Puts the caret in a block, or in none. A block left with nothing in it is gone. */
  const go = (to: number | null, pos: number | 'end' = 'end') => {
    let next = doc.current;
    const from = at.current;
    if (from !== null && from !== to) {
      const left = next.find((b) => b.key === from);
      if (left && left.text.trim() === '') {
        // The blank lines the note starts with stay with whichever block is first.
        const wasFirst = next[0].key === from;
        next = next.filter((b) => b.key !== from);
        if (wasFirst && next.length > 0) next = [{ ...next[0], gap: left.gap }, ...next.slice(1)];
      } else if (left && left.text !== trimmed(left.text)) {
        next = next.map((b) => (b.key === from ? { ...b, text: trimmed(b.text) } : b));
      }
    }
    const target = to === null ? undefined : next.find((b) => b.key === to);
    put(next, target?.key ?? null, target ? (pos === 'end' ? target.text.length : pos) : undefined);
  };

  const startNew = () => {
    const last = doc.current.at(-1);
    if (last && last.text.trim() === '') return go(last.key, 0);
    const made = fresh({ text: '', gap: doc.current.length === 0 ? 0 : 1 });
    doc.current = [...doc.current, made];
    go(made.key, 0);
  };

  /**
   * What the block's field holds now. A blank line in it makes it two blocks, and one at its end starts
   * the block after; the caret stays with the words it was among. `place` moves the caret even when the
   * block is still one, for an edit a key made rather than the field.
   */
  const typed = (key: number, value: string, pos: number, place = false) => {
    const i = doc.current.findIndex((b) => b.key === key);
    if (i < 0) return;
    const was = doc.current[i];
    const parts = splitBlocks(value);
    const ended = parts.length > 0 && !inOpenFence(value) && /\n[ \t]*\n[ \t]*$/.test(value);
    if (parts.length <= 1 && !ended) {
      // Still one block. Blank lines before its first word are nothing, and the caret keeps its place among the words.
      const dropped = parts.length === 1 ? value.split('\n').slice(0, parts[0].line).join('\n').length + (parts[0].line > 0 ? 1 : 0) : 0;
      const text = parts.length === 0 ? value.replace(/\n/g, '') : value.slice(dropped);
      put(
        doc.current.map((b) => (b.key === key ? { ...b, text } : b)),
        key,
        place || text !== value ? Math.max(0, Math.min(text.length, pos - dropped)) : undefined,
      );
    } else {
      const rows = value.split('\n');
      const startOf = (line: number) => rows.slice(0, line).reduce((n, r) => n + r.length + 1, 0);
      const made = parts.map((p, n): Doc => (n === 0 ? { ...was, text: p.text } : fresh(p)));
      let target = made[0];
      let inside = 0;
      parts.forEach((p, n) => {
        if (pos < startOf(p.line)) return;
        target = made[n];
        inside = Math.min(pos - startOf(p.line), p.text.length);
      });
      if (ended) {
        const last = parts.at(-1)!;
        made.push(fresh({ text: '', gap: 1 }));
        if (pos > startOf(last.line) + last.text.length) {
          target = made.at(-1)!;
          inside = 0;
        }
      }
      put([...doc.current.slice(0, i), ...made, ...doc.current.slice(i + 1)], target.key, inside);
    }
    schedule();
  };

  /** ⌫ at a block's start: it goes on from the end of the one before, a line under it. */
  const join = (i: number) => {
    const before = doc.current[i - 1];
    const here = doc.current[i];
    if (here.text.trim() === '') return go(before.key, 'end');
    const head = trimmed(before.text);
    const merged: Doc = { ...before, text: `${head}\n${here.text}` };
    put(
      doc.current.flatMap((b) => (b.key === here.key ? [] : [b.key === before.key ? merged : b])),
      merged.key,
      head.length + 1,
    );
    schedule();
  };

  // --- the list `[[` opens ------------------------------------------------------------------

  const rows = useOffers(note.id, menu ? menu.query : null);
  const row = menu ? Math.min(menu.row, Math.max(0, rows.length - 1)) : 0;
  /** A list Esc closed stays closed for the link it was opened on. */
  const shut = useRef<number | null>(null);

  const look = (el: HTMLTextAreaElement) => {
    const pos = el.selectionStart;
    const m = el.selectionEnd === pos ? /\[\[([^[\]\n|#]*)$/.exec(el.value.slice(0, pos)) : null;
    if (!m) {
      shut.current = null;
      return setMenu(null);
    }
    const from = pos - m[1].length;
    if (shut.current === from) return;
    const point = caretPoint(el, from);
    setMenu((was) => ({ from, query: m[1], row: was && was.query === m[1] ? was.row : 0, left: point.left, top: point.top }));
  };

  const pick = (offer: Offer | undefined) => {
    const el = field.current;
    if (!el || !menu || !offer || at.current === null) return;
    const pos = el.selectionStart;
    const closed = el.value.slice(pos).startsWith(']]');
    typed(at.current, `${el.value.slice(0, menu.from)}${offer.name}${closed ? '' : ']]'}${el.value.slice(pos)}`, menu.from + offer.name.length + 2, true);
    // The link is written: its list stays shut, whatever the field says of its caret before it is drawn again.
    shut.current = menu.from;
    setMenu(null);
    // A name nothing has yet becomes a note, so the link has somewhere to go.
    if (offer.kind === 'new') void act(client.notes.create({ title: offer.name, ifMissing: true }));
  };

  // --- keys ---------------------------------------------------------------------------------

  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>, b: Doc) => {
    // A key that is part of writing a kana or a kanji is the input method's.
    if (e.nativeEvent.isComposing) return;
    const el = e.currentTarget;
    const { value, selectionStart: from, selectionEnd: to } = el;
    const bare = !e.metaKey && !e.ctrlKey && !e.altKey;
    const take = (edit: Edit | null) => {
      if (!edit) return false;
      e.preventDefault();
      typed(b.key, edit.value, edit.pos, true);
      return true;
    };

    if (menu && bare) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const by = e.key === 'ArrowDown' ? 1 : -1;
        return setMenu({ ...menu, row: (row + by + rows.length) % Math.max(1, rows.length) });
      }
      if ((e.key === 'Enter' || e.key === 'Tab') && !e.shiftKey && rows.length > 0) {
        e.preventDefault();
        return pick(rows[row]);
      }
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      return void escape();
    }
    if (!bare) return;

    const i = doc.current.findIndex((x) => x.key === b.key);
    if (from === to && !e.shiftKey) {
      if (e.key === 'ArrowUp' && i > 0 && !value.slice(0, from).includes('\n')) {
        e.preventDefault();
        return go(doc.current[i - 1].key, 'end');
      }
      if (e.key === 'ArrowDown' && i < doc.current.length - 1 && !value.slice(from).includes('\n')) {
        e.preventDefault();
        return go(doc.current[i + 1].key, 0);
      }
      if (e.key === 'Backspace' && from === 0 && i > 0) {
        e.preventDefault();
        return join(i);
      }
    }
    if (e.key === 'Enter' && !e.shiftKey) take(continueList(value, from, to));
    if (e.key === 'Tab') take(indentItem(value, from, e.shiftKey));
  };

  const escape = () => {
    if (menu) {
      shut.current = menu.from;
      setMenu(null);
      return true;
    }
    if (at.current === null) return false;
    field.current?.blur();
    // Without a field to leave (it never took the keyboard), the block is left all the same.
    if (at.current !== null) go(null);
    return true;
  };

  // A click outside takes the keyboard on the way down, and what it lands on must not move before the
  // click is over: the block is left once the button is up.
  const down = useRef(false);
  const later = useRef<(() => void) | null>(null);
  useEffect(() => {
    const press = () => void (down.current = true);
    const release = () => {
      down.current = false;
      const run = later.current;
      later.current = null;
      if (run) setTimeout(run, 0);
    };
    window.addEventListener('mousedown', press, true);
    window.addEventListener('mouseup', release, true);
    return () => {
      window.removeEventListener('mousedown', press, true);
      window.removeEventListener('mouseup', release, true);
    };
  }, []);

  const onBlur = (b: Doc) => {
    // The caret went to another block, and this field went with it.
    if (at.current !== b.key) return;
    setMenu(null);
    // The window lost the keyboard, not the block: it is still being written when the window is back.
    if (!document.hasFocus()) return void flush();
    const leave = () => {
      if (at.current === b.key && document.activeElement !== field.current) go(null);
      void flush();
    };
    if (down.current) later.current = leave;
    else leave();
  };

  const onClick = (e: MouseEvent, b: Doc) => {
    // A link, a checkbox or an image keeps its own click, and words being selected are being copied.
    if ((e.target as HTMLElement).closest('a, button, input, select')) return;
    if (window.getSelection()?.isCollapsed === false) return;
    go(b.key, clickedOffset(b.text, e));
  };

  useImperativeHandle(ref, () => ({
    escape,
    flush,
    focus(where = 'end') {
      const target = where === 'start' ? doc.current[0] : doc.current.at(-1);
      if (!target) return startNew();
      go(target.key, where === 'start' ? 0 : 'end');
    },
  }));

  const starts = firstLines(blocks);
  return (
    <div className="notes-editor">
      {blocks.map((b, i) =>
        b.key === editing ? (
          <div key={b.key} className="notes-writing">
            <Field
              field={field}
              className="notes-field"
              aria-label="Note text"
              aria-autocomplete="list"
              aria-haspopup="listbox"
              aria-controls={menu ? listId : undefined}
              aria-activedescendant={menu && rows.length > 0 ? `${listId}-${row}` : undefined}
              spellCheck
              value={b.text}
              onChange={(e) => {
                typed(b.key, e.target.value, e.target.selectionStart);
                look(e.target);
              }}
              onSelect={(e) => look(e.currentTarget)}
              onKeyDown={(e) => onKey(e, b)}
              onBlur={() => onBlur(b)}
            />
            {menu && rows.length > 0 && (
              <ul id={listId} role="listbox" aria-label="Link to" className="notes-links" style={{ left: menu.left, top: menu.top }}>
                {rows.map((o, n) => (
                  <li
                    key={`${o.kind}-${o.name}`}
                    id={`${listId}-${n}`}
                    role="option"
                    className="notes-link-row"
                    data-dense
                    aria-selected={n === row}
                    // The field keeps the keyboard while a row is clicked.
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => pick(o)}
                  >
                    {o.kind === 'new' ? (
                      `New note “${o.name}”`
                    ) : (
                      <>
                        <span className="notes-link-name">{o.name}</span>
                        {o.kind !== 'note' && <span className="heat-tag">{o.kind}</span>}
                      </>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>
        ) : b.text.trim() === '' ? null : (
          // A click is the mouse's way in; the keyboard's is Return on the note, and the button under the last block.
          <div key={b.key} className="notes-block" onClick={(e) => onClick(e, b)}>
            <Drawn text={b.text} base={starts[i]} hooks={hooks} />
          </div>
        ),
      )}
      <button type="button" className="notes-editor-end" data-dense aria-label="Write at the end of the note" onClick={startNew}>
        {blocks.every((b) => b.text.trim() === '') && editing === null ? 'Write here. Markdown and [[links]] welcome.' : ''}
      </button>
    </div>
  );
});
