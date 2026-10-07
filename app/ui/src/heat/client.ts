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

/**
 * A task type: what a kind of work usually takes. The words in `patterns` pick
 * it when a title holds one whole; a number it leaves out comes from the same
 * type a level down (a course or project, then the space, then the defaults).
 */
export interface TypeDef {
  name: string;
  patterns: string[];
  estMin: number | null;
  difficulty: number | null;
  /** The grade category this type's items count toward, by name. Courses only. */
  category?: string;
}

export interface Space {
  id: Id;
  name: string;
  hue: number;
  groupKind: 'course' | 'milestone' | 'free';
  groupLabel: string;
  types: string[];
  /** The space's own types, between a home's and the defaults. */
  typeDefs?: TypeDef[];
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
  /** Who picked the type: you, Claude, or the title's words. */
  typeBy?: 'you' | 'claude' | 'rule';
  difficulty: number;
  difficultyBy?: 'you' | 'claude' | 'type' | 'default';
  estMin: number | null;
  /** `type`: the task's type gave the minutes. `default`: no type matched, and they are the catch-all's. */
  estBy?: 'you' | 'claude' | 'type' | 'default';
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
  /** The note whose checkbox it was made from. */
  noteId?: Id;
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
  /** The project's own task types. */
  types?: TypeDef[];
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
  /** How many of the category's lowest scores the course drops, when its syllabus says so. */
  dropLowest?: number;
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
  /** `stub`: found from a calendar or a task, and still waiting for its syllabus. None reads as confirmed. */
  status?: 'stub' | 'confirmed';
  /** The course's own task types. */
  types?: TypeDef[];
  /** The syllabus its weights and types were read from. */
  syllabusSource?: { name: string; pages: number; importedAt: number };
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
  /** The mail account it is from: an address in `Snapshot.mailAccounts`. */
  account?: string;
  /** How pressing Claude found it. None reads as normal. */
  priority?: 'urgent' | 'high' | 'normal' | 'low';
  category?: 'school' | 'work' | 'money' | 'people' | 'updates' | 'promotions' | 'other';
}

/** One message of a thread as Claude saved it for Mail's reader: plain text, kept on this Mac. */
export interface MailMessage {
  id?: string;
  from: string;
  to?: string;
  sentAt: number;
  text: string;
}

/** Where a thread sits in the mailbox. A thread with no entry is read and in the inbox. */
export interface MailPlace {
  unread: boolean;
  archived: boolean;
}

/** Something asked for in Mail, waiting for Claude to do it in Gmail, or done, or failed. */
export interface MailAction {
  id: Id;
  kind: 'send' | 'reply' | 'archive' | 'unarchive' | 'markRead' | 'markUnread';
  status: 'queued' | 'done' | 'failed';
  threadId?: string;
  to?: string;
  cc?: string;
  subject?: string;
  body?: string;
  error?: string;
  createdAt: number;
  doneAt?: number;
}

/** How mail is kept in step: the last read's line, what a run is doing now, and the outbox's counts. */
export interface MailSync {
  line: string | null;
  at: number | null;
  doing: 'reading' | 'sending' | null;
  background: boolean;
  queued: number;
  failed: number;
}

/** A mail account Claude reads (docs/SPEC.md 3.10). Learn holds no password for it. */
export interface MailAccount {
  address: string;
  name: string;
  /** `connector`: the mailbox Claude's Gmail connector reads. `forward`: forwarded into it. */
  via: 'connector' | 'forward';
  forwardTo?: string;
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
  /** Where it is filed: a course, a space, or neither. */
  courseId?: Id;
  spaceId?: Id;
  /** Its file in the notes folder, as the core keeps it. */
  file?: string;
  /** When a captured page was taken. */
  capturedAt?: number;
  /** True while a captured page waits in the Notes inbox to be filed. */
  inbox?: boolean;
  attachments?: string[];
  createdAt?: number;
  updatedAt?: number;
  source?: string;
  public?: boolean;
}

