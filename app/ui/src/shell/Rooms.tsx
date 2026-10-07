// The four rooms inside the case (docs/SPEC.md 2.3). Each is built once and
// lives until you quit: a room that isn't current is hidden, never
// unmounted, so switching back finds its scroll, selection, open sheet and
// half-typed text where you left them. A room change is a 140 ms
// cross-fade, and a cut under Reduce Motion (shell.css).
//
// Until their stages fill them, each room is its register's ground and its
// empty line.

import type { ReactNode, Ref } from 'react';
import { HeatView, type HeatHandle, type HeatProps } from '../heat/HeatView';
import { REGISTERS, ROOM_NAMES, ROOMS, type RoomId } from './rooms';

interface Props {
  current: RoomId;
  /** The task the strip's task half opened Heat on. */
  heatTask: string | null;
  /** Heat's own: the task to open, the keyboard handle, and what it tells the status bar. */
  heat: Pick<HeatProps, 'open' | 'onStatus' | 'onSettings'> & { handle: Ref<HeatHandle> };
}

export function Rooms({ current, heatTask, heat }: Props) {
  return (
    <main className="rooms">
      {ROOMS.map((id) => (
        <section
          key={id}
          className={`room register-${REGISTERS[id]}`}
          data-room={id}
          data-current={id === current}
          data-selected-task={id === 'heat' ? (heatTask ?? undefined) : undefined}
          aria-label={ROOM_NAMES[id]}
          aria-hidden={id !== current}
          inert={id !== current}
        >
          {id === 'heat' ? (
            <HeatView ref={heat.handle} open={heat.open} onStatus={heat.onStatus} onSettings={heat.onSettings} />
          ) : (
            BODIES[id]
          )}
        </section>
      ))}
    </main>
  );
}

const EMPTY = 'Nothing here right now.';

const BODIES: Record<Exclude<RoomId, 'heat'>, ReactNode> = {
  space: (
    <div className="room-empty">
      <p className="night-line">{EMPTY}</p>
    </div>
  ),
  console: (
    <div className="room-empty">
      <p className="dmg-screen">{EMPTY.toUpperCase()}</p>
    </div>
  ),
  unquantized: (
    <div className="room-empty">
      <p className="paper-tag">{EMPTY}</p>
    </div>
  ),
};
