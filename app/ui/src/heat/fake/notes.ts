// The fake core's notes (docs/NOTES.md): making, renaming and filing a
// note, a checkbox made a task, search, the capture inbox, Claude's
// suggestions and the quiet notices, with `notes` and `notices` in the
// snapshot. A stand-in, not a port: there is no folder behind it, a page
// that is "captured" is a note with its image on top and nothing read, and
// what the real core asks Claude is whatever a test hands it here:
//
//   addNotice(fake, { kind: 'filed', text: 'Filed to JPN 201 · Oct 7', noteId })
//   suggest(fake, noteId, { tasks: [{ title: 'Study for the quiz' }], terms: ['te-form'] })
//   setCapture(fake, { doing: 'Reading IMG_2211.jpg' })
//   setAttachment(fake, 'attachments/page.jpg', 'data:image/png;base64,…')

import { atMinute } from '../../shared/time/zone';
import type { Id, Note, NoteIndex, NotesSnapshot, NoteSuggestion, Notice, Task } from '../client';
import { courseLabel } from '../fmt';
import { boxesIn, excerptOf, LIST_ITEM, linksIn, MARKER, plainLines, tagsIn, targetOf } from '../notes/text';
import { derive, type Fake, refuse, register } from './core';

/** What the fake keeps beyond the records. */
interface Extra {
  suggestions: Map<Id, NoteSuggestion>;
  notices: Notice[];
  capture: NotesSnapshot['capture'];
  /** An attachment's image by its path; null is one that can't be read. */
  attachments: Map<string, string | null>;
  /** What was shown in the Finder, in order. */
  revealed: string[];
  n: number;
}

const FOLDER = '/Users/you/Music/Wi_WWAV/Notes';
const extras = new WeakMap<Fake, Extra>();

function extra(fake: Fake): Extra {
  let e = extras.get(fake);
  if (!e) {
    e = {
      suggestions: new Map(),
      notices: [],
      capture: {
        folders: [
          '/Users/you/Library/Mobile Documents/com~apple~CloudDocs/Wi-WWAV Inbox',
          '/Users/you/Library/Mobile Documents/com~apple~CloudDocs/Shortcuts/Wi-WWAV Inbox',
        ],
        watching: true,
        waiting: 0,
        doing: null,
        claude: true,
        reader: 'ready',
      },
      attachments: new Map(),
      revealed: [],
      n: 0,
    };
    extras.set(fake, e);
  }
  return e;
}

// --- what a test hands the fake ----------------------------------------------------

/** A quiet notice, as the core would leave one. Answers its id. */
export function addNotice(fake: Fake, notice: Omit<Notice, 'id' | 'at'> & Partial<Pick<Notice, 'id' | 'at'>>): string {
  const e = extra(fake);
  e.n += 1;
  const made: Notice = { id: `${notice.kind}:${e.n}`, at: fake.now + e.n, ...notice };
  e.notices.push(made);
  fake.emit([]);
  return made.id;
}

/** What Claude suggested for a note: tasks that sound due, and key terms. */
export function suggest(fake: Fake, noteId: Id, suggestion: NoteSuggestion) {
  extra(fake).suggestions.set(noteId, suggestion);
  fake.emit([]);
}

export function setCapture(fake: Fake, set: Partial<NotesSnapshot['capture']>) {
  Object.assign(extra(fake).capture, set);
  fake.emit([]);
}

/** An attachment's image; null makes it one that can't be read. */
export function setAttachment(fake: Fake, path: string, dataUrl: string | null) {
  extra(fake).attachments.set(path, dataUrl);
}

/** What `heat.note.reveal` showed, in order. */
export const revealedBy = (fake: Fake) => extra(fake).revealed;