/** One day of a commitment that doesn't go as usual (docs/COMMITMENTS.md). */
export interface CommitmentException {
  date: DayKey;
  kind: 'skip' | 'move';
  toDate?: DayKey;
  start?: number;
  end?: number;
  location?: string;
  note?: string;
  source?: 'you' | 'mail' | 'claude';
}

export type CommitmentKind = 'class' | 'work' | 'commute' | 'other';

/** A fixed thing in the week: a class, a shift, a commute. Times are minutes after local midnight. */
export interface Commitment {
  id: Id;
  title: string;
  kind: CommitmentKind;
  location: string;
  start: number;
  end: number;
  /** Absent: it happens once, on `from`. */
  rrule?: string;
  from: DayKey;
  until?: DayKey;
  exceptions: CommitmentException[];
  bufferBefore: number;
  bufferAfter: number;
  hardness: 'fixed' | 'flexible';
  courseId?: Id;
  spaceId?: Id;
  source: 'you' | 'paste' | 'photo' | 'ics' | 'claude';
  sourceId?: string;
  weekOf?: DayKey;
  feedId?: Id;
}

/** Days classes don't meet. */
export interface TermBreak {
  id: Id;
  title: string;
  from: DayKey;
  to: DayKey;
  source?: string;
}

/** One day a commitment happens on, with every exception and break already applied. */
export interface CommitmentOccurrence {
  commitmentId: Id;
  date: DayKey;
  start: number;
  end: number;
  title: string;
  kind: CommitmentKind;
  location: string;
  bufferBefore: number;
  bufferAfter: number;
  hardness: 'fixed' | 'flexible';
  courseId?: Id;
  spaceId?: Id;
  /** The day an exception moved it from. */
  movedFrom?: DayKey;
  /** Its space's hue; null with no space. */
  hue: number | null;
  /** Its course, as "JPN 101 · Beginning Japanese I"; null with none. */
  label: string | null;
}

/** What a day has left once commitments, their travel and sleep are out. */
export interface FreeTime {
  date: DayKey;
  freeMin: number;
  plannedMin: number;
  overMin: number;
  spans: [number, number][];
  /** "3h 20m free today. Planned 4h. Move 40m?" */
  line: string;
}

export type ScheduleMode = 'schedule' | 'week' | 'breaks';

/** A schedule read and waiting to be applied. The fields below `error` are there once it is ready. */
export interface CommitmentDraft {
  id: Id;
  mode: ScheduleMode;
  source: 'paste' | 'photo' | 'ics';
  fileName: string;
  weekOf: DayKey;
  createdAt: number;
  state: 'reading' | 'ready' | 'failed';
  error?: string;
  items?: {
    title: string;
    kind: CommitmentKind;
    start: number;
    end: number;
    location: string;
    from: DayKey;
    until: DayKey | null;
    days: string[];
    /** "MWF 10:00 AM to 10:50 AM" */
    when: string;
    course: string | null;
    /** False: a commitment just like it is there already, and it won't be added. */
    isNew: boolean;
    newCourse: boolean;
  }[];
  breaks?: { title: string; from: DayKey; to: DayKey; isNew: boolean }[];
  /** How many of that week's shifts accepting it replaces. */
  replaces?: number;
  unread?: number;
  /** "3 classes and 2 shifts. 1 new course." */
  line?: string;
}

/** An exception a mail asked for, waiting for one tap. */
export interface PendingException {
  id: Id;
  commitmentId: Id;
  title: string;
  kind: 'skip' | 'move';
  exception: CommitmentException;
  /** "EGR 101 is canceled Thursday, Oct 8. Skip it?" */
  line: string;
  /** What the tap is called: "Skip it", "Move it". */
  act: string;
  mailThreadId?: string;
  subject?: string;
  createdAt: number;
}

