// The fake core's homes and task types (docs/HEAT.md). A task belongs to a
// course or a project, or just lives in its space; a type says what a kind of
// work usually takes, and is looked for in the task's home, then its space,
// then the defaults below. A new task with no type named gets one from its
// title, and one with no difficulty or minutes named gets the type's. What
// the person sets is theirs: nothing here writes over it unless asked to.
//
// The rule is a plain version of the core's (crates/wi-heat, model/types): a
// type is picked by a whole word or phrase of the title, the longer phrase
// winning; a title that picks nothing gets Other, whose numbers stand until
// Claude gives better ones.

import type { Id, Snapshot, Task, TaskHome, TypeDef } from '../client';
import { courseLabel, plural } from '../fmt';
import { derive, type Fake, refuse, register, snapshotOf, wrap } from './core';

export const OTHER = 'Other';

const def = (name: string, estMin: number, difficulty: number, patterns: string[]): TypeDef => ({
  name,
  patterns,
  estMin,
  difficulty,
});

/** The defaults every space starts from. The order breaks a tie between two phrases of the same length. */
export const DEFAULTS: TypeDef[] = [
  def('Quiz', 20, 2, ['quiz', 'quizzes']),
  def('Worksheet', 45, 2, ['worksheet', 'worksheets']),
  def('Listening', 30, 2, ['listening']),
  def('Lab', 120, 3, ['lab', 'labs', 'laboratory']),
  def('Homework', 90, 3, ['homework', 'hw', 'assignment', 'problem set', 'pset', 'exercises']),
  def('Project', 240, 4, ['project', 'essay', 'paper', 'presentation', 'proposal']),
  def('Email', 10, 1, ['email', 'e-mail', 'reply', 'respond', 'write back']),
  def('Errand', 30, 1, ['errand', 'pick up', 'drop off', 'buy']),
  def('Admin', 20, 1, ['form', 'forms', 'paperwork', 'register', 'registration', 'renew', 'sign up']),
  def('Creative session', 120, 3, ['session', 'mix', 'mixing', 'recording', 'songwriting', 'sketch', 'studio']),
  def(OTHER, 45, 2, []),
];

export type Level = 'course' | 'project' | 'space' | 'global';

/** Where a task's type is looked for, most specific first. */
export interface Levels {
  home: { level: 'course' | 'project'; defs: TypeDef[] } | null;
  space: TypeDef[];
  global: TypeDef[];
}

/** A title's words, lower case, with a number split from the letters it touches: "lab3" reads "lab", "3". */
export function words(text: string): string[] {
  const out: string[] = [];
  let word = '';
  let digits = false;
  for (const c of text) {
    if (!/[\p{L}\p{N}]/u.test(c)) {
      if (word) out.push(word);
      word = '';
      continue;
    }
    const isDigit = /[0-9]/.test(c);
    if (word && isDigit !== digits) {
      out.push(word);
      word = '';
    }
    digits = isDigit;
    word += c.toLowerCase();
  }
  if (word) out.push(word);
  return out;
}

/** How well a type fits a title: the longest of its phrases found whole, as [words, letters]. */
function fit(type: TypeDef, title: string[]): [number, number] | null {
  let best: [number, number] | null = null;
  for (const pattern of type.patterns) {
    const p = words(pattern);
    if (p.length === 0) continue;
    const found = title.some((_, at) => p.every((w, i) => title[at + i] === w));
    if (!found) continue;
    const score: [number, number] = [p.length, p.join('').length];
    if (!best || score[0] > best[0] || (score[0] === best[0] && score[1] > best[1])) best = score;
  }
  return best;
}

/** The type a title picks: the longer phrase wins, and the earlier type breaks a tie. */
export function matchTitle(title: string, defs: readonly TypeDef[]): TypeDef | null {
  const split = words(title);
  let best: { type: TypeDef; score: [number, number] } | null = null;
  for (const type of defs) {
    const score = fit(type, split);
    if (!score) continue;
    if (!best || score[0] > best.score[0] || (score[0] === best.score[0] && score[1] > best.score[1])) {
      best = { type, score };
    }
  }
  return best?.type ?? null;
}

export function byName(name: string, defs: readonly TypeDef[]): TypeDef | null {
  const want = name.trim().toLowerCase();
  return want ? (defs.find((d) => d.name.trim().toLowerCase() === want) ?? null) : null;
}

