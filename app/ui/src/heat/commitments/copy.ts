// Every user-facing string of commitments (docs/COMMITMENTS.md), in the voice
// of model/copy.ts: plain, second person, present tense, no exclamation
// marks. The lines that carry numbers ("3h 20m free today. Planned 4h.") are
// the core's and are shown as given; these are the words around them.

import type { CommitmentKind, ScheduleMode } from '../client';
import { plural } from '../model/copy';

export const kinds: Record<CommitmentKind, string> = {
  class: 'Class',
  work: 'Work',
  commute: 'Commute',
  other: 'Other',
};

/** Monday first, as a timetable reads. */
export const WEEKDAYS: { code: string; letter: string; name: string }[] = [
  { code: 'MO', letter: 'M', name: 'Monday' },
  { code: 'TU', letter: 'T', name: 'Tuesday' },
  { code: 'WE', letter: 'W', name: 'Wednesday' },
  { code: 'TH', letter: 'T', name: 'Thursday' },
  { code: 'FR', letter: 'F', name: 'Friday' },
  { code: 'SA', letter: 'S', name: 'Saturday' },
  { code: 'SU', letter: 'S', name: 'Sunday' },
];

export const layer = {
  moved: 'moved',
  movedFrom: (day: string) => `Moved from ${day}`,
  span: (start: string, end: string) => `${start} to ${end}`,
  more: (n: number) => `+${n}`,
};

export const sheet = {
  new: 'New commitment',
  edit: (title: string) => `Edit ${title}`,
  nameFirst: 'Give the commitment a name first.',
  timesFirst: 'Give it a start and an end time.',
  endsBefore: 'It ends before it starts. A commitment never runs past midnight: one that does is two.',
  dayFirst: 'Pick the days it repeats on, or the one day it happens.',
  travelRange: 'Travel time is between 0 and 240 minutes.',
  codeFirst: 'Type the new course’s code.',
  oneDay: 'No day picked: it happens once, on the day below.',
  untilTerm: 'With no last day, a class ends with the term.',
  travelHint: 'Nothing is planned into the travel time before or after.',
  automatic: 'Automatic',
  noCourse: 'None',
  newCourse: 'A new course…',
  fixed: 'Fixed',
  flexible: 'Flexible',
  add: 'Add commitment',
  save: 'Save',
  added: (title: string) => `Added ${title}.`,
  saved: (title: string) => `Saved ${title}.`,
  deleted: (title: string) => `Deleted ${title}.`,
  exceptions: 'Days that differ',
  skipped: 'Skipped',
  movedTo: (day: string, time: string | null) => `Moved to ${day}${time ? `, ${time}` : ''}`,
  restore: 'Restore',
  skipDay: 'Skip a day',
  moveDay: 'Move a day',
  skipIt: 'Skip it',
  moveIt: 'Move it',
  whichDay: 'Pick the day it would have happened.',
};

export const modes: Record<ScheduleMode, string> = {
  schedule: 'A timetable',
  week: 'This week’s shifts',
  breaks: 'Academic calendar (breaks)',
};

export const importing = {
  title: 'Import schedule',
  hint: 'Learn reads it into a preview. Nothing is written until you apply it.',
  weekOf: 'Week of',
  paste: 'Paste text',
  pasteHint: 'A timetable from the registrar’s page, a rota from a message, a list of dates.',
  pasteFirst: 'Paste the schedule first.',
  read: 'Read it',
  file: 'A photo, a screenshot or an .ics file',
  dropHere: 'Drop the file here, or choose it.',
  choose: 'Choose a file…',
  noPicker: 'Drop the file on this sheet.',
  asFile: 'Drop a photo, a screenshot or an .ics file.',
  address: 'A calendar’s address',
  addressFirst: 'Type the calendar’s address first.',
  keep: 'Keep in step',
  keepHint: 'Learn keeps the address in the Keychain and reads it again every hour.',
  subscribe: 'Subscribe',
  subscribed: (name: string, changed: number, undoKey: string) =>
    `Subscribed to ${name}. ${plural(changed, 'commitment')} added. ${undoKey} undoes it.`,
  reading: (file: string) => (file ? `Reading ${file}…` : 'Reading the schedule…'),
  failed: 'Learn couldn’t read that schedule.',
  already: 'already there',
  newCourse: 'new course',
  include: (title: string) => `Include ${title}`,
  items: 'Commitments',
  breaks: 'Days off',
  nothing: 'Nothing in it is new.',
  unread: (n: number) => `${plural(n, 'line')} couldn’t be read and ${n === 1 ? 'is' : 'are'} left out.`,
  nothingBefore: 'Nothing is written until you apply it.',
  apply: 'Apply',
  discard: 'Discard',
  later: 'Not now',
  applied: (n: { commitments: number; breaks: number; replaced: number; courses: number }, undoKey: string) => {
    const parts = [
      n.commitments > 0 ? `${plural(n.commitments, 'commitment')} added` : null,
      n.breaks > 0 ? `${plural(n.breaks, 'break')} added` : null,
      n.replaced > 0 ? `${n.replaced} replaced` : null,
      n.courses > 0 ? plural(n.courses, 'new course') : null,
    ].filter((p) => p !== null);
    return `${parts.length > 0 ? parts.join(', ') : 'Nothing new'}. ${undoKey} undoes it.`;
  },
  discarded: 'Schedule discarded. Nothing changed.',
};

export const term = {
  title: 'Academic calendar',
  dates: (start: string, end: string) => `The term runs ${start} to ${end}.`,
  noDates: 'The term has no dates yet.',
  setIn: 'Set in Settings → Learn.',
  breaks: 'Days classes don’t meet',
  none: 'No breaks yet.',
  nameFirst: 'Give the break a name first.',
  datesFirst: 'Give the break its first and last day.',
  order: 'The break ends before it starts.',
  add: 'Add break',
  delete: (title: string) => `Delete ${title}`,
  import: 'Import a calendar…',
  added: (title: string) => `Added ${title}.`,
  deleted: (title: string) => `Deleted ${title}.`,
  done: 'Done',
};

export const schedule = {
  heading: 'Schedule',
  none: 'No classes or shifts yet.',
  new: 'New commitment…',
  import: 'Import schedule…',
  calendar: 'Academic calendar…',
  toReview: (n: number) => `${plural(n, 'schedule')} to review`,
  reading: (n: number) => `Reading ${plural(n, 'schedule')}…`,
  failed: (n: number) => `${plural(n, 'schedule')} couldn’t be read`,
  feeds: 'Subscribed calendars',
  removeFeed: (name: string) => `Remove ${name}`,
  feedRemoved: (name: string) => `Removed ${name}. What it added stays.`,
  sleep: 'Sleep',
  toBed: 'To bed',
  up: 'Up',
};

export const pending = {
  later: 'Not now',
  label: 'Changes a mail asked for',
};
