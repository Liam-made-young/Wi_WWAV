// Heat's records, as docs/SPEC.md 3.15 gives them. Instants are epoch ms.
// Days are "YYYY-MM-DD" in the person's time zone. A block's `start` is
// minutes after that day's local midnight, as the time column reads it.

import type { DayKey } from '../../shared/time/zone';

export type { DayKey };
export type Id = string;
/** Where a focus session was started: the three views (docs/SPEC.md 2.1). */
export type View = 'heat' | 'space' | 'console';

export interface Space {
  id: Id;
  name: string;
  hue: number;
  groupKind: 'course' | 'milestone' | 'free';
  groupLabel: string;
  types: string[];
  persona: string;
}

export interface Link {
  kind: 'session' | 'system' | 'work';
  id: Id;
}

/** Where a task came from; Get Info names it ("Brightspace calendar"). */
export type TaskSource = 'you' | 'calendar' | 'ical' | 'mail' | 'capture';

export interface Task {
  id: Id;
  spaceId: Id;
  title: string;
  type: string;
  courseId?: Id;
  projectId?: Id;
  milestoneId?: Id;
  /** The group of a space whose group kind is "free" (Personal's Area). 3.15 has no field for it. */
  group?: string;
  parentTaskId?: Id;
  due: number | null;
  scheduledDate?: DayKey;
  rrule?: string;
  difficulty: number;
  estMin: number | null;
  adjustMin: number;
  notes: string;
  link?: Link;
  done: boolean;
  doneAt: number | null;
  source: TaskSource;
}

/** A finished occurrence of a recurring task. Its presence is the tick. */
export interface TaskOccurrence {
  id: Id;
  taskId: Id;
  date: DayKey;
  doneAt: number;
}

export interface TimeBlock {
  id: Id;
  taskId?: Id;
  habitId?: Id;
  date: DayKey;
  start: number;
  minutes: number;
  origin: 'you' | 'plan';
}

export interface FocusSession {
  id: Id;
  taskId?: Id;
  habitId?: Id;
  startedAt: number;
  endedAt: number;
  focusMin: number;
  interruptions: number;
  view: View;
}

export interface Project {
  id: Id;
  spaceId: Id;
  title: string;
  status: 'active' | 'on_hold' | 'someday' | 'archived';
  targetDate?: DayKey;
  link?: Link;
}

export interface Milestone {
  id: Id;
  spaceId: Id;
  projectId?: Id;
  title: string;
  date: DayKey;
  done: boolean;
  order: number;
  link?: Link;
}

export interface Habit {
  id: Id;
  title: string;
  minutes?: number;
  log: Record<DayKey, true>;
  showCounter: boolean;
}

export interface Term {
  id: Id;
  name: string;
}

export interface GradeCategory {
  id: Id;
  name: string;
  weight: number;
  keywords: string[];
}

export interface LetterStep {
  letter: string;
  min: number;
}

export interface Course {
  id: Id;
  termId: Id;
  code: string;
  name: string;
  categories: GradeCategory[];
  /** The course's own scale, highest first; the default scale when absent. */
  scale?: LetterStep[];
  notes: string;
}

export interface Grade {
  id: Id;
  courseId: Id;
  categoryId: Id | null;
  title: string;
  score: number | null;
  outOf: number;
  dropped: boolean;
  /** A grade notice from mail, waiting for its score ("Enter score"). */
  pending: boolean;
  link?: string;
  source: 'you' | 'mail' | 'valence';
}

export interface Capture {
  id: Id;
  text: string;
  link?: Link;
  triagedAt?: number;
  resultType?: 'task' | 'note' | 'project' | 'upload';
  resultId?: Id;
}

/** An event from a calendar's private iCal address (Google's included): read-only, drawn grey behind blocks (3.5). */
export interface CalendarEvent {
  id: Id;
  title: string;
  start: number;
  end: number;
  allDay: boolean;
}

/** What the syncs remember between runs. 3.15 has no record for it; the artifact keeps it. */
export interface SyncState {
  processedMailIds: string[];
  lastSyncAt: number | null;
}
