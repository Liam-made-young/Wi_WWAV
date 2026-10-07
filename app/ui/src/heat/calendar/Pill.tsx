// A task due on a day (docs/SPEC.md 3.7): a pill with a 3 px left border in
// its heat colour. It selects the task on a click and can be dragged to
// another day to schedule it. A recurring task's occurrence says ↻.

import { clock } from '../fmt';
import type { HeatLevel } from '../client';
import { dragTask } from '../ui';
import type { DueItem } from './items';

const LEVEL: Record<HeatLevel, string> = { Overdue: 'overdue', Hot: 'hot', Warm: 'warm', Cool: 'cool', Done: 'done' };

export function Pill({ item, selected, onSelect }: { item: DueItem; selected: boolean; onSelect(): void }) {
  return (
    <button
      type="button"
      className="heat-pill"
      data-dense
      data-level={item.level ? LEVEL[item.level] : 'none'}
      data-done={item.done}
      data-task={item.taskId}
      aria-pressed={selected}
      aria-label={`${item.title}, due ${clock(item.minute)}${item.done ? ', done' : ''}${item.repeats ? ', repeats' : ''}`}
      title={`${item.title}, due ${clock(item.minute)}`}
      {...dragTask(item.taskId)}
      onClick={(e) => {
        e.stopPropagation();
        onSelect();
      }}
    >
      {item.done ? '✓ ' : ''}
      {item.repeats ? '↻ ' : ''}
      {item.title}
    </button>
  );
}
