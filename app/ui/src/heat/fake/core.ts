// An in-memory stand-in for the core's `heat.*` commands (docs/HEAT.md), for
// tests and for `npm run dev` without a core. It keeps records as the real
// core does and answers the same shapes. The derived values in its snapshot
// come from the TS model (app/ui/src/heat/model), the reference the Rust
// was ported from; the real views never compute them.
//
// Each part of Heat adds its commands and its share of the snapshot in its
// own module (fake/today.ts, fake/grades.ts, …), registered in fake/all.ts:
//
//   register('heat.plan.make', (args, fake) => …);
//   derive((snapshot, fake) => { snapshot.derived.today = … });

import type { DayKey, HeatState, Kind, Records, Snapshot, Transport } from '../client';

export type Store = { [K in Kind]: Map<string, Records[K]> };

export interface Fake {
  store: Store;
  state: HeatState;
  /** Epoch ms; tests move it. */
  now: number;
  zone: string;
  /** Undo labels, newest last, as the journal would hold them. */
  journal: { label: string; before: Partial<Store> }[];
  emit(kinds: Kind[]): void;
  newId(): string;
  /** Runs `change` as one labelled entry, and answers its Edit menu text. */
  write(label: string, kinds: Kind[], change: () => void): { undo: string };
}

type Handler = (args: Record<string, unknown>, fake: Fake) => unknown;
type Deriver = (snapshot: Snapshot, fake: Fake) => void;

const handlers = new Map<string, Handler>();
const derivers: Deriver[] = [];

export function register(cmd: string, handler: Handler) {
  handlers.set(cmd, handler);
}

export function derive(deriver: Deriver) {
  derivers.push(deriver);
}

const KINDS: Kind[] = [
  'space', 'task', 'taskOccurrence', 'timeBlock', 'focusSession', 'project', 'milestone', 'habit',
  'term', 'course', 'grade', 'mailThread', 'calendar', 'capture', 'dailyNote', 'note', 'profileShare',
];

function emptyStore(): Store {
  return Object.fromEntries(KINDS.map((k) => [k, new Map()])) as unknown as Store;
}

function copy(store: Store, kinds: Kind[]): Partial<Store> {
  return Object.fromEntries(kinds.map((k) => [k, new Map(store[k] as Map<string, unknown>)])) as Partial<Store>;
}

export class FakeError extends Error {
  constructor(
    public code: string,
    message: string,
  ) {
    super(message);
  }
}

export function refuse(message: string, code = 'refused'): never {
  throw new FakeError(code, message);
}

/** The day key of `ms` in `zone`. */
export function dayOf(ms: number, zone: string): DayKey {
  return new Intl.DateTimeFormat('en-CA', { timeZone: zone, year: 'numeric', month: '2-digit', day: '2-digit' }).format(ms);
}

export function createFake(seed: Partial<{ [K in Kind]: Records[K][] }> = {}, opts: { now?: number; zone?: string } = {}) {
  const listeners = new Map<string, Set<(p: unknown) => void>>();
  let n = 0;
  const fake: Fake = {
    store: emptyStore(),
    state: { currentTaskId: null, timer: { phase: 'idle', round: 1, endsAt: null }, planDrafts: [] },
    now: opts.now ?? Date.now(),
    zone: opts.zone ?? Intl.DateTimeFormat().resolvedOptions().timeZone,
    journal: [],
    emit(kinds) {
      for (const h of listeners.get('heat') ?? []) h({ kinds });
    },
    newId() {
      n += 1;
      return `fake-${String(n).padStart(6, '0')}`;
    },
    write(label, kinds, change) {
      fake.journal.push({ label, before: copy(fake.store, kinds) });
      change();
      fake.emit(kinds);
      return { undo: `Undo ${label}` };
    },
  };
  for (const kind of KINDS) {
    for (const r of (seed[kind] ?? []) as Records[typeof kind][]) {
      const key = kind === 'dailyNote' ? (r as Records['dailyNote']).date : (r as { id: string }).id;
      (fake.store[kind] as Map<string, unknown>).set(key, r);
    }
  }

  const transport: Transport = {
    async call<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
      const handler = handlers.get(cmd);
      if (!handler) throw new FakeError('unknown_command', `The fake core has no ${cmd}.`);
      return structuredClone(await handler(args, fake)) as T;
    },
    on(event, handler) {
      const set = listeners.get(event) ?? new Set();
      set.add(handler);
      listeners.set(event, set);
      return () => set.delete(handler);
    },
  };
  return { fake, transport };
}

// --- the commands every part shares ---------------------------------------

