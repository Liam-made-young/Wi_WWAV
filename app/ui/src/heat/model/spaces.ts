// Spaces (docs/SPEC.md 3.4): named filters over Today, Tasks, Calendar and
// Mail. Grades, Habits and calendar events ignore them, because they belong
// to the person rather than to a project. Classes, WWAV and Personal are
// created with the artifact's values (3.1), and the WWAV persona is the one
// 3.4 rewrites to cover the app.

import { DAY_MS, byHeat, heatOf } from './heat';
import * as copy from './copy';
import type { Capture, Course, Id, Milestone, Project, Space, Task, TaskOccurrence } from './records';
import { openTasks } from './recurrence';

export interface SidebarData {
  tasks: Task[];
  occurrences: TaskOccurrence[];
  captures: Capture[];
  projects: Project[];
  milestones: Milestone[];
  courses: Course[];
}

export function defaultSpaces(newId: () => Id): Space[] {
  return [
    {
      id: newId(),
      name: 'Classes',
      hue: 211,
      groupKind: 'course',
      groupLabel: 'Course',
      types: ['Homework', 'Quiz', 'Listening', 'Reading', 'Lab', 'Project', 'Exam prep', 'Other'],
      persona: 'a university student keeping up with coursework across several classes.',
    },
    {
      id: newId(),
      name: 'WWAV',
      hue: 6,
      groupKind: 'milestone',
      groupLabel: 'Milestone',
      types: ['Hardware', 'Software', 'Design', 'Music', 'Business', 'Content', 'Other'],
      persona:
        'a solo founder building WWAV: the PRANA handheld (Teensy 4.1, C++ firmware, PCBs), the Wi_WWAV desktop app, and an album.',
    },
    {
      id: newId(),
      name: 'Personal',
      hue: 145,
      groupKind: 'free',
      groupLabel: 'Area',
      types: ['Errand', 'Admin', 'Money', 'Health', 'Home', 'Social', 'Other'],
      persona: 'a person keeping up with errands, admin, money, health, home and friends.',
    },
  ];
}

/** The space filter; null is All. */
export const inSpace = (spaceId: Id | null | undefined) => (t: Task) => !spaceId || t.spaceId === spaceId;

/** The sidebar's All, then each space with its open count. */
export function spaceCounts(
  spaces: readonly Space[],
  data: Pick<SidebarData, 'tasks' | 'occurrences'>,
  now: number,
  tz: string,
): { all: number; spaces: { space: Space; open: number }[] } {
  const open = openTasks(data.tasks, data.occurrences, now, tz);
  return {
    all: open.length,
    spaces: spaces.map((space) => ({ space, open: open.filter(inSpace(space.id)).length })),
  };
}

/**
 * The Tasks sidebar's lists (3.6), each in heat order. Due this week is the
 * window weekly load sums: due within 7 days, overdue included.
 */
export function libraryLists(data: SidebarData, now: number, tz: string, spaceId?: Id | null) {
  const open = byHeat(openTasks(data.tasks.filter(inSpace(spaceId)), data.occurrences, now, tz), now);
  const somedayProjects = new Set(data.projects.filter((p) => p.status === 'someday').map((p) => p.id));
  return {
    inbox: data.captures.filter((c) => c.triagedAt === undefined),
    allOpen: open,
    hot: open.filter((t) => ['Hot', 'Overdue'].includes(heatOf(t, now).level)),
    dueThisWeek: open.filter((t) => t.due !== null && t.due - now <= 7 * DAY_MS),
    scheduled: open.filter((t) => t.scheduledDate !== undefined),
    someday: open.filter((t) => t.projectId !== undefined && somedayProjects.has(t.projectId)),
    done: data.tasks.filter((t) => t.done && inSpace(spaceId)(t)),
  };
}

/** "Courses", "Milestones", "Areas" */
export function groupHeading(space: Space): string {
  const label = space.groupLabel;
  return /[^aeiou]y$/i.test(label) ? `${label.slice(0, -1)}ies` : `${label}s`;
}

/** A task's group as the LCD names it: the course code, the milestone, or the area. */
export function groupName(task: Task, data: Pick<SidebarData, 'courses' | 'milestones'>): string | null {
  if (task.courseId) return data.courses.find((c) => c.id === task.courseId)?.code ?? null;
  if (task.milestoneId) return data.milestones.find((m) => m.id === task.milestoneId)?.title ?? null;
  return task.group ?? null;
}

/** The groups under a space in the sidebar, with their open counts. */
export function groupCounts(
  space: Space,
  data: SidebarData,
  now: number,
  tz: string,
): { id: string; name: string; open: number }[] {
  const all = data.tasks.filter(inSpace(space.id));
  const open = openTasks(all, data.occurrences, now, tz);
  const count = (key: (t: Task) => string | undefined, id: string) => open.filter((t) => key(t) === id).length;
  switch (space.groupKind) {
    case 'course': {
      const ids = new Set(all.map((t) => t.courseId));
      return data.courses
        .filter((c) => ids.has(c.id))
        .sort((a, b) => a.code.localeCompare(b.code))
        .map((c) => ({ id: c.id, name: c.code, open: count((t) => t.courseId, c.id) }));
    }
    case 'milestone':
      return data.milestones
        .filter((m) => m.spaceId === space.id)
        .sort((a, b) => a.order - b.order)
        .map((m) => ({ id: m.id, name: m.title, open: count((t) => t.milestoneId, m.id) }));
    case 'free': {
      const names = [...new Set(all.flatMap((t) => (t.group ? [t.group] : [])))].sort((a, b) => a.localeCompare(b));
      return names.map((g) => ({ id: g, name: g, open: count((t) => t.group, g) }));
    }
  }
}

/** "Add your first WWAV task and Heat will rank it." */
export function tasksEmptyLine(space: Space | null): string {
  return copy.tasks.empty(space?.name ?? null);
}
