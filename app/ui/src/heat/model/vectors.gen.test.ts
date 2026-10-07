// @vitest-environment node
//
// Cross-check vectors for the Rust port of this model (crates/wi-heat/src/model).
//
// Skipped unless WRITE_HEAT_VECTORS=1. When it is on, this file generates
// inputs for every exported function with a seeded PRNG (exhaustive sweeps
// where the space is small), runs the TypeScript over them, and writes
// {input, output} pairs to crates/wi-heat/tests/vectors/<module>.json, one
// case to a line. crates/wi-heat/tests/model_vectors.rs reads those files and
// asserts the Rust gives the same output for every input, floats bit for bit.
//
//   cd app/ui && WRITE_HEAT_VECTORS=1 npx vitest run src/heat/model/vectors.gen.test.ts
//
// Inputs and outputs are plain JSON: what JSON.stringify drops or flattens
// (undefined, NaN) is dropped or flattened the same way on both sides. A
// function that makes ids is given "idPrefix", and the Rust makes the same
// "<prefix>-1", "<prefix>-2", … in the same order.

import { describe, it } from 'vitest';
import {
  addDays,
  atMinute,
  dayKey,
  daysBetween,
  daysInMonth,
  epochOf,
  keyOf,
  keyParts,
  minuteOfDay,
  startOfDay,
  wallTime,
  weekdayOf,
} from '../../shared/time/zone';
import { clock, clockAt, countdown, monthDay, shortMonthDay } from '../../shared/time/format';
import {
  actualMin,
  averageLines,
  estimateContext,
  estimateMin,
  formatMinutes,
  weeklyLoad,
  weeklyLoadLine,
} from './estimate';
import type { Task } from './records';
import { draftKey, acceptDraft, blockLength, dragOntoColumn, initialScrollTop, minutesToY, moveBlock, nowLineY, planMyDay, planNext, planReason, planSections, planSubtitle, resizeBlock, snap, yToMinutes, type PlanData } from './plan';
import { checkOff, finishWithTime, focusLcd, focusStep, focusStrip, initialFocus, isFocusLength, setTook, type FocusEvent, type FocusSettings, type FocusState, type FocusTarget } from './focus';
import { taskHalf, type LcdData } from './lcd';
import { dayLayout, dueAtEndOfDay, monthGrid, page, unscheduledTray, weekDays, type CalendarData } from './calendar';
import { factLines, lastWeekFacts, type ReviewData } from './review';
import { importArtifact, parseHeatExport, type HeatExport } from './importArtifact';
import {
  addHabit,
  doneRecord,
  habitLabel,
  habitLine,
  lastFourteen,
  markHabitDone,
  streak,
  todayOrbs,
  toggleHabit,
  yearGrid,
} from './habits';
import {
  basedOnLine,
  categoryPct,
  currentPct,
  decidedPct,
  gradesWidget,
  guessCategory,
  letterFor,
  weightsLine,
  whatItWouldTake,
} from './grades';
import { byHeat, DAY_MS, duePhrase, heatOf, heatThresholds, nextHeatChange, runway, tubeFill } from './heat';
import {
  checkOccurrence,
  nextOpenOccurrence,
  occurrencesBetween,
  openTasks,
  parseRule,
  recurs,
  reopenOccurrence,
  seriesStart,
  setRepeat,
  taskOccurrences,
  withEffectiveDue,
} from './recurrence';
import { defaultSpaces, groupCounts, groupName, inSpace, libraryLists, spaceCounts } from './spaces';

const ENABLED = (globalThis as { process?: { env?: Record<string, string | undefined> } }).process?.env
  ?.WRITE_HEAT_VECTORS === '1';

// --- A seeded PRNG ----------------------------------------------------------

class Rng {
  private a: number;
  constructor(seed: number) {
    this.a = seed >>> 0;
  }
  next(): number {
    this.a = (this.a + 0x6d2b79f5) >>> 0;
    let t = this.a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }
  /** An integer from lo to hi, both inclusive. */
  int(lo: number, hi: number): number {
    return lo + Math.floor(this.next() * (hi - lo + 1));
  }
  float(lo: number, hi: number): number {
    return lo + this.next() * (hi - lo);
  }
  chance(p: number): boolean {
    return this.next() < p;
  }
  pick<T>(xs: readonly T[]): T {
    return xs[this.int(0, xs.length - 1)];
  }
  /** A weighted choice: [weight, value] pairs. */
  weighted<T>(xs: readonly (readonly [number, T])[]): T {
    const total = xs.reduce((s, [w]) => s + w, 0);
    let x = this.next() * total;
    for (const [w, v] of xs) {
      if ((x -= w) < 0) return v;
    }
    return xs[xs.length - 1][1];
  }
  sample<T>(xs: readonly T[], n: number): T[] {
    const copy = [...xs];
    const out: T[] = [];
    while (out.length < n && copy.length > 0) out.push(copy.splice(this.int(0, copy.length - 1), 1)[0]);
    return out;
  }
}

// --- Collecting cases and writing the files -----------------------------------

type Case = { input: unknown; output: unknown };
const SEED = 20261007;
const collected = new Map<string, Map<string, Case[]>>();
const skipped = new Map<string, number>();

/**
 * One function's cases. `make` builds an input, `run` calls the TypeScript on
 * it. The input is round-tripped through JSON first, so the TypeScript sees
 * exactly the data the Rust is given; a case whose run throws is dropped.
 */
function add<I extends object>(
  module: string,
  fn: string,
  n: number,
  make: (r: Rng, i: number) => I | null,
  run: (input: I) => unknown,
): void {
  const cases: Case[] = [];
  const r = new Rng(SEED ^ hash(`${module}.${fn}`));
  let made = 0;
  for (let i = 0; made < n && i < n * 20; i++) {
    const raw = make(r, made);
    if (raw === null) continue;
    made++;
    const input = JSON.parse(JSON.stringify(raw)) as I;
    try {
      const output = JSON.parse(JSON.stringify(run(input)) ?? 'null');
      cases.push({ input, output });
    } catch {
      skipped.set(`${module}.${fn}`, (skipped.get(`${module}.${fn}`) ?? 0) + 1);
    }
  }
  const fns = collected.get(module) ?? new Map<string, Case[]>();
  fns.set(fn, cases);
  collected.set(module, fns);
}

function hash(text: string): number {
  let h = 2166136261;
  for (let i = 0; i < text.length; i++) h = Math.imul(h ^ text.charCodeAt(i), 16777619);
  return h >>> 0;
}

/** Every case of a list, a line to each, so a diff reads. */
function format(module: string, fns: Map<string, Case[]>): string {
  const body = [...fns.entries()]
    .map(([fn, cases]) => `${JSON.stringify(fn)}: [\n${cases.map((c) => JSON.stringify(c)).join(',\n')}\n]`)
    .join(',\n');
  const dropped = Object.fromEntries([...skipped].filter(([k]) => k.startsWith(`${module}.`)).map(([k, n]) => [k.slice(module.length + 1), n]));
  return `{\n"module": ${JSON.stringify(module)},\n"seed": ${SEED},\n"dropped": ${JSON.stringify(dropped)},\n"functions": {\n${body}\n}\n}\n`;
}

async function write(): Promise<void> {
  const fs = (await import('node:fs' as string)) as {
    mkdirSync(path: URL, options: { recursive: boolean }): void;
    writeFileSync(path: URL, data: string): void;
  };
  const dir = new URL('../../../../../crates/wi-heat/tests/vectors/', import.meta.url);
  fs.mkdirSync(dir, { recursive: true });
  for (const [module, fns] of collected) fs.writeFileSync(new URL(`${module}.json`, dir), format(module, fns));
}

// --- Shared generators --------------------------------------------------------

const ZONES = [
  'America/New_York',
  'UTC',
  'Europe/London',
  'Asia/Tokyo',
  'America/Los_Angeles',
  'Australia/Lord_Howe',
  'Asia/Kolkata',
  'America/Sao_Paulo',
  'Pacific/Auckland',
  'Africa/Cairo',
  'Pacific/Kiritimati',
  'America/St_Johns',
];

const NY = 'America/New_York';

/** The instant a wall clock names in `tz`. */
const wall = (tz: string, y: number, mo: number, d: number, h = 0, mi = 0) =>
  epochOf({ year: y, month: mo, day: d, hour: h, minute: mi }, tz);

/** Every moment a zone's offset changes between two years, found by scanning and bisecting. */
function transitions(tz: string, fromYear: number, toYear: number): number[] {
  const out: number[] = [];
  const off = (ms: number) => {
    const w = wallTime(ms, tz);
    return Date.UTC(w.year, w.month - 1, w.day, w.hour, w.minute, w.second) - Math.floor(ms / 1000) * 1000;
  };
  const step = 6 * 3_600_000;
  let t = Date.UTC(fromYear, 0, 1);
  const end = Date.UTC(toYear + 1, 0, 1);
  let prev = off(t);
  for (t += step; t < end; t += step) {
    const o = off(t);
    if (o !== prev) {
      let lo = t - step;
      let hi = t;
      while (hi - lo > 1000) {
        const mid = Math.floor((lo + hi) / 2000) * 1000;
        if (off(mid) === prev) lo = mid;
        else hi = mid;
      }
      out.push(hi);
      prev = o;
    }
  }
  return out;
}

const TRANSITIONS = new Map(ZONES.map((z) => [z, transitions(z, 2024, 2031)]));

const Y2026 = Date.UTC(2026, 0, 1);

/** An instant: mostly the next few weeks, sometimes years away, sometimes right at a DST change. */
function instant(r: Rng, tz = NY): number {
  const roll = r.next();
  if (roll < 0.15) {
    const ts = TRANSITIONS.get(tz) ?? [];
    if (ts.length > 0) return r.pick(ts) + r.pick([-3_600_000, -60_000, -1000, -1, 0, 1, 1000, 59_999, 3_600_000, 7_200_000]);
  }
  if (roll < 0.6) return Date.UTC(2026, 9, 6, 12) + r.int(-20 * 86_400, 60 * 86_400) * 1000;
  if (roll < 0.9) return Y2026 + r.int(-400 * 86_400, 1500 * 86_400) * 1000;
  return Date.UTC(1990, 0, 1) + r.int(0, 60 * 365 * 86_400) * 1000;
}


// --- Records ----------------------------------------------------------------------------

const SPACE_IDS = ['classes', 'wwav', 'personal'];
const TYPES = ['Homework', 'Quiz', 'Lab', 'Other', 'Reading', 'Exam prep'];
const SOURCES = ['you', 'calendar', 'ical', 'mail', 'capture'] as const;

const RULES = [
  'FREQ=DAILY',
  'FREQ=DAILY;INTERVAL=2;COUNT=5',
  'FREQ=DAILY;INTERVAL=3',
  'FREQ=DAILY;UNTIL=20261031',
  'FREQ=DAILY;UNTIL=20261031T035900Z',
  'FREQ=WEEKLY',
  'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR',
  'FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH',
  'FREQ=WEEKLY;BYDAY=SU,SA;COUNT=6',
  'RRULE:FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=1,-1;COUNT=6',
  'FREQ=MONTHLY',
  'FREQ=MONTHLY;BYMONTHDAY=31',
  'FREQ=MONTHLY;BYMONTHDAY=-1',
  'FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13',
  'FREQ=YEARLY',
  'FREQ=YEARLY;BYMONTHDAY=15',
  'FREQ=YEARLY;INTERVAL=4',
  'FREQ=HOURLY',
  'nonsense',
  '',
  'FREQ=WEEKLY;BYMONTHDAY=3',
  'FREQ=DAILY;COUNT=2;UNTIL=20261231',
  'freq=daily;interval=2',
  ' RRULE:FREQ=DAILY ',
];

