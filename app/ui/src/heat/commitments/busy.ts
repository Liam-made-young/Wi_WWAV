// What P plans around besides blocks and other calendars (docs/COMMITMENTS.md,
// "Planning"): every commitment of a day from the start of its travel to the
// end of it, and sleep. They are handed to today/gap.ts as timed events, the
// shape it already keeps clear of, so the next free gap never lands in a
// class, a shift, a buffer or the night.

import { atMinute, type DayKey } from '../../shared/time/zone';
import type { CalendarEvent, CommitmentsSnapshot } from '../client';

const DAY = 1440;
const clamp = (min: number) => Math.min(DAY, Math.max(0, min));

/** The minutes asleep on any one day: to bed after midnight is one stretch, before it is two. */
export function sleepSpans(sleep: { from: number; to: number } | undefined): [number, number][] {
  if (!sleep || sleep.from === sleep.to) return [];
  const { from, to } = sleep;
  const spans: [number, number][] =
    from > to
      ? [
          [0, to],
          [from, DAY],
        ]
      : [[from, to]];
  return spans.filter(([a, b]) => b > a);
}

/** `date`'s commitments (with their travel) and sleep, as events `nextGap` reads beside the snapshot's own. */
export function busyEvents(commitments: CommitmentsSnapshot | undefined, date: DayKey, tz: string): CalendarEvent[] {
  if (!commitments) return [];
  const event = (id: string, title: string, from: number, to: number): CalendarEvent => ({
    id,
    title,
    start: atMinute(date, from, tz),
    end: atMinute(date, to, tz),
    allDay: false,
  });
  const held = (commitments.days[date] ?? []).flatMap((o) => {
    const from = clamp(o.start - o.bufferBefore);
    const to = clamp(o.end + o.bufferAfter);
    return to > from ? [event(`commitment-${o.commitmentId}-${o.start}`, o.title, from, to)] : [];
  });
  const asleep = sleepSpans(commitments.sleep).map(([from, to]) => event(`sleep-${from}`, 'Sleep', from, to));
  return [...held, ...asleep];
}
