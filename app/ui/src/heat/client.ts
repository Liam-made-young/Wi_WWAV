// Heat's line to the core: every command in docs/HEAT.md, typed, and the
// shapes of the records (docs/SPEC.md 3.16) and of the snapshot. Views read
// derived values from the snapshot and never work them out themselves.
//
// The same calls reach the real core (Tauri, or the dev bridge) or the fake
// one in ./fake for tests and for `npm run dev` without a core:
//
//   const heat = heatClient();             // the real core
//   const heat = heatClient(fakeTransport()); // tests

import { call as coreCall, on as coreOn } from '../bridge';

export type Id = string;
/** "YYYY-MM-DD" in the person's time zone. */
export type DayKey = string;

export interface Link {
  kind: 'session' | 'system' | 'work';
  id: Id;
}

export interface Space {
  id: Id;
  name: string;
  hue: number;
  groupKind: 'course' | 'milestone' | 'free';
  groupLabel: string;
  types: string[];
  persona: string;
}

export type TaskSource = 'you' | 'calendar' | 'ical' | 'mail' | 'capture' | 'claude';

export interface Task {
  id: Id;
  spaceId: Id;
  title: string;
  type: string;
  courseId?: Id;
  projectId?: Id;
  milestoneId?: Id;
  group?: string;
  parentTaskId?: Id;
  /** Epoch ms, or null for none. */
  due: number | null;
  scheduledDate?: DayKey;
  rrule?: string;
  difficulty: number;
  estMin: number | null;
  estBy?: 'you' | 'claude' | 'default';
  estReason?: string;
  adjustMin: number;
  notes: string;
  link?: Link;
  done: boolean;
  doneAt: number | null;
  source: TaskSource;
  sourceId?: string;
  /** The sentence Claude gave when it added the task (3.6). */
  claudeReason?: string;
  public?: boolean;
}

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
  /** Minutes after the day's local midnight. */
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
  source?: 'timer' | 'claude';
  public?: boolean;
}

export interface Project {
  id: Id;
  spaceId: Id;
  title: string;
  status: 'active' | 'on_hold' | 'someday' | 'archived';
  targetDate?: DayKey;
  link?: Link;
  public?: boolean;
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
  public?: boolean;
}

export interface Habit {
  id: Id;
  title: string;
  minutes?: number;
  log: Record<DayKey, true>;
  showCounter: boolean;
  public?: boolean;
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
  scale?: LetterStep[];
  notes: string;
  public?: boolean;
}

export interface Grade {
  id: Id;
  courseId: Id;
  categoryId: Id | null;
  title: string;
  score: number | null;
  outOf: number;
  dropped: boolean;
  pending: boolean;
  link?: string;
  postedAt?: number;
  source: 'you' | 'mail' | 'valence' | 'claude';
  public?: boolean;
  /** The Mail row Claude recorded this notice from (`add_pending_grade`'s `mail_thread_id`); beyond docs/HEAT.md. */
  mailThreadId?: Id;
}

export interface MailThread {
  id: Id;
  gmailThreadId: string;
  subject: string;
  from: string;
  receivedAt: number;
  course?: string;
  state: 'grade' | 'task' | 'nothing';
  reason: string;
  taskId?: Id;
  recordedBy: 'claude';
}

export interface Calendar {
  id: Id;
  name: string;
  kind: 'brightspace' | 'ical';
  keychainRef: string;
  lastSyncedAt: number | null;
}

export interface Capture {
  id: Id;
  text: string;
  link?: Link;
  triagedAt?: number;
  resultType?: 'task' | 'note' | 'project' | 'upload';
  resultId?: Id;
}

export interface DailyNote {
  date: DayKey;
  markdown: string;
  public?: boolean;
}

export interface Note {
  id: Id;
  title?: string;
  markdown: string;
  projectId?: Id;
  link?: Link;
  public?: boolean;
}

export interface ProfileShare {
  id: Id;
  kind: 'now' | 'timeline';
  sourceId: Id;
  text?: string;
  targetId: Id;
  clearsAt?: number;
}

export interface CalendarEvent {
  id: Id;
  calendarId?: Id;
  title: string;
  start: number;
  end: number;
  allDay: boolean;
}

export interface Draft {
  taskId: Id;
  date: DayKey;
  start: number;
  minutes: number;
  leftMin: number;
  reason: string;
  leftLine: string | null;
}

