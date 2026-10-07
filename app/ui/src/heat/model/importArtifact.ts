// Moving in (docs/SPEC.md 3.15): the Heat artifact's JSON export becomes the
// app's records. Each workspace maps to a space, and every id is kept (event
// ids, the em- and gp- hashes, the processed Gmail ids), so the first sync in
// the app finds nothing new instead of everything twice.
//
// The artifact has no export yet; step 0 is a "Download JSON" button on it.
// This is the format that button should write: the plain dump of the records
// 3.1 describes, one array per kind, instants as ISO 8601 strings with their
// offset ("Z" or "-04:00") and days as "YYYY-MM-DD". parseHeatExport checks
// every row against it and refuses the whole file if one row is off, so
// nothing is half-read.
//
//   {
//     "format": "heat-export", "version": 1,
//     "exportedAt": "2026-10-06T12:40:00.000Z",
//     "timeZone": "America/New_York",            // the browser's zone
//     "workspaces": [{ "key": "classes", "name": "Classes", "groupLabel": "Course",
//                      "types": ["Homework", …, "Other"], "persona": "…" }, …],
//     "tasks": [{ "id": "<event id> | em-<hash> | <own id>", "workspace": "classes",
//                 "title": "Grammar quiz 4", "group": "JPN 201" | null,
//                 "type": "Quiz", "due": "<ISO>" | null, "difficulty": 2,
//                 "estMin": 45 | null, "actualMin": 75 | null,   // "Time it took"
//                 "notes": "", "done": false, "doneAt": "<ISO>" | null,
//                 "source": "manual" | "calendar" | "gmail" }, …],
//     "milestones": [{ "id": "…", "workspace": "wwav", "title": "…",
//                      "date": "2026-10-20", "done": false, "order": 1 }, …],
//     "habits": [{ "id": "…", "title": "…", "log": { "2026-10-05": true } }, …],
//     "term": "Fall 2026",
//     "courses": [{ "code": "JPN 201", "name": "…",
//                   "categories": [{ "name": "Quizzes", "weight": 40,
//                                    "keywords": ["quiz", "kanji"] }, …],
//                   "scale": [{ "letter": "A", "min": 93 }, …],   // optional
//                   "sticky": "…" }, …],                          // optional
//     "grades": [{ "id": "<own id> | gp-<hash>", "course": "JPN 201",
//                  "title": "…", "category": "Quizzes" | null,
//                  "score": 18 | null, "outOf": 20, "dropped": false,
//                  "pending": false, "link": "…" }, …],         // link optional
//     "processedMailIds": ["18f2a…", …],                        // the last 400
//     "lastSyncAt": "<ISO>" | null
//   }

import * as copy from './copy';
import type {
  Course,
  Grade,
  Habit,
  Id,
  LetterStep,
  Milestone,
  Space,
  SyncState,
  Task,
  TaskSource,
  Term,
} from './records';
import { defaultSpaces } from './spaces';

export interface HeatExport {
  format: 'heat-export';
  version: 1;
  exportedAt: string;
  timeZone: string;
  workspaces: { key: string; name: string; groupLabel: string; types: string[]; persona: string }[];
  tasks: {
    id: string;
    workspace: string;
    title: string;
    group: string | null;
    type: string;
    due: string | null;
    difficulty: number;
    estMin: number | null;
    actualMin: number | null;
    notes: string;
    done: boolean;
    doneAt: string | null;
    source: 'manual' | 'calendar' | 'gmail';
  }[];
  milestones: { id: string; workspace: string; title: string; date: string; done: boolean; order: number }[];
  habits: { id: string; title: string; log: Record<string, true> }[];
  term: string;
  courses: {
    code: string;
    name: string;
    categories: { name: string; weight: number; keywords: string[] }[];
    scale?: LetterStep[];
    sticky?: string;
  }[];
  grades: {
    id: string;
    course: string;
    title: string;
    category: string | null;
    score: number | null;
    outOf: number;
    dropped: boolean;
    pending: boolean;
    link?: string;
  }[];
  processedMailIds: string[];
  lastSyncAt: string | null;
}

export interface Imported {
  /** Spaces to add; a workspace whose name matches an existing space moves into it. */
  spaces: Space[];
  tasks: Task[];
  milestones: Milestone[];
  habits: Habit[];
  /** Terms and courses to add; a term already there by name, and a course by code in it, are used as they are. */
  terms: Term[];
  courses: Course[];
  grades: Grade[];
  sync: SyncState;
}