function dayOffset(r: Rng, tz: string, now: number, from: number, to: number): string {
  return addDays(dayKey(now, tz), r.int(from, to));
}

/** A due instant near `now`: usually on a whole minute, often at a time a student would give. */
function dueNear(r: Rng, tz: string, now: number): number {
  const day = dayOffset(r, tz, now, -4, 20);
  return atMinute(day, r.pick([0, 9 * 60, 12 * 60, 16 * 60, 23 * 60 + 59, r.int(0, 1439)]), tz);
}

type TaskJson = Record<string, unknown> & { id: string };

function genTask(r: Rng, i: number, now: number, tz: string, ids: string[], recurring = 0.12): TaskJson {
  const done = r.chance(0.22);
  const t: TaskJson = {
    id: `t${i}`,
    spaceId: r.pick(SPACE_IDS),
    title: r.pick(['Grammar quiz 4', 'Problem set', 'Lab report', 'Mix the second verse', `Task ${i}`, 'Read chapter 3', 'Écrire']),
    type: r.pick(TYPES),
  };
  if (r.chance(0.2)) t.courseId = r.pick(['jpn201', 'mth142', 'his101', 'zzz999']);
  if (r.chance(0.08)) t.projectId = r.pick(['p1', 'p2']);
  if (r.chance(0.12)) t.milestoneId = r.pick(['m1', 'm2', 'm9']);
  if (r.chance(0.12)) t.group = r.pick(['Car', 'Apartment', 'apartment', 'Écoute', 'Health', 'Éclair']);
  if (r.chance(0.18) && ids.length > 0) t.parentTaskId = r.chance(0.05) ? t.id : r.pick(ids);
  t.due = r.chance(0.22) ? null : r.chance(0.04) ? now + r.int(-3000, 3000) : dueNear(r, tz, now);
  if (r.chance(0.1)) t.scheduledDate = dayOffset(r, tz, now, -5, 12);
  if (r.chance(recurring)) t.rrule = r.pick(RULES);
  t.difficulty = r.weighted<number>([[12, r.int(1, 5)], [1, r.pick([0, 6, 2.5, 9, -1])]]);
  t.estMin = r.weighted<number | null>([[5, null], [6, r.pick([5, 10, 15, 30, 45, 60, 90, 120, 130, 240])], [1, 0], [1, r.pick([7.5, 44.4, 100.5])]]);
  t.adjustMin = r.weighted<number>([[8, 0], [3, r.int(5, 120)], [1, r.pick([-5, 0.5, 33.3])]]);
  t.notes = '';
  if (r.chance(0.04)) t.link = { kind: r.pick(['session', 'system', 'work']), id: 'x1' };
  t.done = done;
  t.doneAt = done ? (r.chance(0.85) ? now - r.int(0, 20 * 86_400_000) : null) : null;
  t.source = r.pick(SOURCES);
  return t;
}

function genTasks(r: Rng, n: number, now: number, tz: string, recurring = 0.12): TaskJson[] {
  const ids = Array.from({ length: n }, (_, i) => `t${i}`);
  return Array.from({ length: n }, (_, i) => genTask(r, i, now, tz, ids, recurring));
}

function genSession(r: Rng, i: number, now: number, taskIds: string[], habitIds: string[] = []) {
  const endedAt = now - r.int(0, 16 * 86_400_000);
  const focusMin = r.pick([5, 10, 25, 25, 50, r.int(1, 90)]);
  const s: Record<string, unknown> = { id: `s${i}` };
  const roll = r.next();
  if (roll < 0.8 && taskIds.length > 0) s.taskId = r.pick(taskIds);
  else if (roll < 0.95 && habitIds.length > 0) s.habitId = r.pick(habitIds);
  s.startedAt = endedAt - focusMin * 60_000;
  s.endedAt = endedAt;
  s.focusMin = focusMin;
  s.interruptions = r.int(0, 3);
  s.room = 'heat';
  return s;
}

function genSessions(r: Rng, tasks: { id: string }[], now: number, max = 8, habitIds: string[] = []) {
  return Array.from({ length: r.int(0, max) }, (_, i) =>
    genSession(r, i, now, tasks.map((t) => t.id), habitIds),
  );
}

function genOccurrences(r: Rng, tasks: TaskJson[], now: number, tz: string) {
  const recurring = tasks.filter((t) => t.rrule);
  return Array.from({ length: r.chance(0.6) && recurring.length > 0 ? r.int(0, 4) : 0 }, (_, i) => ({
    id: `o${i}`,
    taskId: r.pick(recurring).id,
    date: dayOffset(r, tz, now, -6, 10),
    doneAt: now - r.int(0, 5 * 86_400_000),
  }));
}

function genSpace(r: Rng, i: number) {
  const kind = r.pick(['course', 'milestone', 'free'] as const);
  return {
    id: SPACE_IDS[i % 3],
    name: ['Classes', 'WWAV', 'Personal'][i % 3],
    hue: r.pick([211, 6, 145]),
    groupKind: kind,
    groupLabel: { course: 'Course', milestone: 'Milestone', free: 'Area' }[kind],
    types: r.sample(TYPES, r.int(2, 5)),
    persona: '',
  };
}

function genHabit(r: Rng, i: number, today: string) {
  const log: Record<string, true> = {};
  const run = r.int(0, 20);
  for (let d = 0; d < run; d++) if (r.chance(0.85)) log[addDays(today, -d - (r.chance(0.5) ? 0 : 1))] = true;
  if (r.chance(0.2)) log[addDays(today, -r.int(30, 500))] = true;
  const h: Record<string, unknown> = { id: `h${i}`, title: r.pick(['Practise kanji', 'Stretch', 'Read', `Habit ${i}`]) };
  if (r.chance(0.5)) h.minutes = r.pick([10, 15, 20, 25, 30, 7.5]);
  h.log = log;
  h.showCounter = r.chance(0.3);
  return h;
}

function genBlock(r: Rng, i: number, tasks: { id: string }[], habits: { id?: unknown }[], today: string, tz: string, now: number) {
  const b: Record<string, unknown> = { id: `b${i}` };
  if (r.chance(0.8) && tasks.length > 0) b.taskId = r.pick(tasks).id;
  else if (habits.length > 0) b.habitId = r.pick(habits).id;
  else if (tasks.length > 0) b.taskId = r.pick(tasks).id;
  b.date = r.chance(0.7) ? today : dayOffset(r, tz, now, -2, 8);
  b.start = r.pick([7 * 60, 9 * 60, 9 * 60 + 45, 13 * 60, 14 * 60, 20 * 60, r.int(7 * 60, 23 * 60 + 30)]);
  b.minutes = r.pick([15, 30, 45, 60, 90, 120, r.int(5, 180)]);
  b.origin = r.pick(['you', 'plan']);
  return b;
}

function genEvent(r: Rng, i: number, tz: string, now: number) {
  const day = dayOffset(r, tz, now, -2, 3);
  const allDay = r.chance(0.15);
  const start = allDay ? atMinute(day, 0, tz) : atMinute(day, r.pick([0, 8 * 60, 9 * 60, 22 * 60, r.int(0, 1439)]), tz);
  const end = allDay ? atMinute(addDays(day, r.int(1, 2)), 0, tz) : start + r.pick([30, 60, 75, 120, 600, 1500]) * 60_000;
  return { id: `e${i}`, title: r.pick(['Class', 'Flight', 'Fall break', 'Gym']), start, end, allDay };
}

function genMilestone(r: Rng, i: number, today: string) {
  const m: Record<string, unknown> = { id: `m${i}`, spaceId: r.pick(SPACE_IDS) };
  if (r.chance(0.15)) m.projectId = 'p1';
  m.title = r.pick(['Enclosure v2', 'Firmware 1.0', 'Schematic']);
  m.date = addDays(today, r.int(-14, 14));
  m.done = r.chance(0.5);
  m.order = r.int(0, 5);
  return m;
}

// --- js: the JavaScript the model leans on --------------------------------------

const SPACES = [' ', ' ', '　', '﻿', '\u0085', '​', ' ', '\t', '\n', ' ', '᠎'];

function oddString(r: Rng): string {
  const alphabet = [
    'a', 'b', 'Z', 'k', '0', '9', ' ', '-', '_', '.', ',', '+', '😀', '𐍈', 'é', 'É', 'ß', 'ø', 'ñ', 'İ', 'ı', 'Σ', 'ς', 'я', '日', ' ', '﻿', '\u0085', '́', 'ǆ', 'ﬃ', 'á',
  ];
  return Array.from({ length: r.int(0, 8) }, () => r.pick(alphabet)).join('');
}

function numberLike(r: Rng): number {
  return r.weighted<() => number>([
    [3, () => r.int(-100, 100)],
    [3, () => r.int(-100, 100) + 0.5],
    [2, () => r.int(-1000, 1000) / 10],
    [2, () => r.float(-1e6, 1e6)],
    [1, () => r.pick([0.49999999999999994, -0.49999999999999994, 2.5, -2.5, 0.5, -0.5, 1.5, -1.5, 4503599627370495.5, 1e21, 1e-7, 123456789.123456789])],
    [1, () => r.int(-5, 5) * 10 ** r.int(-9, 25)],
  ])();
}

function isoLike(r: Rng): string {
  const pad = (n: number, w = 2) => String(n).padStart(w, '0');
  const year = r.pick([2026, 2026, 2027, 1999, 2000, 0, 1, 99, 100, 9999, 1970]);
  const month = r.pick([1, 2, 6, 12, 0, 13, r.int(1, 12)]);
  const day = r.pick([1, 15, 28, 29, 30, 31, 0, 32, r.int(1, 28)]);
  const hour = r.pick([0, 8, 12, 23, 24, 25, r.int(0, 23)]);
  const minute = r.pick([0, 30, 59, 60, r.int(0, 59)]);
  const second = r.pick([0, 59, 60, r.int(0, 59)]);
  let s = `${pad(year, 4)}-${pad(month)}-${pad(day)}T${pad(hour)}:${pad(minute)}`;
  if (r.chance(0.7)) {
    s += `:${pad(second)}`;
    if (r.chance(0.5)) s += `.${r.pick(['0', '000', '5', '123', '1234', '0001', '9999', '000000', '0000001', '00', ''])}`;
  }
  const zoneSuffix = r.pick(['Z', 'Z', '+00:00', '-04:00', '+05:30', '-23:59', '+23:59', '+24:00', '-00:00', '+05:60', 'z', '', '+0530']);
  s += zoneSuffix;
  if (r.chance(0.05)) s = s.replace('T', ' ');
  return s;
}

// The shape the export's checker reads an instant by (importArtifact.ts).
const ISO = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2}(\.\d+)?)?(Z|[+-]\d{2}:\d{2})$/;

