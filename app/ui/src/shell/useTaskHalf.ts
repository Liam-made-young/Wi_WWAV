// The strip's task half (2.2): Heat's LCD, read from Heat's records in the
// core. It shows the current task, or else the hottest open task, and is
// worked out again when a record changes and when the minute turns, so
// "Today 4:00 PM" never goes stale. The words themselves are heat/model's.

import { useEffect, useState } from 'react';
import { call } from '../bridge';
import { HEAT_STATE_ID, KINDS } from '../heat/kinds';
import { type HeatLevel, heatOf } from '../heat/model/heat';
import { type LcdData, taskHalf } from '../heat/model/lcd';
import type { Task } from '../heat/model/records';
import { openTasks } from '../heat/model/recurrence';
import { useCoreEvent } from './hooks';

export interface TaskHalf {
  taskId: string | null;
  line1: string;
  line2: string;
  meter: number;
  level: HeatLevel | null;
}

const READ = [KINDS.task, KINDS.occurrence, KINDS.session, KINDS.course, KINDS.milestone, KINDS.state];

type Doc = Record<string, unknown> & { id: string };

// A record another room wrote in part still reads as a task: what it
// leaves out takes Heat's defaults.
function asTask(r: Doc): Task {
  return {
    spaceId: '',
    title: '',
    type: '',
    difficulty: 3,
    estMin: null,
    adjustMin: 0,
    notes: '',
    doneAt: null,
    source: 'you',
    ...r,
    due: typeof r.due === 'number' ? r.due : null,
    done: r.done === true,
  } as Task;
}

async function readAll(): Promise<{ data: LcdData; current: string | null }> {
  const lists = await Promise.all(
    READ.map((kind) => call<{ records: Doc[] }>('records.list', { kind }).then((r) => r.records)),
  );
  const [tasks, occurrences, sessions, courses, milestones, state] = lists;
  const current = state.find((s) => s.id === HEAT_STATE_ID)?.currentTaskId;
  return {
    data: {
      tasks: tasks.map(asTask),
      occurrences: occurrences as never,
      sessions: sessions as never,
      courses: courses as never,
      milestones: milestones as never,
    },
    current: typeof current === 'string' ? current : null,
  };
}

const EMPTY: LcdData = { tasks: [], occurrences: [], sessions: [], courses: [], milestones: [] };

/** The task half, and every open task with its heat for ⌘K. */
export function useTaskHalf(tz: string): { half: TaskHalf; tasks: { id: string; title: string; level: HeatLevel }[] } {
  const [read, setRead] = useState<{ data: LcdData; current: string | null }>({ data: EMPTY, current: null });
  const [now, setNow] = useState(() => Date.now());

  const refresh = () => {
    readAll().then(setRead, () => {});
  };
  useEffect(refresh, []);
  useCoreEvent<{ kinds: string[] }>('records', ({ kinds }) => {
    if (kinds.some((k) => (READ as string[]).includes(k))) refresh();
  });

  // The next minute, then again: due phrases are written to the minute.
  useEffect(() => {
    const t = setTimeout(() => setNow(Date.now()), 60_000 - (now % 60_000));
    return () => clearTimeout(t);
  }, [now]);

  const half = taskHalf(read.data, now, tz, read.current, null);
  const open = openTasks(read.data.tasks, read.data.occurrences, now, tz);
  const tasks = open.map((t) => ({ id: t.id, title: t.title, level: heatOf(t, now).level }));
  const level = tasks.find((t) => t.id === half.taskId)?.level ?? null;
  return { half: { ...half, level }, tasks };
}
