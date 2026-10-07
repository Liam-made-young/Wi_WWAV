// Moving in (docs/SPEC.md 3.15): the Heat artifact's JSON export becomes the
// app's records. Each workspace maps to a space, and every id is kept (event
// ids, the em- and gp- hashes, the processed Gmail ids), so the first sync in
// the app finds nothing new instead of everything twice.
//
// The artifact has no export yet; step 0 is a "Download JSON" button on it.
// This is the format that button should write: the plain dump of the records
// 3.1 describes, one array per kind, dates as ISO 8601 strings.
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
  terms: Term[];
  courses: Course[];
  grades: Grade[];
  sync: SyncState;
}

const LISTS = ['workspaces', 'tasks', 'milestones', 'habits', 'courses', 'grades', 'processedMailIds'] as const;

/** Checks that a parsed file is a Heat export this version can read. */
export function parseHeatExport(json: unknown): HeatExport | { error: string } {
  const x = json as Partial<HeatExport> | null;
  if (!x || typeof x !== 'object' || x.format !== 'heat-export' || typeof x.version !== 'number') {
    return { error: copy.moving.notExport };
  }
  if (x.version > 1) return { error: copy.moving.newer };
  if (LISTS.some((k) => !Array.isArray(x[k])) || typeof x.term !== 'string') return { error: copy.moving.notExport };
  const keys = new Set(x.workspaces!.map((w) => w.key));
  if ([...x.tasks!, ...x.milestones!].some((r) => !keys.has(r.workspace))) return { error: copy.moving.notExport };
  return x as HeatExport;
}

const SOURCES: Record<HeatExport['tasks'][number]['source'], TaskSource> = {
  manual: 'you',
  calendar: 'calendar',
  gmail: 'mail',
};

const parseTime = (iso: string | null) => (iso === null ? null : Date.parse(iso));

export function importArtifact(dump: HeatExport, existing: readonly Space[], newId: () => Id): Imported {
  const defaults = defaultSpaces(() => '');
  const spaces: Space[] = [];
  const spaceFor = new Map<string, Space>();
  for (const w of dump.workspaces) {
    const same = (s: Space) => s.name.toLowerCase() === w.name.toLowerCase();
    let space = existing.find(same);
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

  const term: Term = { id: newId(), name: dump.term };
  const courses: Course[] = dump.courses.map((c) => ({
    id: newId(),
    termId: term.id,
    code: c.code,
    name: c.name,
    categories: c.categories.map((k) => ({ id: newId(), name: k.name, weight: k.weight, keywords: k.keywords })),
    ...(c.scale ? { scale: c.scale } : {}),
    notes: c.sticky ?? '',
  }));
  // A course only a task names is made here, so each course is defined once (3.4).
  const courseFor = (code: string): Course => {
    let c = courses.find((x) => x.code === code);
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
    terms: [term],
    courses,
    grades,
    sync: { processedMailIds: [...dump.processedMailIds], lastSyncAt: parseTime(dump.lastSyncAt) },
  };
}