function jsVectors(): void {
  add('js', 'round', 700, (r) => ({ x: numberLike(r) }), ({ x }) => Math.round(x));
  add('js', 'max2', 300, (r) => ({ a: numberLike(r), b: r.chance(0.2) ? 0 : numberLike(r) }), ({ a, b }) => Math.max(a, b));
  add('js', 'min2', 300, (r) => ({ a: numberLike(r), b: r.chance(0.2) ? 0 : numberLike(r) }), ({ a, b }) => Math.min(a, b));
  add('js', 'numToString', 900, (r) => ({ x: numberLike(r) }), ({ x }) => String(x));
  add(
    'js',
    'toNumber',
    700,
    (r) => ({
      s: r.weighted<() => string>([
        [4, () => String(numberLike(r))],
        [2, () => `${r.pick(SPACES)}${r.int(0, 99)}${r.pick(SPACES)}`],
        [2, () => r.pick(['', ' ', '0x1f', '0X1F', '0b101', '0o17', '0x', '0xg', '1e3', '1E-3', '.5', '5.', '.', '-.5e1', '+7', '-0', 'Infinity', '-Infinity', '+Infinity', 'infinity', 'NaN', 'inf', '1_000', '١٢', '1 2', '12px', '--1', '1e', '1e+', '0.1.2', '٣'])],
        [2, () => oddString(r)],
        [1, () => `${r.int(0, 9999)}-${r.int(0, 99)}`],
      ])(),
    }),
    ({ s }) => Number(s),
  );
  add('js', 'trim', 400, (r) => ({ s: `${r.pick(SPACES)}${oddString(r)}${r.pick(SPACES)}${r.chance(0.3) ? r.pick(SPACES) : ''}` }), ({ s }) => s.trim());
  add(
    'js',
    'normaliseName',
    400,
    (r) => ({ s: `${r.pick(['', ' ', ' '])}${oddString(r)}${r.pick(SPACES)}${r.pick(SPACES)}${oddString(r)}${r.pick(['', ' ', '﻿'])}` }),
    ({ s }) => s.trim().replace(/\s+/g, ' ').toLowerCase(),
  );
  add(
    'js',
    'cmp',
    600,
    (r) => ({ a: oddString(r), b: r.chance(0.3) ? oddString(r) : oddString(r) }),
    ({ a, b }) => (a < b ? -1 : a > b ? 1 : 0),
  );
  add('js', 'utf16Len', 300, (r) => ({ s: oddString(r) }), ({ s }) => s.length);
  add(
    'js',
    'dateUtc',
    700,
    (r) => ({
      args: [
        r.pick([2026, 2026, 1999, 0, 1, 50, 99, 100, -1, 275760, 400000, r.int(1900, 2100)]),
        r.pick([0, 1, 11, 12, 13, -1, -13, 99, r.int(0, 11)]),
        r.pick([1, 0, 31, 32, -5, 100, r.int(1, 28)]),
        r.pick([0, 12, 24, 25, -1, r.int(0, 23)]),
        r.pick([0, 59, 60, 61, -1, r.int(0, 59)]),
        r.pick([0, 59, 60, -1, r.int(0, 59)]),
      ],
    }),
    ({ args }) => Date.UTC(args[0], args[1], args[2], args[3], args[4], args[5]),
  );
  add(
    'js',
    'parseIsoInstant',
    2500,
    (r) => ({ s: isoLike(r) }),
    ({ s }) => (ISO.test(s) && Number.isFinite(Date.parse(s)) ? Date.parse(s) : null),
  );
  // localeCompare: ASCII, where the Rust says it is exact, and the Latin letters it knows.
  const asciiChars = [...'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 _-,;:!?.\'"()[]{}@*/\\&#%`^+<=>|~$\t'];
  const latin = [...'éÉèêëáàâäãåçñóòôöõúùûüýÿíìîïōßøæœ'];
  add(
    'js',
    'localeCompare',
    2500,
    (r) => {
      const word = () =>
        Array.from({ length: r.int(0, 6) }, () => (r.chance(0.12) ? r.pick(latin) : r.pick(asciiChars))).join('');
      const a = word();
      // Often a near miss of the first, so the deeper levels of the comparison get used.
      const b = r.chance(0.5) ? a.replace(/./, (c) => (r.chance(0.5) ? c.toUpperCase() : c.toLowerCase())) : word();
      return { a, b };
    },
    ({ a, b }) => Math.sign(a.localeCompare(b)),
  );
}

// --- zone and format: shared/time ------------------------------------------------

function zoneVectors(): void {
  add('zone', 'wallTime', 900, (r) => {
    const tz = r.pick(ZONES);
    return { ms: instant(r, tz), tz };
  }, ({ ms, tz }) => wallTime(ms, tz));
  add('zone', 'epochOf', 1500, (r) => {
    const tz = r.pick(ZONES);
    const ts = TRANSITIONS.get(tz) ?? [];
    // Mostly wall clocks within a couple of hours of a change, where gaps and repeats are.
    if (r.chance(0.5) && ts.length > 0) {
      const w = wallTime(r.pick(ts) + r.int(-4 * 3600, 4 * 3600) * 1000, tz);
      const out: Record<string, number> = { year: w.year, month: w.month, day: w.day, hour: w.hour, minute: w.minute };
      if (r.chance(0.5)) out.second = r.pick([0, 30, 59]);
      return { w: out, tz };
    }
    const w = wallTime(instant(r, tz), tz);
    return {
      w: {
        year: w.year,
        month: r.chance(0.05) ? r.int(0, 14) : w.month,
        day: r.chance(0.05) ? r.int(0, 33) : w.day,
        hour: r.chance(0.05) ? r.int(0, 26) : w.hour,
        minute: r.chance(0.05) ? r.int(0, 75) : w.minute,
      },
      tz,
    };
  }, ({ w, tz }) => epochOf(w as { year: number; month: number; day: number; hour: number; minute: number; second?: number }, tz));
  add('zone', 'keyOf', 300, (r) => ({ year: r.int(0, 3000), month: r.int(0, 14), day: r.int(0, 40) }), ({ year, month, day }) => keyOf(year, month, day));
  add('zone', 'keyParts', 400, (r) => ({
    key: r.weighted<() => string>([
      [5, () => keyOf(r.int(1990, 2060), r.int(1, 12), r.int(1, 31))],
      // Three parts at least: a key short of one leaves `undefined`, which JSON drops, where the Rust has a NaN.
      [1, () => r.pick(['x-y-z', '2026-10-06-extra', '-1-2-3', '2026-1e1-06', ' 2026 - 10 - 06 ', '0x7ea-10-06', '2026--06', '--', '2026-10-'])],
    ])(),
  }), ({ key }) => keyParts(key));
  add('zone', 'dayKey', 700, (r) => {
    const tz = r.pick(ZONES);
    return { ms: instant(r, tz), tz };
  }, ({ ms, tz }) => dayKey(ms, tz));
  add('zone', 'minuteOfDay', 600, (r) => {
    const tz = r.pick(ZONES);
    return { ms: instant(r, tz), tz };
  }, ({ ms, tz }) => minuteOfDay(ms, tz));
  const anyDay = (r: Rng) => keyOf(r.int(1995, 2070), r.int(1, 12), r.int(1, 31));
  add('zone', 'addDays', 800, (r) => ({ key: anyDay(r), n: r.pick([0, 1, -1, 7, 30, -30, 365, 366, -400, 36525, r.int(-5000, 5000)]) }), ({ key, n }) => addDays(key, n));
  add('zone', 'daysBetween', 600, (r) => ({ a: anyDay(r), b: anyDay(r) }), ({ a, b }) => daysBetween(a, b));
  add('zone', 'weekdayOf', 500, (r) => ({ key: anyDay(r) }), ({ key }) => weekdayOf(key));
  add('zone', 'daysInMonth', 500, (r) => ({ year: r.pick([1900, 2000, 2024, 2026, 2100, r.int(1900, 2200)]), month: r.int(1, 12) }), ({ year, month }) => daysInMonth(year, month));
  add('zone', 'atMinute', 1200, (r) => {
    const tz = r.pick(ZONES);
    const key = dayKey(instant(r, tz), tz);
    return { key, minutes: r.pick([0, 1, 59, 60, 119, 120, 150, 179, 180, 1439, 1440, 1441, 2880, -1, -60, r.int(0, 1440), r.int(-3000, 3000)]), tz };
  }, ({ key, minutes, tz }) => atMinute(key, minutes, tz));
  add('zone', 'startOfDay', 800, (r) => {
    const tz = r.pick(ZONES);
    return { key: dayKey(instant(r, tz), tz), tz };
  }, ({ key, tz }) => startOfDay(key, tz));
  add('zone', 'zoneStarts', 120, (r) => {
    // Midnight of the days around each change: where "start of day" is not 00:00 plus 24 hours.
    const tz = r.pick(ZONES);
    const ts = TRANSITIONS.get(tz) ?? [];
    const t = ts.length > 0 ? r.pick(ts) : instant(r, tz);
    return { key: addDays(dayKey(t, tz), r.int(-2, 2)), tz };
  }, ({ key, tz }) => ({ start: startOfDay(key, tz), next: startOfDay(addDays(key, 1), tz) }));
}

function formatVectors(): void {
  add('format', 'clock', 700, (r) => ({ minutes: r.pick([0, 59, 60, 719, 720, 779, 780, 1439, 1440, -1, -60, 1500, 0.5, 59.5, 719.5, 1439.5, -0.5, r.int(-3000, 3000), r.float(-3000, 3000)]) }), ({ minutes }) => clock(minutes));
  add('format', 'clockAt', 500, (r) => {
    const tz = r.pick(ZONES);
    return { ms: instant(r, tz), tz };
  }, ({ ms, tz }) => clockAt(ms, tz));
  const anyDay = (r: Rng) => keyOf(r.int(1995, 2070), r.int(1, 12), r.int(1, 31));
  add('format', 'monthDay', 400, (r) => ({ key: anyDay(r) }), ({ key }) => monthDay(key));
  add('format', 'shortMonthDay', 400, (r) => ({ key: anyDay(r) }), ({ key }) => shortMonthDay(key));
  add('format', 'countdown', 600, (r) => ({ ms: r.pick([0, 1, 999, 1000, 1001, 59_999, 60_000, 1_500_000, 24 * 60_000 + 59_000, -5, -1000, r.int(-100_000, 6_000_000), r.float(0, 3_600_000)]) }), ({ ms }) => countdown(ms));
}

// --- heat ---------------------------------------------------------------------------

/** What heat reads of a task, with the odd values a row might hold. */
function heatInput(r: Rng, now: number) {
  const roll = r.next();
  const due =
    roll < 0.12
      ? null
      : roll < 0.3
        ? now + r.int(-3 * 86_400_000, 3_600_000)
        : roll < 0.8
          ? now + r.float(-2, 14) * DAY_MS
          : now + r.pick([0, -1, 1, DAY_MS, -DAY_MS, 7 * DAY_MS, 11 * DAY_MS + 1]);
  return {
    done: r.chance(0.12),
    due: due === null ? null : r.chance(0.8) ? Math.round(due) : due,
    difficulty: r.weighted<number>([[10, r.int(1, 5)], [1, r.pick([0, -2, 6, 9, 2.5, 3.5, 0.5, 4.49999])]]),
  };
}