// --- what a note links to ------------------------------------------------------------

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();
const squashed = (text: string) => text.replace(/\s+/g, '').toLowerCase();
const notesOf = (fake: Fake) => [...fake.store.note.values()];
const titled = (fake: Fake, title: string) => notesOf(fake).find((n) => n.title !== undefined && same(n.title, title));
const courseCalled = (fake: Fake, code: string) =>
  [...fake.store.course.values()].find((c) => squashed(c.code) === squashed(code));

/** A link finds a note by its title, else a course by its code, else a task by its title. */
function resolve(fake: Fake, target: string): Pick<NoteIndex['links'][number], 'kind' | 'id'> {
  const note = titled(fake, target);
  if (note) return { kind: 'note', id: note.id };
  const course = courseCalled(fake, target);
  if (course) return { kind: 'course', id: course.id };
  const task = [...fake.store.task.values()].find((t) => same(t.title, target));
  return task ? { kind: 'task', id: task.id } : { kind: 'missing', id: null };
}

/** The space a course's work lives in: the first whose groups are courses. */
const courseSpace = (fake: Fake) => [...fake.store.space.values()].find((s) => s.groupKind === 'course');

function placeOf(fake: Fake, note: Note): { label: string | null; hue: number | null } {
  const course = note.courseId ? fake.store.course.get(note.courseId) : undefined;
  if (course) return { label: courseLabel(course), hue: courseSpace(fake)?.hue ?? null };
  const space = note.spaceId ? fake.store.space.get(note.spaceId) : undefined;
  return space ? { label: space.name, hue: space.hue } : { label: null, hue: null };
}

derive((snap, fake) => {
  const e = extra(fake);
  const notes = snap.records.note;
  const index: Record<Id, NoteIndex> = {};
  for (const note of notes) {
    index[note.id] = {
      title: note.title ?? '',
      excerpt: excerptOf(note.markdown),
      tags: tagsIn(note.markdown),
      links: linksIn(note.markdown).map((l) => ({ ...l, ...resolve(fake, l.target) })),
      backlinks: [],
      boxes: boxesIn(note.markdown).map((b) => {
        const task = b.taskId ? fake.store.task.get(b.taskId) : undefined;
        return task ? { ...b, done: task.done } : { ...b, taskId: null };
      }),
      updatedAt: note.updatedAt ?? null,
      ...placeOf(fake, note),
    };
  }
  for (const note of notes) {
    const rows = note.markdown.split('\n');
    for (const link of index[note.id].links) {
      if (link.kind !== 'note' || !link.id || link.id === note.id) continue;
      index[link.id].backlinks.push({ id: note.id, title: note.title ?? '', line: excerptOf(rows[link.line]) });
    }
  }

  // Newest first; of two written in the same moment, the one made later.
  const order = notes
    .map((n, at) => ({ id: n.id, when: n.updatedAt ?? n.createdAt ?? 0, at }))
    .sort((a, b) => b.when - a.when || b.at - a.at)
    .map((n) => n.id);
  const counts = new Map<string, number>();
  for (const note of notes) for (const tag of index[note.id].tags) counts.set(tag, (counts.get(tag) ?? 0) + 1);

  snap.notes = {
    folder: FOLDER,
    index,
    order,
    tags: [...counts]
      .map(([tag, count]) => ({ tag, count }))
      .sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag)),
    inbox: order.filter((id) => fake.store.note.get(id)?.inbox === true),
    suggestions: Object.fromEntries([...e.suggestions].filter(([id]) => fake.store.note.has(id))),
    daily: titled(fake, snap.date)?.id ?? null,
    capture: e.capture,
  };
  snap.notices = [...e.notices].sort((a, b) => b.at - a.at);
});

// --- making, saving and filing ---------------------------------------------------------

function noteOf(fake: Fake, id: unknown): Note {
  const note = fake.store.note.get(id as Id);
  if (!note) refuse('No note has that id.');
  return note;
}

/** "Untitled", then "Untitled 2": a title is a note's name, so no two share one. */
function freeTitle(fake: Fake): string {
  for (let n = 1; ; n++) {
    const title = n === 1 ? 'Untitled' : `Untitled ${n}`;
    if (!titled(fake, title)) return title;
  }
}