const each = (levels: Levels): { level: Level; defs: TypeDef[] }[] => [
  ...(levels.home ? [levels.home] : []),
  { level: 'space', defs: levels.space },
  { level: 'global', defs: levels.global },
];

function find(levels: Levels, pick: (defs: TypeDef[]) => TypeDef | null): { level: Level; type: TypeDef } | null {
  for (const { level, defs } of each(levels)) {
    const type = pick(defs);
    if (type) return { level, type };
  }
  return null;
}

/** The type a title picks across the levels, or Other. */
export function pickType(title: string, levels: Levels): string {
  return find(levels, (defs) => matchTitle(title, defs))?.type.name ?? OTHER;
}

export interface Fill {
  estMin: number;
  difficulty: number;
  /** Where the type was found; null when nothing matched and the numbers are the catch-all's. */
  from: Level | null;
}

/**
 * The minutes and difficulty for a task of type `kind` titled `title`. The
 * type is found by its name, most specific level first; a name no level
 * defines (a space's own word, "Music") is read from the title instead. A
 * number the type leaves out comes from the same name further down.
 */
export function fill(kind: string, title: string, levels: Levels): Fill {
  const other = byName(OTHER, levels.global);
  const catchAll = { estMin: other?.estMin ?? 45, difficulty: other?.difficulty ?? 2 };
  const isOther = !kind.trim() || kind.trim().toLowerCase() === OTHER.toLowerCase();
  const found = isOther
    ? null
    : (find(levels, (defs) => byName(kind, defs)) ?? find(levels, (defs) => matchTitle(title, defs)));
  if (!found) return { ...catchAll, from: null };
  const below = (pick: (d: TypeDef) => number | null): number | null => {
    for (const defs of [levels.space, levels.global]) {
      const n = byName(found.type.name, defs);
      if (n && pick(n) !== null) return pick(n);
    }
    return null;
  };
  const estMin = found.type.estMin ?? below((d) => d.estMin);
  const difficulty = found.type.difficulty ?? below((d) => d.difficulty);
  return {
    estMin: estMin ?? catchAll.estMin,
    difficulty: difficulty ?? catchAll.difficulty,
    // A type that names no minutes anywhere is as good as no match.
    from: estMin === null ? null : found.level,
  };
}

/** The levels a task's type is looked for in: its course's types or its project's, then its space's, then the defaults. */
export function levelsOf(fake: Fake, task: Pick<Task, 'spaceId' | 'courseId' | 'projectId'>): Levels {
  const course = task.courseId ? fake.store.course.get(task.courseId) : undefined;
  const project = task.projectId ? fake.store.project.get(task.projectId) : undefined;
  return {
    home: course
      ? { level: 'course', defs: course.types ?? [] }
      : project
        ? { level: 'project', defs: project.types ?? [] }
        : null,
    space: fake.store.space.get(task.spaceId)?.typeDefs ?? [],
    global: DEFAULTS,
  };
}

/** Who a task's minutes are by, for a record older than `estBy`. */
const estBy = (t: Task) => t.estBy ?? (t.estMin !== null && t.estMin > 0 ? 'you' : 'default');

/**
 * A task with its types applied: a type picked from the title is picked
 * again, and minutes and a difficulty no one set are the type's. `force`
 * drops what the person set by hand on them.
 */