export interface HeatTimer {
  phase: 'focus' | 'break' | 'idle';
  round: number;
  /** Epoch ms while the round or break is running; null while paused, waiting or idle. */
  endsAt: number | null;
  /** Beyond 3.16's three fields, so the LCD can draw a paused round and a break that waits for a press. */
  running?: boolean;
  /** Time left while paused or waiting. */
  leftMs?: number;
  /** The whole length of this focus round or break. */
  lengthMs?: number;
  /** The focus length chosen: 25, 50 or a custom 10-90. */
  focusMin?: number;
  /** What the round in progress is on. */
  taskId?: Id | null;
  /** Or the habit it is on, which ticks itself when the round reaches the habit's length; beyond docs/HEAT.md. */
  habitId?: Id | null;
  /** What just ended ("Focus done. 25m logged to Mix the second verse."), until the next press. */
  note?: string | null;
}

export interface HeatState {
  currentTaskId?: Id | null;
  timer: HeatTimer;
  planDrafts: Draft[];
}

/** The kinds `heat.put`, `heat.patch` and `heat.delete` take. */
export interface Records {
  space: Space;
  task: Task;
  taskOccurrence: TaskOccurrence;
  timeBlock: TimeBlock;
  focusSession: FocusSession;
  project: Project;
  milestone: Milestone;
  habit: Habit;
  term: Term;
  course: Course;
  grade: Grade;
  mailThread: MailThread;
  calendar: Calendar;
  capture: Capture;
  dailyNote: DailyNote;
  note: Note;
  profileShare: ProfileShare;
}
export type Kind = keyof Records;

export type HeatLevel = 'Overdue' | 'Hot' | 'Warm' | 'Cool' | 'Done';

export interface TaskDerived {
  heat: { v: number | null; level: HeatLevel };
  actualMin: number;
  estimate: { min: number; by: 'you' | 'claude' | 'default'; reason: string | null };
  /** A recurring task's next occurrence. */
  next?: DayKey | null;
}

/** The School sheet's values (3.11). The iCal address itself is in the Keychain: only whether one is saved is read back. */
export interface School {
  name: string;
  host: string;
  icalSaved: boolean;
  codePattern: string;
  termStart: DayKey;
  termEnd: DayKey;
}

export interface Snapshot {
  now: number;
  date: DayKey;
  zone: string;
  records: { [K in Kind]: Records[K][] };
  heatState: HeatState;
  events: CalendarEvent[];
  /** The School sheet as saved, or null before one is; beyond docs/HEAT.md. */
  school?: School | null;
  derived: {
    tasks: Record<Id, TaskDerived>;
    today: {
      header: string;
      planned: Id[];
      dueToday: Id[];
      recurringToday: { kind: 'task' | 'habit'; id: Id }[];
      hotUnplanned: Id[];
    };
    hotTasks: Id[];
    lists: Record<'inbox' | 'allOpen' | 'hot' | 'dueThisWeek' | 'scheduled' | 'someday' | 'done', Id[]>;
    averages: { space?: string | null; type: string; minutes: number; count: number }[];
    courses: Record<
      Id,
      {
        currentPct: number | null;
        decidedPct: number;
        letter: string | null;
        weights: string | null;
        /** Each category's percentage so far, null while nothing in it is graded; beyond docs/HEAT.md. */
        categories?: Record<Id, number | null>;
      }
    >;
    habits: Record<Id, { today: boolean; record: string; /** The streak sentence, only for a habit whose counter is switched on; beyond docs/HEAT.md. */ counter?: string }>;
    status: string;
    /** A recurring task's occurrences in the snapshot's window, for Calendar's pills and flags. */
    occurrences?: { taskId: Id; date: DayKey; done: boolean }[];
  };
}

export interface PublicView {
  now: { text: string } | null;
  timelines: { projectId: Id; targetId: Id; title: string | null; milestones: { id: Id; title: string; date: DayKey; done: boolean }[] }[];
  items: Partial<Record<Kind, Record<string, unknown>[]>>;
}

export interface ClaudeSettings {
  helper: string;
  desktop: string;
  code: string;
  tools: { name: string; on: boolean }[];
  recent: { txnId: string; label: string; reason: string; at: number; undone: boolean }[];
}

/** What every write answers: the Edit menu's text for it, or null if nothing changed. */
export interface Undo {
  undo: string | null;
}

/** The two things a client needs: a call, and an event to listen to. */
export interface Transport {
  call<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  on(event: string, handler: (payload: unknown) => void): () => void;
}

const real: Transport = {
  call: <T>(cmd: string, args: Record<string, unknown> = {}) => coreCall<T>(cmd, args),
  on: (event, handler) => coreOn(event, handler),
};

