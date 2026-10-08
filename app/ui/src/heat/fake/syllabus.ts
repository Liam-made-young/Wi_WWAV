// The fake core's syllabus import (docs/HEAT.md). A syllabus is read into a
// draft: the course it names, its weights, its task types, and the tasks and
// dates it gives. A draft writes nothing. `heat.syllabus.accept` applies all
// of it as one journal entry, so one ⌘Z takes it back; `heat.syllabus.discard`
// drops it. Drafts ride in the snapshot (`syllabus.drafts`), so the preview is
// there after a relaunch and when Claude finishes reading in the background.
//
// The real core has Claude read the PDF. Here a path's draft answers
// `reading` and then turns into a canned one, and the JSON form is read as
// written:
//
//   { fileName?, pages?, course: { code, name, term? },
//     weights: [{ category, percent, dropLowest? }],
//     types:   [{ name, patterns?, estMin?, difficulty?, category? }],
//     tasks:   [{ title, type?, due?, estMin? }] }

import type { Course, GradeCategory, Id, SyllabusDraft, Task, Term, TypeDef } from '../client';
import { courseLabel, plural } from '../fmt';
import * as copy from '../model/copy';
import { derive, type Fake, refuse, register } from './core';
import { applyTypes, DEFAULTS, fill, levelsOf, matchTitle, OTHER } from './types';

export interface SyllabusJson {
  fileName?: string;
  pages?: number;
  course?: { code?: string; name?: string; term?: string };
  weights?: { category: string; percent: number; dropLowest?: number | null }[];
  types?: (Partial<TypeDef> & { name: string })[];
  tasks?: { title: string; type?: string; due?: number | null; estMin?: number }[];
}

const drafts = new WeakMap<Fake, SyllabusDraft[]>();
const held = new WeakSet<Fake>();

export function draftsOf(fake: Fake): SyllabusDraft[] {
  let list = drafts.get(fake);
  if (!list) drafts.set(fake, (list = []));
  return list;
}

derive((snap, fake) => {
  snap.syllabus = { drafts: draftsOf(fake) };
});

const squashed = (text: string) => text.replace(/\s+/g, '').toLowerCase();
const pct = (x: number) => String(Math.round(x * 10) / 10);

/** The sentence for weights that don't add to 100, in the words a saved course's get. */
function weightsFlag(total: number): string | null {
  const t = Math.round(total * 10) / 10;
  if (t === 100) return null;
  return t < 100 ? copy.grades.weightsShort(pct(t), pct(100 - t)) : copy.grades.weightsOver(pct(t), pct(t - 100));
}

