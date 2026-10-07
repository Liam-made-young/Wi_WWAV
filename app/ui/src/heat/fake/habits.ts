// The fake core's Habits (docs/HEAT.md, docs/SPEC.md 3.9): each habit's "today"
// and its growing record ("Done 41 days since August 26") in the snapshot. A
// streak counter is a per-habit setting, off by default, and then the snapshot
// carries its sentence too. A habit has up to 6 (`heat.put` refuses a 7th),
// and deleting one takes its blocks with it. The rules are the TS model's.

import { doneRecord, streak } from '../model/habits';
import * as copy from '../model/copy';
import type * as M from '../model/records';
import { derive, refuse, wrap } from './core';

derive((snap) => {
  for (const h of snap.records.habit) {
    const own = h as unknown as M.Habit;
    const days = streak(own, snap.date);
    snap.derived.habits[h.id] = {
      today: h.log[snap.date] === true,
      record: doneRecord(own, snap.date),
      ...(h.showCounter && days > 0 ? { counter: copy.habits.streak(days) } : {}),
    };
  }
});

wrap('heat.delete', (next) => (args, fake) => {
  if (args.kind !== 'habit') return next(args, fake);
  const id = args.id as string;
  if (!fake.store.habit.has(id)) refuse('No habit has that id.');
  return fake.write('delete habit', ['habit', 'timeBlock'], () => {
    fake.store.habit.delete(id);
    for (const [bid, b] of fake.store.timeBlock) if (b.habitId === id) fake.store.timeBlock.delete(bid);
  });
});