/** Where a command files a note: a course or a space, by id or by name. */
function placeFrom(fake: Fake, args: Record<string, unknown>): Pick<Note, 'courseId' | 'spaceId'> {
  if (typeof args.courseId === 'string' || typeof args.course === 'string') {
    const course =
      typeof args.courseId === 'string' ? fake.store.course.get(args.courseId) : courseCalled(fake, args.course as string);
    if (!course) refuse(`No course is called ${String(args.course ?? args.courseId)}.`);
    const space = courseSpace(fake);
    return { courseId: course.id, ...(space ? { spaceId: space.id } : {}) };
  }
  if (typeof args.spaceId === 'string' || typeof args.space === 'string') {
    const space =
      typeof args.spaceId === 'string'
        ? fake.store.space.get(args.spaceId)
        : [...fake.store.space.values()].find((s) => same(s.name, args.space as string));
    if (!space) refuse(`No space is called ${String(args.space ?? args.spaceId)}.`);
    return { spaceId: space.id };
  }
  return {};
}

function make(fake: Fake, title: string, markdown: string, more: Partial<Note> = {}): Note {
  return { id: fake.newId(), title, markdown, createdAt: fake.now, updatedAt: fake.now, public: false, ...more };
}

register('heat.note.create', (args, fake) => {
  const title = typeof args.title === 'string' && args.title.trim() ? args.title.trim() : freeTitle(fake);
  const there = titled(fake, title);
  if (there) {
    if (args.ifMissing === true) return { note: there, existed: true, undo: null };
    refuse(`A note is already called ${there.title}.`);
  }
  const note = make(fake, title, typeof args.markdown === 'string' ? args.markdown : '', placeFrom(fake, args));
  const { undo } = fake.write('add note', ['note'], () => fake.store.note.set(note.id, note));
  return { note, undo };
});

const escaped = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const targetsOf = (line: string) => [...line.matchAll(/\[\[([^[\]|]+)(?:\|[^[\]]+)?\]\]/g)].map((m) => targetOf(m[1]));

/** Every `[[was]]`, `[[was|shown]]` and `[[was#heading]]` outside code, pointed at `now`. */
function relink(markdown: string, was: string, now: string): string {
  const link = new RegExp(`\\[\\[\\s*${escaped(was)}\\s*([#|][^\\[\\]]*)?\\]\\]`, 'gi');
  const plain = plainLines(markdown);
  return markdown
    .split('\n')
    .map((line, i) =>
      targetsOf(plain[i]).some((t) => same(t, was)) ? line.replace(link, (_all, rest = '') => `[[${now}${rest}]]`) : line,
    )
    .join('\n');
}

register('heat.note.save', (args, fake) => {
  const was = noteOf(fake, args.id);
  const title = typeof args.title === 'string' ? args.title.trim() : undefined;
  const renaming = title !== undefined && title !== (was.title ?? '');
  if (renaming) {
    if (!title) refuse('Give the note a name first.');
    const other = titled(fake, title);
    if (other && other.id !== was.id) refuse(`A note is already called ${other.title}.`);
  }
  const markdown = typeof args.markdown === 'string' ? args.markdown : was.markdown;
  if (!renaming && markdown === was.markdown) return { note: was, renamed: 0, undo: null };

  let renamed = 0;
  const note: Note = { ...was, ...(renaming ? { title } : {}), markdown, updatedAt: fake.now };
  const { undo } = fake.write(renaming ? 'rename note' : 'edit note', ['note'], () => {
    // A title is a note's name everywhere: every link to it follows, in the same entry.
    if (renaming && was.title) {
      for (const other of notesOf(fake)) {
        const own = other.id === was.id;
        const text = relink(own ? note.markdown : other.markdown, was.title, title!);
        if (own) note.markdown = text;
        else if (text !== other.markdown) {
          fake.store.note.set(other.id, { ...other, markdown: text });
          renamed += 1;
        }
      }
    }
    fake.store.note.set(note.id, note);
  });
  return { note, renamed, undo };
});