/** A syllabus as written out, made into the draft a person previews. */
function read(fake: Fake, json: SyllabusJson, courseId: Id | null, base: Pick<SyllabusDraft, 'id' | 'createdAt'>): SyllabusDraft {
  const fileName = json.fileName ?? 'syllabus.json';
  const pages = json.pages ?? 1;
  const named = json.course?.code?.trim() ?? '';
  const target =
    (courseId ? fake.store.course.get(courseId) : undefined) ??
    (named ? [...fake.store.course.values()].find((c) => squashed(c.code) === squashed(named)) : undefined);
  const code = named || target?.code || '';
  const draft: SyllabusDraft = { ...base, courseId: target?.id ?? null, fileName, pages, state: 'ready' };
  if (!code) return { ...draft, state: 'failed', error: `${fileName} doesn’t say which course it is for.` };

  const name = json.course?.name?.trim() || target?.name || '';
  const terms = [...fake.store.term.values()];
  const term = json.course?.term?.trim() || terms.find((t) => t.id === target?.termId)?.name || terms.at(-1)?.name || '';

  const weights = (json.weights ?? []).map((w) => ({
    category: String(w.category).trim(),
    percent: Number(w.percent) || 0,
    dropLowest: w.dropLowest ?? null,
  }));
  const weightsTotal = Math.round(weights.reduce((sum, w) => sum + w.percent, 0) * 10) / 10;
  const types: TypeDef[] = (json.types ?? []).map((t) => ({
    name: t.name.trim(),
    patterns: t.patterns ?? [t.name.trim().toLowerCase()],
    estMin: t.estMin ?? null,
    difficulty: t.difficulty ?? null,
    ...(t.category ? { category: t.category } : {}),
  }));

  // A task the course already holds under the same title is a date change, or nothing; any other is new.
  const own = target ? [...fake.store.task.values()].filter((t) => t.courseId === target.id) : [];
  const levels = { home: { level: 'course' as const, defs: types }, space: [], global: DEFAULTS };
  const newTasks: NonNullable<SyllabusDraft['newTasks']> = [];
  const dateChanges: NonNullable<SyllabusDraft['dateChanges']> = [];
  for (const t of json.tasks ?? []) {
    const was = own.find((o) => squashed(o.title) === squashed(t.title));
    const due = t.due ?? null;
    if (was) {
      if (due !== null && was.due !== due) dateChanges.push({ taskId: was.id, title: was.title, from: was.due, to: due });
      continue;
    }
    const type = t.type ?? (matchTitle(t.title, types) ?? matchTitle(t.title, DEFAULTS))?.name ?? OTHER;
    newTasks.push({ title: t.title, type, due, estMin: t.estMin ?? fill(type, t.title, levels).estMin });
  }

  const label = courseLabel({ code, name });
  const parts = [
    label,
    ...(weights.length > 0 ? [weights.map((w) => `${w.category} ${pct(w.percent)}%`).join(', ')] : []),
    plural(types.length, 'type'),
    `${plural(newTasks.length, 'new task')}, ${plural(dateChanges.length, 'date')} changed`,
  ];
  return {
    ...draft,
    course: { code, name, term, label, isNew: !target },
    weights,
    weightsTotal,
    weightsFlag: weights.length > 0 ? weightsFlag(weightsTotal) : null,
    types,
    newTasks,
    dateChanges,
    line: `${parts.join('. ')}.`,
  };
}

/** What a PDF "reads" as here: the same few weights and types for any course. */
const canned = (fileName: string): SyllabusJson => ({
  fileName,
  pages: 6,
  weights: [
    { category: 'Labs', percent: 40 },
    { category: 'Quizzes', percent: 20, dropLowest: 1 },
    { category: 'Final', percent: 40 },
  ],
  types: [
    { name: 'Lab', patterns: ['lab', 'labs'], estMin: 150, difficulty: 3, category: 'Labs' },
    { name: 'Quiz', patterns: ['quiz', 'quizzes'], estMin: 25, difficulty: 2, category: 'Quizzes' },
  ],
  tasks: [{ title: 'Lab 1' }, { title: 'Quiz 1' }],
});

/** Keeps a PDF's draft at `reading` until `finishReading` is called, so a test can look at it. */
export function holdReading(fake: Fake) {
  held.add(fake);
}

/** The reading of a PDF ends: its draft turns ready, or fails with `error`. */
export function finishReading(fake: Fake, draftId: Id, error?: string) {
  const list = draftsOf(fake);
  const at = list.findIndex((d) => d.id === draftId && d.state === 'reading');
  if (at < 0) return;
  const was = list[at];
  list[at] = error
    ? { ...was, state: 'failed', error }
    : read(fake, canned(was.fileName), was.courseId, { id: was.id, createdAt: was.createdAt });
  fake.emit([]);
}

register('heat.course.importSyllabus', (args, fake) => {
  const courseId = (args.courseId as Id | undefined) ?? null;
  if (courseId && !fake.store.course.has(courseId)) refuse('No course has that id.');
  const base = { id: fake.newId(), createdAt: fake.now };
  let draft: SyllabusDraft;
  if (typeof args.path === 'string') {
    const fileName = args.path.split(/[\\/]/).pop() ?? args.path;
    if (!/\.pdf$/i.test(fileName)) refuse('Learn reads a syllabus from a PDF.');
    draft = { ...base, courseId, fileName, pages: 0, state: 'reading' };
    // The answer comes first, then the reading ends, as it does when Claude reads the file.
    if (!held.has(fake)) setTimeout(() => finishReading(fake, draft.id), 0);
  } else if (args.json && typeof args.json === 'object') {
    draft = read(fake, args.json as SyllabusJson, courseId, base);
  } else refuse('A syllabus comes as a PDF’s path or as JSON.');
  draftsOf(fake).push(draft);
  fake.emit([]);
  return { draft };
});

