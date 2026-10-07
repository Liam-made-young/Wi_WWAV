// Every user-facing Heat string, in the spec's words (docs/SPEC.md chapter 3,
// 2.2, 2.11 and 8.10). Plain, second person, present tense; no exclamation
// marks; real numbers instead of adjectives. Where the spec gives a pattern
// rather than a sentence, the function here is the one place that fills it in.

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

// --- 3.1: heat, the LCD and the sidebar -------------------------------------

// Due phrases: "Today 4:00 PM", "Tomorrow 11:59 PM", "2d overdue".
export const due = {
  today: (time: string) => `Today ${time}`,
  tomorrow: (time: string) => `Tomorrow ${time}`,
  onDay: (day: string, time: string) => `${day} ${time}`,
  overdue: (amount: string) => `${amount} overdue`,
};

// "Homework 1h 15m (6)"
export const averageTime = (type: string, time: string, count: number) => `${type} ${time} (${count})`;

// "This week: 3h 20m across 5 tasks"
export const weeklyLoad = (time: string, count: number) => `This week: ${time} across ${plural(count, 'task')}`;

// The Now strip's task half (2.2): "Hot: Grammar quiz 4".
export const strip = {
  task: (level: string, title: string) => `${level}: ${title}`,
  allClear: 'All clear',
  nothingOpen: 'Nothing open right now.',
};

// Where a task came from, as Get Info names it.
export const source = {
  you: 'Added by you',
  calendar: 'Brightspace calendar',
  ical: 'Brightspace calendar',
  mail: 'Mail',
  capture: 'Quick capture',
};

// --- 3.3: the tabs ----------------------------------------------------------

// Each tab's "+" and its one secondary act. Mail has no "+".
export const tabs = {
  plus: {
    Today: 'A task straight into the plan',
    Tasks: 'A task',
    Calendar: 'A task due 11:59 PM on the selected day',
    Grades: 'A grade',
    Habits: 'A habit',
  },
  secondary: {
    Today: 'Plan my day',
    Tasks: 'Triage inbox',
    Calendar: 'Today',
    Grades: 'Add course',
    Habits: 'Show the year',
    Mail: 'Sync now',
  },
};

// ⌘Z names what it will undo: "Undo mark done", "Undo move block".
export const undo = {
  label: (action: string) => `Undo ${action}`,
  markDone: 'mark done',
  moveBlock: 'move block',
};

// --- 3.4 and 3.6: spaces and Tasks -------------------------------------------

export const tasks = {
  empty: (space: string | null) => `Add your first ${space ? `${space} ` : ''}task and Heat will rank it.`,
  rankIt: 'Heat will rank it.',
  nameFirst: 'Give the task a name first.',
  all: 'All',
  newSpace: 'New space…',
  library: 'Library',
  inbox: 'Inbox',
  allOpen: 'All open',
  hot: 'Hot',
  dueThisWeek: 'Due this week',
  scheduled: 'Scheduled',
  someday: 'Someday',
  done: 'Done',
  averageTime: 'Your average time',
  markDone: 'Mark done',
  took: 'Took',
  edit: 'Edit',
};

// Repeat presets, as the PKM's task panel offers them.
export const repeat = {
  none: 'Does not repeat',
  daily: 'Daily',
  weekdays: 'Every weekday',
  weekly: 'Weekly',
  monthly: 'Monthly',
  custom: 'Custom…',
};

// --- 3.5: Today ---------------------------------------------------------------

export const today = {
  header: (day: string) => `Today, ${day}`,
  subtitle: (blocks: number, planned: string, dueToday: number) =>
    `${plural(blocks, 'block')} · ${planned} planned · ${dueToday} due today`,
  planned: 'Planned',
  dueToday: 'Due today, not planned',
  recurring: 'Recurring today ↻',
  hot: 'Hot, not planned',
  empty: 'Nothing planned yet. Drag a task onto the time column, or press Plan my day.',
  dailyNote: "How's the day going? Markdown + [[wikilinks]] welcome.",
  blockEnds: (time: string) => `Block ends ${time}`,
};

