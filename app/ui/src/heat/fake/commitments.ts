// The fake core's commitments (docs/COMMITMENTS.md): a class, a shift, a
// commute, the days each really happens on, and what the snapshot says of
// them (`commitments`: each day's, what is next, what today has left, what
// overlaps, what waits). A schedule comes in as a draft and writes nothing:
// `heat.commitment.draft.accept` applies it as one journal entry, so one ⌘Z
// takes it back. It is a stand-in, not a port: weekly rules and one-offs,
// skip and move exceptions, breaks for classes and the term's end, and the
// lines in the core's words.
//
// The real core has Claude read pasted text or a photo. Here such a draft
// answers `reading` and then turns into a canned one; a test holds it there
// and ends the reading itself:
//
//   holdReading(fake);
//   readyDraft(fake, draftId, [{ title, start, end, days | date, … }], [{ title, from, to }]);
//   failDraft(fake, draftId, 'One sentence.');
//   addPending(fake, { commitmentId, date });   // a mail that cancels a class, waiting for its tap

import { clock, longDay, shortMonthDay, WEEKDAYS } from '../../shared/time/format';
import { addDays, type DayKey, daysBetween, keyParts, minuteOfDay, weekdayOf } from '../../shared/time/zone';
import type {
  CalendarEvent,
  Commitment,
  CommitmentDraft,
  CommitmentException,
  CommitmentInput,
  CommitmentKind,
  CommitmentOccurrence,
  CommitmentsSnapshot,
  Course,
  FreeTime,
  Id,
  PendingException,
  ScheduleMode,
  TermBreak,
  TimeBlock,
} from '../client';
import { sleepSpans } from '../commitments/busy';
import { courseLabel, formatMinutes, plural } from '../fmt';
import { busySpans } from '../today/gap';
import { dayOf, derive, type Fake, refuse, register, snapshotOf, wrap } from './core';
// The School sheet's part comes first, so `snapshot.school` (the term's dates) is there when this one reads it.
import './settings';
import { extra as todayOf } from './today';

/** One line of a schedule as it was read, before it is checked against what is there. */
export interface ScheduleItem {
  title: string;
  kind?: CommitmentKind;
  start: number | string;
  end: number | string;
  /** Weekday codes; none means once, on `date` (or `from`). */
  days?: string[];
  date?: DayKey;
  from?: DayKey;
  until?: DayKey | null;
  location?: string;
  course?: string | null;
}

export interface ScheduleBreak {
  title: string;
  from: DayKey;
  to: DayKey;
}

/** What the fake keeps beyond the records: the drafts, what a mail asked for, the feeds and sleep. */
interface Extra {
  drafts: CommitmentDraft[];
  pending: PendingException[];
  feeds: CommitmentsSnapshot['feeds'];
  sleep: { from: number; to: number };
  /** The days the last `heat.snapshot` asked for. */
  window: { from: DayKey; to: DayKey } | null;
  held: boolean;
}

const extras = new WeakMap<Fake, Extra>();

function extra(fake: Fake): Extra {
  let x = extras.get(fake);
  if (!x) {
    // 11 PM and 7 AM until set.
    x = { drafts: [], pending: [], feeds: [], sleep: { from: 23 * 60, to: 7 * 60 }, window: null, held: false };
    extras.set(fake, x);
  }
  return x;
}

export const draftsOf = (fake: Fake): CommitmentDraft[] => extra(fake).drafts;

// --- the days a commitment happens on ---------------------------------------------------

const CODES = ['SU', 'MO', 'TU', 'WE', 'TH', 'FR', 'SA'];
const ORDER = ['MO', 'TU', 'WE', 'TH', 'FR', 'SA', 'SU'];
const LETTERS: Record<string, string> = { MO: 'M', TU: 'T', WE: 'W', TH: 'Th', FR: 'F', SA: 'Sa', SU: 'Su' };
const NOUN: Record<CommitmentKind, string> = { class: 'class', work: 'shift', commute: 'commute', other: 'commitment' };

const squashed = (text: string) => text.replace(/\s+/g, '').toLowerCase();
const inOrder = (days: readonly string[]) => ORDER.filter((d) => days.includes(d));
const mondayOf = (day: DayKey) => addDays(day, -((weekdayOf(day) + 6) % 7));
const span = (start: number, end: number) => `${clock(start)} to ${clock(end)}`;
/** "10:00", "9:35": a readout's time, with no AM or PM. */
const readout = (min: number) => clock(min).replace(/ [AP]M$/, '');
/** "Thursday, Oct 8" */
const dayWords = (day: DayKey) => `${WEEKDAYS[weekdayOf(day)]}, ${shortMonthDay(day)}`;

/** A weekly rule's weekday codes and how many weeks apart; null for something that happens once. */
function ruleOf(c: Pick<Commitment, 'rrule' | 'from'>): { days: string[]; interval: number } | null {
  if (!c.rrule) return null;
  const parts = Object.fromEntries(c.rrule.split(';').map((p) => p.split('=') as [string, string]));
  const days = (parts.BYDAY ?? '').split(',').filter(Boolean);
  return { days: days.length > 0 ? inOrder(days) : [CODES[weekdayOf(c.from)]], interval: Number(parts.INTERVAL) || 1 };
}