/** What the app already holds, so a second import (a fresher download) defines nothing twice. */
export interface Existing {
  spaces?: readonly Space[];
  terms?: readonly Term[];
  courses?: readonly Course[];
}

const SOURCES: Record<HeatExport['tasks'][number]['source'], TaskSource> = {
  manual: 'you',
  calendar: 'calendar',
  gmail: 'mail',
};

// --- Checking the file, row by row ------------------------------------------

type Row = Record<string, unknown>;
const isRow = (v: unknown): v is Row => typeof v === 'object' && v !== null && !Array.isArray(v);
const isString = (v: unknown): v is string => typeof v === 'string';
const isNumber = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const isMinutes = (v: unknown) => v === null || (isNumber(v) && v >= 0);
const isStrings = (v: unknown) => Array.isArray(v) && v.every(isString);
const ISO = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:\d{2})$/;
const isInstant = (v: unknown) => isString(v) && ISO.test(v) && Number.isFinite(Date.parse(v));
const isDay = (v: unknown) => isString(v) && /^\d{4}-\d{2}-\d{2}$/.test(v);
const optional = (v: unknown, check: (v: unknown) => boolean) => v === undefined || check(v);
const orNull = (check: (v: unknown) => boolean) => (v: unknown) => v === null || check(v);
const every = (v: unknown, check: (r: Row) => boolean) => Array.isArray(v) && v.every((r) => isRow(r) && check(r));

const workspaceOk = (w: Row) =>
  isString(w.key) && isString(w.name) && isString(w.groupLabel) && isStrings(w.types) && isString(w.persona);

const taskOk = (keys: Set<unknown>) => (t: Row) =>
  isString(t.id) &&
  keys.has(t.workspace) &&
  isString(t.title) &&
  orNull(isString)(t.group) &&
  isString(t.type) &&
  orNull(isInstant)(t.due) &&
  isNumber(t.difficulty) &&
  t.difficulty >= 1 &&
  t.difficulty <= 5 &&
  isMinutes(t.estMin) &&
  isMinutes(t.actualMin) &&
  isString(t.notes) &&
  typeof t.done === 'boolean' &&
  orNull(isInstant)(t.doneAt) &&
  isString(t.source) &&
  Object.hasOwn(SOURCES, t.source);

const milestoneOk = (keys: Set<unknown>) => (m: Row) =>
  isString(m.id) &&
  keys.has(m.workspace) &&
  isString(m.title) &&
  isDay(m.date) &&
  typeof m.done === 'boolean' &&
  isNumber(m.order);

const habitOk = (h: Row) =>
  isString(h.id) &&
  isString(h.title) &&
  isRow(h.log) &&
  Object.entries(h.log).every(([k, v]) => isDay(k) && v === true);

const courseOk = (c: Row) =>
  isString(c.code) &&
  isString(c.name) &&
  every(c.categories, (k) => isString(k.name) && isNumber(k.weight) && isStrings(k.keywords)) &&
  optional(c.scale, (v) => every(v, (x) => isString(x.letter) && isNumber(x.min))) &&
  optional(c.sticky, isString);

const gradeOk = (g: Row) =>
  isString(g.id) &&
  isString(g.course) &&
  isString(g.title) &&
  orNull(isString)(g.category) &&
  orNull(isNumber)(g.score) &&
  isNumber(g.outOf) &&
  typeof g.dropped === 'boolean' &&
  typeof g.pending === 'boolean' &&
  optional(g.link, isString);

/** Checks that a parsed file is a Heat export this version can read, every row of it. */
export function parseHeatExport(json: unknown): HeatExport | { error: string } {
  const notExport = { error: copy.moving.notExport };
  if (!isRow(json) || json.format !== 'heat-export') return notExport;
  const x = json;
  if (Number.isInteger(x.version) && (x.version as number) > 1) return { error: copy.moving.newer };
  if (x.version !== 1) return notExport;
  if (!isInstant(x.exportedAt) || !isString(x.timeZone) || !isString(x.term) || !orNull(isInstant)(x.lastSyncAt)) {
    return notExport;
  }
  if (!every(x.workspaces, workspaceOk)) return notExport;
  const keys = new Set((x.workspaces as Row[]).map((w) => w.key));
  const ok =
    every(x.tasks, taskOk(keys)) &&
    every(x.milestones, milestoneOk(keys)) &&
    every(x.habits, habitOk) &&
    every(x.courses, courseOk) &&
    every(x.grades, gradeOk) &&
    isStrings(x.processedMailIds);
  return ok ? (x as unknown as HeatExport) : notExport;
}