export function heatClient(t: Transport = real) {
  const c = t.call;
  return {
    /** Fires after any Heat change, the app's own or Claude's. */
    onChange: (handler: (kinds: Kind[]) => void) => t.on('heat', (p) => handler(((p as { kinds?: Kind[] })?.kinds ?? []) as Kind[])),

    snapshot: (date: DayKey, from?: DayKey, to?: DayKey) => c<Snapshot>('heat.snapshot', { date, from, to }),
    whatItWouldTake: (courseId: Id, letter: string) => c<{ text: string }>('heat.whatItWouldTake', { courseId, letter }),
    reviewWeek: (weekStart: DayKey) => c<Record<string, unknown>>('heat.review.week', { weekStart }),
    publicView: () => c<PublicView>('heat.publicView'),
    /** Your galaxy's solar systems, for the sidebar's drop targets and a project's timeline; beyond docs/HEAT.md. */
    galaxy: () => c<{ systems: { id: Id; title: string }[] }>('heat.galaxy'),

    put: <K extends Kind>(kind: K, record: Omit<Records[K], 'id'> & { id?: Id }) =>
      c<{ record: Records[K] } & Undo>('heat.put', { kind, record }),
    patch: <K extends Kind>(kind: K, id: Id, set: Partial<Records[K]>) => c<{ record: Records[K] } & Undo>('heat.patch', { kind, id, set }),
    delete: (kind: Kind, id: Id) => c<Undo>('heat.delete', { kind, id }),

    done: (taskId: Id, done: boolean, date?: DayKey) => c<{ task: Task; took?: string } & Undo>('heat.done', { taskId, done, date }),
    estimate: (taskId: Id, set: { difficulty?: number; estMin?: number }) =>
      c<{ task: Task; clamped: boolean } & Undo>('heat.estimate', { taskId, ...set }),
    tookTime: (taskId: Id, minutes: number) => c<{ task: Task } & Undo>('heat.tookTime', { taskId, minutes }),

    planMake: (date: DayKey, dayEnds?: number) => c<{ drafts: Draft[]; unplanned: Id[]; minutesLeft: number }>('heat.plan.make', { date, dayEnds }),
    planAccept: (date: DayKey, taskIds?: Id[]) => c<{ blocks: TimeBlock[] } & Undo>('heat.plan.accept', { date, taskIds }),
    planClear: () => c<Record<string, never>>('heat.plan.clear'),
    putBlock: (block: Omit<TimeBlock, 'id' | 'origin'> & { id?: Id }) => c<{ block: TimeBlock } & Undo>('heat.block.put', block),

    setCurrent: (taskId: Id | null) => c<Record<string, never>>('heat.current.set', { taskId }),
    focus: (
      step: 'start' | 'pause' | 'resume' | 'interrupt' | 'stop' | 'finish',
      args: { taskId?: Id; habitId?: Id; length?: number } = {},
    ) =>
      c<{ heatState: HeatState; logged?: FocusSession } & Undo>(`heat.focus.${step}`, args),

    capture: (text: string, link?: Link) => c<{ capture: Capture; inbox: string } & Undo>('heat.capture.add', { text, link }),
    triage: (id: Id, to: 'task' | 'note' | 'project' | 'upload', record?: Record<string, unknown>) =>
      c<{ result: unknown } & Undo>('heat.capture.triage', { id, to, record }),

    score: (gradeId: Id, score: number) => c<{ grade: Grade } & Undo>('heat.score', { gradeId, score }),
    setPublic: (kind: Kind, id: Id, isPublic: boolean) =>
      c<{ record: unknown; sentence?: string } & Undo>('heat.public.set', { kind, id, public: isPublic }),
    shareNow: (taskId: Id, text: string) => c<{ share: ProfileShare } & Undo>('heat.share.now', { taskId, text }),
    shareTimeline: (projectId: Id, targetId: Id) => c<{ share: ProfileShare } & Undo>('heat.share.timeline', { projectId, targetId }),
    hideShare: (id: Id) => c<Undo>('heat.share.hide', { id }),
    reviewComplete: (weekStart: DayKey, note: string) => c<{ note: Note } & Undo>('heat.review.complete', { weekStart, note }),
    importArtifact: (json: unknown) => c<{ counts: Record<string, number> } & Undo>('heat.import', { json }),

    calendars: {
      add: (name: string, kind: Calendar['kind'], url: string) => c<{ calendar: Calendar }>('heat.calendars.add', { name, kind, url }),
      remove: (id: Id) => c<Record<string, never>>('heat.calendars.remove', { id }),
      sync: (id?: Id) => c<{ line: string }>('heat.calendars.sync', { id }),
    },
    setSchool: (school: { name: string; host: string; icalUrl?: string; codePattern: string; termStart: DayKey; termEnd: DayKey }) =>
      c<Record<string, never>>('heat.school.set', school),

    claude: {
      get: () => c<ClaudeSettings>('heat.claude.get'),
      setTool: (name: string, on: boolean) => c<Record<string, never>>('heat.claude.setTool', { name, on }),
      undo: (txnId: string) => c<{ label: string }>('history.undoEntry', { txnId }),
    },
  };
}

export type HeatClient = ReturnType<typeof heatClient>;
