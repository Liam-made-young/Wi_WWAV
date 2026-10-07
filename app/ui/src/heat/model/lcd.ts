// The Now strip's task half (docs/SPEC.md 2.2), which is Heat's LCD moved
// into the title bar (3.1, 3.3). It shows the current task, or else the
// hottest open task: "Hot: Grammar quiz 4" over "Tomorrow 11:59 PM, JPN 102.
// This week: 3h 20m across 5 tasks", or, while focus runs, "Today 4:00 PM,
// JPN 201 · focus 18:40 left". The meter is that task's heat.

import * as copy from './copy';
import { estimateContext, weeklyLoad, weeklyLoadLine } from './estimate';
import { byHeat, duePhrase, heatOf, tubeFill } from './heat';
import type { Course, FocusSession, Id, Milestone, Task, TaskOccurrence } from './records';
import { openTasks } from './recurrence';
import { groupName } from './spaces';

export interface LcdData {
  tasks: Task[];
  occurrences: TaskOccurrence[];
  sessions: FocusSession[];
  courses: Course[];
  milestones: Milestone[];
}

/**
 * `focus` is focus.ts's strip text ("focus 18:40 left"), or null when no
 * round is under way.
 */
export function taskHalf(
  data: LcdData,
  now: number,
  tz: string,
  currentTaskId: Id | null,
  focus: string | null,
): { taskId: Id | null; line1: string; line2: string; meter: number } {
  const open = openTasks(data.tasks, data.occurrences, now, tz);
  const shown = open.find((t) => t.id === currentTaskId) ?? byHeat(open, now)[0];
  if (!shown) return { taskId: null, line1: copy.strip.allClear, line2: copy.strip.nothingOpen, meter: 0 };
  const heat = heatOf(shown, now);
  const about = [shown.due === null ? null : duePhrase(shown.due, now, tz), groupName(shown, data)]
    .filter((x) => x !== null)
    .join(', ');
  const tail = focus ?? weeklyLoadLine(weeklyLoad(open, estimateContext(data.tasks, data.sessions), now));
  const line2 = about ? `${about}${focus ? ' · ' : '. '}${tail}` : tail;
  return { taskId: shown.id, line1: copy.strip.task(heat.level, shown.title), line2, meter: tubeFill(heat) };
}