/** The days its rule gives from `from` to `to`, before breaks and exceptions. A class with no last day ends with the term. */
function meets(c: Commitment, from: DayKey, to: DayKey, termEnd: DayKey | null): DayKey[] {
  const rule = ruleOf(c);
  if (!rule) return c.from >= from && c.from <= to ? [c.from] : [];
  const last = c.until ?? (c.kind === 'class' ? termEnd : null);
  const end = last && last < to ? last : to;
  const out: DayKey[] = [];
  for (let d = c.from > from ? c.from : from; d <= end; d = addDays(d, 1)) {
    if (!rule.days.includes(CODES[weekdayOf(d)])) continue;
    if (Math.floor(daysBetween(mondayOf(c.from), d) / 7) % rule.interval !== 0) continue;
    out.push(d);
  }
  return out;
}

const inBreak = (breaks: readonly TermBreak[], day: DayKey) => breaks.some((b) => b.from <= day && day <= b.to);

/** The term's last day, as the School sheet has it. */
const termEndOf = (fake: Fake): DayKey | null => snapshotOf(fake).school?.termEnd || null;

/** A commitment's course and colour: its own space, else the space grouped by course. */
function placeOf(fake: Fake, c: Commitment) {
  const course = c.courseId ? fake.store.course.get(c.courseId) : undefined;
  const space =
    (c.spaceId ? fake.store.space.get(c.spaceId) : undefined) ??
    (course ? [...fake.store.space.values()].find((s) => s.groupKind === 'course') : undefined);
  return { spaceId: space?.id, hue: space?.hue ?? null, label: course ? courseLabel(course) : null };
}

/** Each day `c` really happens on from `from` to `to`: its rule, less breaks and skipped days, with moved days where they went. */
function occurrencesOf(fake: Fake, c: Commitment, from: DayKey, to: DayKey, termEnd: DayKey | null): CommitmentOccurrence[] {
  const breaks = [...fake.store.termBreak.values()];
  const place = placeOf(fake, c);
  const base = {
    commitmentId: c.id,
    title: c.title,
    kind: c.kind,
    bufferBefore: c.bufferBefore,
    bufferAfter: c.bufferAfter,
    hardness: c.hardness,
    ...(c.courseId ? { courseId: c.courseId } : {}),
    ...(place.spaceId ? { spaceId: place.spaceId } : {}),
    hue: place.hue,
    label: place.label,
  };
  const out: CommitmentOccurrence[] = [];
  // A month either side, so a day moved into the window from outside it is found.
  for (const date of meets(c, addDays(from, -31), addDays(to, 31), termEnd)) {
    if (c.kind === 'class' && inBreak(breaks, date)) continue;
    const ex = c.exceptions.find((e) => e.date === date);
    if (ex?.kind === 'skip') continue;
    if (ex?.kind === 'move') {
      const start = ex.start ?? c.start;
      out.push({
        ...base,
        date: ex.toDate ?? date,
        start,
        end: ex.end ?? start + (c.end - c.start),
        location: ex.location ?? c.location,
        movedFrom: date,
      });
    } else out.push({ ...base, date, start: c.start, end: c.end, location: c.location });
  }
  return out.filter((o) => o.date >= from && o.date <= to);
}

/** "MWF 10:00 AM to 10:50 AM", or "Oct 9, 5:00 PM to 9:00 PM" for something that happens once. */
function whenOf(item: { days: readonly string[]; from: DayKey; start: number; end: number }): string {
  const days = inOrder(item.days)
    .map((d) => LETTERS[d])
    .join('');
  return days ? `${days} ${span(item.start, item.end)}` : `${shortMonthDay(item.from)}, ${span(item.start, item.end)}`;
}

function rangeOf(c: Commitment, termEnd: DayKey | null): string {
  if (!c.rrule) return shortMonthDay(c.from);
  const last = c.until ?? (c.kind === 'class' ? termEnd : null);
  return last ? `${shortMonthDay(c.from)} to ${shortMonthDay(last)}` : `From ${shortMonthDay(c.from)}`;
}

// --- a day's free time ------------------------------------------------------------------