export interface CommitmentsSnapshot {
  days: Record<DayKey, CommitmentOccurrence[]>;
  list: { id: Id; when: string; range: string; days: string[]; hue: number | null; label: string | null }[];
  next: { commitmentId: Id; title: string; date: DayKey; start: number; leaveAt: number | null; line: string } | null;
  free: FreeTime;
  conflicts: { blockId: Id; commitmentId: Id; date: DayKey; bufferOnly: boolean; line: string }[];
  sleep: { from: number; to: number };
  term: { start: DayKey | null; end: DayKey | null };
  drafts: CommitmentDraft[];
  pending: PendingException[];
  feeds: { id: Id; name: string; lastSyncedAt: number | null }[];
}

/** What `heat.commitment.create` and `.update` take. Times are minutes or a time such as "10:00" or "5pm". */
export interface CommitmentInput {
  title?: string;
  kind?: CommitmentKind;
  start?: number | string;
  end?: number | string;
  /** Weekday codes: MO TU WE TH FR SA SU. An empty list: no repeat. */
  days?: string[];
  /** One day, for something that happens once. */
  date?: DayKey;
  rrule?: string;
  interval?: number;
  from?: DayKey;
  until?: DayKey | null;
  location?: string;
  bufferBefore?: number;
  bufferAfter?: number;
  hardness?: 'fixed' | 'flexible';
  courseId?: Id | null;
  /** A course by its code; a class naming one Learn doesn't hold makes a stub. */
  course?: string;
  spaceId?: Id | null;
  space?: string;
}

/** What a note links to, what links to it, its tags and its checkboxes (docs/NOTES.md). */
export interface NoteIndex {
  title: string;
  excerpt: string;
  tags: string[];
  links: { target: string; shown: string | null; kind: 'note' | 'course' | 'task' | 'missing'; id: Id | null; line: number }[];
  backlinks: { id: Id; title: string; line: string }[];
  /** A box that is a task carries `taskId`, and its `done` is the task's. */
  boxes: { line: number; text: string; done: boolean; taskId: Id | null }[];
  updatedAt: number | null;
  /** Its course ("JPN 101 · Beginning Japanese I") or its space's name; null for neither. */
  label: string | null;
  hue: number | null;
}

export interface NoteSuggestion {
  tasks: { title: string; due?: DayKey | null; taskId?: Id | null }[];
  terms: string[];
}

export interface NotesSnapshot {
  /** The notes folder on disk. */
  folder: string;
  index: Record<Id, NoteIndex>;
  /** Newest first. */
  order: Id[];
  tags: { tag: string; count: number }[];
  /** Captured pages waiting to be filed. */
  inbox: Id[];
  suggestions: Record<Id, NoteSuggestion>;
  /** Today's daily note, the note titled with the day. */
  daily: Id | null;
  capture: {
    folders: string[];
    watching: boolean;
    waiting: number;
    /** "Reading IMG_2211.jpg", while a page is being read. */
    doing: string | null;
    /** Whether a captured page may be sent to Claude. */
    claude: boolean;
    /** The on-device reader: there, being built, or not to be had. */
    reader: 'ready' | 'building' | 'missing';
  };
}

/** One line Learn shows quietly, with its undo or its one act. */
export interface Notice {
  id: string;
  kind: 'filed' | 'leave' | 'exception' | 'capture';
  text: string;
  at: number;
  noteId?: Id;
  undo?: { txnId: string };
  act?: { label: string; cmd: string; args: Record<string, unknown> };
}

export interface NoteHit {
  id: Id;
  title: string;
  snippet: string;
  score: number;
}

/** Where a note is filed to: a course, a space, or `none`. */
export type NotePlace = { courseId: Id } | { course: string } | { spaceId: Id } | { space: string } | { none: true };

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
  commitment: Commitment;
  termBreak: TermBreak;
}
export type Kind = keyof Records;

export type HeatLevel = 'Overdue' | 'Hot' | 'Warm' | 'Cool' | 'Done';

/** Where a task belongs: its course or its project. A task with neither just lives in its space. */
export interface TaskHome {
  kind: 'course' | 'project';
  id: Id;
  label: string;
}

/** A task's minutes, who gave them, and for a type's the level the type was found at. */
export interface TaskEstimate {
  min: number;
  by: 'you' | 'claude' | 'type' | 'default';
  reason: string | null;
  typeFrom?: 'course' | 'project' | 'space' | 'global' | null;
}

