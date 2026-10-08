import { describe, expect, it } from 'vitest';
import { heatClient, type TypeDef } from '../client';
import { createFake } from './all';
import { holdReading, finishReading } from './syllabus';
import { DEFAULTS, fill, type Levels, matchTitle, pickType, words } from './types';

// The fake core's homes, types and syllabus drafts, as docs/HEAT.md's contract
// gives them. What a fail looks like: a type picked by part of a word; a
// course's type losing to a default of the same name; a new task with nothing
// chosen left without a type, minutes or a difficulty, or one with all three
// chosen having any of them written over; a syllabus draft writing anything
// before it is accepted.

const def = (name: string, patterns: string[], estMin: number | null, difficulty: number | null): TypeDef => ({
  name,
  patterns,
  estMin,
  difficulty,
});

describe('the type a title picks', () => {
  const name = (title: string) => matchTitle(title, DEFAULTS)?.name ?? null;

  it('splits a number from the word it touches', () => {
    expect(words('HW2: Lab3 (第2課)')).toEqual(['hw', '2', 'lab', '3', '第', '2', '課']);
  });

  it('is found by a whole word or phrase, in any case', () => {
    expect(name('Online Vocabulary Quiz (第2課)')).toBe('Quiz');
    expect(name('lab3')).toBe('Lab');
    expect(name('Problem set 6')).toBe('Homework');
    expect(name('Renew passport')).toBe('Admin');
    expect(name('Mix the second verse')).toBe('Creative session');
    // "lab" inside "Syllabus" doesn't count.
    expect(name('Syllabus check')).toBeNull();
  });

  it('goes to the longer phrase, and the earlier type breaks a tie', () => {
    const defs = [def('Quiz', ['quiz'], 20, 2), def('Lab quiz', ['lab quiz'], 15, 1), def('Lab', ['lab'], 120, 3)];
    expect(matchTitle('Lab quiz 2', defs)!.name).toBe('Lab quiz');
    expect(matchTitle('Quiz on lab safety', defs)!.name).toBe('Quiz');
  });

  it('is looked for in the home, then the space, then the defaults', () => {
    const levels: Levels = {
      home: { level: 'course', defs: [def('Lab', ['lab', 'recitation'], 180, 4)] },
      space: [def('Quiz', ['quiz'], 30, null)],
      global: DEFAULTS,
    };
    expect(fill('Lab', 'lab3', levels)).toEqual({ estMin: 180, difficulty: 4, from: 'course' });
    // The space's quiz gives the minutes; its difficulty comes from the default of that name.
    expect(fill('Quiz', 'Quiz 2', levels)).toEqual({ estMin: 30, difficulty: 2, from: 'space' });
    expect(fill('Homework', 'Homework 3', levels)).toEqual({ estMin: 90, difficulty: 3, from: 'global' });
    expect(pickType('Recitation 4', levels)).toBe('Lab');
    // A name no level defines is read from the title, and then it is the catch-all's.
    expect(fill('Music', 'Mix the second verse', levels)).toEqual({ estMin: 120, difficulty: 3, from: 'global' });
    expect(fill('Reading', 'Reading response 3', levels)).toEqual({ estMin: 45, difficulty: 2, from: null });
    expect(pickType('Reading response 3', levels)).toBe('Other');
  });
});

function boot() {
  const { fake, transport } = createFake(
    {
      space: [
        { id: 'sp', name: 'Classes', hue: 211, groupKind: 'course', groupLabel: 'Course', types: ['Homework', 'Other'], persona: '' },
      ],
      term: [{ id: 'term', name: 'Fall 2026' }],
      course: [
        { id: 'ele', termId: 'term', code: 'ELE 209', name: 'Intro to Computer Systems Lab', categories: [], notes: '', status: 'stub' },
        { id: 'jpn', termId: 'term', code: 'JPN 101', name: 'JPN 101', categories: [{ id: 'q', name: 'Quizzes', weight: 100, keywords: [] }], notes: '' },
      ],
    },
    { now: Date.UTC(2026, 9, 7, 14), zone: 'America/New_York' },
  );
  const heat = heatClient(transport);
  const blank = { spaceId: 'sp', due: null, adjustMin: 0, notes: '', done: false, doneAt: null, source: 'you' as const };
  return { fake, heat, call: transport.call, blank };
}