register('heat.syllabus.discard', (args, fake) => {
  const list = draftsOf(fake);
  const at = list.findIndex((d) => d.id === args.draftId);
  if (at < 0) refuse('No syllabus draft has that id.');
  list.splice(at, 1);
  fake.emit([]);
  return {};
});

register('heat.syllabus.accept', (args, fake) => {
  const list = draftsOf(fake);
  const draft = list.find((d) => d.id === args.draftId);
  if (!draft) refuse('No syllabus draft has that id.');
  if (draft.state !== 'ready' || !draft.course) refuse('This syllabus isn’t read yet.');
  const from = draft.course;
  let course!: Course;
  const counts = { newTasks: 0, dateChanges: 0, retimed: 0 };

  const { undo } = fake.write('import syllabus', ['term', 'course', 'task'], () => {
    const was =
      (draft.courseId ? fake.store.course.get(draft.courseId) : undefined) ??
      [...fake.store.course.values()].find((c) => squashed(c.code) === squashed(from.code));
    let termId = was?.termId;
    if (!termId) {
      const terms = [...fake.store.term.values()];
      let term: Term | undefined = terms.find((t) => t.name === from.term) ?? (from.term ? undefined : terms.at(-1));
      if (!term) {
        term = { id: fake.newId(), name: from.term || 'Term' };
        fake.store.term.set(term.id, term);
      }
      termId = term.id;
    }
    // A category the course already has keeps its id and keywords, so its grades stay filed.
    const categories: GradeCategory[] = (draft.weights ?? []).map((w) => {
      const had = was?.categories.find((c) => squashed(c.name) === squashed(w.category));
      const { dropLowest: _was, ...kept } = had ?? { id: `cat-${fake.newId()}`, name: w.category, weight: 0, keywords: [] };
      return { ...kept, weight: w.percent, ...(w.dropLowest ? { dropLowest: w.dropLowest } : {}) };
    });
    course = {
      ...(was ?? { id: fake.newId(), termId, code: from.code, name: from.name, categories: [], notes: '', public: false }),
      ...(categories.length > 0 ? { categories } : {}),
      types: draft.types ?? [],
      status: 'confirmed',
      syllabusSource: { name: draft.fileName, pages: draft.pages, importedAt: fake.now },
    };
    fake.store.course.set(course.id, course);

    const spaces = [...fake.store.space.values()];
    const spaceId = (spaces.find((s) => s.groupKind === 'course') ?? spaces[0])?.id ?? '';
    const touched = new Set<Id>();
    for (const t of draft.newTasks ?? []) {
      const blank: Task = {
        id: fake.newId(),
        spaceId,
        title: t.title,
        type: t.type,
        typeBy: 'rule',
        courseId: course.id,
        due: t.due,
        difficulty: 0,
        estMin: null,
        adjustMin: 0,
        notes: '',
        done: false,
        doneAt: null,
        source: 'you',
        public: false,
      };
      const numbers = fill(t.type, t.title, levelsOf(fake, blank));
      const by = numbers.from ? 'type' : 'default';
      fake.store.task.set(blank.id, {
        ...blank,
        difficulty: numbers.difficulty,
        difficultyBy: by,
        estMin: t.estMin,
        estBy: by,
      });
      touched.add(blank.id);
      counts.newTasks += 1;
    }
    for (const change of draft.dateChanges ?? []) {
      const task = fake.store.task.get(change.taskId);
      if (!task) continue;
      fake.store.task.set(task.id, { ...task, due: change.to });
      counts.dateChanges += 1;
    }
    // The course's types now apply to the tasks it already had, where nothing was set by hand.
    for (const [id, task] of fake.store.task) {
      if (task.courseId !== course.id || task.done || touched.has(id)) continue;
      const next = applyTypes(fake, task);
      if (JSON.stringify(next) === JSON.stringify(task)) continue;
      fake.store.task.set(id, next);
      counts.retimed += 1;
    }
    // The draft is spent before the views hear of the change.
    list.splice(list.indexOf(draft), 1);
  });
  return { course, counts, undo };
});