export function applyTypes(fake: Fake, task: Task, opts: { force?: boolean } = {}): Task {
  const next: Task = { ...task };
  const levels = levelsOf(fake, next);
  if (next.typeBy === 'rule') next.type = pickType(next.title, levels);
  const numbers = fill(next.type, next.title, levels);
  const by = numbers.from ? 'type' : 'default';
  const fixed = (who: string | undefined) => who === 'claude' || (who === 'you' && !opts.force);
  if (!fixed(estBy(next))) {
    next.estMin = numbers.estMin;
    next.estBy = by;
    delete next.estReason;
  }
  // A record older than `difficultyBy` reads its difficulty as whoever made its estimate made it.
  if (!fixed(next.difficultyBy ?? estBy(next))) {
    next.difficulty = numbers.difficulty;
    next.difficultyBy = by;
  }
  return next;
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** The open tasks `keep` names that applying the types again would change, each as it would become. */
function moved(fake: Fake, keep: (t: Task) => boolean, opts: { force?: boolean } = {}): Task[] {
  return [...fake.store.task.values()]
    .filter((task) => !task.done && keep(task))
    .flatMap((task) => {
      const next = applyTypes(fake, task, opts);
      return same(next, task) ? [] : [next];
    });
}

/** Applies the types again to the tasks `keep` names, inside a write; answers how many changed. */
function reapply(fake: Fake, keep: (t: Task) => boolean, opts: { force?: boolean } = {}): number {
  const tasks = moved(fake, keep, opts);
  for (const task of tasks) fake.store.task.set(task.id, task);
  return tasks.length;
}

// --- a new task, and a task edited ------------------------------------------------

wrap('heat.put', (next) => (args, fake) => {
  if (args.kind !== 'task') return next(args, fake);
  const sent = args.record as Partial<Task>;
  if (sent.id && fake.store.task.has(sent.id)) return next(args, fake);
  // What was sent is the person's own; what was left out is filled in from the type.
  const record = { ...sent } as Task;
  const levels = levelsOf(fake, record);
  if (sent.type === undefined) {
    record.type = pickType(String(record.title ?? ''), levels);
    record.typeBy = 'rule';
  } else record.typeBy ??= 'you';
  const numbers = fill(record.type, String(record.title ?? ''), levels);
  const by = numbers.from ? 'type' : 'default';
  if (sent.difficulty === undefined) {
    record.difficulty = numbers.difficulty;
    record.difficultyBy = by;
  } else record.difficultyBy ??= 'you';
  if (sent.estMin === undefined) {
    record.estMin = numbers.estMin;
    record.estBy = by;
  } else if (sent.estMin !== null) record.estBy ??= 'you';
  return next({ ...args, record }, fake);
});

wrap('heat.patch', (next) => (args, fake) => {
  if (args.kind !== 'task') return next(args, fake);
  const sent = args.set as Partial<Task>;
  const set: Partial<Task> = { ...sent };
  if ('type' in sent) set.typeBy = 'you';
  if ('difficulty' in sent) set.difficultyBy = 'you';
  if ('estMin' in sent && sent.estMin !== null) set.estBy = 'you';
  return next({ ...args, set }, fake);
});

register('heat.task.setType', (args, fake) => {
  const was = fake.store.task.get(args.taskId as Id);
  if (!was) refuse('No task has that id.');
  const type = typeof args.type === 'string' ? args.type.trim() : '';
  // A type named is the person's; none hands the choice back to the title's words.
  const task = applyTypes(fake, type ? { ...was, type, typeBy: 'you' } : { ...was, typeBy: 'rule' });
  const { undo } = fake.write('set type', ['task'], () => fake.store.task.set(task.id, task));
  return { task, undo };
});

register('heat.task.reapplyDefaults', (args, fake) => {
  const keep = (t: Task) =>
    args.all === true ||
    (args.taskId !== undefined && t.id === args.taskId) ||
    (args.courseId !== undefined && t.courseId === args.courseId) ||
    (args.projectId !== undefined && t.projectId === args.projectId) ||
    (args.spaceId !== undefined && t.spaceId === args.spaceId);
  if (args.taskId !== undefined && !fake.store.task.has(args.taskId as Id)) refuse('No task has that id.');
  let changed = 0;
  const { undo } = fake.write('reapply defaults', ['task'], () => {
    changed = reapply(fake, keep, { force: args.force === true });
  });
  return { changed, undo: changed > 0 ? undo : null };
});

// --- a course, a project or a space edited -------------------------------------------

/** The three records that hold types, and the field a task names each by. */
const HOLDS_TYPES = { course: 'courseId', project: 'projectId', space: 'spaceId' } as const;
type Holder = keyof typeof HOLDS_TYPES;
const holdsTypes = (kind: unknown): kind is Holder => typeof kind === 'string' && kind in HOLDS_TYPES;

/**
 * One of them saved: its fields change and its types are applied again to
 * its tasks, as one entry. `retimed` is how many tasks that moved.
 */
function saveHolder(fake: Fake, kind: Holder, id: Id, record: Record<string, unknown>) {
  const map = fake.store[kind] as unknown as Map<Id, Record<string, unknown>>;
  const mine = (t: Task) => t[HOLDS_TYPES[kind]] === id;
  // Looked at first with the record as it will be, so the entry names tasks only when some move.
  const was = map.get(id)!;
  map.set(id, record);
  const retimed = moved(fake, mine).length;
  map.set(id, was);
  const { undo } = fake.write(`edit ${kind}`, retimed > 0 ? [kind, 'task'] : [kind], () => {
    map.set(id, record);
    reapply(fake, mine);
  });
  return { record, retimed, undo };
}

const stored = (fake: Fake, kind: Holder, id: unknown) =>
  (fake.store[kind] as unknown as Map<Id, Record<string, unknown>>).get(id as Id);

// `heat.patch` and `heat.put` on one of the three answer `retimed` too; a refusal is still the plain command's.
wrap('heat.patch', (next) => (args, fake) => {
  if (!holdsTypes(args.kind)) return next(args, fake);
  const was = stored(fake, args.kind, args.id);
  const set = args.set as Record<string, unknown>;
  if (!was || 'public' in set) return next(args, fake);
  return saveHolder(fake, args.kind, args.id as Id, { ...was, ...set });
});

wrap('heat.put', (next) => (args, fake) => {
  if (!holdsTypes(args.kind)) return next(args, fake);
  const record = args.record as Record<string, unknown>;
  const was = stored(fake, args.kind, record.id);
  // A new one has no tasks yet; one made public here is refused by the plain command.
  if (!was || (record.public === true && was.public !== true)) {
    return { ...(next(args, fake) as object), retimed: 0 };
  }
  return saveHolder(fake, args.kind, record.id as Id, { ...record });
});

for (const kind of ['course', 'project'] as const) {
  register(`heat.${kind}.update`, (args, fake) => {
    const was = stored(fake, kind, args.id);
    if (!was) refuse(`No ${kind} has that id.`);
    const set = args.set as Record<string, unknown>;
    if ('public' in set) refuse('The Public switch has its own command.');
    return saveHolder(fake, kind, args.id as Id, { ...was, ...set });
  });
}

// --- what the snapshot carries ------------------------------------------------------

/** A task's home as the snapshot names it: its course, else its project, else none. */
export function homeOf(snap: Snapshot, task: Task): TaskHome | null {
  const course = task.courseId ? snap.records.course.find((c) => c.id === task.courseId) : undefined;
  if (course) return { kind: 'course', id: course.id, label: courseLabel(course) };
  const project = task.projectId ? snap.records.project.find((p) => p.id === task.projectId) : undefined;
  return project ? { kind: 'project', id: project.id, label: project.title } : null;
}

/** Open tasks whose minutes are still the catch-all's: what Claude is asked to better. */
const unscored = (snap: Snapshot) =>
  snap.records.task.filter((t) => {
    const e = snap.derived.tasks[t.id]?.estimate;
    return !t.done && e?.by === 'default' && !e.typeFrom;
  });

derive((snap, fake) => {
  for (const task of snap.records.task) {
    const d = snap.derived.tasks[task.id];
    if (!d) continue;
    d.home = homeOf(snap, task);
    const by = d.estimate.by;
    d.estimate.typeFrom = by === 'you' || by === 'claude' ? null : fill(task.type, task.title, levelsOf(fake, task)).from;
  }
  const names = (defs: TypeDef[] | undefined) => (defs ?? []).map((d) => d.name);
  const byId = <T extends { id: Id }>(rows: T[], defs: (row: T) => TypeDef[] | undefined) =>
    Object.fromEntries(rows.map((r) => [r.id, names(defs(r))]));
  snap.derived.types = {
    global: names(DEFAULTS),
    spaces: byId(snap.records.space, (s) => s.typeDefs),
    courses: byId(snap.records.course, (c) => c.types),
    projects: byId(snap.records.project, (p) => p.types),
  };
  snap.derived.unscored = unscored(snap).length;
});

/** Claude is asked once, in the background, for the tasks no type matched. The fake only says how many. */
register('heat.tasks.score', (_args, fake) => {
  const asked = snapshotOf(fake).derived.unscored ?? 0;
  return {
    asked,
    line: asked > 0 ? `Asked Claude to estimate ${plural(asked, 'task')}.` : 'Every task has an estimate.',
  };
});
