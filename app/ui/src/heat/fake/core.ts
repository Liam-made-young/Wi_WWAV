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

/** One record a journal entry changed: what it was, and what it became (undefined is absent). */
export interface RowChange {
  kind: Kind;
  key: string;
  before: unknown;
  after: unknown;
}

/** One entry of the journal: who made the change, with which tool and why (docs/HEAT.md, "The journal"). */
export interface JournalEntry {
  id: string;
  label: string;
  rows: RowChange[];
  actor: 'you' | 'claude';
  tool?: string;
  reason?: string;
  at: number;
  undone: boolean;
}

export interface WriteMeta {
  actor?: 'you' | 'claude';
  tool?: string;
  reason?: string;
}

export interface Fake {
  store: Store;
  state: HeatState;
  /** Epoch ms; tests move it. */
  now: number;
  zone: string;
  /** Undo labels, newest last, as the journal would hold them. */
  journal: JournalEntry[];
  emit(kinds: Kind[]): void;
  newId(): string;
  /** Runs `change` as one labelled entry, and answers its Edit menu text and the entry's id. */
  write(label: string, kinds: Kind[], change: () => void, meta?: WriteMeta): { undo: string; txnId: string };
}

type Handler = (args: Record<string, unknown>, fake: Fake) => unknown;
type Deriver = (snapshot: Snapshot, fake: Fake) => void;

const handlers = new Map<string, Handler>();
const derivers: Deriver[] = [];

export function register(cmd: string, handler: Handler) {
  handlers.set(cmd, handler);
}

/** Puts a check (or a change) around a command another part registered: the rules `heat.put` and `heat.patch` hold per kind. */
export function wrap(cmd: string, wrapper: (next: Handler) => Handler) {
  const was = handlers.get(cmd);
  if (!was) throw new Error(`Nothing is registered for ${cmd} to wrap.`);
  handlers.set(cmd, wrapper(was));
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

/** The records that differ between two copies of the store, for the kinds named. */
function rowsChanged(before: Partial<Store>, store: Store, kinds: Kind[]): RowChange[] {
  const rows: RowChange[] = [];
  for (const kind of kinds) {
    const was = before[kind] as Map<string, unknown>;
    const now = store[kind] as Map<string, unknown>;
    for (const key of new Set([...was.keys(), ...now.keys()])) {
      if (was.get(key) === now.get(key)) continue;
      const clone = (v: unknown) => (v === undefined ? undefined : structuredClone(v));
      rows.push({ kind, key, before: clone(was.get(key)), after: clone(now.get(key)) });
    }
  }
  return rows;
}

const rowKey = (r: RowChange) => `${r.kind}\u0000${r.key}`;

/** Puts a journal entry's records back as they were. */
function restore(fake: Fake, entry: JournalEntry) {
  for (const r of entry.rows) {
    const map = fake.store[r.kind] as Map<string, unknown>;
    if (r.before === undefined) map.delete(r.key);
    else map.set(r.key, structuredClone(r.before));
  }
  fake.emit([...new Set(entry.rows.map((r) => r.kind))]);
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
  let txn = 0;
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
    write(label, kinds, change, meta = {}) {
      const before = copy(fake.store, kinds);
      change();
      txn += 1;
      const id = `txn-${String(txn).padStart(6, '0')}`;
      fake.journal.push({
        id,
        label,
        rows: rowsChanged(before, fake.store, kinds),
        actor: meta.actor ?? 'you',
        ...(meta.tool ? { tool: meta.tool } : {}),
        ...(meta.reason ? { reason: meta.reason } : {}),
        at: fake.now,
        undone: false,
      });
      fake.emit(kinds);
      return { undo: `Undo ${label}`, txnId: id };
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
  if (!KINDS.includes(kind)) refuse(`Learn keeps no ${kind}.`);
  const record = { ...(args.record as Record<string, unknown>) };
  const isNew = kind === 'dailyNote' ? !fake.store.dailyNote.has(record.date as string) : !record.id;
  if (kind !== 'dailyNote' && !record.id) record.id = fake.newId();
  if (kind === 'habit' && isNew && fake.store.habit.size >= 6) refuse('Habit limit reached');
  // Every record starts private, and only the Public switch changes that (3.15).
  const was = (fake.store[kind] as unknown as Map<string, Record<string, unknown>>).get(keyOf(kind, record));
  if (record.public === true && was?.public !== true) refuse('The Public switch has its own command.');
  if (isNew && kind !== 'mailThread' && record.public === undefined) record.public = false;
  const { undo } = fake.write(isNew ? `add ${kind}` : `edit ${kind}`, [kind], () => {
    (fake.store[kind] as Map<string, unknown>).set(keyOf(kind, record), record);
  });
  return { record, undo };
});

/** The Edit menu's words for a patch (docs/HEAT.md, "The journal"). */
function patchLabel(kind: Kind, was: Record<string, unknown>, set: Record<string, unknown>): string {
  if (kind === 'task' && 'title' in set) return 'rename task';
  if (kind === 'task' && ('difficulty' in set || 'estMin' in set)) return 'estimate';
  if (kind === 'habit' && 'log' in set && Object.keys(set).length === 1) {
    const days = (log: unknown) => Object.keys((log ?? {}) as object).length;
    return days(set.log) >= days(was.log) ? 'tick habit' : 'untick habit';
  }
  return `edit ${kind}`;
}

register('heat.patch', (args, fake) => {
  const kind = args.kind as Kind;
  const map = fake.store[kind] as unknown as Map<string, Record<string, unknown>>;
  const was = map.get(args.id as string);
  if (!was) refuse(`No ${kind} has that id.`);
  const set = args.set as Record<string, unknown>;
  if ('public' in set) refuse('The Public switch has its own command.');
  const label = patchLabel(kind, was, set);
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

/** The newest entry still done: what ⌘Z would undo. */
const newestDone = (fake: Fake) => {
  for (let i = fake.journal.length - 1; i >= 0; i--) if (!fake.journal[i].undone) return i;
  return -1;
};

/** Undo the newest entry, as ⌘Z in Heat would. */
register('history.undo', (_args, fake) => {
  const at = newestDone(fake);
  if (at < 0) refuse('Nothing to undo.', 'nothing_to_undo');
  const [last] = fake.journal.splice(at, 1);
  restore(fake, last);
  return { label: last.label };
});

/**
 * Undoes one entry out of order, for Settings → Claude's list. Refused when
 * a later entry that is still done touched any of the same records.
 */
register('history.undoEntry', (args, fake) => {
  const at = fake.journal.findIndex((e) => e.id === args.txnId);
  if (at < 0) refuse('That change is no longer in the history.', 'no_such_entry');
  const entry = fake.journal[at];
  if (entry.undone) refuse('That change was already undone.', 'already_undone');
  const mine = new Set(entry.rows.map(rowKey));
  const later = fake.journal.slice(at + 1).some((e) => !e.undone && e.rows.some((r) => mine.has(rowKey(r))));
  if (later) refuse('This changed again since. Undo the later change first.', 'changed_since');
  restore(fake, entry);
  entry.undone = true;
  return { label: entry.label };
});

register('history.get', (_args, fake) => {
  const at = newestDone(fake);
  return { undo: at >= 0 ? `Undo ${fake.journal[at].label}` : null, redo: null, cant: null };
});
