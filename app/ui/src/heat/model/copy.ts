// Every user-facing Heat string, in the spec's words (docs/SPEC.md chapter 3,
// 2.11 and 8.10). Plain, second person, present tense; no exclamation marks.
// Where the spec gives a pattern rather than a sentence, the function here is
// the one place that fills it in.

// Due phrases (3.1): "Today 4:00 PM", "Tomorrow 11:59 PM", "2d overdue".
export const due = {
  today: (time: string) => `Today ${time}`,
  tomorrow: (time: string) => `Tomorrow ${time}`,
  onDay: (day: string, time: string) => `${day} ${time}`,
  overdue: (amount: string) => `${amount} overdue`,
};

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

// The sidebar's "Your average time" (3.1).
export const averageTime = (type: string, time: string, count: number) => `${type} ${time} (${count})`;

// The LCD's second line (3.1).
export const weeklyLoad = (time: string, count: number) => `This week: ${time} across ${plural(count, 'task')}`;

// Repeat presets, as the PKM's task panel offers them (3.6).
export const repeat = {
  none: 'Does not repeat',
  daily: 'Daily',
  weekdays: 'Every weekday',
  weekly: 'Weekly',
  monthly: 'Monthly',
  custom: 'Custom…',
};

// Today (3.5).
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

// Plan my day's drafts (3.5): "Due tomorrow 11:59 PM, Hot."
export const draft = {
  reason: (phrase: string, level: string) =>
    `Due ${phrase.replace(/^(Today|Tomorrow) /, (w) => w.toLowerCase())}, ${level}.`,
  overdue: (phrase: string) => `${phrase}.`,
  noDue: 'No due date.',
  leftToPlan: (time: string) => `${time} left to plan`,
};

// The Pomodoro LCD (3.5) and the Now strip (2.2).
export const focus = {
  line: (round: number, of: number, title: string) => `Focus ${round} of ${of} · ${title}`,
  done: (time: string, title: string) => `Focus done. ${time} logged to ${title}.`,
  breakWaits: (length: string) => `Break ${length}. Press F to start it.`,
  breakLine: 'Break',
  stopped: (time: string, title: string) => `Focus stopped. ${time} logged to ${title}.`,
  breakDone: (round: number, of: number) => `Break done. Press F to start focus ${round} of ${of}.`,
  strip: (left: string) => `focus ${left} left`,
  breakStrip: (left: string) => `break ${left} left`,
  chime: 'Chime when focus ends',
};

// Checking a task off (3.1, 3.5).
export const check = {
  title: 'Time it took',
  hint: 'This trains your time averages for this type of task.',
  done: (time: string, sessions: number) =>
    sessions > 0 ? `Done. Took ${time} across ${plural(sessions, 'focus session')}.` : `Done. Took ${time}.`,
};

// The right column's widgets (3.5).
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
};
