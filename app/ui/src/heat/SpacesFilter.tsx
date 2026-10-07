// The sidebar's spaces filter (docs/SPEC.md 3.4): All, then each space with
// its hue dot and its open count, then "New space…". A space filters Today,
// Tasks, Calendar and Mail; Grades, Habits and calendar events belong to the
// person rather than a project, so they ignore it and the filter says so.

import type { CSSProperties } from 'react';
import { useFrame } from './frame';
import { copy } from './fmt';
import { useHeat } from './store';

const IGNORES: Partial<Record<string, string>> = {
  grades: 'Grades ignore spaces.',
  habits: 'Habits ignore spaces.',
};

export function SpacesFilter({ onNewSpace }: { onNewSpace(): void }) {
  const { snap, idx } = useHeat();
  const { spaceId, setSpace, tab } = useFrame();
  const open = snap?.derived.lists.allOpen ?? [];
  const counts = new Map<string, number>();
  for (const id of open) {
    const space = idx.task.get(id)?.spaceId;
    if (space) counts.set(space, (counts.get(space) ?? 0) + 1);
  }
  const why = IGNORES[tab];

  return (
    <nav className="heat-spaces" aria-label="Spaces">
      <h2 className="heat-side-heading" data-text="secondary">
        Spaces
      </h2>
      <Row label={copy.tasks.all} count={open.length} on={spaceId === null} off={!!why} onPick={() => setSpace(null)} />
      {(snap?.records.space ?? []).map((s) => (
        <Row
          key={s.id}
          label={s.name}
          hue={s.hue}
          count={counts.get(s.id) ?? 0}
          on={spaceId === s.id}
          off={!!why}
          onPick={() => setSpace(s.id)}
        />
      ))}
      <button type="button" className="heat-side-row heat-side-new" data-dense onClick={onNewSpace}>
        {copy.tasks.newSpace}
      </button>
      {why && (
        <p className="heat-side-why" data-text="secondary">
          {why}
        </p>
      )}
    </nav>
  );
}

function Row({
  label,
  hue,
  count,
  on,
  off,
  onPick,
}: {
  label: string;
  hue?: number;
  count: number;
  on: boolean;
  off: boolean;
  onPick(): void;
}) {
  return (
    <button
      type="button"
      className="heat-side-row"
      data-dense
      aria-current={on && !off ? 'true' : undefined}
      aria-disabled={off || undefined}
      onClick={() => !off && onPick()}
    >
      {hue !== undefined && <span className="heat-dot" style={{ '--hue': hue } as CSSProperties} aria-hidden="true" />}
      <span className="heat-side-name">{label}</span>
      <span className="heat-side-count" data-text="secondary" aria-label={`${count} open`}>
        {count}
      </span>
    </button>
  );
}