// Plan my day's drafts: "Due tomorrow 11:59 PM, Hot."
export const draft = {
  reason: (phrase: string, level: string) =>
    `Due ${phrase.replace(/^(Today|Tomorrow) /, (w) => w.toLowerCase())}, ${level}.`,
  overdue: (phrase: string) => `${phrase}.`,
  noDue: 'No due date.',
  leftToPlan: (time: string) => `${time} left to plan`,
};

// The Pomodoro LCD, and the strip's "focus 18:40 left" (2.2).
export const focus = {
  line: (round: number, of: number, title: string) => `Focus ${round} of ${of} · ${title}`,
  done: (time: string, title: string) => `Focus done. ${time} logged to ${title}.`,
  doneUnlogged: 'Focus done.',
  breakWaits: (length: string) => `Break ${length}. Press F to start it.`,
  breakLine: 'Break',
  stopped: (time: string, title: string) => `Focus stopped. ${time} logged to ${title}.`,
  breakDone: (round: number, of: number) => `Break done. Press F to start focus ${round} of ${of}.`,
  strip: (left: string) => `focus ${left} left`,
  breakStrip: (left: string) => `break ${left} left`,
  pulledAway: 'Pulled away',
  chime: 'Chime when focus ends',
};

// Checking a task off.
export const check = {
  title: 'Time it took',
  hint: 'This trains your time averages for this type of task.',
  done: (time: string, sessions: number) =>
    sessions > 0 ? `Done. Took ${time} across ${plural(sessions, 'focus session')}.` : `Done. Took ${time}.`,
};

// The right column's widgets.
export const widgets = {
  now: 'Now',
  habits: 'Habits',
  hotTasks: 'Hot tasks',
  mail: 'Mail',
  grades: 'Grades',
  nowEmpty: 'Nothing is current. Pick a task and press C, or drag one here.',
  habitsEmpty: 'No habits yet.',
  hotEmpty: 'Nothing is hot.',
  mailEmpty: 'No new school mail.',
  startFocus: 'Start focus',
  done: 'Done',
  openLink: 'Open link',
  openSession: 'Open session',
  openGrades: 'Open Grades',
  openInMail: 'Open in Mail',
};

// --- 3.7: Calendar ------------------------------------------------------------

export const calendar = {
  more: (n: number) => `${n} more`,
  dueFlag: (time: string) => `due ${time}`,
  unscheduled: 'Unscheduled',
  month: 'Month',
  week: 'Week',
  day: 'Day',
};

// --- 3.8: Grades --------------------------------------------------------------

// "An A", "a B": the article a letter takes when read aloud.
const withArticle = (letter: string, capital: boolean) => {
  const a = /^[AEFHILMNORSX]/.test(letter) ? 'an' : 'a';
  return `${capital ? a[0].toUpperCase() + a.slice(1) : a} ${letter}`;
};

export const grades = {
  header: (term: string) => `${term} grades`,
  basedOn: (pct: string) => `Based on ${pct}% of the course so far`,
  newGradePosted: 'New grade posted',
  enterScore: 'Enter score',
  toEnter: (n: number) => `${plural(n, 'new grade')} to enter`,
  weightsShort: (total: string, rest: string) => `Weights add to ${total}%. The other ${rest}% is unassigned.`,
  weightsOver: (total: string, over: string) => `Weights add to ${total}%. That is ${over}% more than 100.`,
  syllabusFound: (n: number, total: string) =>
    `Claude found ${plural(n, 'category', 'categories')} adding to ${total}%. Check them against the syllabus.`,
  need: (letter: string, min: string, need: string, left: string) =>
    `To finish with ${withArticle(letter, false)} (${min}%), you need ${need}% on the remaining ${left}%.`,
  outOfReach: (letter: string, best: string, bestLetter: string) =>
    `${withArticle(letter, true)} is out of reach; the highest possible is ${best}% (${bestLetter}).`,
  safe: (letter: string, left: string) =>
    `You keep ${withArticle(letter, false)} even with 0% on the remaining ${left}%.`,
  allGraded: (pct: string, letter: string) => `Every category is graded. The course stands at ${pct}% (${letter}).`,
  save: 'Save',
};

