// What Learn is showing, said to the prompt box (ask/context.ts): the tab,
// what is selected, and the items in view, by their titles and ids. It is
// read from the snapshot the tabs draw from, so it names what the person
// sees without any tab having to say so itself. The Database and Wiki tabs
// add their own parts.

import { useEffect } from 'react';
import type { Id } from '../heat/client';
import type { Selection, TabId } from '../heat/frame';
import { useHeat } from '../heat/store';
import { setScreen } from './context';

const MOST = 40;

export function useLearnScreen(tab: TabId, selection: Selection, spaceId: Id | null): void {
  const { snap, idx, date } = useHeat();
  useEffect(() => {
    if (!snap) return;
    const task = (id: Id) => {
      const t = idx.task.get(id);
      if (!t) return null;
      const course = t.courseId ? idx.course.get(t.courseId)?.code : undefined;
      return { table: 'task', id, title: t.title, ...(course ? { course } : {}) };
    };
    const tasks = (ids: readonly Id[]) =>
      ids
        .map(task)
        .filter((t) => t !== null)
        .slice(0, MOST);
    const d = snap.derived;
    let visible: unknown[] = [];
    if (tab === 'today') {
      const planned = d.today.planned.map((b) => idx.block.get(b)?.taskId).filter((id): id is Id => !!id);
      visible = tasks([...new Set([...planned, ...d.today.dueToday, ...d.today.hotUnplanned])]);
    } else if (tab === 'tasks' || tab === 'calendar') {
      visible = tasks(d.lists.allOpen.filter((id) => !spaceId || idx.task.get(id)?.spaceId === spaceId));
    } else if (tab === 'grades') {
      visible = snap.records.course.slice(0, MOST).map((c) => ({ table: 'course', id: c.id, title: c.code, name: c.name }));
    } else if (tab === 'habits') {
      visible = snap.records.habit.slice(0, MOST).map((h) => ({ table: 'habit', id: h.id, title: h.title }));
    } else if (tab === 'mail') {
      visible = (snap.records.mailThread as unknown as { id: string; subject?: string; from?: string }[])
        .slice(0, MOST)
        .map((m) => ({ table: 'mailThread', id: m.id, title: m.subject ?? '', from: m.from }));
    }
    const picked = selection.taskId ?? (selection.blockId ? idx.block.get(selection.blockId)?.taskId : undefined);
    const space = spaceId ? idx.space.get(spaceId)?.name : undefined;
    setScreen('learn', {
      tab,
      date,
      ...(space ? { space } : {}),
      selection: picked ? [task(picked)].filter(Boolean) : [],
      visible,
    });
  }, [snap, idx, date, tab, selection, spaceId]);
}