function heatVectors(): void {
  const NOW = Date.UTC(2026, 9, 6, 12, 40);
  add('heat', 'runway', 400, (r) => ({ difficulty: r.pick([1, 2, 3, 4, 5, 0, -3, 6, 9, 2.5, 3.5, 0.5, 4.5, 5.5, 1.49999, r.float(-5, 12)]) }), ({ difficulty }) => runway(difficulty));
  add('heat', 'heatThresholds', 400, (r, i) => ({ difficulty: i < 5 ? i + 1 : r.pick([0, 6, 2.5, 7, -1, r.float(-3, 8)]) }), ({ difficulty }) => heatThresholds(difficulty));
  add('heat', 'tubeFill', 300, (r) => ({ h: r.pick([{ level: 'Warm', v: 0.5 }, { level: 'Overdue', v: 1.1 }, { level: 'Cool', v: 0.05 }, { level: 'Done', v: null }, { level: 'Hot', v: r.float(0.7, 1) }, { level: 'Cool', v: r.float(0, 0.34) }]) }), ({ h }) => tubeFill(h as never));
  // Every difficulty against a sweep of days left, each side of the thresholds, and now to the millisecond.
  const sweep: { difficulty: number; days: number }[] = [];
  for (let difficulty = 1; difficulty <= 5; difficulty++) {
    for (let q = -12; q <= 60; q++) sweep.push({ difficulty, days: q / 4 });
    const t = heatThresholds(difficulty);
    for (const at of [t.warmAt, t.hotAt, 0]) for (const d of [-1, 0, 1]) sweep.push({ difficulty, days: at + d / DAY_MS });
  }
  add('heat', 'heatOf', sweep.length + 600, (r, i) => {
    if (i < sweep.length) return { t: { done: false, due: Math.round(NOW + sweep[i].days * DAY_MS), difficulty: sweep[i].difficulty }, now: NOW };
    const now = instant(r);
    return { t: heatInput(r, now), now };
  }, ({ t, now }) => heatOf(t, now));
  add('heat', 'byHeat', 500, (r) => {
    const now = instant(r);
    const n = r.int(0, 9);
    return { items: Array.from({ length: n }, (_, i) => ({ id: `t${i}`, ...heatInput(r, now) })), now };
  }, ({ items, now }) => byHeat(items, now).map((t) => t.id));
  add('heat', 'duePhrase', 1200, (r) => {
    const tz = r.pick(ZONES);
    const now = instant(r, tz);
    const due = r.weighted<() => number>([
      [3, () => now + r.int(-3 * 86_400_000, 14 * 86_400_000)],
      [2, () => now - r.int(0, 3_600_000)],
      [1, () => now + r.pick([0, 1, -1, 86_400_000, -86_400_000])],
      [2, () => wall(tz, 2026, 10, r.int(5, 25), r.pick([0, 9, 16, 23]), r.pick([0, 59, 30]))],
      [1, () => instant(r, tz)],
    ])();
    return { due, now, tz };
  }, ({ due, now, tz }) => duePhrase(due, now, tz));
  add('heat', 'nextHeatChange', 300, (r) => {
    const now = instant(r);
    const n = r.int(0, 4);
    // Whole milliseconds only: a due that isn't one sends the TypeScript's search round for ever.
    return { items: Array.from({ length: n }, () => ({ ...heatInput(r, now), due: r.chance(0.15) ? null : Math.round(now + r.float(-0.5, 12) * DAY_MS) })), now };
  }, ({ items, now }) => nextHeatChange(items, now));
}

// --- estimate -----------------------------------------------------------------------

function estimateVectors(): void {
  add('estimate', 'formatMinutes', 600, (r) => ({ minutes: r.pick([0, 1, 44.5, 44.6, 45, 59, 59.5, 60, 61, 75, 119.5, 120, 190, 1440, -5, -60, -61, -0.4, 0.4, 0.5, r.int(0, 700), r.float(-100, 700)]) }), ({ minutes }) => formatMinutes(minutes));
  const world = (r: Rng) => {
    const now = instant(r);
    const tasks = genTasks(r, r.int(0, 6), now, NY, 0);
    return { now, tasks, sessions: genSessions(r, tasks, now, 5) };
  };
  add('estimate', 'actualMin', 500, (r) => {
    const w = world(r);
    return { task: w.tasks.length > 0 ? r.pick(w.tasks) : genTask(r, 0, w.now, NY, []), sessions: w.sessions };
  }, ({ task, sessions }) => actualMin(task as never, sessions as never));
  add('estimate', 'estimateContext', 300, (r) => world(r), ({ tasks, sessions }) => {
    const ctx = estimateContext(tasks as never, sessions as never);
    const sorted = <T>(m: Map<string, T>) => Object.fromEntries([...m.entries()].sort(([a], [b]) => (a < b ? -1 : 1)));
    return {
      averages: sorted(ctx.averages),
      children: Object.fromEntries([...ctx.children.entries()].map(([k, v]) => [k, v.map((t) => t.id)]).sort(([a], [b]) => ((a as string) < (b as string) ? -1 : 1))),
    };
  });
  add('estimate', 'estimateMin', 500, (r) => {
    const w = world(r);
    if (w.tasks.length === 0) return null;
    return { ...w, taskId: r.pick(w.tasks).id };
  }, ({ tasks, sessions, taskId }) => {
    const ctx = estimateContext(tasks as never, sessions as never);
    return estimateMin(tasks.find((t) => t.id === taskId) as never, ctx);
  });
  add('estimate', 'averageLines', 300, (r) => {
    const w = world(r);
    // Done tasks with time, so the averages exist.
    const tasks = w.tasks.map((t) => (r.chance(0.5) ? { ...t, done: true, adjustMin: r.int(10, 120) } : t));
    return { space: genSpace(r, r.int(0, 2)), tasks, sessions: w.sessions };
  }, ({ space, tasks, sessions }) => averageLines(space as never, estimateContext(tasks as never, sessions as never)));
  add('estimate', 'weeklyLoad', 500, (r) => world(r), ({ tasks, sessions, now }) =>
    weeklyLoad(tasks as never, estimateContext(tasks as never, sessions as never), now));
  add('estimate', 'weeklyLoadLine', 300, (r) => ({ load: { minutes: r.pick([0, 1, 45, 200, 190, r.int(0, 1000), 44.6], ), count: r.pick([0, 1, 2, 5, r.int(0, 20)]) } }), ({ load }) => weeklyLoadLine(load));
}


// --- recurrence ---------------------------------------------------------------------------

/** A rule string put together from parts, some of them wrong. */
function ruleText(r: Rng): string {
  const parts: string[] = [];
  const freq = r.pick(['DAILY', 'WEEKLY', 'MONTHLY', 'YEARLY', 'daily', 'Weekly', 'HOURLY', 'SECONDLY', '', 'DAILY ']);
  if (r.chance(0.95)) parts.push(`FREQ=${freq}`);
  if (r.chance(0.35)) parts.push(`${r.pick(['INTERVAL', 'INTERVAL', 'interval', 'ınterval', 'İNTERVAL'])}=${r.pick(['1', '2', '3', '5', '12', '0', '-1', 'x', '', '007', '2.5', '99999', '1e2', '٣'])}`);
  if (r.chance(0.3)) parts.push(`BYDAY=${r.pick(['MO', 'TU,TH', 'MO,TU,WE,TH,FR', 'SA,SU', 'su,mo', 'MO,MO,TU', '2TU', 'XX', '', 'TU,'])}`);
  if (r.chance(0.25)) parts.push(`BYMONTHDAY=${r.pick(['1', '15', '31', '-1', '1,15', '1,-1', '0', '32', '-32', 'x', '', '5,', '07', '-0'])}`);
  if (r.chance(0.2)) parts.push(`COUNT=${r.pick(['1', '3', '6', '10', '0', '-2', 'x', '2.5', '012'])}`);
  if (r.chance(0.2)) {
    parts.push(`UNTIL=${r.pick(['20261031', '20261008T035900Z', '20271231T235959Z', '20260101', '2026103', '20261031T0359Z', '20261031T035900', 'x', '', '20261331', '00500101', '20261031t035900z', '00500101T035900Z', '19991231T235959Z', '20260229T000000Z'])}`);
  }
  if (r.chance(0.08)) parts.push(r.pick(['WKST=SU', 'BYSETPOS=1', 'BYMONTH=3', 'X-FOO=1', '', 'FREQ']));
  for (let i = parts.length - 1; i > 0; i--) {
    const j = r.int(0, i);
    [parts[i], parts[j]] = [parts[j], parts[i]];
  }
  let text = parts.join(r.chance(0.97) ? ';' : ';;');
  if (r.chance(0.1)) text = `RRULE:${text}`;
  if (r.chance(0.04)) text = `rrule:${text}`;
  if (r.chance(0.05)) text = `${r.pick([' ', '\u00a0', '\ufeff', '\u0085', '\t', '\u2003'])}${text}${r.pick([' ', '\u00a0', '\ufeff', '\u0085', '\n'])}`;
  if (r.chance(0.03)) text = `${text};`;
  return text;
}

/** A rule that parses and can't search for ever: no huge interval. */
function goodRule(r: Rng): string {
  for (;;) {
    const text = r.chance(0.5) ? r.pick(RULES) : ruleText(r);
    const rule = parseRule(text);
    if (rule && rule.interval <= 60) return text;
  }
}

const seriesZone = (r: Rng) => r.weighted<string>([[6, NY], [1, 'Australia/Lord_Howe'], [1, 'Europe/London'], [1, 'Asia/Kolkata'], [1, 'UTC']]);

