// The Focus layout's top bar (docs/FOCUS.md): the readout and nothing else.
// The Learn / Space / Console switch is put away; it comes down when the
// cursor touches the window's top edge, when the keyboard reaches it, or it
// is skipped altogether with ⌥1, ⌥2 and ⌥3. The bar is the window's handle,
// as the title bar was.

import { useEffect, useRef, useState } from 'react';
import { keys } from '../shell/platform';
import { ROOM_NAMES, ROOMS, type RoomId } from '../shell/rooms';
import { Readout } from './Readout';
import type { ReadoutText } from './words';

interface Props {
  room: RoomId;
  text: ReadoutText;
  /** Something is loaded in the player: the switch offers the way to it. */
  playing: boolean;
  onRoom(room: RoomId): void;
  onPlayer(): void;
  onSettings(): void;
}

/** How long the switch stays once the cursor has left the bar. */
const LINGER = 400;

export function TopBar({ room, text, playing, onRoom, onPlayer, onSettings }: Props) {
  const [revealed, setRevealed] = useState(false);
  const leaving = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(leaving.current), []);

  const stay = () => clearTimeout(leaving.current);
  const leave = () => {
    clearTimeout(leaving.current);
    leaving.current = setTimeout(() => setRevealed(false), LINGER);
  };
  const pick = (run: () => void) => () => {
    run();
    setRevealed(false);
  };

  return (
    <header className="top-bar" data-revealed={revealed} data-tauri-drag-region onMouseEnter={stay} onMouseLeave={leave}>
      <div className="traffic" data-tauri-drag-region />
      <Readout text={text} />
      <span className="top-edge" aria-hidden="true" onMouseEnter={() => setRevealed(true)} />
      <nav className="top-switch" aria-label="Views">
        {ROOMS.map((id, i) => (
          <button
            key={id}
            type="button"
            className="prism-plain"
            aria-current={id === room ? 'page' : undefined}
            title={`${ROOM_NAMES[id]} (${keys(`⌥${i + 1}`)})`}
            onClick={pick(() => onRoom(id))}
          >
            {ROOM_NAMES[id]}
          </button>
        ))}
        <span className="top-switch-gap" />
        {playing && (
          <button type="button" className="prism-plain" onClick={pick(onPlayer)}>
            Player
          </button>
        )}
        <button type="button" className="prism-plain" onClick={pick(onSettings)}>
          Settings
        </button>
      </nav>
    </header>
  );
}