register('heat.snapshot', (args, fake) => {
  const date = (args.date as DayKey) ?? dayOf(fake.now, fake.zone);
  const records = Object.fromEntries(KINDS.map((k) => [k, [...fake.store[k].values()]])) as Snapshot['records'];
  const snapshot: Snapshot = {
    now: fake.now,
    date,
    zone: fake.zone,
    records,
    heatState: fake.state,
    events: [],
    derived: {
      tasks: {},
      today: { header: '', planned: [], dueToday: [], recurringToday: [], hotUnplanned: [] },
      hotTasks: [],
      lists: { inbox: [], allOpen: [], hot: [], dueThisWeek: [], scheduled: [], someday: [], done: [] },
      averages: [],
      courses: {},
      habits: {},
      status: 'Saved on this Mac',
    },
  };
  for (const d of derivers) d(snapshot, fake);
  return snapshot;
});

const keyOf = (kind: Kind, record: Record<string, unknown>) => (kind === 'dailyNote' ? (record.date as string) : (record.id as string));

register('heat.put', (args, fake) => {
  const kind = args.kind as Kind;
  if (!KINDS.includes(kind)) refuse(`Heat keeps no ${kind}.`);
  const record = { ...(args.record as Record<string, unknown>) };
  const isNew = kind === 'dailyNote' ? !fake.store.dailyNote.has(record.date as string) : !record.id;
  if (kind !== 'dailyNote' && !record.id) record.id = fake.newId();
  if (kind === 'habit' && isNew && fake.store.habit.size >= 6) refuse('Habit limit reached');
  if (isNew && kind !== 'mailThread' && record.public === undefined) record.public = false;
  const { undo } = fake.write(isNew ? `add ${kind}` : `edit ${kind}`, [kind], () => {
    (fake.store[kind] as Map<string, unknown>).set(keyOf(kind, record), record);
  });
  return { record, undo };
});

register('heat.patch', (args, fake) => {
  const kind = args.kind as Kind;
  const map = fake.store[kind] as unknown as Map<string, Record<string, unknown>>;
  const was = map.get(args.id as string);
  if (!was) refuse(`No ${kind} has that id.`);
  const set = args.set as Record<string, unknown>;
  if ('public' in set) refuse('The Public switch has its own command.');
  const label = kind === 'task' && 'title' in set ? 'rename task' : kind === 'task' && ('difficulty' in set || 'estMin' in set) ? 'estimate' : `edit ${kind}`;
  const record = { ...was, ...set };
  const { undo } = fake.write(label, [kind], () => map.set(args.id as string, record));
  return { record, undo };
});

register('heat.delete', (args, fake) => {
  const kind = args.kind as Kind;
  const map = fake.store[kind] as Map<string, unknown>;
  if (!map.has(args.id as string)) refuse(`No ${kind} has that id.`);
  const kinds: Kind[] = kind === 'task' ? ['task', 'timeBlock', 'taskOccurrence'] : [kind];
  return fake.write(kind === 'timeBlock' ? 'remove block' : `delete ${kind}`, kinds, () => {
    map.delete(args.id as string);
    if (kind === 'task') {
      for (const [id, b] of fake.store.timeBlock) if (b.taskId === args.id) fake.store.timeBlock.delete(id);
      for (const [id, o] of fake.store.taskOccurrence) if (o.taskId === args.id) fake.store.taskOccurrence.delete(id);
    }
  });
});

register('heat.public.set', (args, fake) => {
  const kind = args.kind as Kind;
  const map = fake.store[kind] as unknown as Map<string, Record<string, unknown>>;
  const was = map.get(args.id as string);
  if (!was) refuse(`No ${kind} has that id.`);
  if (kind === 'mailThread') refuse('Mail is never public.');
  const on = args.public === true;
  const record = { ...was, public: on };
  const { undo } = fake.write(on ? 'make public' : 'make private', [kind], () => map.set(args.id as string, record));
  const sentence =
    kind === 'grade'
      ? 'Grades are private by default. Your school keeps them as education records. Turning this on shows this grade to anyone who opens your sun.'
      : undefined;
  return { record, undo, sentence };
});

/** Undo the newest entry, as ⌘Z in Heat would. */
register('history.undo', (_args, fake) => {
  const last = fake.journal.pop();
  if (!last) refuse('Nothing to undo.', 'nothing_to_undo');
  for (const [kind, map] of Object.entries(last.before)) (fake.store as Record<string, unknown>)[kind] = map;
  fake.emit(Object.keys(last.before) as Kind[]);
  return { label: last.label };
});

register('history.get', (_args, fake) => {
  const last = fake.journal.at(-1);
  return { undo: last ? `Undo ${last.label}` : null, redo: null, cant: null };
});