function recurrenceVectors(): void {
  add('recurrence', 'parseRule', 2500, (r) => ({ text: r.chance(0.35) ? r.pick(RULES) : ruleText(r) }), ({ text }) => parseRule(text));
  add('recurrence', 'occurrencesBetween', 500, (r) => {
    const tz = seriesZone(r);
    const day = addDays(dayKey(instant(r, tz), tz), r.pick([0, 0, -400, -3000, 1, 17]));
    const from = addDays(day, r.pick([-30, -1, 0, 0, 5, 40, 400, 3000]));
    return {
      rule: goodRule(r),
      start: { day, minute: r.pick([0, 9 * 60, 23 * 60 + 59, 2 * 60 + 30, 1 * 60 + 30, r.int(0, 1439)]) },
      tz,
      from,
      to: addDays(from, r.pick([0, 1, 7, 35, 90, 400])),
    };
  }, ({ rule, start, tz, from, to }) => occurrencesBetween(parseRule(rule)!, start, tz, from, to));
  const withRule = (r: Rng, now: number, tz: string) => {
    const t = genTask(r, 0, now, tz, [], 0.7);
    if (t.rrule && r.chance(0.2)) t.due = null;
    return t;
  };
  add('recurrence', 'seriesStart', 400, (r) => {
    const tz = seriesZone(r);
    const now = instant(r, tz);
    return { task: withRule(r, now, tz), tz };
  }, ({ task, tz }) => seriesStart(task as never, tz));
  add('recurrence', 'taskOccurrences', 500, (r) => {
    const tz = seriesZone(r);
    const now = instant(r, tz);
    const from = dayOffset(r, tz, now, -10, 10);
    return { task: withRule(r, now, tz), tz, from, to: addDays(from, r.pick([0, 6, 30, 90])) };
  }, ({ task, tz, from, to }) => taskOccurrences(task as never, tz, from, to));
  const withTicks = (r: Rng) => {
    const tz = seriesZone(r);
    const now = instant(r, tz);
    const task = withRule(r, now, tz);
    const occurrences = r.chance(0.6)
      ? Array.from({ length: r.int(1, 5) }, (_, i) => ({ id: `o${i}`, taskId: r.chance(0.9) ? task.id : 'other', date: dayOffset(r, tz, now, -3, 14), doneAt: now - r.int(0, 3 * 86_400_000) }))
      : [];
    return { task, occurrences, now, tz };
  };
  add('recurrence', 'nextOpenOccurrence', 700, withTicks, ({ task, occurrences, now, tz }) => nextOpenOccurrence(task as never, occurrences, now, tz));
  add('recurrence', 'recurs', 400, (r) => ({ task: withRule(r, instant(r), NY) }), ({ task }) => recurs(task as never));
  add('recurrence', 'setRepeat', 400, (r) => {
    const tz = seriesZone(r);
    const now = instant(r, tz);
    return { task: withRule(r, now, tz), rule: r.chance(0.2) ? null : goodRule(r), now, tz };
  }, ({ task, rule, now, tz }) => setRepeat(task as never, rule, now, tz));
  add('recurrence', 'withEffectiveDue', 600, withTicks, ({ task, occurrences, now, tz }) => withEffectiveDue(task as never, occurrences, now, tz));
  add('recurrence', 'checkOccurrence', 500, (r) => ({ ...withTicks(r), idPrefix: r.pick(['occ', 'id']) }), ({ task, occurrences, now, tz, idPrefix }) => {
    let n = 0;
    return checkOccurrence(task as never, occurrences, now, tz, () => `${idPrefix}-${++n}`);
  });
  add('recurrence', 'reopenOccurrence', 300, (r) => {
    const w = withTicks(r);
    const date = w.occurrences.length > 0 ? r.pick(w.occurrences).date : '2026-10-06';
    return { occurrences: w.occurrences, taskId: r.chance(0.8) ? w.task.id : 'other', date };
  }, ({ occurrences, taskId, date }) => reopenOccurrence(occurrences, taskId, date));
  add('recurrence', 'openTasks', 400, (r) => {
    const tz = seriesZone(r);
    const now = instant(r, tz);
    const tasks = genTasks(r, r.int(0, 6), now, tz, 0.4);
    return { tasks, occurrences: genOccurrences(r, tasks, now, tz), now, tz };
  }, ({ tasks, occurrences, now, tz }) => openTasks(tasks as never, occurrences, now, tz));
}

// --- spaces -------------------------------------------------------------------------------

function genCapture(r: Rng, i: number, now: number) {
  const c: Record<string, unknown> = { id: `c${i}`, text: r.pick(['fix the snare at 1:32', 'a thought', 'call the bank']) };
  if (r.chance(0.1)) c.link = { kind: 'work', id: 'w1' };
  if (r.chance(0.4)) c.triagedAt = now - r.int(0, 86_400_000);
  if (r.chance(0.2)) c.resultType = r.pick(['task', 'note', 'project', 'upload']);
  if (r.chance(0.2)) c.resultId = 'r1';
  return c;
}

function genCourse(r: Rng, id: string, code: string) {
  const c: Record<string, unknown> = {
    id,
    termId: 'fall26',
    code,
    name: r.pick(['Japanese', 'Calculus', 'History']),
    categories: genCategories(r),
  };
  if (r.chance(0.3)) c.scale = r.pick([[{ letter: 'A', min: 90 }, { letter: 'B', min: 80 }, { letter: 'C', min: 70 }, { letter: 'F', min: 0 }], [{ letter: 'A', min: 90 }, { letter: 'B', min: 80 }, { letter: 'D', min: 60 }], [{ letter: 'P', min: 50 }, { letter: 'F', min: 0 }]]);
  c.notes = '';
  return c;
}

function genCategories(r: Rng) {
  const n = r.int(0, 5);
  const weights = r.pick([[25, 20, 20, 35], [40, 40, 20], [50, 50], [60, 40], [33.3, 33.3, 33.4], [10, 20, 30, 40, 10], [100], [20, 80], [34, 66]]);
  return Array.from({ length: Math.min(n, weights.length) || r.int(0, 1) }, (_, i) => ({
    id: `k${i}`,
    name: r.pick(['Homework', 'Quizzes', 'Exit tickets', 'Labs', 'Final']) + i,
    weight: r.chance(0.9) ? weights[i] : r.pick([0, 12.5, 7.7]),
    keywords: r.sample(['quiz', 'Lab', 'lab 5a', 'exit ticket', 'kanji', 'homework', 'Edfinity', ' ', '', 'é'], r.int(0, 3)),
  }));
}

function spacesVectors(): void {
  add('spaces', 'defaultSpaces', 20, (r) => ({ idPrefix: r.pick(['space', 'id', 's']) }), ({ idPrefix }) => {
    let n = 0;
    return defaultSpaces(() => `${idPrefix}-${++n}`);
  });
  add('spaces', 'inSpace', 300, (r) => {
    const now = instant(r);
    return { spaceId: r.pick([null, '', 'classes', 'wwav', 'zzz']), tasks: genTasks(r, r.int(0, 6), now, NY, 0) };
  }, ({ spaceId, tasks }) => (tasks as never as Task[]).filter(inSpace(spaceId)).map((t) => t.id));
  const sidebar = (r: Rng, now: number, tz: string) => {
    const tasks = genTasks(r, r.int(0, 6), now, tz, 0.25);
    return {
      tasks,
      occurrences: genOccurrences(r, tasks, now, tz),
      captures: Array.from({ length: r.int(0, 3) }, (_, i) => genCapture(r, i, now)),
      projects: Array.from({ length: r.int(0, 3) }, (_, i) => ({ id: `p${i + 1}`, spaceId: r.pick(SPACE_IDS), title: `Project ${i}`, status: r.pick(['active', 'on_hold', 'someday', 'archived']) })),
      milestones: Array.from({ length: r.int(0, 4) }, (_, i) => genMilestone(r, i + 1, dayKey(now, tz))),
      courses: [genCourse(r, 'jpn201', 'JPN 201'), genCourse(r, 'mth142', 'MTH 142'), genCourse(r, 'his101', 'HIS 101'), genCourse(r, 'zzz999', 'jpn  101')].slice(0, r.int(0, 4)),
    };
  };
  add('spaces', 'spaceCounts', 250, (r) => {
    const now = instant(r);
    const d = sidebar(r, now, NY);
    return { spaces: [0, 1, 2].map((i) => genSpace(r, i)), tasks: d.tasks, occurrences: d.occurrences, now, tz: NY };
  }, ({ spaces, tasks, occurrences, now, tz }) => spaceCounts(spaces as never, { tasks: tasks as never, occurrences }, now, tz));
  add('spaces', 'libraryLists', 250, (r) => {
    const now = instant(r);
    return { data: sidebar(r, now, NY), now, tz: NY, spaceId: r.pick([null, null, 'classes', 'wwav', 'personal']) };
  }, ({ data, now, tz, spaceId }) => libraryLists(data as never, now, tz, spaceId));
  add('spaces', 'groupName', 400, (r) => {
    const now = instant(r);
    const d = sidebar(r, now, NY);
    return { task: d.tasks.length > 0 ? r.pick(d.tasks) : genTask(r, 0, now, NY, []), courses: d.courses, milestones: d.milestones };
  }, ({ task, courses, milestones }) => groupName(task as never, { courses: courses as never, milestones: milestones as never }));
  add('spaces', 'groupCounts', 250, (r) => {
    const now = instant(r);
    const space = genSpace(r, r.int(0, 2));
    return { space, data: sidebar(r, now, NY), now, tz: NY };
  }, ({ space, data, now, tz }) => groupCounts(space as never, data as never, now, tz));
}

// --- grades -------------------------------------------------------------------------------------

function genGrades(r: Rng, categories: { id: string }[], n: number, courseIds: string[] = ['jpn201']) {
  return Array.from({ length: n }, (_, i) => {
    const outOf = r.weighted<number>([[8, 100], [3, r.pick([10, 20, 25, 3, 7])], [1, 0]]);
    const g: Record<string, unknown> = {
      id: `g${i}`,
      courseId: r.chance(0.9) ? courseIds[0] : r.pick(courseIds),
      categoryId: categories.length > 0 && r.chance(0.92) ? r.pick(categories).id : null,
      title: r.pick(['Exit Ticket 12', 'Kanji quiz 6', 'Lab 5a write-up', 'Midterm', 'Labor day essay', 'EDFINITY 4.2']),
      score: r.chance(0.1) ? null : r.weighted<number>([[6, r.int(0, outOf || 10)], [3, Math.round(r.float(0, outOf || 10) * 10) / 10], [1, r.float(0, 100)]]),
      outOf,
      dropped: r.chance(0.08),
      pending: r.chance(0.08),
    };
    if (r.chance(0.1)) g.link = 'https://brightspace.uri.edu/d2l/home';
    g.source = r.pick(['you', 'mail', 'valence']);
    return g;
  });
}

function gradeWorld(r: Rng) {
  const course = genCourse(r, 'jpn201', 'JPN 201') as { categories: { id: string }[] };
  return { course, grades: genGrades(r, course.categories, r.int(0, 6)) };
}

