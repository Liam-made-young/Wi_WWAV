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
import { byHeat, DAY_MS, duePhrase, heatOf, heatThresholds, nextHeatChange, runway, tubeFill } from './heat';

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

function genBlock(r: Rng, i: number, tasks: { id: string }[], habits: { id: string }[], today: string, tz: string, now: number) {
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
    const tasks = genTasks(r, r.int(0, 9), now, NY, 0);
    return { now, tasks, sessions: genSessions(r, tasks, now) };
  };
  add('estimate', 'actualMin', 500, (r) => {
    const w = world(r);
    return { task: w.tasks.length > 0 ? r.pick(w.tasks) : genTask(r, 0, w.now, NY, []), sessions: w.sessions };
  }, ({ task, sessions }) => actualMin(task as never, sessions as never));
  add('estimate', 'estimateContext', 500, (r) => world(r), ({ tasks, sessions }) => {
    const ctx = estimateContext(tasks as never, sessions as never);
    const sorted = <T>(m: Map<string, T>) => Object.fromEntries([...m.entries()].sort(([a], [b]) => (a < b ? -1 : 1)));
    return {
      averages: sorted(ctx.averages),
      children: Object.fromEntries([...ctx.children.entries()].map(([k, v]) => [k, v.map((t) => t.id)]).sort(([a], [b]) => ((a as string) < (b as string) ? -1 : 1))),
    };
  });
  add('estimate', 'estimateMin', 800, (r) => {
    const w = world(r);
    if (w.tasks.length === 0) return null;
    return { ...w, taskId: r.pick(w.tasks).id };
  }, ({ tasks, sessions, taskId }) => {
    const ctx = estimateContext(tasks as never, sessions as never);
    return estimateMin(tasks.find((t) => t.id === taskId) as never, ctx);
  });
  add('estimate', 'averageLines', 400, (r) => {
    const w = world(r);
    // Done tasks with time, so the averages exist.
    const tasks = w.tasks.map((t) => (r.chance(0.5) ? { ...t, done: true, adjustMin: r.int(10, 120) } : t));
    return { space: genSpace(r, r.int(0, 2)), tasks, sessions: w.sessions };
  }, ({ space, tasks, sessions }) => averageLines(space as never, estimateContext(tasks as never, sessions as never)));
  add('estimate', 'weeklyLoad', 700, (r) => world(r), ({ tasks, sessions, now }) =>
    weeklyLoad(tasks as never, estimateContext(tasks as never, sessions as never), now));
  add('estimate', 'weeklyLoadLine', 300, (r) => ({ load: { minutes: r.pick([0, 1, 45, 200, 190, r.int(0, 1000), 44.6], ), count: r.pick([0, 1, 2, 5, r.int(0, 20)]) } }), ({ load }) => weeklyLoadLine(load));
}

// Generators for the modules still to come.
void [genOccurrences, genHabit, genBlock, genEvent, genMilestone];

// --- Entry -------------------------------------------------------------------------------

describe.skipIf(!ENABLED)('writing the cross-check vectors', () => {
  it('generates every module and writes the files', async () => {
    jsVectors();
    zoneVectors();
    formatVectors();
    heatVectors();
    estimateVectors();
    await write();
  }, 600_000);
});