/** What `date` has left: the day (from now, today) less commitments with their travel, sleep and timed events. */
function freeOf(
  fake: Fake,
  date: DayKey,
  occurrences: readonly CommitmentOccurrence[],
  events: readonly CalendarEvent[],
  blocks: readonly TimeBlock[],
): FreeTime {
  const today = date === dayOf(fake.now, fake.zone);
  const from = today ? minuteOfDay(fake.now, fake.zone) : 0;
  const taken = new Array<boolean>(1440).fill(false);
  const busy: [number, number][] = [
    ...occurrences.map((o): [number, number] => [o.start - o.bufferBefore, o.end + o.bufferAfter]),
    ...sleepSpans(extra(fake).sleep),
    ...busySpans([], events, date, fake.zone),
  ];
  for (const [a, b] of busy) for (let m = Math.max(0, a); m < Math.min(1440, b); m++) taken[m] = true;
  const spans: [number, number][] = [];
  for (let m = from; m < 1440; m++) {
    if (taken[m]) continue;
    const last = spans.at(-1);
    if (last && last[1] === m) last[1] = m + 1;
    else spans.push([m, m + 1]);
  }
  const freeMin = spans.reduce((n, [a, b]) => n + (b - a), 0);
  const plannedMin = blocks
    .filter((b) => b.date === date)
    .reduce((n, b) => n + Math.max(0, Math.min(1440, b.start + b.minutes) - Math.max(from, b.start)), 0);
  const overMin = Math.max(0, plannedMin - freeMin);
  const where = today ? 'today' : `on ${shortMonthDay(date)}`;
  const line = `${formatMinutes(freeMin)} free ${where}. Planned ${formatMinutes(plannedMin)}.${
    overMin > 0 ? ` Move ${formatMinutes(overMin)}?` : ''
  }`;
  return { date, freeMin, plannedMin, overMin, spans, line };
}

// --- the snapshot -------------------------------------------------------------------------

// The days a snapshot was asked for are its window; the derive below reads them.
wrap('heat.snapshot', (next) => (args, fake) => {
  const { from, to } = args as { from?: DayKey; to?: DayKey };
  extra(fake).window = from && to ? { from, to } : null;
  return next(args, fake);
});

derive((snap, fake) => {
  const x = extra(fake);
  const date = snap.date;
  const term = { start: snap.school?.termStart || null, end: snap.school?.termEnd || null };
  const from = x.window ? (x.window.from < date ? x.window.from : date) : addDays(date, -45);
  const to = x.window ? (x.window.to > date ? x.window.to : date) : addDays(date, 130);

  const days: CommitmentsSnapshot['days'] = {};
  for (const c of snap.records.commitment) {
    for (const o of occurrencesOf(fake, c, from, to, term.end)) days[o.date] = [...(days[o.date] ?? []), o];
  }
  for (const list of Object.values(days)) list.sort((a, b) => a.start - b.start);

  const nowMin = minuteOfDay(fake.now, fake.zone);
  const coming = (days[date] ?? []).find((o) => o.start >= nowMin);
  const leaveAt = coming && coming.bufferBefore > 0 ? coming.start - coming.bufferBefore : null;

  const conflicts: CommitmentsSnapshot['conflicts'] = [];
  for (const b of snap.records.timeBlock) {
    for (const o of days[b.date] ?? []) {
      const end = b.start + b.minutes;
      if (end <= o.start - o.bufferBefore || b.start >= o.end + o.bufferAfter) continue;
      const bufferOnly = end <= o.start || b.start >= o.end;
      conflicts.push({
        blockId: b.id,
        commitmentId: o.commitmentId,
        date: b.date,
        bufferOnly,
        line: bufferOnly
          ? `Overlaps the travel time for ${o.title} (${span(o.start, o.end)}).`
          : `Overlaps ${o.title} (${span(o.start, o.end)}).`,
      });
    }
  }

  snap.commitments = {
    days,
    list: snap.records.commitment.map((c) => {
      const rule = ruleOf(c);
      const place = placeOf(fake, c);
      return {
        id: c.id,
        when: whenOf({ days: rule?.days ?? [], from: c.from, start: c.start, end: c.end }),
        range: rangeOf(c, term.end),
        days: rule?.days ?? [],
        hue: place.hue,
        label: place.label,
      };
    }),
    next: coming
      ? {
          commitmentId: coming.commitmentId,
          title: coming.title,
          date,
          start: coming.start,
          leaveAt,
          line: `NEXT ${coming.title} ${readout(coming.start)}${leaveAt === null ? '' : ` · LEAVE ${readout(leaveAt)}`}`,
        }
      : null,
    free: freeOf(fake, date, days[date] ?? [], todayOf(fake).events, snap.records.timeBlock),
    conflicts,
    sleep: x.sleep,
    term,
    drafts: x.drafts,
    pending: x.pending,
    feeds: x.feeds,
  };
});

// --- one commitment, by hand --------------------------------------------------------------

