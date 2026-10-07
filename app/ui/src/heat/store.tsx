// The one place Heat's views read from: the snapshot of `heat.snapshot`,
// refetched whenever the core says `heat` changed and when the minute turns
// (heat values and phrases like "3h overdue" move with the clock), and the
// client that writes. Everything the views draw comes from here; nothing
// sorts, clamps, plans or works out a heat value (docs/HEAT.md).
//
// The provider sits above the shell, so the Now strip, ⌘⇧N and ⌘K read the
// same snapshot Heat does. It also keeps the focus timer honest across
// views: a round whose time is up is finished here, whichever room is open.

import { createContext, type ReactNode, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { type DayKey, dayKey } from '../shared/time/zone';
import {
  type Capture,
  type Course,
  type Habit,
  type HeatClient,
  heatClient,
  type Id,
  type MailThread,
  type Milestone,
  type Project,
  type Snapshot,
  type Space,
  type Task,
  type TimeBlock,
} from './client';
import { chimeOn, ringChime } from './chime';
import { sentenceOf } from './fmt';

/** Every record by id, built once per snapshot, so a view looks a task up instead of searching. */
export interface Index {
  space: Map<Id, Space>;
  task: Map<Id, Task>;
  block: Map<Id, TimeBlock>;
  course: Map<Id, Course>;
  milestone: Map<Id, Milestone>;
  project: Map<Id, Project>;
  habit: Map<Id, Habit>;
  capture: Map<Id, Capture>;
  /** Every thread Claude recorded, newest first: the one list Mail and the Mail widget both read. */
  mail: MailThread[];
  /** A recurring task's ticked days. */
  ticked: Map<Id, Set<DayKey>>;
  /** How many focus sessions a task has. */
  sessions: Map<Id, number>;
}

function byId<T extends { id: Id }>(rows: T[] | undefined): Map<Id, T> {
  return new Map((rows ?? []).map((r) => [r.id, r]));
}

export function buildIndex(snap: Snapshot | null): Index {
  const records = snap?.records;
  const ticked = new Map<Id, Set<DayKey>>();
  for (const o of records?.taskOccurrence ?? []) {
    const days = ticked.get(o.taskId) ?? new Set<DayKey>();
    days.add(o.date);
    ticked.set(o.taskId, days);
  }
  const sessions = new Map<Id, number>();
  for (const s of records?.focusSession ?? []) if (s.taskId) sessions.set(s.taskId, (sessions.get(s.taskId) ?? 0) + 1);
  return {
    space: byId(records?.space),
    task: byId(records?.task),
    block: byId(records?.timeBlock),
    course: byId(records?.course),
    milestone: byId(records?.milestone),
    project: byId(records?.project),
    habit: byId(records?.habit),
    capture: byId(records?.capture),
    mail: [...(records?.mailThread ?? [])].sort((a, b) => (b.receivedAt ?? 0) - (a.receivedAt ?? 0)),
    ticked,
    sessions,
  };
}

export interface HeatStore {
  client: HeatClient;
  /** Null until the first answer. */
  snap: Snapshot | null;
  /** Why the last fetch failed, in one sentence; null when it worked. */
  error: string | null;
  idx: Index;
  tz: string;
  /** Today, in the person's zone. */
  date: DayKey;
  /** The core's clock, moved on from the snapshot's `now`. */
  now(): number;
  refetch(): Promise<void>;
  /** The window Calendar draws, or null for the default month around today. */
  setRange(range: { from: DayKey; to: DayKey } | null): void;
  /** A sentence for the status bar, held 2.6 s: "Done. Took 1h 15m across 3 focus sessions." */
  say(text: string): void;
  message: { text: string; n: number } | null;
  /** Runs a write. A refusal is said in its own sentence, and the answer is null. */
  act<T>(write: Promise<T>): Promise<T | null>;
}

const Context = createContext<HeatStore | null>(null);

const systemZone = () => Intl.DateTimeFormat().resolvedOptions().timeZone;
const MESSAGE_MS = 2600;

interface Props {
  /** The core's client; the fake one in tests. */
  client?: HeatClient;
  /** The machine's clock, for tests that hold time still or move it. */
  clock?: () => number;
  children: ReactNode;
}

export function HeatProvider({ client, clock = Date.now, children }: Props) {
  const c = useMemo(() => client ?? heatClient(), [client]);
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<{ text: string; n: number } | null>(null);
  const [range, setRangeState] = useState<{ from: DayKey; to: DayKey } | null>(null);

  // The core's clock is the snapshot's `now` plus what the machine's clock has run since.
  const offset = useRef(0);
  const clockRef = useRef(clock);
  clockRef.current = clock;
  const now = useCallback(() => clockRef.current() + offset.current, []);
  const zone = useRef(systemZone());
  const wanted = useRef(range);
  wanted.current = range;
  const latest = useRef(0);

  const refetch = useCallback(async () => {
    const mine = ++latest.current;
    try {
      // Calendar's window always takes in today, which Today and the strip draw from.
      const today = dayKey(now(), zone.current);
      const r = wanted.current;
      const s = await c.snapshot(
        today,
        r ? (r.from < today ? r.from : today) : undefined,
        r ? (r.to > today ? r.to : today) : undefined,
      );
      if (mine !== latest.current) return;
      offset.current = s.now - clockRef.current();
      zone.current = s.zone;
      setSnap(s);
      setError(null);
    } catch (e) {
      if (mine === latest.current) setError(sentenceOf(e));
    }
  }, [c, now]);

  useEffect(() => c.onChange(() => void refetch()), [c, refetch]);
  useEffect(() => {
    void refetch();
  }, [range, refetch]);

  // The minute turns: heat values and "3h overdue" move on.
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    const next = () => {
      timer = setTimeout(
        () => {
          void refetch();
          next();
        },
        60_000 - (now() % 60_000) + 50,
      );
    };
    next();
    return () => clearTimeout(timer);
  }, [now, refetch]);

  const say = useCallback((text: string) => setMessage({ text, n: now() + Math.random() }), [now]);
  useEffect(() => {
    if (!message) return;
    const t = setTimeout(() => setMessage(null), MESSAGE_MS);
    return () => clearTimeout(t);
  }, [message]);

  const act = useCallback(
    async <T,>(write: Promise<T>): Promise<T | null> => {
      try {
        return await write;
      } catch (e) {
        say(sentenceOf(e));
        return null;
      }
    },
    [say],
  );

  // A round or break whose time is up is finished here, whichever room is open
  // (2.3). The core ends it at its own endsAt whatever the call, so a call that
  // comes early or twice changes nothing: this looks twice a second while a
  // round runs, and asks again a few times if the round is somehow still on.
  const timer = snap?.heatState.timer;
  const running = timer ? (timer.running ?? timer.endsAt !== null) : false;
  const endsAt = running ? (timer?.endsAt ?? null) : null;
  useEffect(() => {
    if (endsAt === null) return;
    let asking = false;
    let asked = 0;
    const look = setInterval(() => {
      if (asking || asked >= 8 || now() < endsAt) return;
      asking = true;
      asked += 1;
      c.focus('finish').then(
        (r) => {
          asking = false;
          // The round ended (it logged, and a break now waits): ring if the person asked for the chime.
          if (r.logged && r.heatState.timer.phase === 'break' && chimeOn()) ringChime();
        },
        () => (asking = false),
      );
    }, 500);
    return () => clearInterval(look);
  }, [endsAt, c, now]);

  const idx = useMemo(() => buildIndex(snap), [snap]);
  const date = snap?.date ?? dayKey(clock(), zone.current);

  const store = useMemo<HeatStore>(
    () => ({
      client: c,
      snap,
      error,
      idx,
      tz: snap?.zone ?? zone.current,
      date,
      now,
      refetch,
      setRange: (r) => setRangeState((was) => (was?.from === r?.from && was?.to === r?.to ? was : r)),
      say,
      message,
      act,
    }),
    [c, snap, error, idx, date, now, refetch, say, message, act],
  );

  return <Context.Provider value={store}>{children}</Context.Provider>;
}

export function useHeat(): HeatStore {
  const store = useContext(Context);
  if (!store) throw new Error('Learn is read through a HeatProvider.');
  return store;
}

/** The core's clock, re-read every `everyMs`: a second while a countdown shows, a minute otherwise. */
export function useNow(everyMs = 60_000): number {
  const { now } = useHeat();
  const [, tick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => tick((n) => n + 1), everyMs);
    return () => clearInterval(t);
  }, [everyMs]);
  return now();
}