const parseTime = (iso: string | null) => (iso === null ? null : Date.parse(iso));

// Names and codes match whatever their case and spacing: "jpn  201" is JPN 201.
const sameName = (a: string, b: string) => {
  const norm = (x: string) => x.trim().replace(/\s+/g, ' ').toLowerCase();
  return norm(a) === norm(b);
};

export function importArtifact(dump: HeatExport, existing: Existing, newId: () => Id): Imported {
  const defaults = defaultSpaces(() => '');
  const spaces: Space[] = [];
  const spaceFor = new Map<string, Space>();
  for (const w of dump.workspaces) {
    const same = (s: Space) => s.name.toLowerCase() === w.name.toLowerCase();
    let space = existing.spaces?.find(same);
    if (!space) {
      const groupKind = w.groupLabel === 'Course' ? 'course' : w.groupLabel === 'Milestone' ? 'milestone' : 'free';
      const hue = defaults.find(same)?.hue ?? (spaces.length * 120) % 360;
      space = {
        id: newId(),
        name: w.name,
        hue,
        groupKind,
        groupLabel: w.groupLabel,
        types: w.types,
        persona: w.persona,
      };
      spaces.push(space);
    }
    spaceFor.set(w.key, space);
  }

  const known = existing.terms?.find((t) => sameName(t.name, dump.term));
  const term: Term = known ?? { id: newId(), name: dump.term };
  const terms = known ? [] : [term];
  // Each course is defined once (3.4): one already in the term is used as it is.
  const inTerm = (existing.courses ?? []).filter((c) => c.termId === term.id);
  const courses: Course[] = [];
  const findCourse = (code: string) => [...inTerm, ...courses].find((c) => sameName(c.code, code));
  for (const c of dump.courses) {
    if (findCourse(c.code)) continue;
    courses.push({
      id: newId(),
      termId: term.id,
      code: c.code,
      name: c.name,
      categories: c.categories.map((k) => ({ id: newId(), name: k.name, weight: k.weight, keywords: k.keywords })),
      ...(c.scale ? { scale: c.scale } : {}),
      notes: c.sticky ?? '',
    });
  }
  // A course only a task names is made here, once.
  const courseFor = (code: string): Course => {
    let c = findCourse(code);
    if (!c) {
      c = { id: newId(), termId: term.id, code, name: code, categories: [], notes: '' };
      courses.push(c);
    }
    return c;
  };

  const milestones: Milestone[] = dump.milestones.map((m) => ({
    id: m.id,
    spaceId: spaceFor.get(m.workspace)!.id,
    title: m.title,
    date: m.date,
    done: m.done,
    order: m.order,
  }));

  const tasks: Task[] = dump.tasks.map((t) => {
    const space = spaceFor.get(t.workspace)!;
    const link: Partial<Task> = {};
    if (t.group !== null) {
      const milestone = milestones.find((m) => m.spaceId === space.id && m.title === t.group);
      if (space.groupKind === 'course') link.courseId = courseFor(t.group).id;
      else if (space.groupKind === 'milestone' && milestone) link.milestoneId = milestone.id;
      else link.group = t.group;
    }
    return {
      id: t.id,
      spaceId: space.id,
      title: t.title,
      type: t.type,
      ...link,
      due: parseTime(t.due),
      difficulty: t.difficulty,
      estMin: t.estMin,
      // The artifact has no focus sessions, so its typed time is all hand adjustment.
      adjustMin: t.actualMin ?? 0,
      notes: t.notes,
      done: t.done,
      doneAt: parseTime(t.doneAt),
      source: SOURCES[t.source],
    };
  });

  const grades: Grade[] = dump.grades.map((g) => {
    const course = courseFor(g.course);
    return {
      id: g.id,
      courseId: course.id,
      categoryId: course.categories.find((k) => k.name === g.category)?.id ?? null,
      title: g.title,
      score: g.score,
      outOf: g.outOf,
      dropped: g.dropped,
      pending: g.pending,
      ...(g.link ? { link: g.link } : {}),
      // Pending grades come from grade notices in mail, under gp- hashes.
      source: g.pending || g.id.startsWith('gp-') ? 'mail' : 'you',
    };
  });

  return {
    spaces,
    tasks,
    milestones,
    habits: dump.habits.map((h) => ({ id: h.id, title: h.title, log: { ...h.log }, showCounter: false })),
    terms,
    courses,
    grades,
    sync: { processedMailIds: [...dump.processedMailIds], lastSyncAt: parseTime(dump.lastSyncAt) },
  };
}