export interface TaskDerived {
  heat: { v: number | null; level: HeatLevel };
  actualMin: number;
  estimate: TaskEstimate;
  home?: TaskHome | null;
  /** A recurring task's next occurrence. */
  next?: DayKey | null;
}

export interface CourseDerived {
  currentPct: number | null;
  decidedPct: number;
  letter: string | null;
  /** The sentence for weights that don't add to 100; null when they do, and while the course waits for its syllabus. */
  weights: string | null;
  /** Each category's percentage so far, null while nothing in it is graded. */
  categories?: Record<Id, number | null>;
  /** "ELE 209 · Intro to Computer Systems Lab", or the code alone when the name is empty or says the same. */
  label?: string;
  /** The weights add to 0: there is nothing to work a grade out from until a syllabus is imported. */
  needsSyllabus?: boolean;
  status?: 'stub' | 'confirmed';
  /** The term's name. */
  term?: string;
}

/**
 * A syllabus read and waiting to be accepted. Nothing in it is written until
 * `heat.syllabus.accept`; the fields below `error` are there once it is ready.
 */
export interface SyllabusDraft {
  id: Id;
  courseId: Id | null;
  fileName: string;
  pages: number;
  createdAt: number;
  state: 'reading' | 'ready' | 'failed';
  /** One sentence, when it failed. */
  error?: string;
  course?: { code: string; name: string; term: string; label: string; isNew: boolean };
  weights?: { category: string; percent: number; dropLowest: number | null }[];
  weightsTotal?: number;
  /** "Weights add to 95%. The other 5% is unassigned.", or null when they add to 100. */
  weightsFlag?: string | null;
  types?: TypeDef[];
  newTasks?: { title: string; type: string; due: number | null; estMin: number }[];
  dateChanges?: { taskId: Id; title: string; from: number | null; to: number }[];
  /** "ELE 209 · Intro to Computer Systems Lab. Labs 40%, Quizzes 20%, Final 40%. 5 types. 3 new tasks, 2 dates changed." */
  line?: string;
}

/** Which tasks `heat.task.reapplyDefaults` goes over. */
export type ReapplyTarget = { taskId: Id } | { courseId: Id } | { projectId: Id } | { spaceId: Id } | { all: true };

/**
 * What `heat.put` takes: a record with or without its id. A new task may leave
 * out its type, difficulty and minutes, and the core fills them in; whatever
 * is sent is taken as the person's own choice.
 */
export type PutRecord<K extends Kind> = K extends 'task'
  ? Omit<Task, 'id' | 'type' | 'difficulty' | 'estMin'> & Partial<Pick<Task, 'id' | 'type' | 'difficulty' | 'estMin'>>
  : Omit<Records[K], 'id'> & { id?: Id };

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
  /** The mail accounts, for Mail's switcher; beyond docs/HEAT.md. */
  mailAccounts?: MailAccount[];
  /** Syllabuses read and waiting for Accept or Discard. */
  syllabus?: { drafts: SyllabusDraft[] };
  /** Each thread's place, by Gmail's thread id. */
  mailState?: Record<string, MailPlace>;
  mailSync?: MailSync;
  /** Commitments: each day's, what is next, what today has left, what waits (docs/COMMITMENTS.md). */
  commitments?: CommitmentsSnapshot;
  /** Notes: links, backlinks, tags, the inbox, the capture folder (docs/NOTES.md). */
  notes?: NotesSnapshot;
  /** Quiet notices, newest first. */
  notices?: Notice[];
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
    courses: Record<Id, CourseDerived>;
    habits: Record<Id, { today: boolean; record: string; /** The streak sentence, only for a habit whose counter is switched on; beyond docs/HEAT.md. */ counter?: string }>;
    status: string;
    /** A recurring task's occurrences in the snapshot's window, for Calendar's pills and flags. */
    occurrences?: { taskId: Id; date: DayKey; done: boolean }[];
    /** The type names a picker may offer, by where each is defined. */
    types?: { global: string[]; spaces: Record<Id, string[]>; courses: Record<Id, string[]>; projects: Record<Id, string[]> };
    /** How many tasks still wait for an estimate better than the catch-all. */
    unscored?: number;
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