/** Minutes after midnight from 600, "10:00" or "5pm". */
function minutesOf(value: unknown, what: string): number {
  if (typeof value === 'number' && Number.isFinite(value)) return Math.round(value);
  const m = /^\s*(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\s*$/i.exec(String(value ?? ''));
  if (!m) refuse(`Give it ${what} time, such as 10:00 or 5pm.`);
  const hour = m[3] ? (Number(m[1]) % 12) + (m[3].toLowerCase() === 'pm' ? 12 : 0) : Number(m[1]);
  return hour * 60 + Number(m[2] ?? 0);
}

const weekly = (days: readonly string[], interval?: number) =>
  `FREQ=WEEKLY${interval && interval > 1 ? `;INTERVAL=${interval}` : ''};BYDAY=${inOrder(days).join(',')}`;

/**
 * `was` with what a sheet or Claude sent laid over it, checked. A course
 * named by a code Learn doesn't hold comes back as `code`, for the write to
 * make its stub in the same entry.
 */
function fields(fake: Fake, was: Commitment, a: CommitmentInput): { commitment: Commitment; code: string | null } {
  const c: Commitment = { ...was, exceptions: [...was.exceptions] };
  if (a.title !== undefined) c.title = String(a.title).trim();
  if (a.kind) c.kind = a.kind;
  if (a.start !== undefined) c.start = minutesOf(a.start, 'a start');
  if (a.end !== undefined) c.end = minutesOf(a.end, 'an end');
  if (a.location !== undefined) c.location = String(a.location).trim();
  if (a.bufferBefore !== undefined) c.bufferBefore = Math.round(a.bufferBefore);
  if (a.bufferAfter !== undefined) c.bufferAfter = Math.round(a.bufferAfter);
  if (a.hardness) c.hardness = a.hardness;
  if (a.rrule !== undefined) {
    if (a.rrule) c.rrule = a.rrule;
    else delete c.rrule;
  }
  if (a.days !== undefined) {
    if (a.days.length > 0) c.rrule = weekly(a.days, a.interval);
    else delete c.rrule;
  }
  if (a.date) {
    c.from = a.date;
    if (!a.days?.length && !a.rrule) delete c.rrule;
  }
  if (a.from) c.from = a.from;
  if (a.until !== undefined) {
    if (a.until) c.until = a.until;
    else delete c.until;
  }
  if (!c.rrule) delete c.until;

  let code: string | null = null;
  if (a.courseId !== undefined) {
    if (a.courseId && !fake.store.course.has(a.courseId)) refuse('No course has that id.');
    if (a.courseId) c.courseId = a.courseId;
    else delete c.courseId;
  } else if (a.course?.trim()) {
    const named = a.course.trim();
    const held = [...fake.store.course.values()].find((k) => squashed(k.code) === squashed(named));
    if (held) c.courseId = held.id;
    else code = named;
  }
  if (a.spaceId !== undefined) {
    if (a.spaceId && !fake.store.space.has(a.spaceId)) refuse('No space has that id.');
    if (a.spaceId) c.spaceId = a.spaceId;
    else delete c.spaceId;
  } else if (a.space?.trim()) {
    const named = a.space.trim();
    const space = [...fake.store.space.values()].find((s) => squashed(s.name) === squashed(named));
    if (!space) refuse(`Learn has no space named ${named}.`);
    c.spaceId = space.id;
  }

  if (!c.title) refuse('Give the commitment a name first.');
  if (c.end <= c.start) refuse('It ends before it starts.');
  if (c.start < 0 || c.end > 1440) refuse('A commitment never runs past midnight: one that does is two.');
  if ([c.bufferBefore, c.bufferAfter].some((b) => !(b >= 0 && b <= 240))) refuse('Travel time is between 0 and 240 minutes.');
  if (c.until && c.until < c.from) refuse('Its last day is before its first.');
  return { commitment: c, code };
}

/** A course found from a schedule, still waiting for its syllabus, as a sync makes one. Called inside a write. */
function stubCourse(fake: Fake, code: string): Course {
  let term = [...fake.store.term.values()].at(-1);
  if (!term) {
    term = { id: fake.newId(), name: 'Term' };
    fake.store.term.set(term.id, term);
  }
  const course: Course = {
    id: fake.newId(),
    termId: term.id,
    code,
    name: '',
    categories: [],
    notes: '',
    status: 'stub',
    public: false,
  };
  fake.store.course.set(course.id, course);
  return course;
}

/** A commitment by its id, or by its title when only one has it. */
function find(fake: Fake, id: unknown): Commitment {
  const byId = fake.store.commitment.get(String(id));
  if (byId) return byId;
  const named = [...fake.store.commitment.values()].filter((c) => squashed(c.title) === squashed(String(id ?? '')));
  if (named.length === 1) return named[0];
  return refuse(named.length > 1 ? `More than one commitment is called ${id}.` : 'No commitment has that id.');
}

const lineOf = (c: Commitment) => whenOf({ days: ruleOf(c)?.days ?? [], from: c.from, start: c.start, end: c.end });

register('heat.commitment.create', (args, fake) => {
  const a = args as CommitmentInput;
  if (a.start === undefined || a.end === undefined) refuse('Give it a start and an end time.');
  const blank: Commitment = {
    id: fake.newId(),
    title: '',
    kind: a.course || a.courseId ? 'class' : 'other',
    location: '',
    start: 0,
    end: 0,
    from: dayOf(fake.now, fake.zone),
    exceptions: [],
    bufferBefore: 0,
    bufferAfter: 0,
    hardness: 'fixed',
    source: 'you',
  };
  const { commitment, code } = fields(fake, blank, a);
  let course: Course | null = null;
  const { undo } = fake.write('add commitment', ['commitment', 'course', 'term'], () => {
    if (code) {
      course = stubCourse(fake, code);
      commitment.courseId = course.id;
    }
    fake.store.commitment.set(commitment.id, commitment);
  });
  return { commitment, course, line: `Added ${commitment.title}, ${lineOf(commitment)}.`, undo };
});

register('heat.commitment.update', (args, fake) => {
  const was = find(fake, args.id);
  const { commitment, code } = fields(fake, was, (args.set ?? {}) as CommitmentInput);
  const { undo } = fake.write('edit commitment', ['commitment', 'course', 'term'], () => {
    if (code) commitment.courseId = stubCourse(fake, code).id;
    fake.store.commitment.set(commitment.id, commitment);
  });
  return { commitment, line: `Saved ${commitment.title}, ${lineOf(commitment)}.`, undo };
});

/** Writes one day's exception over whatever that day had, as one entry: "skip class", "move shift". */
function except(fake: Fake, c: Commitment, exception: CommitmentException) {
  if (!meets(c, exception.date, exception.date, termEndOf(fake)).includes(exception.date)) {
    refuse(`${c.title} doesn’t meet on ${longDay(exception.date)}.`);
  }
  const commitment: Commitment = {
    ...c,
    exceptions: [...c.exceptions.filter((e) => e.date !== exception.date), exception].sort((a, b) =>
      a.date.localeCompare(b.date),
    ),
  };
  const { undo } = fake.write(`${exception.kind} ${NOUN[c.kind]}`, ['commitment'], () =>
    fake.store.commitment.set(c.id, commitment),
  );
  const line =
    exception.kind === 'skip'
      ? `${c.title} is skipped ${dayWords(exception.date)}.`
      : `${c.title} moves from ${dayWords(exception.date)} to ${dayWords(exception.toDate ?? exception.date)}, ${clock(exception.start ?? c.start)}.`;
  return { commitment, line, undo };
}

register('heat.commitment.addException', (args, fake) => {
  const c = find(fake, args.id);
  const a = args as unknown as CommitmentException;
  if (!a.date) refuse('Say which day.');
  if (a.kind !== 'skip' && a.kind !== 'move') refuse('A day is skipped or moved.');
  const start = a.start === undefined ? undefined : minutesOf(a.start, 'a start');
  const end = a.end === undefined ? undefined : minutesOf(a.end, 'an end');
  if (start !== undefined && end !== undefined && end <= start) refuse('It ends before it starts.');
  return except(fake, c, {
    date: a.date,
    kind: a.kind,
    ...(a.kind === 'move' && a.toDate ? { toDate: a.toDate } : {}),
    ...(a.kind === 'move' && start !== undefined ? { start } : {}),
    ...(a.kind === 'move' && end !== undefined ? { end } : {}),
    ...(a.location ? { location: a.location } : {}),
    ...(a.note ? { note: a.note } : {}),
    source: a.source ?? 'you',
  });
});

register('heat.commitment.removeException', (args, fake) => {
  const c = find(fake, args.id);
  if (!c.exceptions.some((e) => e.date === args.date)) refuse(`${c.title} has nothing different on that day.`);
  const commitment: Commitment = { ...c, exceptions: c.exceptions.filter((e) => e.date !== args.date) };
  const { undo } = fake.write(`restore ${NOUN[c.kind]}`, ['commitment'], () => fake.store.commitment.set(c.id, commitment));
  return { commitment, undo };
});

// --- a schedule read into a draft ------------------------------------------------------------

type DraftItem = NonNullable<CommitmentDraft['items']>[number];

/** Whether no commitment with the same title, times and rule is there already (a week's own shifts aside: those are replaced). */
function isNew(fake: Fake, item: Pick<DraftItem, 'title' | 'start' | 'end' | 'days' | 'from'>, draft: CommitmentDraft): boolean {
  return ![...fake.store.commitment.values()].some((c) => {
    if (draft.mode === 'week' && c.kind === 'work' && c.weekOf === draft.weekOf) return false;
    const rule = ruleOf(c);
    return (
      squashed(c.title) === squashed(item.title) &&
      c.start === item.start &&
      c.end === item.end &&
      (rule ? rule.days.join() === inOrder(item.days).join() : item.days.length === 0 && c.from === item.from)
    );
  });
}

const breakIsNew = (fake: Fake, b: ScheduleBreak) =>
  ![...fake.store.termBreak.values()].some((t) => t.from === b.from && t.to === b.to);

const counted = (n: number, kind: CommitmentKind) => plural(n, NOUN[kind], kind === 'class' ? 'classes' : undefined);

/** A time as it was read, or null for one that isn't a time. */
function timeOf(value: unknown): number | null {
  try {
    return minutesOf(value, 'a');
  } catch {
    return null;
  }
}

/** A schedule as read, made into the draft a person previews: each line checked, and said whether it is new. */
function read(fake: Fake, base: CommitmentDraft, lines: ScheduleItem[], days: ScheduleBreak[]): CommitmentDraft {
  const today = dayOf(fake.now, fake.zone);
  let unread = 0;
  const items: DraftItem[] = [];
  for (const line of lines) {
    const title = String(line.title ?? '').trim();
    const start = timeOf(line.start);
    const end = timeOf(line.end);
    // A line with no time, or an end before its start, is counted and left out.
    if (!title || start === null || end === null || end <= start) {
      unread += 1;
      continue;
    }
    const course = line.course?.trim() || null;
    const kind: CommitmentKind = line.kind ?? (course ? 'class' : base.mode === 'week' ? 'work' : 'other');
    const item = {
      title,
      kind,
      start,
      end,
      location: line.location ?? '',
      from: line.date ?? line.from ?? today,
      until: line.until ?? null,
      days: inOrder(line.date ? [] : (line.days ?? [])),
      course,
    };
    items.push({
      ...item,
      when: whenOf(item),
      isNew: isNew(fake, item, base),
      newCourse: course !== null && ![...fake.store.course.values()].some((c) => squashed(c.code) === squashed(course)),
    });
  }
  const breaks = days.map((b) => ({ ...b, isNew: breakIsNew(fake, b) }));
  const replaces =
    base.mode === 'week'
      ? [...fake.store.commitment.values()].filter((c) => c.kind === 'work' && c.weekOf === base.weekOf).length
      : 0;

  const fresh = items.filter((i) => i.isNew);
  const kinds = (['class', 'work', 'commute', 'other'] as const)
    .map((k) => [k, fresh.filter((i) => i.kind === k).length] as const)
    .filter(([, n]) => n > 0)
    .map(([k, n]) => counted(n, k));
  const newBreaks = breaks.filter((b) => b.isNew).length;
  if (newBreaks > 0) kinds.push(plural(newBreaks, 'break'));
  const newCourses = new Set(fresh.filter((i) => i.newCourse).map((i) => squashed(i.course ?? ''))).size;
  const already = items.length - fresh.length + breaks.length - newBreaks;
  const parts = [
    kinds.length > 0 ? kinds.join(' and ') : 'Nothing new',
    ...(replaces > 0 ? [`Replaces ${plural(replaces, 'shift')} in the week of ${shortMonthDay(base.weekOf)}`] : []),
    ...(newCourses > 0 ? [plural(newCourses, 'new course')] : []),
    ...(already > 0 ? [`${already} already there`] : []),
  ];
  return { ...base, state: 'ready', items, breaks, replaces, unread, line: `${parts.join('. ')}.` };
}

/** What pasted text or a photo "reads" as here, for each way of reading it. */
function canned(fake: Fake, draft: CommitmentDraft): { items: ScheduleItem[]; breaks: ScheduleBreak[] } {
  const today = dayOf(fake.now, fake.zone);
  if (draft.mode === 'breaks') {
    const { year } = keyParts(today);
    return { items: [], breaks: [{ title: 'Thanksgiving recess', from: `${year}-11-25`, to: `${year}-11-29` }] };
  }
  if (draft.mode === 'week') {
    const shift = (n: number, start: number, end: number): ScheduleItem => ({
      title: 'Campus store',
      kind: 'work',
      date: addDays(draft.weekOf, n),
      start,
      end,
      location: 'Memorial Union',
    });
    return { items: [shift(1, 17 * 60, 21 * 60), shift(5, 10 * 60, 16 * 60)], breaks: [] };
  }
  return {
    items: [
      { title: 'JPN 101', days: ['MO', 'WE', 'FR'], start: 600, end: 650, location: 'Swan Hall 201', course: 'JPN 101', from: today },
      { title: 'EGR 101', days: ['TU', 'TH'], start: 840, end: 915, location: 'Bliss Hall 190', course: 'EGR 101', from: today },
    ],
    breaks: [],
  };
}

/** Keeps a draft at `reading` until `readyDraft` or `failDraft` is called, so a test can look at it. */
export function holdReading(fake: Fake) {
  extra(fake).held = true;
}

const readingAt = (fake: Fake, draftId: Id) =>
  extra(fake).drafts.findIndex((d) => d.id === draftId && d.state === 'reading');

/** The reading ends: the draft turns ready with these lines and days off (the canned ones when none are given). */
export function readyDraft(fake: Fake, draftId: Id, items?: ScheduleItem[], breaks: ScheduleBreak[] = []) {
  const at = readingAt(fake, draftId);
  if (at < 0) return;
  const list = extra(fake).drafts;
  const from = items ? { items, breaks } : canned(fake, list[at]);
  list[at] = read(fake, list[at], from.items, from.breaks);
  fake.emit([]);
}

/** The reading ends badly: the draft says why, in one sentence. */
export function failDraft(fake: Fake, draftId: Id, error: string) {
  const at = readingAt(fake, draftId);
  if (at < 0) return;
  const list = extra(fake).drafts;
  list[at] = { ...list[at], state: 'failed', error };
  fake.emit([]);
}

const MODES: ScheduleMode[] = ['schedule', 'week', 'breaks'];
const fileOf = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** A draft at `reading`, not yet kept. */
function blank(fake: Fake, args: Record<string, unknown>, source: CommitmentDraft['source'], fileName: string): CommitmentDraft {
  const mode = (args.mode as ScheduleMode | undefined) ?? 'schedule';
  if (!MODES.includes(mode)) refuse('A schedule is read as a timetable, a week’s shifts or an academic calendar.');
  return {
    id: fake.newId(),
    mode,
    source,
    fileName,
    weekOf: mondayOf((args.weekOf as DayKey | undefined) ?? dayOf(fake.now, fake.zone)),
    createdAt: fake.now,
    state: 'reading',
  };
}

/** Keeps a draft. One still being read turns ready a moment after the answer, as Claude's does, unless a test holds it. */
function keep(fake: Fake, draft: CommitmentDraft): { draft: CommitmentDraft } {
  const x = extra(fake);
  x.drafts.push(draft);
  if (draft.state === 'reading' && !x.held) setTimeout(() => readyDraft(fake, draft.id), 0);
  fake.emit([]);
  return { draft };
}

register('heat.commitment.importText', (args, fake) => {
  const json = args.json as { items?: ScheduleItem[]; breaks?: ScheduleBreak[] } | undefined;
  if (!json && !String(args.text ?? '').trim()) refuse('Paste the schedule first.');
  const draft = blank(fake, args, 'paste', '');
  // What the caller read itself is ready at once.
  return keep(fake, json ? read(fake, draft, json.items ?? [], json.breaks ?? []) : draft);
});

register('heat.commitment.importImage', (args, fake) => {
  const name = fileOf(String(args.path ?? ''));
  if (!/\.(png|jpe?g|heic|webp|gif|tiff?|bmp)$/i.test(name)) refuse('Learn reads a schedule from a photo or a screenshot.');
  return keep(fake, blank(fake, args, 'photo', name));
});

register('heat.commitment.importIcs', (args, fake) => {
  const url = typeof args.url === 'string' ? args.url.trim() : '';
  const path = typeof args.path === 'string' ? args.path : '';
  if (!url && !path && !String(args.text ?? '').trim()) refuse('A calendar comes as an .ics file, its text or its address.');
  if (path && !/\.ics$/i.test(path)) refuse('Learn reads a calendar from an .ics file.');
  if (url && !/^(https?|webcal):\/\//i.test(url)) refuse('A calendar’s address starts with https:// or webcal://.');
  const fileName = path ? fileOf(path) : url ? url.replace(/^\w+:\/\//, '').split('/')[0] : 'calendar.ics';
  const name = String(args.name ?? '').trim() || fileName.replace(/\.ics$/i, '');
  // A calendar file is read in the core, with no model, so its draft is ready at once. Here it always holds one shift.
  const draft = read(fake, blank(fake, args, 'ics', fileName), [{ title: name, kind: 'work', days: ['SA'], start: 540, end: 780 }], []);
  if (args.subscribe !== true) return keep(fake, draft);

  // Kept in step: the address is held and its commitments are written at once.
  if (!url) refuse('Only a calendar’s address can be kept in step.');
  const feed = { id: fake.newId(), name, lastSyncedAt: fake.now };
  let changed = 0;
  const { undo } = fake.write('schedule sync', ['commitment'], () => {
    for (const item of draft.items ?? []) {
      if (!item.isNew) continue;
      const c = fromItem(fake, item, 'ics');
      fake.store.commitment.set(c.id, { ...c, feedId: feed.id });
      changed += 1;
    }
  });
  extra(fake).feeds.push(feed);
  fake.emit([]);
  return { feed, changed, undo };
});

/** A draft's line as the commitment it becomes. */
function fromItem(fake: Fake, item: DraftItem, source: Commitment['source'], weekOf?: DayKey): Commitment {
  return {
    id: fake.newId(),
    title: item.title,
    kind: item.kind,
    location: item.location,
    start: item.start,
    end: item.end,
    ...(item.days.length > 0 ? { rrule: weekly(item.days) } : {}),
    from: item.from,
    ...(item.until && item.days.length > 0 ? { until: item.until } : {}),
    exceptions: [],
    bufferBefore: 0,
    bufferAfter: 0,
    hardness: 'fixed',
    source,
    ...(weekOf && item.kind === 'work' ? { weekOf } : {}),
  };
}

register('heat.commitment.draft.accept', (args, fake) => {
  const list = extra(fake).drafts;
  const draft = list.find((d) => d.id === args.draftId);
  if (!draft) refuse('No schedule draft has that id.');
  if (draft.state !== 'ready') refuse('This schedule isn’t read yet.');
  const skip = new Set((args.skip as number[] | undefined) ?? []);
  const label = draft.mode === 'week' ? "this week's shifts" : draft.mode === 'breaks' ? 'import breaks' : 'import schedule';
  const done = { commitments: 0, breaks: 0, replaced: 0, courses: [] as Course[] };

  const { undo } = fake.write(label, ['commitment', 'termBreak', 'course', 'term'], () => {
    // A week's shifts replace that week's, and no other week is touched.
    if (draft.mode === 'week') {
      for (const [id, c] of fake.store.commitment) {
        if (c.kind !== 'work' || c.weekOf !== draft.weekOf) continue;
        fake.store.commitment.delete(id);
        done.replaced += 1;
      }
    }
    (draft.items ?? []).forEach((item, i) => {
      if (skip.has(i) || !isNew(fake, item, draft)) return;
      const c = fromItem(fake, item, draft.source, draft.mode === 'week' ? draft.weekOf : undefined);
      if (item.kind === 'class' && item.course) {
        const code = item.course;
        let course = [...fake.store.course.values()].find((k) => squashed(k.code) === squashed(code));
        if (!course) done.courses.push((course = stubCourse(fake, code)));
        c.courseId = course.id;
      }
      fake.store.commitment.set(c.id, c);
      done.commitments += 1;
    });
    for (const b of draft.breaks ?? []) {
      if (!breakIsNew(fake, b)) continue;
      const id = fake.newId();
      fake.store.termBreak.set(id, { id, title: b.title, from: b.from, to: b.to, source: draft.source });
      done.breaks += 1;
    }
    // The draft is spent before the views hear of the change.
    list.splice(list.indexOf(draft), 1);
  });
  return { ...done, undo };
});

register('heat.commitment.draft.discard', (args, fake) => {
  const list = extra(fake).drafts;
  const at = list.findIndex((d) => d.id === args.draftId);
  if (at < 0) refuse('No schedule draft has that id.');
  list.splice(at, 1);
  fake.emit([]);
  return {};
});

// --- what a mail asked for --------------------------------------------------------------------

/** A mail that cancels (or moves) one day of a commitment, waiting for its one tap. */
export function addPending(
  fake: Fake,
  ask: { commitmentId: Id; date: DayKey; kind?: 'skip' | 'move'; toDate?: DayKey; start?: number; subject?: string },
): PendingException {
  const c = find(fake, ask.commitmentId);
  const kind = ask.kind ?? 'skip';
  const pending: PendingException = {
    id: `mail-${fake.newId()}`,
    commitmentId: c.id,
    title: c.title,
    kind,
    exception: {
      date: ask.date,
      kind,
      ...(ask.toDate ? { toDate: ask.toDate } : {}),
      ...(ask.start !== undefined ? { start: ask.start } : {}),
      source: 'mail',
    },
    line:
      kind === 'skip'
        ? `${c.title} is canceled ${dayWords(ask.date)}. Skip it?`
        : `${c.title} is moved from ${dayWords(ask.date)}. Move it?`,
    act: kind === 'skip' ? 'Skip it' : 'Move it',
    ...(ask.subject ? { subject: ask.subject } : {}),
    createdAt: fake.now,
  };
  extra(fake).pending.push(pending);
  fake.emit([]);
  return pending;
}

/** What waits under `id`, taken off the list: it isn't offered again. */
function takePending(fake: Fake, id: unknown): PendingException {
  const list = extra(fake).pending;
  const at = list.findIndex((p) => p.id === id);
  if (at < 0) refuse('Nothing is waiting under that id.');
  return list.splice(at, 1)[0];
}

register('heat.commitment.exception.confirm', (args, fake) => {
  const waiting = extra(fake).pending.find((p) => p.id === args.id);
  if (!waiting) refuse('Nothing is waiting under that id.');
  // Refused for a day it doesn't meet on, and then it still waits.
  const answer = except(fake, find(fake, waiting.commitmentId), waiting.exception);
  takePending(fake, args.id);
  fake.emit([]);
  return answer;
});

register('heat.commitment.exception.dismiss', (args, fake) => {
  takePending(fake, args.id);
  fake.emit([]);
  return {};
});

// --- subscribed calendars, free time and sleep ----------------------------------------------

register('heat.commitment.feed.sync', (args, fake) => {
  for (const f of extra(fake).feeds) if (!args.id || f.id === args.id) f.lastSyncedAt = fake.now;
  fake.emit([]);
  return { changed: 0 };
});

register('heat.commitment.feed.remove', (args, fake) => {
  const x = extra(fake);
  if (!x.feeds.some((f) => f.id === args.id)) refuse('No subscribed calendar has that id.');
  // What it made stays.
  x.feeds = x.feeds.filter((f) => f.id !== args.id);
  fake.emit([]);
  return {};
});

register('heat.planner.freeTime', (args, fake) => {
  const today = dayOf(fake.now, fake.zone);
  const from = (args.from as DayKey | undefined) ?? (args.date as DayKey | undefined) ?? today;
  const to = (args.to as DayKey | undefined) ?? from;
  if (to < from || daysBetween(from, to) > 62) refuse('Ask for up to two months of days, first day first.');
  const snap = snapshotOf(fake);
  const termEnd = snap.school?.termEnd || null;
  const days = [];
  for (let d = from; d <= to; d = addDays(d, 1)) {
    const commitments = snap.records.commitment
      .flatMap((c) => occurrencesOf(fake, c, d, d, termEnd))
      .sort((a, b) => a.start - b.start);
    days.push({ ...freeOf(fake, d, commitments, todayOf(fake).events, snap.records.timeBlock), commitments });
  }
  return { days };
});

register('heat.sleep.set', (args, fake) => {
  const [from, to] = [Number(args.from), Number(args.to)];
  if (![from, to].every((m) => Number.isInteger(m) && m >= 0 && m < 1440)) {
    refuse('Sleep is two times of day: to bed, and up.');
  }
  extra(fake).sleep = { from, to };
  fake.emit([]);
  return {};
});