describe('the fake core’s types', () => {
  it('fills a new task’s type, minutes and difficulty, and leaves alone what was sent', async () => {
    const { heat, blank } = boot();
    const lab = (await heat.put('task', { ...blank, title: 'Lab 2 report', courseId: 'ele' })).record;
    expect(lab).toMatchObject({ type: 'Lab', typeBy: 'rule', estMin: 120, estBy: 'type', difficulty: 3, difficultyBy: 'type' });
    const none = (await heat.put('task', { ...blank, title: 'Sort the drawer' })).record;
    expect(none).toMatchObject({ type: 'Other', typeBy: 'rule', estMin: 45, estBy: 'default', difficulty: 2, difficultyBy: 'default' });
    const own = (await heat.put('task', { ...blank, title: 'Lab 3', type: 'Homework', difficulty: 5, estMin: 15 })).record;
    expect(own).toMatchObject({ type: 'Homework', typeBy: 'you', estMin: 15, estBy: 'you', difficulty: 5, difficultyBy: 'you' });

    const snap = await heat.snapshot('2026-10-07');
    expect(snap.derived.tasks[lab.id]).toMatchObject({
      home: { kind: 'course', id: 'ele', label: 'ELE 209 · Intro to Computer Systems Lab' },
      estimate: { min: 120, by: 'type', typeFrom: 'global' },
    });
    expect(snap.derived.tasks[none.id]).toMatchObject({ home: null, estimate: { min: 45, by: 'default', typeFrom: null } });
    expect(snap.derived.tasks[own.id].estimate).toMatchObject({ min: 15, by: 'you', typeFrom: null });
    expect(snap.derived.unscored).toBe(1);
    expect(await heat.scoreTasks()).toEqual({ asked: 1, line: 'Asked Claude to estimate 1 task.' });
  });

  it('carries each course’s label, status and whether it waits for a syllabus, and the types a picker offers', async () => {
    const { heat } = boot();
    await heat.updateCourse('ele', { types: [{ name: 'Lab', patterns: ['lab'], estMin: 150, difficulty: 3 }] });
    const { derived } = await heat.snapshot('2026-10-07');
    expect(derived.courses.ele).toMatchObject({
      label: 'ELE 209 · Intro to Computer Systems Lab',
      needsSyllabus: true,
      status: 'stub',
      term: 'Fall 2026',
    });
    expect(derived.courses.jpn).toMatchObject({ label: 'JPN 101', needsSyllabus: false, status: 'confirmed' });
    expect(derived.types).toEqual({
      global: DEFAULTS.map((d) => d.name),
      spaces: { sp: [] },
      courses: { ele: ['Lab'], jpn: [] },
      projects: {},
    });
  });

  it('applies a course’s types to its tasks when the course is saved, except what was set by hand', async () => {
    const { heat, blank } = boot();
    const auto = (await heat.put('task', { ...blank, title: 'Lab 1', courseId: 'ele' })).record;
    const mine = (await heat.put('task', { ...blank, title: 'Lab 2', courseId: 'ele', estMin: 200 })).record;
    const r = await heat.updateCourse('ele', { types: [{ name: 'Lab', patterns: ['lab'], estMin: 150, difficulty: 4 }] });
    expect(r).toMatchObject({ retimed: 2, undo: 'Undo edit course' });
    const tasks = new Map((await heat.snapshot('2026-10-07')).records.task.map((t) => [t.id, t]));
    expect(tasks.get(auto.id)).toMatchObject({ estMin: 150, estBy: 'type', difficulty: 4 });
    // The minutes typed stay; the difficulty nobody set follows the type.
    expect(tasks.get(mine.id)).toMatchObject({ estMin: 200, estBy: 'you', difficulty: 4, difficultyBy: 'type' });

    expect(await heat.reapplyDefaults({ courseId: 'ele' })).toEqual({ changed: 0, undo: null });
    expect(await heat.reapplyDefaults({ courseId: 'ele', force: true })).toEqual({ changed: 1, undo: 'Undo reapply defaults' });
    expect((await heat.snapshot('2026-10-07')).records.task.find((t) => t.id === mine.id)).toMatchObject({
      estMin: 150,
      estBy: 'type',
    });
  });

  it('marks what a patch or an estimate changes as the person’s own', async () => {
    const { heat, blank } = boot();
    const t = (await heat.put('task', { ...blank, title: 'Quiz 1' })).record;
    expect((await heat.patch('task', t.id, { type: 'Homework' })).record).toMatchObject({ typeBy: 'you', estBy: 'type' });
    expect((await heat.estimate(t.id, { difficulty: 5 })).task).toMatchObject({ difficultyBy: 'you', estBy: 'type' });
    expect((await heat.estimate(t.id, { estMin: 33 })).task).toMatchObject({ estMin: 33, estBy: 'you' });
    expect((await heat.setType(t.id, null)).task).toMatchObject({ type: 'Quiz', typeBy: 'rule', estMin: 33, difficulty: 5 });
  });
});

