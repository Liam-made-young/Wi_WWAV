// A sample for `npm run dev` on the fake core: the three spaces of 3.4, a
// course and a few grades, a dozen tasks (one recurring, one with subtasks,
// one Claude added, one done with focus logged), blocks and a grey event
// on today's column, an inbox, habits and school mail, and two commitments
// (a class on Tuesdays and Thursdays, a shift this evening). Everything is
// placed relative to `now`, so the sample reads sensibly whenever it is run.

import { addDays, atMinute, type DayKey, dayKey, weekdayOf } from '../../shared/time/zone';
import type { CalendarEvent, Id, Kind, Note, Records, Task } from '../client';
import { defaultSpaces } from '../model/spaces';
import { ulidAt } from '../ulid';

export type Seed = Partial<{ [K in Kind]: Records[K][] }>;

export interface Seeded {
  records: Seed;
  events: CalendarEvent[];
  currentTaskId: Id;
  /** A few notes that link to each other, to a course and to a task, with one page waiting in the Notes inbox. Beside the records, for whoever wants them in the store. */
  notes: Note[];
}

const SPACE_IDS = ['sp-classes', 'sp-wwav', 'sp-personal'];

export function seed(now: number, tz: string): Seeded {
  const today = dayKey(now, tz);
  const day = (n: number) => addDays(today, n);
  /** A wall-clock time `n` days from today. */
  const at = (n: number, h: number, m = 0) => atMinute(day(n), h * 60 + m, tz);
  let i = 0;
  const [classes, wwav, personal] = defaultSpaces(() => SPACE_IDS[i++]);

  const task = (id: Id, over: Partial<Task> & Pick<Task, 'spaceId' | 'title' | 'type'>): Task => ({
    id,
    due: null,
    difficulty: 3,
    estMin: null,
    adjustMin: 0,
    notes: '',
    done: false,
    doneAt: null,
    source: 'you',
    public: false,
    ...over,
  });

  const tasks: Task[] = [
    task('t-quiz4', {
      spaceId: classes.id,
      title: 'Grammar quiz 4',
      type: 'Quiz',
      courseId: 'c-jpn201',
      due: at(1, 23, 59),
      difficulty: 3,
      source: 'ical',
    }),
    task('t-kanji7', {
      spaceId: classes.id,
      title: 'Kanji worksheet 7',
      type: 'Homework',
      courseId: 'c-jpn201',
      due: at(0, 16),
      difficulty: 2,
      estMin: 45,
      estBy: 'claude',
      estReason: 'A one-page worksheet.',
      source: 'ical',
    }),
    task('t-pset5', {
      spaceId: classes.id,
      title: 'Problem set 5',
      type: 'Homework',
      courseId: 'c-mth142',
      due: at(2, 21),
      difficulty: 4,
      estMin: 90,
      estBy: 'you',
      scheduledDate: day(1),
    }),
    task('t-listen', {
      spaceId: classes.id,
      title: 'Listening practice',
      type: 'Listening',
      courseId: 'c-jpn201',
      due: at(0, 20),
      difficulty: 1,
      estMin: 20,
      estBy: 'you',
      rrule: 'FREQ=WEEKLY;BYDAY=MO,WE,FR',
    }),
    task('t-verse', {
      spaceId: wwav.id,
      title: 'Mix the second verse',
      type: 'Music',
      milestoneId: 'ms-mixed',
      projectId: 'proj-ep',
      due: at(3, 18),
      difficulty: 4,
      estMin: 120,
      estBy: 'you',
      notes: 'Vocals sit too far back after the bridge.',
    }),
    task(ulidAt(now - 18 * 3_600_000, 'PCBQ000000000000'), {
      spaceId: wwav.id,
      title: 'Order PCBs',
      type: 'Hardware',
      due: at(6, 17),
      difficulty: 2,
      estMin: 30,
      estBy: 'claude',
      estReason: 'A single order, like the last one.',
      source: 'claude',
      claudeReason: 'The board house’s Oct 6 email says the quote expires Oct 13.',
      notes: 'From mail: https://mail.google.com/mail/u/0/#inbox/example',
    }),
    task('t-ep', {
      spaceId: wwav.id,
      title: 'Release EP v1',
      type: 'Business',
      projectId: 'proj-ep',
      due: at(12, 12),
      difficulty: 3,
    }),
    task('t-master', {
      spaceId: wwav.id,
      title: 'Master the tracks',
      type: 'Music',
      projectId: 'proj-ep',
      parentTaskId: 't-ep',
      due: at(10, 18),
      difficulty: 3,
      estMin: 90,
      estBy: 'you',
    }),
    task('t-art', {
      spaceId: wwav.id,
      title: 'Finish the cover art',
      type: 'Design',
      projectId: 'proj-ep',
      parentTaskId: 't-ep',
      due: at(11, 18),
      difficulty: 2,
      estMin: 60,
      estBy: 'you',
    }),
    task('t-passport', {
      spaceId: personal.id,
      title: 'Renew passport',
      type: 'Admin',
      group: 'Admin',
      due: at(5, 17),
      difficulty: 2,
      estMin: 40,
      estBy: 'you',
    }),
    task('t-dentist', {
      spaceId: personal.id,
      title: 'Call the dentist',
      type: 'Health',
      group: 'Health',
      projectId: 'proj-someday',
      difficulty: 1,
    }),
    task('t-syllabus', {
      spaceId: classes.id,
      title: 'Read the syllabus',
      type: 'Reading',
      courseId: 'c-mth142',
      due: at(-1, 12),
      difficulty: 1,
      done: true,
      doneAt: at(-1, 11, 30),
    }),
  ];

  const session = (id: Id, taskId: Id, n: number, min: number) => ({
    id,
    taskId,
    startedAt: at(-1, 10 + n),
    endedAt: at(-1, 10 + n, min),
    focusMin: min,
    interruptions: 0,
    source: 'timer' as const,
    public: false,
  });

  const block = (id: Id, taskId: Id, h: number, m: number, minutes: number, origin: 'you' | 'plan' = 'you') => ({
    id,
    taskId,
    date: today as DayKey,
    start: h * 60 + m,
    minutes,
    origin,
  });

  const habitLog = (back: number[]) => Object.fromEntries(back.map((n) => [day(-n), true as const]));

  const note = (id: Id, title: string, hoursAgo: number, markdown: string, over: Partial<Note> = {}): Note => ({
    id,
    title,
    markdown,
    createdAt: now - (hoursAgo + 1) * 3_600_000,
    updatedAt: now - hoursAgo * 3_600_000,
    public: false,
    ...over,
  });
  const notes: Note[] = [
    note(
      'n-verbs',
      'Verb groups',
      30,
      [
        '# Verb groups',
        '',
        'Group one verbs end in an う sound; group two drop る. #grammar',
        '',
        '| Group | Example | Te-form |',
        '|---|---|---|',
        '| One | 書く | 書いて |',
        '| Two | 食べる | 食べて |',
        '',
        'See [[Te-form drills]] for practice.',
      ].join('\n'),
      { courseId: 'c-jpn201', spaceId: classes.id },
    ),
    note(
      'n-drills',
      'Te-form drills',
      20,
      [
        'Drills for the rule in [[Verb groups]]. #grammar #practice',
        '',
        '- [ ] Do worksheet 4',
        '- [x] Read section 3.2',
        '- [ ] Ask about [[Grammar quiz 4]] in [[JPN 201]]',
        '',
        'More at [[Keigo]], once it is written.',
      ].join('\n'),
      { courseId: 'c-jpn201', spaceId: classes.id },
    ),
    note(
      'n-mix',
      'Mix notes',
      6,
      [
        'Vocals sit **too far back** after the bridge. See [[Mix the second verse]].',
        '',
        '```',
        'EQ: -2 dB at 300 Hz',
        '```',
        '',
        '> Less is more. #mixing',
      ].join('\n'),
      { spaceId: wwav.id },
    ),
    note('n-page', 'IMG_2211', 1, ['![](attachments/IMG_2211.jpg)', '', 'Te-form: 食べる becomes 食べて.'].join('\n'), {
      inbox: true,
      capturedAt: now - 3_600_000,
      attachments: ['attachments/IMG_2211.jpg'],
      source: 'capture',
    }),
  ];

  return {
    currentTaskId: 't-verse',
    notes,
    events: [
      {
        id: 'ev-lecture',
        calendarId: 'cal-school',
        title: 'JPN 201 lecture',
        start: at(0, 10),
        end: at(0, 11, 15),
        allDay: false,
      },
      {
        id: 'ev-studio',
        calendarId: 'cal-studio',
        title: 'Studio time',
        start: at(0, 14),
        end: at(0, 15),
        allDay: false,
      },
      {
        id: 'ev-office',
        calendarId: 'cal-school',
        title: 'Office hours',
        start: at(1, 11),
        end: at(1, 12),
        allDay: false,
      },
      { id: 'ev-break', calendarId: 'cal-school', title: 'Fall break', start: at(8, 0), end: at(9, 0), allDay: true },
    ],
    records: {
      space: [classes, wwav, personal],
      term: [{ id: 'term-fall-26', name: 'Fall 2026' }],
      course: [
        {
          id: 'c-jpn201',
          termId: 'term-fall-26',
          code: 'JPN 201',
          name: 'Intermediate Japanese',
          categories: [
            { id: 'cat-hw', name: 'Homework', weight: 25, keywords: ['worksheet', 'kanji'] },
            { id: 'cat-quiz', name: 'Quizzes', weight: 35, keywords: ['quiz'] },
            { id: 'cat-exam', name: 'Exams', weight: 40, keywords: ['exam'] },
          ],
          notes: '',
          public: false,
        },
        {
          id: 'c-mth142',
          termId: 'term-fall-26',
          code: 'MTH 142',
          name: 'Calculus II',
          categories: [
            { id: 'cat-psets', name: 'Problem sets', weight: 30, keywords: ['problem set'] },
            { id: 'cat-tests', name: 'Tests', weight: 70, keywords: ['test'] },
          ],
          notes: '',
          public: false,
        },
      ],
      grade: [
        {
          id: 'g-1',
          courseId: 'c-jpn201',
          categoryId: 'cat-quiz',
          title: 'Grammar quiz 3',
          score: 17,
          outOf: 20,
          dropped: false,
          pending: false,
          source: 'you',
          public: false,
        },
        {
          id: 'g-2',
          courseId: 'c-jpn201',
          categoryId: 'cat-hw',
          title: 'Kanji worksheet 6',
          score: 9,
          outOf: 10,
          dropped: false,
          pending: false,
          source: 'you',
          public: false,
        },
        {
          id: 'g-3',
          courseId: 'c-jpn201',
          categoryId: 'cat-quiz',
          title: 'Kanji quiz 3',
          score: null,
          outOf: 100,
          dropped: false,
          pending: true,
          link: 'https://brightspace.uri.edu/d2l/lms/grades/my_grades/main.d2l',
          postedAt: now - 26 * 3_600_000,
          source: 'claude',
          public: false,
          mailThreadId: 'mail-2',
        },
        {
          id: 'g-4',
          courseId: 'c-mth142',
          categoryId: 'cat-psets',
          title: 'Problem set 4',
          score: 41,
          outOf: 50,
          dropped: false,
          pending: false,
          source: 'you',
          public: false,
        },
      ],
      project: [
        { id: 'proj-ep', spaceId: wwav.id, title: 'EP', status: 'active', targetDate: day(40), public: false },
        { id: 'proj-someday', spaceId: personal.id, title: 'Someday', status: 'someday', public: false },
      ],
      milestone: [
        {
          id: 'ms-mixed',
          spaceId: wwav.id,
          projectId: 'proj-ep',
          title: 'EP v1 mixed',
          date: day(14),
          done: false,
          order: 1,
          public: false,
        },
      ],
      task: tasks,
      focusSession: [session('fs-1', 't-syllabus', 0, 25), session('fs-2', 't-syllabus', 1, 25)],
      timeBlock: [
        block('b-kanji', 't-kanji7', 9, 0, 45),
        block('b-verse', 't-verse', 13, 0, 90),
        block('b-pset', 't-pset5', 15, 30, 60, 'plan'),
      ],
      habit: [
        {
          id: 'h-kanji',
          title: 'Practise kanji',
          minutes: 20,
          log: habitLog([1, 2, 3, 4, 5, 6, 7, 8, 9]),
          showCounter: false,
          public: false,
        },
        { id: 'h-stretch', title: 'Stretch', log: habitLog([1, 3]), showCounter: false, public: false },
        { id: 'h-read', title: 'Read ten pages', log: habitLog([0, 1, 2]), showCounter: false, public: false },
      ],
      commitment: [
        {
          id: 'cm-mth142',
          title: 'MTH 142',
          kind: 'class',
          location: 'Lippitt Hall 205',
          start: 9 * 60 + 30,
          end: 10 * 60 + 45,
          rrule: 'FREQ=WEEKLY;BYDAY=TU,TH',
          from: day(-28),
          exceptions: [],
          bufferBefore: 15,
          bufferAfter: 10,
          hardness: 'fixed',
          courseId: 'c-mth142',
          source: 'you',
        },
        {
          // A shift on today's weekday, so the sample's Today always has something coming.
          id: 'cm-store',
          title: 'Campus store',
          kind: 'work',
          location: 'Memorial Union',
          start: 18 * 60,
          end: 21 * 60,
          rrule: `FREQ=WEEKLY;BYDAY=${['SU', 'MO', 'TU', 'WE', 'TH', 'FR', 'SA'][weekdayOf(today)]}`,
          from: day(-28),
          exceptions: [],
          bufferBefore: 20,
          bufferAfter: 20,
          hardness: 'fixed',
          spaceId: personal.id,
          source: 'you',
        },
      ],
      capture: [
        { id: 'cap-1', text: 'fix the snare at 1:32' },
        { id: 'cap-2', text: 'email Prof. Tanaka about the extension' },
        { id: 'cap-3', text: 'https://example.com/why-tape-saturation-works' },
      ],
      mailThread: [
        {
          id: 'mail-1',
          gmailThreadId: 'g-1',
          subject: 'Quiz 4 moved to Thursday',
          from: 'Prof. Tanaka <tanaka@uri.edu>',
          receivedAt: now - 3 * 3_600_000,
          course: 'JPN 201',
          state: 'task',
          reason: 'The email moves the quiz, so the task’s due date matches.',
          taskId: 't-quiz4',
          recordedBy: 'claude',
        },
        {
          id: 'mail-2',
          gmailThreadId: 'g-2',
          subject: 'Grade posted: Kanji quiz 3',
          from: 'Brightspace <noreply@brightspace.uri.edu>',
          receivedAt: now - 26 * 3_600_000,
          course: 'JPN 201',
          state: 'grade',
          reason: 'A grade notice with no score in it.',
          recordedBy: 'claude',
        },
        {
          id: 'mail-3',
          gmailThreadId: 'g-3',
          subject: 'Fall break reminder',
          from: 'Registrar <registrar@uri.edu>',
          receivedAt: now - 50 * 3_600_000,
          state: 'nothing',
          reason: 'No deadline in it.',
          recordedBy: 'claude',
        },
      ],
    },
  };
}