/** A course, project or space saved: how many tasks and grades its types and categories moved. Other kinds leave it out. */
export interface Retimed {
  retimed?: number;
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

    put: <K extends Kind>(kind: K, record: PutRecord<K>) => c<{ record: Records[K] } & Retimed & Undo>('heat.put', { kind, record }),
    patch: <K extends Kind>(kind: K, id: Id, set: Partial<Records[K]>) =>
      c<{ record: Records[K] } & Retimed & Undo>('heat.patch', { kind, id, set }),
    delete: (kind: Kind, id: Id) => c<Undo>('heat.delete', { kind, id }),

    done: (taskId: Id, done: boolean, date?: DayKey) => c<{ task: Task; took?: string } & Undo>('heat.done', { taskId, done, date }),
    estimate: (taskId: Id, set: { difficulty?: number; estMin?: number }) =>
      c<{ task: Task; clamped: boolean } & Undo>('heat.estimate', { taskId, ...set }),
    tookTime: (taskId: Id, minutes: number) => c<{ task: Task } & Undo>('heat.tookTime', { taskId, minutes }),
    /** A task's type, as the person's own choice; null hands it back to the title's words. */
    setType: (taskId: Id, type: string | null) => c<{ task: Task } & Undo>('heat.task.setType', { taskId, type }),
    /** The types' minutes and difficulty again, for the tasks named; `force` drops what was set by hand on them. */
    reapplyDefaults: (target: ReapplyTarget & { force?: boolean }) =>
      c<{ changed: number } & Undo>('heat.task.reapplyDefaults', target),
    /** Asks Claude once, in the background, to estimate the tasks no type matched. */
    scoreTasks: () => c<{ asked: number; line: string }>('heat.tasks.score'),

    /** A course's fields; its types are then applied again to its tasks that nothing was set by hand on. */
    updateCourse: (id: Id, set: Partial<Course>) =>
      c<{ record: Course; retimed: number } & Undo>('heat.course.update', { id, set }),
    updateProject: (id: Id, set: Partial<Project>) =>
      c<{ record: Project; retimed: number } & Undo>('heat.project.update', { id, set }),
    /**
     * Reads a syllabus into a draft. A PDF's draft answers `reading` and turns
     * `ready` or `failed` later, with a `heat` event; nothing is written until it is accepted.
     */
    importSyllabus: (from: ({ path: string } | { json: unknown }) & { courseId?: Id }) =>
      c<{ draft: SyllabusDraft }>('heat.course.importSyllabus', from),
    syllabus: {
      /** The draft's weights, types, tasks and dates, as one undo step. */
      accept: (draftId: Id) =>
        c<{ course: Course; counts: { newTasks: number; dateChanges: number; retimed: number } } & Undo>(
          'heat.syllabus.accept',
          { draftId },
        ),
      discard: (draftId: Id) => c<Record<string, never>>('heat.syllabus.discard', { draftId }),
    },

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

