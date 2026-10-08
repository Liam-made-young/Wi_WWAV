// The fake core's share of the Focus layout: `snapshot.focus` and the
// commands that go with it (docs/FOCUS.md), answered from the same rules the
// core ports (focus/model.ts). What the core remembers in its own table, the
// fake remembers per fake.

import { type Fake, derive, dayOf, refuse, register, snapshotOf } from '../heat/fake/core';
import type { Kind } from '../heat/client';
import { type Action, emptyMemory, eventsFromChanges, type FocusMemory, focusState, type Priority } from './model';

interface Kept extends FocusMemory {
  /** How much of the journal has been looked at for what Claude changed. */
  seen: number;
}

const kept = new WeakMap<Fake, Kept>();

/** What this fake remembers: tests read it, and may fill its queue. */
export function focusMemory(fake: Fake): Kept {
  let m = kept.get(fake);
  if (!m) {
    m = { ...emptyMemory(), seen: 0 };
    kept.set(fake, m);
  }
  return m;
}

/** Entries Claude wrote since the last look become events, as the core's watcher makes them of the helper's. */
function notice(fake: Fake, m: Kept) {
  for (const entry of fake.journal.slice(m.seen)) {
    if (entry.actor !== 'claude' || entry.undone) continue;
    for (const e of eventsFromChanges(entry.rows, entry.at)) {
      const same = JSON.stringify(e);
      if (!m.queue.some((q) => JSON.stringify(q) === same)) m.queue.push(e);
    }
  }
  m.seen = fake.journal.length;
  if (m.queue.length > 50) m.queue = m.queue.slice(-50);
}

derive((snapshot, fake) => {
  const m = focusMemory(fake);
  notice(fake, m);
  snapshot.focus = focusState(snapshot, m);
});

// The views hear of a change to what Focus remembers as they hear of any other.
const changed = (fake: Fake) => fake.emit(['focus' as Kind]);

register('heat.interrupt.dismiss', (args, fake) => {
  const m = focusMemory(fake);
  const id = String(args.id ?? '');
  m.dismissed[id] = fake.now;
  m.queue = m.queue.filter((e) => !(e.kind === 'custom' && e.id === id));
  changed(fake);
  return {};
});

register('heat.interrupt.raise', (args, fake) => {
  const id = String(args.id ?? '');
  const line = String(args.line ?? '');
  if (!id || !line) refuse('An interrupt needs an id and a line.', 'bad_args');
  const m = focusMemory(fake);
  m.queue = m.queue.filter((e) => !(e.kind === 'custom' && e.id === id));
  m.queue.push({
    kind: 'custom',
    id,
    source: String(args.source ?? 'custom'),
    line,
    action: (args.action as Action | null | undefined) ?? null,
    changesNext: args.changesNext === true,
    priority: (args.priority as Priority | undefined) ?? 'normal',
    at: fake.now,
  });
  changed(fake);
  return { queued: true };
});

register('heat.focus.snooze', (_args, fake) => {
  focusMemory(fake).fixSnoozed = dayOf(fake.now, fake.zone);
  changed(fake);
  return {};
});

register('heat.focus.state', (args, fake) => snapshotOf(fake, (args.date as string | undefined) ?? undefined).focus);
register('heat.entropy', (_args, fake) => snapshotOf(fake).focus?.entropy);
