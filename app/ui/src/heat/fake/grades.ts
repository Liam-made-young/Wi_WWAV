// The fake core's Grades (docs/HEAT.md, docs/SPEC.md 3.8): each course's
// percentage, letter and weights sentence in the snapshot, "what it would
// take", and the one way a pending grade gets its score: the person types it
// (`heat.score`). Claude's `add_pending_grade` has no score argument, and
// nothing here lets a patch or a put slip one in either. The arithmetic is
// the TS model's, the reference the Rust was ported from; the views never
// call it.

import type { Grade, Id, Task } from '../client';
import { courseLabel } from '../fmt';
import {
  categoryPct,
  currentPct,
  decidedPct,
  guessCategory,
  letterFor,
  weightsLine,
  whatItWouldTake,
} from '../model/grades';
import type * as M from '../model/records';
import { derive, type Fake, refuse, register, wrap } from './core';

const asCourse = (c: unknown) => c as M.Course;
const asGrades = (g: unknown) => g as M.Grade[];

derive((snap) => {
  for (const course of snap.records.course) {
    const own = snap.records.grade.filter((g) => g.courseId === course.id);
    const pct = currentPct(asCourse(course), asGrades(own));
    // Weights that add to nothing give no grade to work out: the course waits for its syllabus.
    const needsSyllabus = course.categories.reduce((sum, c) => sum + (Number(c.weight) || 0), 0) === 0;
    snap.derived.courses[course.id] = {
      currentPct: pct,
      decidedPct: decidedPct(asCourse(course), asGrades(own)),
      // A course with no scale of its own (absent, or cleared with null) uses the usual one.
      letter: pct === null ? null : letterFor(pct, asCourse(course).scale ?? undefined),
      // A course waiting for its syllabus isn't told its weights add to 0%: the ask for the syllabus covers it.
      weights: needsSyllabus ? null : weightsLine(asCourse(course)),
      categories: Object.fromEntries(course.categories.map((c) => [c.id, categoryPct(c.id, asGrades(own))])),
      label: courseLabel(course),
      needsSyllabus,
      status: course.status ?? 'confirmed',
      term: snap.records.term.find((t) => t.id === course.termId)?.name ?? '',
    };
  }
});

const gradesOf = (fake: Fake, courseId: Id) => [...fake.store.grade.values()].filter((g) => g.courseId === courseId);

register('heat.whatItWouldTake', (args, fake) => {
  const course = fake.store.course.get(args.courseId as Id);
  if (!course) refuse('No course has that id.');
  const letter = String(args.letter);
  const text = whatItWouldTake(asCourse(course), asGrades(gradesOf(fake, course.id)), letter);
  if (text === null) refuse(`${letter} isn’t on this course’s scale.`);
  return { text };
});

/** The person typed a score into a pending grade's banner. Nothing else gives one. */
register('heat.score', (args, fake) => {
  const grade = fake.store.grade.get(args.gradeId as Id);
  if (!grade) refuse('No grade has that id.');
  if (!grade.pending) refuse('Only a grade waiting for its score takes one here. Edit the grade instead.');
  const score = args.score;
  if (typeof score !== 'number' || !Number.isFinite(score) || score < 0) refuse('Type the score as a number.');
  const record: Grade = { ...grade, score, pending: false };
  const { undo } = fake.write('enter score', ['grade'], () => fake.store.grade.set(record.id, record));
  return { grade: record, undo };
});

const WAITING = 'A grade waiting for its score takes it through Enter score.';

// A pending grade is scored by typing and by nothing else: not a patch, not a put.
wrap('heat.patch', (next) => (args, fake) => {
  if (args.kind === 'grade') {
    const was = fake.store.grade.get(args.id as Id);
    const set = args.set as Record<string, unknown>;
    if (was?.pending && ('score' in set || 'pending' in set)) refuse(WAITING);
  }
  return next(args, fake);
});

wrap('heat.put', (next) => (args, fake) => {
  if (args.kind === 'grade') {
    const record = { ...(args.record as Record<string, unknown>) } as Partial<Grade> & Record<string, unknown>;
    const course = fake.store.course.get(record.courseId as Id);
    if (!course) refuse('No course has that id.');
    const was = record.id ? fake.store.grade.get(record.id) : undefined;
    if (was?.pending && (record.score !== null || record.pending === false)) refuse(WAITING);
    if (!was && record.pending === true && record.score !== null && record.score !== undefined) refuse(WAITING);
    if (!was && typeof record.score === 'number' && record.pending !== false) record.pending = false;
    // A new grade with no category named is filed by the course's keywords.
    if (record.categoryId === undefined)
      record.categoryId = guessCategory(String(record.title ?? ''), course.categories);
    return next({ ...args, record }, fake);
  }
  return next(args, fake);
});

// A grade waiting for its score follows its course, and stays private until it has one.
wrap('heat.public.set', (next) => (args, fake) => {
  if (args.kind === 'grade' && fake.store.grade.get(args.id as Id)?.pending && args.public === true) {
    refuse('A grade waiting for its score stays private. Enter the score first.');
  }
  return next(args, fake);
});

// A course takes its grades with it, and the tasks that named it lose the name.
wrap('heat.delete', (next) => (args, fake) => {
  if (args.kind !== 'course') return next(args, fake);
  const id = args.id as Id;
  if (!fake.store.course.has(id)) refuse('No course has that id.');
  return fake.write('delete course', ['course', 'grade', 'task'], () => {
    fake.store.course.delete(id);
    for (const [gid, g] of fake.store.grade) if (g.courseId === id) fake.store.grade.delete(gid);
    for (const [tid, t] of fake.store.task) {
      if (t.courseId !== id) continue;
      const { courseId: _gone, ...rest } = t;
      fake.store.task.set(tid, rest as Task);
    }
  });
});