// --- 3.9: Habits --------------------------------------------------------------

export const habits = {
  limit: 'Habit limit reached',
  nameFirst: 'Give the habit a name first.',
  advice: 'Keep them small enough that you never skip.',
  ofDone: (done: number, all: number) => `${done} of ${all} done`,
  record: (days: number, since: string) => `Done ${plural(days, 'day')} since ${since}`,
  notYet: 'Not done yet',
  streak: (days: number) => `${days}-day streak`,
  showCounter: 'Show a streak counter',
};

// --- 3.10: Mail -----------------------------------------------------------------

export const mail = {
  brightspace: 'Brightspace',
  school: 'School',
  gradePosted: 'Grade posted',
  taskMade: 'Task made',
  nothingToDo: 'Nothing to do',
  loadImages: 'Load images',
  makeTask: 'Make a task',
  openInGmail: 'Open in Gmail',
  fromMail: 'From mail:',
  testing: 'Google asks you to sign in again every 7 days while Heat is in testing.',
};

// --- 3.1 and 3.11: syncing -----------------------------------------------------

// "Synced 3:41 PM: 2 new tasks, 1 date change, 1 new grade posted"
export const sync = {
  synced: (time: string, n: { newTasks: number; dateChanges: number; newGrades: number }) => {
    const parts = [
      n.newTasks > 0 ? plural(n.newTasks, 'new task') : null,
      n.dateChanges > 0 ? plural(n.dateChanges, 'date change') : null,
      n.newGrades > 0 ? `${plural(n.newGrades, 'new grade')} posted` : null,
    ].filter((p) => p !== null);
    return `Synced ${time}: ${parts.length > 0 ? parts.join(', ') : 'nothing new'}`;
  },
  syncing: 'Syncing…',
  syncNow: 'Sync now',
  gone: 'No longer in Brightspace',
  signInExpired: 'Google sign-in has expired. Sign in again to read Brightspace mail.',
};

// --- 2.11 and 3.12: Claude ----------------------------------------------------------

export const claude = {
  estimate: (time: string) => `Claude's estimate: ${time}. It read the title, the notes and your past averages.`,
  askToScore: 'Ask Claude to score',
  consent: "Heat will send this task's title, type and notes, and your average minutes per type. Nothing else.",
  turnOn: 'Turn on scoring',
  notNow: 'Not now',
  off: 'Claude scoring is off. Set difficulty yourself.',
  tooMany: 'Too many requests. Wait a minute, then try again.',
  needsConnection: 'Needs a connection',
  dailyLimit: "Claude's 50 calls for today are used. They come back at midnight.",
  moreEmails: "More emails left, they'll come in on the next sync.",
};

// --- 3.13: capture, projects and the weekly review ------------------------------------

// "4 in inbox · captured ✓"
export const capture = {
  footer: (inInbox: number, justCaptured: boolean) => `${inInbox} in inbox${justCaptured ? ' · captured ✓' : ''}`,
  toTask: '→ task',
  toNote: '→ note',
  toProject: '→ project',
  toUpload: '→ upload',
  inboxZero: 'Inbox zero ✓',
};

export const projects = {
  makeTasks: (n: number) => `Make ${plural(n, 'task')} from To finish?`,
  newReleasePlan: 'New release plan',
  releasePhases: ['Pre-release', 'Release day', 'Post-release'],
};

export const review = {
  steps: ['Inbox', 'Last week', 'Projects', 'Next week', 'Note'],
  space: (name: string, done: number, focus: string) => `${name}: ${plural(done, 'task')} done, ${focus} of focus`,
  habits: (focus: string) => `Habits: ${focus} of focus`,
  milestone: (title: string) => `Milestone reached: ${title}`,
  accuracy: (type: string, estimated: string, took: string, n: number) =>
    `${type}: estimated ${estimated}, took ${took} across ${n}`,
  noteHeadings: ['What moved', 'What slipped', "Next week's one thing"],
  complete: 'Review complete ✓',
};

// --- 3.14: connections ----------------------------------------------------------------