    mail: {
      /** A thread's saved text, by Gmail's thread id; null before Claude has saved it. */
      text: (threadId: string) => c<{ messages: MailMessage[] | null }>('heat.mail.text', { threadId }),
      /** A new mail, into the outbox and out at once. */
      send: (mail: { to: string; cc?: string; subject: string; body: string }) =>
        c<{ action: MailAction }>('heat.mail.send', mail),
      reply: (threadId: string, body: string, to?: string, cc?: string) =>
        c<{ action: MailAction }>('heat.mail.reply', { threadId, body, to, cc }),
      archive: (threadId: string, archived: boolean) => c<{ action: MailAction }>('heat.mail.archive', { threadId, archived }),
      mark: (threadId: string, unread: boolean) => c<{ action: MailAction }>('heat.mail.mark', { threadId, unread }),
      /** Threads whose subject, sender or saved text hold every word. */
      search: (q: string) => c<{ threadIds: string[] }>('heat.mail.search', { q }),
      outbox: () => c<{ actions: MailAction[] }>('heat.mail.outbox'),
      retry: (id: Id) => c<Record<string, never>>('heat.mail.outbox.retry', { id }),
      discard: (id: Id) => c<Record<string, never>>('heat.mail.outbox.discard', { id }),
      /** Sends what waits, then reads what is new. Answers at once; `mailSync` says how it goes. */
      sync: () => c<{ started: boolean }>('heat.mail.sync'),
      setBackground: (on: boolean) => c<Record<string, never>>('heat.mail.background.set', { on }),
    },
    calendars: {
      add: (name: string, kind: Calendar['kind'], url: string) => c<{ calendar: Calendar }>('heat.calendars.add', { name, kind, url }),
      remove: (id: Id) => c<Record<string, never>>('heat.calendars.remove', { id }),
      sync: (id?: Id) => c<{ line: string }>('heat.calendars.sync', { id }),
    },
    setSchool: (school: { name: string; host: string; icalUrl?: string; codePattern: string; termStart: DayKey; termEnd: DayKey }) =>
      c<Record<string, never>>('heat.school.set', school),

