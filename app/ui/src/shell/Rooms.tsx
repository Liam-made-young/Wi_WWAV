// The four rooms inside the case (docs/SPEC.md 2.3). Each is built once and
// lives until you quit: a room that isn't current is hidden, never
// unmounted, so switching back finds its scroll, selection, open sheet and
// half-typed text where you left them. A room change is a 140 ms
// cross-fade, and a cut under Reduce Motion (shell.css).
//
// Until their stages fill them, each room is its register's ground and its
// empty line.

import type { ReactNode } from 'react';
import { REGISTERS, ROOM_NAMES, ROOMS, type RoomId } from './rooms';

interface Props {
  current: RoomId;
  /** The task the strip's task half opened Heat on. */
  heatTask: string | null;
}

export function Rooms({ current, heatTask }: Props) {
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
          {BODIES[id]}
        </section>
      ))}
    </main>
  );
}

const EMPTY = 'Nothing here right now.';

const BODIES: Record<RoomId, ReactNode> = {
  heat: (
    <div className="room-empty">
      <h1 className="heat-heading">Today</h1>
      <p>{EMPTY}</p>
    </div>
  ),
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
