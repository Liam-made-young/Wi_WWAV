// The fake core's sharing and public view (docs/HEAT.md, docs/SPEC.md 3.15).
// Everything is private. Two things are the default public version, and each
// shows once you press Show: the Now making line, and the timelines of
// projects linked to a solar system. Any other record goes public through its
// own switch (`heat.public.set`). `heat.publicView` is exactly what someone
// opening your sun would see: only each record's own fields, and never a
// number worked out across records, a count or a comparison.

import type { DayKey, Id, Kind, ProfileShare, PublicView } from '../client';
import { derive, type Fake, refuse, register } from './core';

const WEEK = 7 * 86_400_000;

/** The systems of the fake's galaxy: what a project's timeline can be linked to. */
const SYSTEMS = [
  { id: 'sys-world-ending', title: 'World Ending' },
  { id: 'sys-covers', title: 'Covers' },
];

register('heat.galaxy', () => ({ systems: SYSTEMS }));

/** A Now making line clears quietly when its task is done, or after 7 days. */
const live = (fake: Fake, share: ProfileShare) =>
  !(share.clearsAt !== undefined && share.clearsAt <= fake.now) &&
  !(share.kind === 'now' && fake.store.task.get(share.sourceId)?.done === true);

derive((snap, fake) => {
  snap.records.profileShare = snap.records.profileShare.filter((s) => live(fake, s));
});

const shares = (fake: Fake) => [...fake.store.profileShare.values()].filter((s) => live(fake, s));

register('heat.share.now', (args, fake) => {
  const task = fake.store.task.get(args.taskId as Id);
  if (!task) refuse('No task has that id.');
  const text = String(args.text ?? '').trim();
  if (!text) refuse('Write the line first.');
  const share: ProfileShare = {
    id: fake.newId(),
    kind: 'now',
    sourceId: task.id,
    text,
    targetId: 'galaxy',
    clearsAt: fake.now + WEEK,
  };
  // One line at a time: showing a new one takes the old one down in the same step.
  const { undo } = fake.write('show Now making', ['profileShare'], () => {
    for (const [id, s] of fake.store.profileShare) if (s.kind === 'now') fake.store.profileShare.delete(id);
    fake.store.profileShare.set(share.id, share);
  });
  return { share, undo };
});

register('heat.share.timeline', (args, fake) => {
  const project = fake.store.project.get(args.projectId as Id);
  if (!project) refuse('No project has that id.');
  const targetId = String(args.targetId);
  if (!SYSTEMS.some((s) => s.id === targetId)) refuse('Your galaxy has no solar system with that id.');
  const share: ProfileShare = { id: fake.newId(), kind: 'timeline', sourceId: project.id, targetId };
  const { undo } = fake.write('show timeline', ['profileShare', 'project'], () => {
    for (const [id, s] of fake.store.profileShare)
      if (s.kind === 'timeline' && s.sourceId === project.id) fake.store.profileShare.delete(id);
    fake.store.profileShare.set(share.id, share);
    fake.store.project.set(project.id, { ...project, link: { kind: 'system', id: targetId } });
  });
  return { share, undo };
});

register('heat.share.hide', (args, fake) => {
  const share = fake.store.profileShare.get(args.id as Id);
  if (!share) refuse('Nothing is shown with that id.');
  return fake.write(share.kind === 'now' ? 'hide Now making' : 'hide timeline', ['profileShare'], () =>
    fake.store.profileShare.delete(share.id),
  );
});

// --- what someone opening your sun sees ---------------------------------------------

const days = (log: Record<DayKey, true>) =>
  Object.keys(log)
    .filter((d) => log[d])
    .sort();

register('heat.publicView', (_args, fake): PublicView => {
  const all = shares(fake);
  const now = all.find((s) => s.kind === 'now');
  const only = <T extends { public?: boolean }>(m: Map<string, T>) => [...m.values()].filter((r) => r.public === true);
  const items: PublicView['items'] = {};
  const put = (kind: Kind, rows: Record<string, unknown>[]) => {
    if (rows.length > 0) items[kind] = rows;
  };
  // Only the fields of that one record (3.15): never notes, difficulty, estimates, heat, time spent, a streak or a percentage.
  put(
    'task',
    only(fake.store.task).map((t) => ({ id: t.id, title: t.title, due: t.due, done: t.done })),
  );
  put(
    'project',
    only(fake.store.project).map((p) => ({
      id: p.id,
      title: p.title,
      status: p.status,
      targetDate: p.targetDate ?? null,
    })),
  );
  put(
    'milestone',
    only(fake.store.milestone).map((m) => ({ id: m.id, title: m.title, date: m.date, done: m.done })),
  );
  put(
    'habit',
    only(fake.store.habit).map((h) => ({ id: h.id, title: h.title, days: days(h.log) })),
  );
  put(
    'note',
    only(fake.store.note).map((n) => ({ id: n.id, title: n.title ?? null, text: n.markdown })),
  );
  put(
    'course',
    only(fake.store.course).map((c) => ({ id: c.id, code: c.code, name: c.name })),
  );
  put(
    'grade',
    only(fake.store.grade)
      .filter((g) => !g.pending)
      .map((g) => ({
        id: g.id,
        course: fake.store.course.get(g.courseId)?.code ?? '',
        item: g.title,
        score: g.score,
        outOf: g.outOf,
      })),
  );
  put(
    'focusSession',
    only(fake.store.focusSession).map((f) => ({
      id: f.id,
      task:
        (f.taskId ? fake.store.task.get(f.taskId)?.title : f.habitId ? fake.store.habit.get(f.habitId)?.title : '') ??
        '',
      startedAt: f.startedAt,
      focusMin: f.focusMin,
    })),
  );
  return {
    now: now?.text ? { text: now.text } : null,
    timelines: all
      .filter((s) => s.kind === 'timeline')
      .map((s) => ({
        projectId: s.sourceId,
        targetId: s.targetId,
        title: fake.store.project.get(s.sourceId)?.title ?? null,
        milestones: [...fake.store.milestone.values()]
          .filter((m) => m.projectId === s.sourceId)
          .sort((a, b) => a.order - b.order)
          .map((m) => ({ id: m.id, title: m.title, date: m.date, done: m.done })),
      })),
    items,
  };
});