    commitments: {
      create: (input: CommitmentInput) =>
        c<{ commitment: Commitment; course: Course | null; line: string } & Undo>('heat.commitment.create', { ...input }),
      /** `id` may be a title when only one commitment has it. */
      update: (id: Id, set: CommitmentInput) =>
        c<{ commitment: Commitment; line: string } & Undo>('heat.commitment.update', { id, set }),
      remove: (id: Id) => c<Undo>('heat.delete', { kind: 'commitment', id }),
      /** One day skipped or moved. Refused for a day it doesn't meet on. */
      addException: (id: Id, exception: CommitmentException) =>
        c<{ commitment: Commitment; line: string } & Undo>('heat.commitment.addException', { id, ...exception }),
      removeException: (id: Id, date: DayKey) =>
        c<{ commitment: Commitment } & Undo>('heat.commitment.removeException', { id, date }),
      /** Pasted text, read by Claude into a draft that answers `reading`; nothing is written until it is accepted. */
      importText: (text: string, mode: ScheduleMode = 'schedule', weekOf?: DayKey) =>
        c<{ draft: CommitmentDraft }>('heat.commitment.importText', { text, mode, weekOf }),
      /** A photo or screenshot of a schedule, by its path. */
      importImage: (path: string, mode: ScheduleMode = 'schedule', weekOf?: DayKey) =>
        c<{ draft: CommitmentDraft }>('heat.commitment.importImage', { path, mode, weekOf }),
      /** An `.ics` file, its text, or its address. With `subscribe` the address is kept and read again every hour. */
      importIcs: (
        from: ({ path: string } | { url: string } | { text: string }) & { mode?: ScheduleMode; subscribe?: boolean; name?: string },
      ) => c<{ draft?: CommitmentDraft; feed?: { id: Id; name: string }; changed?: number } & Partial<Undo>>('heat.commitment.importIcs', from),
      accept: (draftId: Id, skip?: number[]) =>
        c<{ commitments: number; breaks: number; replaced: number; courses: Course[] } & Undo>(
          'heat.commitment.draft.accept',
          { draftId, skip },
        ),
      discard: (draftId: Id) => c<Record<string, never>>('heat.commitment.draft.discard', { draftId }),
      /** The one tap for what a mail asked for. */
      confirm: (id: Id) => c<{ commitment: Commitment; line: string } & Undo>('heat.commitment.exception.confirm', { id }),
      dismiss: (id: Id) => c<Record<string, never>>('heat.commitment.exception.dismiss', { id }),
      syncFeed: (id?: Id) => c<{ changed: number }>('heat.commitment.feed.sync', { id }),
      removeFeed: (id: Id) => c<Record<string, never>>('heat.commitment.feed.remove', { id }),
      /** What each day has left, and what is fixed in it. Today when nothing is named. */
      freeTime: (range: { date?: DayKey } | { from: DayKey; to: DayKey } = {}) =>
        c<{ days: (FreeTime & { commitments: CommitmentOccurrence[] })[] }>('heat.planner.freeTime', range),
      /** When the person sleeps: minutes after midnight, to bed and up. */
      setSleep: (from: number, to: number) => c<Record<string, never>>('heat.sleep.set', { from, to }),
    },
    notes: {
      /** A title another note has is refused, unless `ifMissing`, which answers the note that is there. */
      create: (note: { title?: string; markdown?: string; ifMissing?: boolean } & Partial<Exclude<NotePlace, { none: true }>> = {}) =>
        c<{ note: Note; existed?: boolean } & Undo>('heat.note.create', note),
      /** A new title is the note's new name everywhere: links to it are rewritten in the same entry. */
      save: (id: Id, set: { markdown?: string; title?: string }) =>
        c<{ note: Note; renamed: number } & Undo>('heat.note.save', { id, ...set }),
      remove: (id: Id) => c<Undo>('heat.delete', { kind: 'note', id }),
      /** The note titled with the day; with `markdown` it is written, and made if it isn't there. */
      daily: (date: DayKey, markdown?: string) => c<{ note: Note | null } & Partial<Undo>>('heat.note.daily', { date, markdown }),
      search: (q: string, limit?: number) => c<{ hits: NoteHit[] }>('heat.note.search', { q, limit }),
      /** Adds `[[to]]`; a name nothing has becomes a new, empty note. */
      link: (id: Id, to: string) => c<{ note: Note; created: Note | null; line: string } & Undo>('heat.note.link', { id, to }),
      file: (id: Id, place: NotePlace) => c<{ note: Note; line: string } & Undo>('heat.note.file', { id, ...place }),
      /** A checkbox line becomes a task, and stays linked to it. */
      taskFromLine: (id: Id, line: number) => c<{ task: Task; note: Note } & Undo>('heat.note.taskFromLine', { id, line }),
      toggleBox: (id: Id, line: number, done: boolean) =>
        c<{ note: Note; task: Task | null } & Undo>('heat.note.toggleBox', { id, line, done }),
      acceptSuggestion: (noteId: Id, index: number) => c<{ task: Task } & Undo>('heat.note.suggestion.accept', { noteId, index }),
      dismissSuggestions: (noteId: Id) => c<Record<string, never>>('heat.note.suggestion.dismiss', { noteId }),
      /** An image of the notes folder, by the path the note writes (`attachments/…`), as a data URL. */
      attachment: (path: string) => c<{ dataUrl: string }>('heat.note.attachment', { path }),
      /** Shows the note's file, or the folder, in the Finder. */
      reveal: (id?: Id) => c<{ path: string }>('heat.note.reveal', { id }),
      sync: () => c<{ changed: number }>('heat.note.sync'),
    },
    pages: {
      /** Reads what waits in the capture inbox now, or the one file named. */
      process: (path?: string) => c<{ started: boolean; waiting: number }>('heat.capture.process', { path }),
      /** Copies files, or a pasted image, into the inbox, where they are read like any other. */
      add: (from: { paths: string[] } | { name: string; base64: string }) => c<{ added: number }>('heat.capture.inbox.add', from),
      setClaude: (on: boolean) => c<Record<string, never>>('heat.capture.settings.set', { claude: on }),
    },
    notices: {
      dismiss: (id: string) => c<Record<string, never>>('heat.notice.dismiss', { id }),
      /** A notice's one act is a command of the core, run as it is given. */
      run: (cmd: string, args: Record<string, unknown>) => c<Record<string, unknown>>(cmd, args),
      undo: (txnId: string) => c<{ label: string }>('history.undoEntry', { txnId }),
    },

    claude: {
      get: () => c<ClaudeSettings>('heat.claude.get'),
      setTool: (name: string, on: boolean) => c<Record<string, never>>('heat.claude.setTool', { name, on }),
      undo: (txnId: string) => c<{ label: string }>('history.undoEntry', { txnId }),
    },
  };
}

export type HeatClient = ReturnType<typeof heatClient>;