describe('the fake core’s syllabus drafts', () => {
  it('reads a PDF into a draft that is reading, then ready, and writes nothing until it is accepted', async () => {
    const { fake, heat, call } = boot();
    holdReading(fake);
    await expect(heat.importSyllabus({ path: '/tmp/notes.txt' })).rejects.toThrow('Learn reads a syllabus from a PDF.');
    const { draft } = await heat.importSyllabus({ path: '/Users/you/ELE 209 syllabus.pdf', courseId: 'ele' });
    expect(draft).toMatchObject({ courseId: 'ele', fileName: 'ELE 209 syllabus.pdf', state: 'reading' });
    expect((await heat.snapshot('2026-10-07')).syllabus!.drafts).toEqual([draft]);

    const heard: unknown[] = [];
    heat.onChange((kinds) => heard.push(kinds));
    finishReading(fake, draft.id);
    expect(heard).toHaveLength(1);
    const [ready] = (await heat.snapshot('2026-10-07')).syllabus!.drafts;
    expect(ready).toMatchObject({
      id: draft.id,
      state: 'ready',
      course: { code: 'ELE 209', label: 'ELE 209 · Intro to Computer Systems Lab', term: 'Fall 2026', isNew: false },
      weightsTotal: 100,
      weightsFlag: null,
      line: 'ELE 209 · Intro to Computer Systems Lab. Labs 40%, Quizzes 20%, Final 40%. 2 types. 2 new tasks, 0 dates changed.',
    });
    expect(fake.journal).toEqual([]);
    expect(fake.store.task.size).toBe(0);

    const r = await heat.syllabus.accept(draft.id);
    expect(r).toMatchObject({ counts: { newTasks: 2, dateChanges: 0, retimed: 0 }, undo: 'Undo import syllabus' });
    expect(r.course).toMatchObject({ id: 'ele', status: 'confirmed', syllabusSource: { name: 'ELE 209 syllabus.pdf' } });
    expect(r.course.categories.map((c) => [c.name, c.weight, c.dropLowest ?? null])).toEqual([
      ['Labs', 40, null],
      ['Quizzes', 20, 1],
      ['Final', 40, null],
    ]);
    const after = await heat.snapshot('2026-10-07');
    expect(after.syllabus!.drafts).toEqual([]);
    expect(after.derived.courses.ele).toMatchObject({ needsSyllabus: false, status: 'confirmed' });
    expect(after.records.task.map((t) => [t.title, t.type, t.estMin, t.courseId])).toEqual([
      ['Lab 1', 'Lab', 150, 'ele'],
      ['Quiz 1', 'Quiz', 25, 'ele'],
    ]);
    // One entry, so one ⌘Z.
    expect(fake.journal.map((e) => e.label)).toEqual(['import syllabus']);
    await call('history.undo', {});
    expect(fake.store.task.size).toBe(0);
    expect(fake.store.course.get('ele')).toMatchObject({ status: 'stub', categories: [] });
  });

  it('makes the course a syllabus names when Learn has none, and fails in a sentence when it names none', async () => {
    const { fake, heat } = boot();
    const { draft } = await heat.importSyllabus({
      json: { fileName: 'bio.pdf', course: { code: 'BIO 101', name: 'Biology', term: 'Spring 2027' }, weights: [{ category: 'Exams', percent: 110 }] },
    });
    expect(draft).toMatchObject({
      state: 'ready',
      courseId: null,
      course: { label: 'BIO 101 · Biology', isNew: true },
      weightsFlag: 'Weights add to 110%. That is 10% more than 100.',
    });
    const { course } = await heat.syllabus.accept(draft.id);
    expect(course).toMatchObject({ code: 'BIO 101', name: 'Biology', public: false });
    expect([...fake.store.term.values()].map((t) => t.name)).toEqual(['Fall 2026', 'Spring 2027']);

    const failed = (await heat.importSyllabus({ json: { fileName: 'blank.pdf' } })).draft;
    expect(failed).toMatchObject({ state: 'failed', error: 'blank.pdf doesn’t say which course it is for.' });
    await expect(heat.syllabus.accept(failed.id)).rejects.toThrow('This syllabus isn’t read yet.');
    await heat.syllabus.discard(failed.id);
    expect((await heat.snapshot('2026-10-07')).syllabus!.drafts).toEqual([]);
  });
});