register('heat.note.daily', (args, fake) => {
  const date = String(args.date ?? '');
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) refuse('A daily note is named for its day.');
  const was = titled(fake, date);
  if (typeof args.markdown !== 'string') return { note: was ?? null };
  if (was) {
    if (was.markdown === args.markdown) return { note: was, undo: null };
    const note = { ...was, markdown: args.markdown, updatedAt: fake.now };
    const { undo } = fake.write('edit note', ['note'], () => fake.store.note.set(note.id, note));
    return { note, undo };
  }
  // Nothing is made for an empty day.
  if (args.markdown.trim() === '') return { note: null };
  const note = make(fake, date, args.markdown);
  const { undo } = fake.write('add note', ['note'], () => fake.store.note.set(note.id, note));
  return { note, undo };
});

register('heat.note.search', (args, fake) => {
  const words = String(args.q ?? '')
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
  if (words.length === 0) return { hits: [] };
  const hits = notesOf(fake).flatMap((note) => {
    const title = (note.title ?? '').toLowerCase();
    const text = note.markdown.toLowerCase();
    if (!words.every((w) => title.includes(w) || text.includes(w))) return [];
    // The line that holds a word, or the note's first words when only its title does.
    const line = note.markdown.split('\n').find((l) => words.some((w) => l.toLowerCase().includes(w)));
    const inTitle = words.filter((w) => title.includes(w)).length;
    return [
      {
        id: note.id,
        title: note.title ?? '',
        snippet: excerptOf(line ?? note.markdown, 120),
        score: inTitle * 10 + words.filter((w) => text.includes(w)).length,
      },
    ];
  });
  hits.sort((a, b) => b.score - a.score || a.title.localeCompare(b.title));
  return { hits: hits.slice(0, typeof args.limit === 'number' ? args.limit : 20) };
});

register('heat.note.link', (args, fake) => {
  const was = noteOf(fake, args.id);
  const to = String(args.to ?? '').trim();
  if (!to) refuse('Say what to link to.');
  const fresh = resolve(fake, to).kind === 'missing' ? make(fake, to, '') : null;
  const text = was.markdown.replace(/\n+$/, '');
  const note = { ...was, markdown: `${text}${text ? '\n\n' : ''}[[${to}]]`, updatedAt: fake.now };
  const { undo } = fake.write('link note', ['note'], () => {
    if (fresh) fake.store.note.set(fresh.id, fresh);
    fake.store.note.set(note.id, note);
  });
  return { note, created: fresh, line: `Linked ${was.title ?? 'the note'} to ${to}.`, undo };
});

register('heat.note.file', (args, fake) => {
  const was = noteOf(fake, args.id);
  const place = args.none === true ? {} : placeFrom(fake, args);
  if (args.none !== true && !place.courseId && !place.spaceId) refuse('Say where to file it: a course, a space, or neither.');
  // Filing takes it out of the Notes inbox, wherever it goes.
  const note: Note = { ...was, updatedAt: fake.now };
  delete note.courseId;
  delete note.spaceId;
  delete note.inbox;
  Object.assign(note, place);
  const { undo } = fake.write('file note', ['note'], () => fake.store.note.set(note.id, note));
  const course = place.courseId ? fake.store.course.get(place.courseId) : undefined;
  const to = course?.code ?? (place.spaceId ? fake.store.space.get(place.spaceId)?.name : undefined);
  return { note, line: to ? `Filed to ${to}` : 'Filed to no course or space', undo };
});

// --- checkboxes ---------------------------------------------------------------------------

/** A checkbox line of a note, or a refusal. */
function boxAt(note: Note, line: unknown) {
  const box = boxesIn(note.markdown).find((b) => b.line === line);
  if (!box) refuse('That line isn’t a checkbox.');
  return box;
}