function gradesVectors(): void {
  add('grades', 'categoryPct', 500, (r) => {
    const w = gradeWorld(r);
    return { categoryId: w.course.categories.length > 0 && r.chance(0.9) ? r.pick(w.course.categories).id : 'zzz', grades: w.grades };
  }, ({ categoryId, grades }) => categoryPct(categoryId, grades as never));
  add('grades', 'currentPct', 500, gradeWorld, ({ course, grades }) => currentPct(course as never, grades as never));
  add('grades', 'decidedPct', 400, gradeWorld, ({ course, grades }) => decidedPct(course as never, grades as never));
  const thresholds = [93, 90, 87, 83, 80, 77, 73, 70, 67, 60, 0];
  add('grades', 'letterFor', thresholds.length * 6 + 500, (r, i) => {
    const scale = r.chance(0.35) ? r.pick([[{ letter: 'A', min: 90 }, { letter: 'B', min: 80 }, { letter: 'C', min: 70 }, { letter: 'F', min: 0 }], [{ letter: 'A', min: 90 }, { letter: 'D', min: 60 }], []]) : undefined;
    if (i < thresholds.length * 6) {
      const at = thresholds[Math.floor(i / 6)];
      return { pct: at + [-1e-9, -0.01, 0, 0.01, 1e-9, 0.5][i % 6], ...(scale ? { scale } : {}) };
    }
    return { pct: r.pick([r.float(-10, 110), r.int(0, 100), r.int(0, 1000) / 10]), ...(scale ? { scale } : {}) };
  }, ({ pct, scale }) => letterFor(pct, scale));
  add('grades', 'basedOnLine', 300, gradeWorld, ({ course, grades }) => basedOnLine(course as never, grades as never));
  add('grades', 'weightsLine', 500, (r) => ({ course: { ...(genCourse(r, 'jpn201', 'JPN 201') as object), categories: Array.from({ length: r.int(0, 5) }, (_, i) => ({ id: `k${i}`, name: `k${i}`, weight: r.pick([10, 20, 25, 33.3, 33.4, 12.5, 40, 65, 35, 0.1, r.float(0, 60)]), keywords: [] })) } }), ({ course }) => weightsLine(course as never));
  const letters = ['A', 'A-', 'B+', 'B', 'B-', 'C+', 'C', 'C-', 'D+', 'D', 'F', 'P', 'Z', ''];
  // Two-category courses over every weight and a spread of scores, as the findings test sweeps them.
  const sweep: { w: number; score: number; outOf: number; letter: string }[] = [];
  for (let w = 10; w < 100; w += 10) for (const [score, outOf] of [[0, 1], [2, 3], [5, 6], [50, 100], [93, 100], [67, 100]]) for (const letter of ['A', 'B-', 'C', 'D+', 'F']) sweep.push({ w, score, outOf, letter });
  add('grades', 'whatItWouldTake', sweep.length + 450, (r, i) => {
    if (i < sweep.length) {
      const { w, score, outOf, letter } = sweep[i];
      const course = { id: 'jpn201', termId: 't', code: 'JPN 201', name: 'x', categories: [{ id: 'done', name: 'done', weight: w, keywords: [] }, { id: 'left', name: 'left', weight: 100 - w, keywords: [] }], notes: '' };
      return { course, grades: [{ id: 'g', courseId: 'jpn201', categoryId: 'done', title: 't', score, outOf, dropped: false, pending: false, source: 'you' }], letter };
    }
    const w = gradeWorld(r);
    return { ...w, letter: r.pick(letters) };
  }, ({ course, grades, letter }) => whatItWouldTake(course as never, grades as never, letter));
  add('grades', 'guessCategory', 500, (r) => ({
    title: r.pick(['Exit Ticket 12', 'EDFINITY 4.2', 'Kanji quiz 7', 'Lab 3: Titration', 'Lab 5a write-up', 'Labor history essay', 'Midterm', 'lab', 'Lab5a', 'my-lab!', 'écoute lab é', '😀 lab 😀', '']),
    categories: genCategories(r),
  }), ({ title, categories }) => guessCategory(title, categories));
  add('grades', 'gradesWidget', 300, (r) => {
    const courses = [genCourse(r, 'jpn201', 'JPN 201'), genCourse(r, 'mth142', 'MTH 142'), genCourse(r, 'his101', 'HIS 101')].slice(0, r.int(0, 3));
    const cats = courses.flatMap((c) => (c as { categories: { id: string }[] }).categories);
    const grades = genGrades(r, cats, r.int(0, 7), ['jpn201', 'mth142', 'his101']);
    return { courses, grades };
  }, ({ courses, grades }) => gradesWidget(courses as never, grades as never));
}

// --- habits --------------------------------------------------------------------------------------

function habitsVectors(): void {
  add('habits', 'addHabit', 400, (r) => {
    const n = r.pick([0, 1, 3, 5, 6, 7]);
    return {
      habits: Array.from({ length: n }, (_, i) => ({ id: `h${i}`, title: `Habit ${i}`, log: {}, showCounter: false })),
      title: r.pick(['Practise kanji', '  ', '', ' Stretch ', ' x ', '﻿', '\u0085', 'a']),
      ...(r.chance(0.5) ? { minutes: r.pick([10, 20, 0, 7.5]) } : {}),
      idPrefix: r.pick(['h', 'habit']),
    };
  }, ({ habits, title, minutes, idPrefix }) => {
    let n = 0;
    return addHabit(habits as never, minutes === undefined ? { title } : { title, minutes }, () => `${idPrefix}-${++n}`);
  });
  const habitWorld = (r: Rng) => {
    const now = instant(r);
    const today = dayKey(now, NY);
    return { habit: genHabit(r, 0, today), today, now };
  };
  add('habits', 'habitLabel', 300, (r) => ({ habit: genHabit(r, 0, '2026-10-06') }), ({ habit }) => habitLabel(habit as never));
  add('habits', 'markHabitDone', 400, (r) => { const w = habitWorld(r); return { habit: w.habit, day: r.chance(0.5) ? w.today : addDays(w.today, r.int(-30, 3)) }; }, ({ habit, day }) => markHabitDone(habit as never, day));
  add('habits', 'toggleHabit', 400, (r) => { const w = habitWorld(r); return { habit: w.habit, day: r.chance(0.5) ? w.today : addDays(w.today, r.int(-30, 3)) }; }, ({ habit, day }) => toggleHabit(habit as never, day));
  add('habits', 'todayOrbs', 250, (r) => {
    const now = instant(r);
    const today = dayKey(now, NY);
    return { habits: Array.from({ length: r.int(0, 4) }, (_, i) => genHabit(r, i, today)), now, tz: NY };
  }, ({ habits, now, tz }) => todayOrbs(habits as never, now, tz));
  add('habits', 'lastFourteen', 300, (r) => { const w = habitWorld(r); return { habit: w.habit, today: w.today }; }, ({ habit, today }) => lastFourteen(habit as never, today));
  add('habits', 'yearGrid', 60, (r) => { const w = habitWorld(r); return { habit: w.habit, today: w.today }; }, ({ habit, today }) => yearGrid(habit as never, today));
  add('habits', 'doneRecord', 500, (r) => { const w = habitWorld(r); return { habit: w.habit, today: w.today }; }, ({ habit, today }) => doneRecord(habit as never, today));
  add('habits', 'streak', 500, (r) => { const w = habitWorld(r); return { habit: w.habit, today: w.today }; }, ({ habit, today }) => streak(habit as never, today));
  add('habits', 'habitLine', 500, (r) => { const w = habitWorld(r); return { habit: w.habit, today: w.today }; }, ({ habit, today }) => habitLine(habit as never, today));
}


// --- plan -----------------------------------------------------------------------------------

function planWorld(r: Rng, tz = seriesZone(r)) {
  const now = instant(r, tz);
  const today = dayKey(now, tz);
  // Often a fuller day: more tasks, most of them open.
  const full = r.chance(0.6);
  const tasks = genTasks(r, full ? r.int(2, 7) : r.int(0, 3), now, tz, 0.15).map((t) =>
    full && r.chance(0.7) ? { ...t, done: false, doneAt: null } : t,
  );
  const habits = Array.from({ length: r.int(0, 3) }, (_, i) => genHabit(r, i, today));
  const blocks = Array.from({ length: r.int(0, 4) }, (_, i) => {
    const b = genBlock(r, i, tasks, habits, today, tz, now);
    if (r.chance(0.1)) {
      delete b.taskId;
      delete b.habitId;
    }
    return b;
  });
  const events = Array.from({ length: r.int(0, 3) }, (_, i) => genEvent(r, i, tz, now));
  const data = {
    tasks,
    occurrences: genOccurrences(r, tasks, now, tz),
    blocks,
    events,
    sessions: genSessions(r, tasks, now, 4, habits.map((h) => h.id as string)),
    habits,
  };
  return { now, tz, data };
}

function blockTarget(r: Rng, w: ReturnType<typeof planWorld>) {
  const h = w.data.habits;
  if (h.length > 0 && r.chance(0.3)) return { habitId: r.chance(0.9) ? (r.pick(h).id as string) : 'nope' };
  return { taskId: w.data.tasks.length > 0 && r.chance(0.9) ? r.pick(w.data.tasks).id : 'nope' };
}

function planVectors(): void {
  add('plan', 'minutesToY', 300, (r) => ({ min: r.pick([420, 480, 1440, 0, -10, 9 * 60 + 30, r.float(0, 1500), r.int(0, 1500)]) }), ({ min }) => minutesToY(min));
  add('plan', 'yToMinutes', 300, (r) => ({ y: r.pick([0, 44, 66, 748, -50, r.float(-50, 800), r.int(0, 800)]) }), ({ y }) => yToMinutes(y));
  add('plan', 'snap', 500, (r) => ({ min: r.pick([487, 488, 480, 7.5, 7.4999, 22.5, -7.5, r.float(0, 1500), r.int(0, 1500), r.int(0, 3000) / 2]) }), ({ min }) => snap(min));
  add('plan', 'initialScrollTop', 300, (r) => ({ nowMin: r.int(0, 1440), viewportPx: r.pick([300, 600, 800, 1000, 748, r.int(100, 900)]) }), ({ nowMin, viewportPx }) => initialScrollTop(nowMin, viewportPx));
  add('plan', 'nowLineY', 300, (r) => ({ nowMin: r.pick([419, 420, 421, 1439, 1440, 0, 570, r.int(0, 1500)]) }), ({ nowMin }) => nowLineY(nowMin));
  add('plan', 'blockLength', 400, (r) => ({ estimate: r.pick([0, 1, 14, 15, 16, 29.5, 30, 45, 46, 90, 130, 135, -5, r.int(0, 300), r.float(0, 300)]) }), ({ estimate }) => blockLength(estimate));
  add('plan', 'planNext', 400, (r) => {
    const w = planWorld(r);
    return { target: blockTarget(r, w), data: w.data, now: w.now, tz: w.tz, idPrefix: 'blk' };
  }, ({ target, data, now, tz, idPrefix }) => {
    let n = 0;
    return planNext(target, data as unknown as PlanData, now, tz, () => `${idPrefix}-${++n}`);
  });
  add('plan', 'dragOntoColumn', 300, (r) => {
    const w = planWorld(r);
    return { target: blockTarget(r, w), y: r.pick([minutesToY(9 * 60 + 7), minutesToY(23 * 60 + 30), -20, 800, r.float(-30, 800)]), date: dayKey(w.now, w.tz), data: w.data, idPrefix: 'blk' };
  }, ({ target, y, date, data, idPrefix }) => {
    let n = 0;
    return dragOntoColumn(target, y, date, data as unknown as PlanData, () => `${idPrefix}-${++n}`);
  });
  add('plan', 'resizeBlock', 400, (r) => {
    const w = planWorld(r);
    const b = genBlock(r, 0, w.data.tasks, w.data.habits, dayKey(w.now, w.tz), w.tz, w.now);
    return { block: b, bottomY: r.pick([minutesToY(10 * 60 + 10), minutesToY(9 * 60 + 2), minutesToY(24 * 60) + 200, -50, r.float(-50, 900)]) };
  }, ({ block, bottomY }) => resizeBlock(block as never, bottomY));
  add('plan', 'moveBlock', 400, (r) => {
    const w = planWorld(r);
    const b = genBlock(r, 0, w.data.tasks, w.data.habits, dayKey(w.now, w.tz), w.tz, w.now);
    return { block: b, topY: r.pick([minutesToY(13 * 60 + 5), -50, 748, r.float(-50, 900)]) };
  }, ({ block, topY }) => moveBlock(block as never, topY));
  add('plan', 'planReason', 600, (r) => {
    const tz = seriesZone(r);
    const now = instant(r, tz);
    return { task: genTask(r, 0, now, tz, [], 0), now, tz };
  }, ({ task, now, tz }) => planReason(task as never, now, tz));
  add('plan', 'planMyDay', 500, (r) => {
    const w = planWorld(r);
    const options: Record<string, unknown> = {};
    if (r.chance(0.3)) options.dayEndsAt = r.pick([9 * 60 + 30, 17 * 60, 23 * 60, 24 * 60, 22 * 60 + 20, 8 * 60]);
    if (r.chance(0.3)) options.spaceId = r.pick(['classes', 'wwav', 'personal', '']);
    return { data: w.data, now: w.now, tz: w.tz, options };
  }, ({ data, now, tz, options }) => planMyDay(data as unknown as PlanData, now, tz, options));
  add('plan', 'acceptDraft', 200, (r) => {
    const w = planWorld(r);
    const drafts = planMyDay(w.data as unknown as PlanData, w.now, w.tz);
    if (drafts.length === 0) return null;
    return { draft: r.pick(drafts), idPrefix: 'blk' };
  }, ({ draft, idPrefix }) => {
    let n = 0;
    return acceptDraft(draft, () => `${idPrefix}-${++n}`);
  });
  add('plan', 'draftKey', 300, (r) => {
    const w = planWorld(r);
    const drafts = r.chance(0.9) ? planMyDay(w.data as unknown as PlanData, w.now, w.tz) : [];
    return { drafts, key: r.pick(['Enter', 'Escape', 'p', '', 'Tab', 'enter']), idPrefix: 'blk' };
  }, ({ drafts, key, idPrefix }) => {
    let n = 0;
    return draftKey(drafts, key, () => `${idPrefix}-${++n}`);
  });
  add('plan', 'planSections', 300, (r) => {
    const w = planWorld(r);
    return { data: w.data, now: w.now, tz: w.tz, spaceId: r.pick([undefined, undefined, 'classes', 'wwav', 'personal']) };
  }, ({ data, now, tz, spaceId }) => planSections(data as unknown as PlanData, now, tz, spaceId));
  add('plan', 'planSubtitle', 300, (r) => {
    const w = planWorld(r);
    return { data: w.data, now: w.now, tz: w.tz, spaceId: r.pick([undefined, undefined, 'classes', 'wwav', 'personal']) };
  }, ({ data, now, tz, spaceId }) => planSubtitle(data as unknown as PlanData, now, tz, spaceId));
}