export const connections = {
  yourGalaxy: 'Your galaxy',
  lineHint: 'This line will show on your galaxy',
  show: 'Show',
  nowMaking: (text: string) => `Now making: ${text}`,
  payoutTask: 'Finish payout setup so your shelf can open',
};

// --- 3.15: data and moving in --------------------------------------------------------

export const status = {
  saved: (syncedAt: string | null) => `Saved on this Mac${syncedAt ? ` · Synced ${syncedAt}` : ''}`,
  failed: "Couldn't save that change. Check your connection and try again.",
  empty: 'Nothing here right now.',
  end: "That's everything.",
};

export const moving = {
  notExport: 'This file isn’t a Heat export.',
  newer: 'This export is from a newer Heat. Update Wi_WWAV, then try again.',
};

// --- The Public switch, in the words 3.15 gives each kind of record --------------------
//
// What a switch says before it is turned on, and what it says while it is on.
// A record shows only its own fields; nothing worked out across records ever does.

export const publicSwitch = {
  label: 'Public',
  task: {
    off: 'Private. Turn this on to show the task’s title, due date and whether it is done to anyone who opens your sun.',
    on: 'Anyone who opens your sun can see this task’s title, due date and whether it is done.',
  },
  project: {
    off: 'Private. Turn this on to show the project’s title, status and target date to anyone who opens your sun.',
    on: 'Anyone who opens your sun can see this project’s title, status and target date.',
  },
  milestone: {
    off: 'Private. Turn this on to show the milestone’s title, date and whether it is reached to anyone who opens your sun.',
    on: 'Anyone who opens your sun can see this milestone’s title, date and whether it is reached.',
  },
  habit: {
    off: 'Private. Turn this on to show the habit’s title and its day-by-day grid to anyone who opens your sun. A streak or running count never leaves your Mac.',
    on: 'Anyone who opens your sun can see this habit’s title and its day-by-day grid.',
  },
  note: {
    off: 'Private. Turn this on to show the note’s text to anyone who opens your sun.',
    on: 'Anyone who opens your sun can read this note.',
  },
  course: {
    off: 'Private. Turn this on to show the course’s code and name to anyone who opens your sun. Its grades, percentage and letter stay with you.',
    on: 'Anyone who opens your sun can see this course’s code and name. Its grades, percentage and letter stay with you.',
  },
  grade: {
    off: 'Grades are private by default. Your school keeps them as education records. Turning this on shows this grade to anyone who opens your sun.',
    on: 'Anyone who opens your sun can see this grade’s course, item, score and what it was out of. The course’s percentage and letter stay with you.',
  },
  pendingGrade: 'A grade waiting for its score stays private. Enter the score first.',
  focusSession: {
    off: 'Private. Turn this on to show the task’s title, the date and this session’s minutes to anyone who opens your sun.',
    on: 'Anyone who opens your sun can see the task’s title, the date and this session’s minutes.',
  },
};

// --- Grades, as the tab words it (3.8) --------------------------------------------------

export const gradesUi = {
  noCourses: 'No courses yet. Add one to start keeping grades.',
  addCourseFirst: 'Add a course first.',
  addCourse: 'Add course',
  addGrade: 'Add grade',
  nothingGraded: 'Nothing is graded yet.',
  noItems: 'No grades here yet.',
  noCategory: 'No category',
  courseNeedsCode: 'Give the course a code first.',
  gradeNeedsName: 'Give the grade a name first.',
  categoryNeedsName: 'Give every category a name.',
  typeScore: 'Type the score as a number.',
  typeOutOf: 'Type what it is out of as a number above 0.',
  scaleOrder: 'List the scale from the highest to the lowest.',
  scaleNeedsLetters: 'Give every step a letter and a percentage from 0 to 100.',
  weightsHint: 'Weights are percentages of the course. They should add to 100.',
  keywordsHint: 'Keywords file new grades here: a grade whose name holds one goes in this category.',
  entered: (title: string, score: number, outOf: number) => `${title}: ${score} out of ${outOf}.`,
  whatItWouldTake: 'What it would take',
  aimFor: 'Aim for',
  noScale: 'This course uses the usual scale.',
  scaleOwn: 'This course has its own scale',
};