const withLine = (markdown: string, line: number, change: (text: string) => string) =>
  markdown
    .split('\n')
    .map((text, i) => (i === line ? change(text) : text))
    .join('\n');

/** A task in the note's space and course. */
function taskFor(fake: Fake, note: Note, title: string, more: Partial<Task> = {}): Task {
  const space =
    (note.spaceId ? fake.store.space.get(note.spaceId) : undefined) ?? courseSpace(fake) ?? [...fake.store.space.values()][0];
  return {
    id: fake.newId(),
    spaceId: space?.id ?? '',
    title,
    type: 'Other',
    ...(note.courseId ? { courseId: note.courseId } : {}),
    due: null,
    difficulty: 3,
    estMin: null,
    adjustMin: 0,
    notes: '',
    done: false,
    doneAt: null,
    source: 'you',
    public: false,
    ...more,
  };
}

register('heat.note.taskFromLine', (args, fake) => {
  const was = noteOf(fake, args.id);
  const box = boxAt(was, args.line);
  if (box.taskId && fake.store.task.has(box.taskId)) refuse('That checkbox is already a task.');
  const task = taskFor(fake, was, box.text.trim() || 'Untitled', {
    noteId: was.id,
    ...(box.done ? { done: true, doneAt: fake.now } : {}),
  });
  const note = {
    ...was,
    markdown: withLine(was.markdown, box.line, (text) => `${text.replace(MARKER, '').trimEnd()} ^t-${task.id}`),
    updatedAt: fake.now,
  };
  const { undo } = fake.write('task from note', ['note', 'task'], () => {
    fake.store.task.set(task.id, task);
    fake.store.note.set(note.id, note);
  });
  return { task, note, undo };
});

register('heat.note.toggleBox', (args, fake) => {
  const was = noteOf(fake, args.id);
  const box = boxAt(was, args.line);
  const done = args.done === true;
  const held = box.taskId ? fake.store.task.get(box.taskId) : undefined;
  const task = held ? { ...held, done, doneAt: done ? fake.now : null } : null;
  const note = {
    ...was,
    markdown: withLine(was.markdown, box.line, (text) =>
      text.replace(
        LIST_ITEM,
        (_all, indent: string, mark: string, space: string, _box, rest: string) =>
          `${indent}${mark}${space}[${done ? 'x' : ' '}] ${rest}`,
      ),
    ),
    updatedAt: fake.now,
  };
  const { undo } = fake.write(done ? 'check box' : 'uncheck box', ['note', 'task'], () => {
    fake.store.note.set(note.id, note);
    if (task) fake.store.task.set(task.id, task);
  });
  return { note, task, undo };
});

// --- what Claude suggested -------------------------------------------------------------

register('heat.note.suggestion.accept', (args, fake) => {
  const note = noteOf(fake, args.noteId);
  const suggestion = extra(fake).suggestions.get(note.id);
  const asked = suggestion?.tasks[args.index as number];
  if (!suggestion || !asked) refuse('That suggestion is no longer there.');
  if (asked.taskId && fake.store.task.has(asked.taskId)) refuse('That task was already added.');
  const task = taskFor(fake, note, asked.title, asked.due ? { due: atMinute(asked.due, 23 * 60 + 59, fake.zone) } : {});
  asked.taskId = task.id;
  const { undo } = fake.write('add task', ['task'], () => fake.store.task.set(task.id, task));
  return { task, undo };
});

register('heat.note.suggestion.dismiss', (args, fake) => {
  extra(fake).suggestions.delete(args.noteId as Id);
  fake.emit([]);
  return {};
});

// --- the folder -----------------------------------------------------------------------------