// --- focus ----------------------------------------------------------------------------------

function genTarget(r: Rng): FocusTarget {
  return r.weighted<() => FocusTarget>([
    [3, () => ({ kind: 'task', id: r.pick(['mix', 'quiz', 'a', 'b']), title: r.pick(['Mix the second verse', 'Grammar quiz 4', 'a', 'Écrire 😀']) })],
    [2, () => ({ kind: 'habit', id: 'kanji', title: 'Practise kanji', minutes: r.pick([2, 5, 10, 20, 25]) })],
    [1, () => ({ kind: 'habit', id: 'stretch', title: 'Stretch' })],
  ])();
}

function genFocusEvent(r: Rng): FocusEvent {
  return r.weighted<() => FocusEvent>([
    [4, () => ({ type: 'press', ...(r.chance(0.6) ? { target: genTarget(r) } : {}), ...(r.chance(0.3) ? { room: r.pick(['heat', 'space', 'console'] as const) } : {}) })],
    [4, () => ({ type: 'tick' })],
    [2, () => ({ type: 'stop' })],
    [2, () => ({ type: 'pulledAway' })],
    [2, () => ({ type: 'setTarget', target: r.chance(0.8) ? genTarget(r) : null })],
    [1, () => ({ type: 'setLength', minutes: r.pick([10, 25, 50, 90, 9, 91, 12.5, 45, 0]) })],
  ])();
}

/** States a timer really reaches: random events from the start, at times that move on. */
function focusWalks(r: Rng, walks: number, steps: number) {
  const out: { state: FocusState; event: FocusEvent; now: number; settings: FocusSettings }[] = [];
  for (let w = 0; w < walks; w++) {
    let state = initialFocus();
    let now = Date.UTC(2026, 9, 6, 13) + r.int(0, 3 * 86_400_000);
    const settings = { chime: r.chance(0.4) };
    for (let i = 0; i < steps; i++) {
      now += r.pick([0, 1000, 20_000, 60_000, 5 * 60_000, 12 * 60_000, 25 * 60_000, r.int(0, 40 * 60_000)]);
      const event = genFocusEvent(r);
      out.push({ state, event, now, settings });
      state = focusStep(state, event, now, settings).state;
    }
  }
  return out;
}

function focusVectors(): void {
  add('focus', 'isFocusLength', 200, (r) => ({ minutes: r.pick([9, 10, 11, 45, 89, 90, 91, 12.5, 0, -10, 25, r.int(0, 100), r.float(0, 100)]) }), ({ minutes }) => isFocusLength(minutes));
  add('focus', 'initialFocus', 1, () => ({}), () => initialFocus());
  const walks = focusWalks(new Rng(SEED ^ 0x70c5), 30, 25);
  add('focus', 'focusStep', walks.length, (_, i) => walks[i], ({ state, event, now, settings }) => focusStep(state, event, now, settings));
  add('focus', 'focusLcd', walks.length, (r, i) => ({ state: walks[i].state, now: walks[i].now + r.pick([0, 1000, 59_000, 600_000]) }), ({ state, now }) => focusLcd(state, now));
  add('focus', 'focusStrip', walks.length, (r, i) => ({ state: walks[i].state, now: walks[i].now + r.pick([0, 1000, 59_000, 600_000]) }), ({ state, now }) => focusStrip(state, now));
  const checkWorld = (r: Rng) => {
    const tz = seriesZone(r);
    const now = instant(r, tz);
    const tasks = genTasks(r, 1, now, tz, 0.4);
    const task = { ...tasks[0], id: 't0' };
    const sessions = Array.from({ length: r.int(0, 4) }, (_, i) => ({ ...genSession(r, i, now, ['t0', 'other']) }));
    const occurrences = r.chance(0.5) ? Array.from({ length: r.int(1, 3) }, (_, i) => ({ id: `o${i}`, taskId: 't0', date: dayOffset(r, tz, now, -4, 6), doneAt: now - r.int(0, 4 * 86_400_000) })) : [];
    return { task, sessions, occurrences, now, tz, idPrefix: 'id' };
  };
  add('focus', 'checkOff', 700, checkWorld, ({ task, sessions, occurrences, now, tz, idPrefix }) => {
    let n = 0;
    return checkOff(task as never, { sessions: sessions as never, occurrences }, now, tz, () => `${idPrefix}-${++n}`);
  });
  add('focus', 'finishWithTime', 200, (r) => { const w = checkWorld(r); return { task: w.task, minutes: r.pick([0, 40, 75, 12.5]), now: w.now }; }, ({ task, minutes, now }) => finishWithTime(task as never, minutes, now));
  add('focus', 'setTook', 300, (r) => { const w = checkWorld(r); return { task: w.task, sessions: w.sessions, minutes: r.pick([0, 40, 75, 12.5, 200]) }; }, ({ task, sessions, minutes }) => setTook(task as never, sessions as never, minutes));
}

// --- lcd ---------------------------------------------------------------------------------------

function lcdVectors(): void {
  add('lcd', 'taskHalf', 400, (r) => {
    const tz = seriesZone(r);
    const now = instant(r, tz);
    const tasks = genTasks(r, r.int(0, 6), now, tz, 0.2);
    const data = {
      tasks,
      occurrences: genOccurrences(r, tasks, now, tz),
      sessions: genSessions(r, tasks, now, 4),
      courses: [genCourse(r, 'jpn201', 'JPN 201'), genCourse(r, 'mth142', 'MTH 142')],
      milestones: Array.from({ length: r.int(0, 2) }, (_, i) => genMilestone(r, i + 1, dayKey(now, tz))),
    };
    return {
      data,
      now,
      tz,
      currentTaskId: r.chance(0.5) && tasks.length > 0 ? r.pick(tasks).id : r.pick([null, 'nope']),
      focus: r.pick([null, null, 'focus 18:40 left', 'break 4:00 left', '']),
    };
  }, ({ data, now, tz, currentTaskId, focus }) => taskHalf(data as unknown as LcdData, now, tz, currentTaskId, focus));
}

// --- calendar -----------------------------------------------------------------------------------

function calendarWorld(r: Rng) {
  const w = planWorld(r);
  const today = dayKey(w.now, w.tz);
  const data: Record<string, unknown> = { ...w.data, milestones: Array.from({ length: r.int(0, 3) }, (_, i) => genMilestone(r, i + 1, today)) };
  delete data.sessions;
  if (r.chance(0.2)) delete data.habits;
  return { ...w, data, today };
}

function calendarVectors(): void {
  add('calendar', 'monthGrid', 120, (r) => {
    const w = calendarWorld(r);
    return { anchor: addDays(w.today, r.pick([0, 0, 15, -20, 40, 400])), data: w.data, now: w.now, tz: w.tz };
  }, ({ anchor, data, now, tz }) => monthGrid(anchor, data as unknown as CalendarData, now, tz));
  add('calendar', 'weekDays', 200, (r) => ({ anchor: addDays('2026-10-06', r.int(-800, 800)) }), ({ anchor }) => weekDays(anchor));
  add('calendar', 'dayLayout', 400, (r) => {
    const w = calendarWorld(r);
    return { day: addDays(w.today, r.pick([0, 0, 1, -1, 2, 3])), data: w.data, now: w.now, tz: w.tz };
  }, ({ day, data, now, tz }) => dayLayout(day, data as unknown as CalendarData, now, tz));
  add('calendar', 'unscheduledTray', 300, (r) => {
    const w = calendarWorld(r);
    return { anchor: addDays(w.today, r.pick([0, 0, 7, -7, 3])), data: w.data, now: w.now, tz: w.tz };
  }, ({ anchor, data, now, tz }) => unscheduledTray(anchor, data as unknown as CalendarData, now, tz));
  add('calendar', 'page', 600, (r) => ({ mode: r.pick(['month', 'week', 'day'] as const), anchor: addDays('2026-10-06', r.int(-900, 900)), dir: r.pick([1, -1] as const) }), ({ mode, anchor, dir }) => page(mode, anchor, dir));
  add('calendar', 'dueAtEndOfDay', 300, (r) => {
    const tz = r.pick(ZONES);
    return { day: dayKey(instant(r, tz), tz), tz };
  }, ({ day, tz }) => dueAtEndOfDay(day, tz));
}

// --- review ---------------------------------------------------------------------------------------

function reviewWorld(r: Rng) {
  const tz = seriesZone(r);
  const now = atMinute(addDays(dayKey(instant(r, tz), tz), r.int(0, 6)), r.pick([0, 600, 1439]), tz);
  const tasks = genTasks(r, r.int(0, 7), now, tz, 0.15).map((t) => {
    // Most done tasks were done last week, and some of those took time.
    if (t.done) return { ...t, doneAt: now - r.int(0, 10 * 86_400_000), adjustMin: r.chance(0.6) ? r.int(5, 120) : t.adjustMin };
    return t;
  });
  const habitIds = ['kanji', 'stretch'];
  const data = {
    spaces: [0, 1, 2].slice(0, r.int(1, 3)).map((i) => genSpace(r, i)),
    tasks,
    occurrences: Array.from({ length: r.int(0, 3) }, (_, i) => ({ id: `o${i}`, taskId: tasks.length > 0 ? r.pick(tasks).id : 'x', date: dayOffset(r, tz, now, -8, 0), doneAt: now - r.int(0, 9 * 86_400_000) })),
    sessions: Array.from({ length: r.int(0, 6) }, (_, i) => genSession(r, i, now, tasks.map((t) => t.id), habitIds)),
    milestones: Array.from({ length: r.int(0, 3) }, (_, i) => genMilestone(r, i + 1, dayKey(now, tz))),
  };
  return { data, now, tz };
}

