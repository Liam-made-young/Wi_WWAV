// Builders shared by the model's tests. Times are written as New York wall
// clocks, the zone 3.11 says URI's students get from the system.

import { epochOf } from '../../shared/time/zone';
import type { FocusSession, Habit, Task, TimeBlock } from './records';

export const NY = 'America/New_York';

/** "2026-10-06 08:40" in New York, as epoch ms. */
export function ny(text: string): number {
  const [date, time = '00:00'] = text.split(' ');
  const [year, month, day] = date.split('-').map(Number);
  const [hour, minute] = time.split(':').map(Number);
  return epochOf({ year, month, day, hour, minute }, NY);
}

let serial = 0;

export function task(over: Partial<Task> = {}): Task {
  serial += 1;
  return {
    id: `task-${serial}`,
    spaceId: 'classes',
    title: `Task ${serial}`,
    type: 'Homework',
    due: null,
    difficulty: 3,
    estMin: null,
    adjustMin: 0,
    notes: '',
    done: false,
    doneAt: null,
    source: 'you',
    ...over,
  };
}

export function session(over: Partial<FocusSession> = {}): FocusSession {
  serial += 1;
  return {
    id: `session-${serial}`,
    startedAt: 0,
    endedAt: 0,
    focusMin: 25,
    interruptions: 0,
    view: 'heat',
    ...over,
  };
}

export function block(over: Partial<TimeBlock> = {}): TimeBlock {
  serial += 1;
  return { id: `block-${serial}`, date: '2026-10-06', start: 9 * 60, minutes: 30, origin: 'you', ...over };
}

export function habit(over: Partial<Habit> = {}): Habit {
  serial += 1;
  return { id: `habit-${serial}`, title: `Habit ${serial}`, log: {}, showCounter: false, ...over };
}

/** Ids for records a function makes: "id-1", "id-2", … */
export function ids(prefix = 'id'): () => string {
  let n = 0;
  return () => `${prefix}-${++n}`;
}
