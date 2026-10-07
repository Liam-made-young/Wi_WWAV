// The strip's task half (2.2): Heat's LCD, read from the snapshot Heat reads.
// It shows the current task, or else the hottest open task, and is worked
// out again when Heat changes and when the minute turns, so "Today 4:00 PM"
// never goes stale; while a focus round runs it also ticks every second, for
// "focus 18:42 left". The words themselves are heat/model's; the heat level
// and the order of open tasks are the snapshot's.

import { useMemo } from 'react';
import type { HeatLevel } from '../heat/client';
import { type LcdData, taskHalf } from '../heat/model/lcd';
import type { Task } from '../heat/model/records';
import { useHeat, useNow } from '../heat/store';
import { stripText } from '../heat/timer';

export interface TaskHalf {
  taskId: string | null;
  line1: string;
  line2: string;
  meter: number;
  level: HeatLevel | null;
}

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

const EMPTY: LcdData = { tasks: [], occurrences: [], sessions: [], courses: [], milestones: [] };

/** The task half, and every open task with its heat for ⌘K. */
export function useTaskHalf(): { half: TaskHalf; tasks: { id: string; title: string; level: HeatLevel }[] } {
  const { snap, idx, tz } = useHeat();
  const timer = snap?.heatState.timer;
  const counting = timer !== undefined && timer.phase !== 'idle' && (timer.running ?? timer.endsAt !== null);
  const now = useNow(counting ? 1000 : 30_000);

  const data = useMemo<LcdData>(() => {
    if (!snap) return EMPTY;
    const r = snap.records;
    return {
      tasks: (r.task as unknown as Doc[]).map(asTask),
      occurrences: r.taskOccurrence as never,
      sessions: r.focusSession as never,
      courses: r.course as never,
      milestones: r.milestone as never,
    };
  }, [snap]);

  const half = taskHalf(data, now, tz, snap?.heatState.currentTaskId ?? null, stripText(timer, now));
  const tasks = (snap?.derived.lists.allOpen ?? []).flatMap((id) => {
    const t = idx.task.get(id);
    const level = snap?.derived.tasks[id]?.heat.level;
    return t && level ? [{ id, title: t.title, level }] : [];
  });
  const level = tasks.find((t) => t.id === half.taskId)?.level ?? null;
  return { half: { ...half, level }, tasks };
}