function reviewVectors(): void {
  add('review', 'lastWeekFacts', 400, reviewWorld, ({ data, now, tz }) => lastWeekFacts(data as unknown as ReviewData, now, tz));
  add('review', 'factLines', 400, (r) => {
    const w = reviewWorld(r);
    return { facts: lastWeekFacts(w.data as unknown as ReviewData, w.now, w.tz) };
  }, ({ facts }) => factLines(facts));
}


// --- importArtifact -------------------------------------------------------------------------------

const COURSE_CODES = ['JPN 201', 'MTH 142', 'HIS 101', 'jpn  201', ' MTH142 ', 'Écrire 1'];

/** An instant the export might hold: usually valid, now and then a wrong one. */
function exportInstant(r: Rng, bad = 0.04): string {
  if (r.chance(bad)) return isoLike(r);
  const ms = Date.UTC(2026, 9, 6, 12, 40) + r.int(-20 * 86_400_000, 60 * 86_400_000);
  const offset = r.pick(['Z', 'Z', '-04:00', '+05:30']);
  const d = new Date(ms).toISOString();
  return offset === 'Z' ? d : d.replace('Z', offset);
}

function genExport(r: Rng, bad = 0.04): Record<string, unknown> {
  const spaceDefs = [
    { key: 'classes', name: 'Classes', groupLabel: 'Course', types: ['Homework', 'Quiz', 'Other'], persona: 'a student' },
    { key: 'wwav', name: 'WWAV', groupLabel: 'Milestone', types: ['Hardware', 'Other'], persona: 'a founder' },
    { key: 'personal', name: 'personal', groupLabel: 'Area', types: ['Errand', 'Other'], persona: 'a person' },
    { key: 'lab', name: 'Lab Notebook', groupLabel: r.pick(['Thing', 'Course', 'Milestone', '']), types: [], persona: '' },
    { key: 'extra', name: 'Extra', groupLabel: 'Area', types: ['x'], persona: 'p' },
  ];
  const workspaces = r.sample(spaceDefs, r.int(1, 4));
  const keys = workspaces.map((w) => w.key);
  const codes = r.sample(COURSE_CODES, r.int(0, 3));
  const categoryNames = ['Quizzes', 'Exit tickets', 'Labs'];
  const courses = codes.map((code, i) => ({
    code: i % 2 === 0 ? code : code.toUpperCase(),
    name: r.pick(['Intermediate Japanese', 'Calculus', '']),
    categories: Array.from({ length: r.int(0, 3) }, (_, k) => ({ name: categoryNames[k], weight: r.pick([40, 60, 25, 12.5]), keywords: r.sample(['quiz', 'kanji', 'exit ticket', ''], r.int(0, 3)) })),
    ...(r.chance(0.3) ? { scale: [{ letter: 'A', min: 90 }, { letter: 'F', min: 0 }] } : {}),
    ...(r.chance(0.4) ? { sticky: r.pick(['Office hours Tue 2 PM', '']) } : {}),
  }));
  const milestones = Array.from({ length: r.int(0, 3) }, (_, i) => ({ id: `ms-${i}`, workspace: r.pick(keys), title: r.pick(['Enclosure v2', 'Firmware 1.0']), date: r.pick(['2026-10-20', '2026-11-01', '2026-99-99']), done: r.chance(0.3), order: r.int(0, 4) }));
  const groupNames = [...codes, 'Enclosure v2', 'Firmware 1.0', 'Car', 'Apartment', 'MTH 142', 'mth 142'];
  const tasks = Array.from({ length: r.int(0, 4) }, (_, i) => ({
    id: r.pick([`t-${i}`, `em-${i}abc`, `5h3k${i}_20261008T035900Z`]),
    workspace: r.pick(keys),
    title: r.pick(['Grammar quiz 4', 'Read chapter 3', 'Route the PCB']),
    group: r.chance(0.3) ? null : r.pick(groupNames),
    type: r.pick(['Quiz', 'Reading', 'Hardware', 'Errand']),
    due: r.chance(0.3) ? null : exportInstant(r, bad),
    difficulty: r.weighted<number>([[6, r.int(1, 5)], [1, r.pick([1.5, 4.9, 2.5])]]),
    estMin: r.chance(0.5) ? null : r.pick([0, 15, 45, 120, 7.5]),
    actualMin: r.chance(0.6) ? null : r.pick([0, 30, 75]),
    notes: r.pick(['', 'From mail: …']),
    done: r.chance(0.3),
    doneAt: r.chance(0.7) ? null : exportInstant(r, bad),
    source: r.pick(['manual', 'calendar', 'gmail']),
  }));
  const grades = Array.from({ length: r.int(0, 4) }, (_, i) => ({
    id: r.pick([`gr-${i}`, `gp-${i}ff`, `own-${i}`]),
    course: r.pick([...codes, ...COURSE_CODES]),
    title: r.pick(['Kanji quiz 6', 'Exit Ticket 12']),
    category: r.chance(0.3) ? null : r.pick(categoryNames),
    score: r.chance(0.2) ? null : r.pick([18, 0, 7.5]),
    outOf: r.pick([20, 10, 100, 0]),
    dropped: r.chance(0.1),
    pending: r.chance(0.2),
    ...(r.chance(0.3) ? { link: r.pick(['https://brightspace.uri.edu/d2l/home', '']) } : {}),
  }));
  return {
    format: 'heat-export',
    version: 1,
    exportedAt: '2026-10-06T12:40:00.000Z',
    timeZone: 'America/New_York',
    workspaces,
    tasks,
    milestones,
    habits: Array.from({ length: r.int(0, 2) }, (_, i) => ({ id: `hb-${i}`, title: 'Practise kanji', log: Object.fromEntries(Array.from({ length: r.int(0, 4) }, (_, k) => [addDays('2026-10-06', -k), true])) })),
    term: r.pick(['Fall 2026', 'fall  2026', 'Spring 2027']),
    courses,
    grades,
    processedMailIds: Array.from({ length: r.int(0, 3) }, (_, i) => `18f2${i}`),
    lastSyncAt: r.chance(0.3) ? null : exportInstant(r, bad),
  };
}

/** One thing in a valid export made wrong, or optional made absent. */
function spoil(r: Rng, x: Record<string, unknown>): Record<string, unknown> {
  const y = JSON.parse(JSON.stringify(x)) as Record<string, any>;
  const pickRow = (list: string): Record<string, any> | null => (y[list].length > 0 ? r.pick(y[list] as Record<string, any>[]) : null);
  const wrong = r.pick([123, 'x', null, true, [], {}, -1, 2.5, '2026-10-06T12:40:00Z', 'next Wednesday', 0]);
  switch (r.int(0, 15)) {
    case 0: y.version = r.pick([0, 1.5, '1', 2, 3, null, 1.0, 1e3, 'x']); break;
    case 1: delete y[r.pick(['format', 'version', 'exportedAt', 'timeZone', 'workspaces', 'tasks', 'milestones', 'habits', 'term', 'courses', 'grades', 'processedMailIds', 'lastSyncAt'])]; break;
    case 2: y[r.pick(['exportedAt', 'timeZone', 'term', 'lastSyncAt', 'workspaces', 'tasks', 'processedMailIds'])] = wrong; break;
    case 3: { const t = pickRow('tasks'); if (t) t[r.pick(['id', 'workspace', 'title', 'group', 'type', 'due', 'difficulty', 'estMin', 'actualMin', 'notes', 'done', 'doneAt', 'source'])] = wrong; break; }
    case 4: { const t = pickRow('tasks'); if (t) t.difficulty = r.pick([0, 1, 5, 5.0001, 0.9999, -1, 6, 'x']); break; }
    case 5: { const t = pickRow('tasks'); if (t) t.due = isoLike(r); break; }
    case 6: y[r.pick(['tasks', 'milestones', 'habits', 'courses', 'grades'])] = [null]; break;
    case 7: y[r.pick(['tasks', 'milestones', 'habits', 'courses', 'grades'])] = [r.pick([[], 'x', 5])]; break;
    case 8: { const m = pickRow('milestones'); if (m) m[r.pick(['id', 'workspace', 'title', 'date', 'done', 'order'])] = wrong; break; }
    case 9: { const h = pickRow('habits'); if (h) h.log = r.pick([{ yesterday: true }, { '2026-10-05': false }, { '2026-10-05': 1 }, [], null, { '2026-1-5': true }]); break; }
    case 10: { const c = pickRow('courses'); if (c) c[r.pick(['code', 'name', 'categories', 'scale', 'sticky'])] = wrong; break; }
    case 11: { const c = pickRow('courses'); if (c && c.categories.length > 0) c.categories[0][r.pick(['name', 'weight', 'keywords'])] = wrong; break; }
    case 12: { const g = pickRow('grades'); if (g) g[r.pick(['id', 'course', 'title', 'category', 'score', 'outOf', 'dropped', 'pending', 'link'])] = wrong; break; }
    case 13: y.processedMailIds = r.pick([[42], ['a', 1], 'x', null]); break;
    case 14: { const w = pickRow('workspaces'); if (w) w[r.pick(['key', 'name', 'groupLabel', 'types', 'persona'])] = wrong; break; }
    default: y.format = r.pick(['heat', 'Heat-Export', '', null]); break;
  }
  return y;
}

function importVectors(): void {
  add('importArtifact', 'parseHeatExport', 700, (r) => ({
    json: r.weighted<() => unknown>([
      [4, () => genExport(r)],
      [6, () => spoil(r, genExport(r))],
      [1, () => r.pick([null, 5, 'x', [], {}, { tasks: [] }, { format: 'heat-export' }])],
    ])(),
  }), ({ json }) => parseHeatExport(json));
  // importArtifact takes an export parseHeatExport accepted, so every instant in it is one that reads.
  add('importArtifact', 'importArtifact', 300, (r) => {
    const dump = genExport(r, 0);
    const existing: Record<string, unknown> = {};
    if (r.chance(0.4)) {
      let n = 0;
      existing.spaces = defaultSpaces(() => `space-${++n}`);
    }
    if (r.chance(0.4)) existing.terms = [{ id: 'term-1', name: r.pick(['Fall 2026', ' fall 2026', 'Spring 2027']) }];
    if (r.chance(0.4)) {
      existing.courses = (dump.courses as { code: string }[])
        .filter(() => r.chance(0.6))
        .map((c, i) => ({ id: `course-${i}`, termId: r.pick(['term-1', 'term-9']), code: r.pick([c.code, c.code.toLowerCase(), 'ZZZ 1']), name: 'x', categories: [{ id: `cat-${i}`, name: 'Quizzes', weight: 10, keywords: [] }], notes: '' }));
    }
    return { dump, existing, idPrefix: 'new' };
  }, ({ dump, existing, idPrefix }) => {
    let n = 0;
    return importArtifact(dump as unknown as HeatExport, existing, () => `${idPrefix}-${++n}`);
  });
}

// --- Entry -------------------------------------------------------------------------------

describe.skipIf(!ENABLED)('writing the cross-check vectors', () => {
  it('generates every module and writes the files', async () => {
    jsVectors();
    zoneVectors();
    formatVectors();
    heatVectors();
    estimateVectors();
    recurrenceVectors();
    spacesVectors();
    gradesVectors();
    habitsVectors();
    planVectors();
    focusVectors();
    lcdVectors();
    calendarVectors();
    reviewVectors();
    importVectors();
    await write();
  }, 600_000);
});
