// Small pieces every Heat tab draws the same way: the heat tube, a space's
// dot, a dragged task.

import type { CSSProperties, DragEvent } from 'react';
import type { HeatLevel, Id } from './client';
import { TASK_DRAG } from './frame';

const FILL: Record<HeatLevel, string> = { Overdue: 'overdue', Hot: 'hot', Warm: 'warm', Cool: 'cool', Done: 'done' };

/** The badge: a 46 × 11 glossy tube filled to `v`, with the level in words beside it, because colour is never alone. */
export function Heat({ level, v, word = true }: { level: HeatLevel; v: number | null; word?: boolean }) {
  const fill = v === null ? 0 : Math.min(1, v);
  return (
    <span className="heat-badge" data-level={FILL[level]}>
      <span className="heat-tube" aria-hidden="true">
        <span className="heat-tube-fill" style={{ '--v': fill } as CSSProperties} />
      </span>
      {word && <span className="heat-level">{level}</span>}
    </span>
  );
}

export function SpaceDot({ hue }: { hue: number | undefined }) {
  return <span className="heat-dot" style={{ '--hue': hue ?? 0 } as CSSProperties} aria-hidden="true" />;
}

/** What a row or pill needs to be dragged somewhere: its task's id in the drag. */
export function dragTask(taskId: Id) {
  return {
    draggable: true,
    onDragStart: (e: DragEvent) => {
      e.dataTransfer.setData(TASK_DRAG, taskId);
      e.dataTransfer.setData('text/plain', taskId);
      e.dataTransfer.effectAllowed = 'copyMove';
    },
  };
}

/** The task a drag carries, or null if it carries none. */
export function draggedTask(e: DragEvent): Id | null {
  return e.dataTransfer.getData(TASK_DRAG) || null;
}

export function carriesTask(e: DragEvent): boolean {
  return e.dataTransfer.types.includes(TASK_DRAG);
}
