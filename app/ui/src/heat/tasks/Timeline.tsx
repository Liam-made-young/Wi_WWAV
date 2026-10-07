// The milestone timeline (docs/SPEC.md 3.1, 3.6): with a space of milestones
// selected it sits above the list: a blue groove filled to a red "Today"
// marker, with a 16 px glass bead for each milestone. A click on a bead
// filters the list to it; reaching a milestone fills its bead.

import type { CSSProperties } from 'react';
import { daysBetween } from '../../shared/time/zone';
import { shortMonthDay } from '../../shared/time/format';
import type { Id } from '../client';
import { useHeat } from '../store';

export function Timeline({ spaceId, picked, onPick }: { spaceId: Id; picked: Id | null; onPick(id: Id): void }) {
  const { snap, date } = useHeat();
  const beads = (snap?.records.milestone ?? []).filter((m) => m.spaceId === spaceId).sort((a, b) => a.order - b.order);
  if (beads.length === 0) return null;
  const first = beads.reduce((lo, m) => (m.date < lo ? m.date : lo), date);
  const last = beads.reduce((hi, m) => (m.date > hi ? m.date : hi), date);
  const span = Math.max(1, daysBetween(first, last));
  const at = (day: string) => `${(daysBetween(first, day) / span) * 100}%`;

  return (
    <div className="heat-timeline" aria-label="Milestones">
      <div className="heat-groove">
        <span className="heat-groove-fill" style={{ width: at(date) }} />
        <span className="heat-today-marker" style={{ left: at(date) }} data-text="secondary">
          Today
        </span>
        {beads.map((m) => (
          <button
            key={m.id}
            type="button"
            className="heat-bead"
            data-dense
            data-done={m.done}
            aria-pressed={picked === m.id}
            aria-label={`${m.title}, ${shortMonthDay(m.date)}${m.done ? ', reached' : ''}`}
            title={`${m.title}, ${shortMonthDay(m.date)}`}
            style={{ '--at': at(m.date) } as CSSProperties}
            onClick={() => onPick(m.id)}
          >
            <span className="heat-bead-glass" />
          </button>
        ))}
      </div>
    </div>
  );
}