/** A grey page with the file's name on it, for an attachment nothing was handed for. */
const page = (name: string) =>
  `data:image/svg+xml;utf8,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320"><rect width="480" height="320" fill="#e4e4e4"/><text x="240" y="166" text-anchor="middle" font-family="sans-serif" font-size="18" fill="#4a4a4a">${name.replace(/[<>&]/g, '')}</text></svg>`,
  )}`;

register('heat.note.attachment', (args, fake) => {
  const path = String(args.path ?? '');
  const held = extra(fake).attachments.get(path);
  if (held === null || !path) refuse(`${path || 'That file'} isn’t in the notes folder.`);
  return { dataUrl: held ?? page(path.split('/').at(-1) ?? path) };
});

register('heat.note.reveal', (args, fake) => {
  const note = typeof args.id === 'string' ? noteOf(fake, args.id) : null;
  const path = note ? `${FOLDER}/${note.file ?? `${note.title ?? note.id}.md`}` : FOLDER;
  extra(fake).revealed.push(path);
  return { path };
});

register('heat.note.sync', () => ({ changed: 0 }));

// --- the capture inbox ------------------------------------------------------------------------

const MIME: Record<string, string> = {
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  heic: 'image/heic',
  pdf: 'application/pdf',
};

/** A page as the real core leaves one it could neither read nor place: its image on top, in the Notes inbox, and one notice. */
function capture(fake: Fake, name: string): Note {
  const stem = name.replace(/\.[^.]+$/, '') || 'Page';
  let title = stem;
  for (let n = 2; titled(fake, title); n++) title = `${stem} ${n}`;
  const path = `attachments/${name}`;
  const note = make(fake, title, `![](${/\s/.test(path) ? `<${path}>` : path})`, {
    inbox: true,
    capturedAt: fake.now,
    attachments: [path],
    source: 'capture',
  });
  const { txnId } = fake.write('capture', ['note'], () => fake.store.note.set(note.id, note));
  addNotice(fake, { kind: 'capture', text: `Added ${title} to the Notes inbox`, noteId: note.id, undo: { txnId } });
  return note;
}

const nameOf = (path: string) => path.split('/').at(-1) ?? path;

register('heat.capture.inbox.add', (args, fake) => {
  if (typeof args.name === 'string' && typeof args.base64 === 'string') {
    const ext = args.name.split('.').at(-1)?.toLowerCase() ?? '';
    extra(fake).attachments.set(`attachments/${args.name}`, `data:${MIME[ext] ?? 'image/png'};base64,${args.base64}`);
    capture(fake, args.name);
    return { added: 1 };
  }
  const paths = Array.isArray(args.paths) ? (args.paths as string[]) : [];
  if (paths.length === 0) refuse('There was nothing to add.');
  // The real core copies each into the inbox and reads it a moment later; here it is read at once.
  for (const path of paths) capture(fake, nameOf(path));
  return { added: paths.length };
});

register('heat.capture.process', (args, fake) => {
  const e = extra(fake);
  if (typeof args.path === 'string') {
    const note = capture(fake, nameOf(args.path));
    return args.wait === true
      ? { notes: [{ id: note.id, title: note.title, line: `Added ${note.title} to the Notes inbox` }] }
      : { started: true, waiting: e.capture.waiting };
  }
  const waiting = e.capture.waiting;
  e.capture.waiting = 0;
  fake.emit([]);
  return args.wait === true ? { notes: [] } : { started: waiting > 0, waiting };
});

register('heat.capture.shortcut', () => ({ path: '/Users/you/Music/Wi_WWAV/Notes/Send to Wi-WWAV.shortcut', inICloud: false }));

register('heat.capture.settings.set', (args, fake) => {
  extra(fake).capture.claude = args.claude === true;
  fake.emit([]);
  return {};
});

// --- notices ------------------------------------------------------------------------------------

register('heat.notice.dismiss', (args, fake) => {
  const e = extra(fake);
  e.notices = e.notices.filter((n) => n.id !== args.id);
  fake.emit([]);
  return {};
});
