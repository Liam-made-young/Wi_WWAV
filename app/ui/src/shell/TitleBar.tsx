// The case's title bar (docs/SPEC.md 2.1): 52 pt of brushed metal in every
// room, light and dark, holding the traffic lights, the room switcher, the
// Now strip, the search pill and the astronaut chip. The case never
// changes, so the controls never move.

import type { ReactNode } from 'react';
import { keys } from './platform';
import { ROOM_NAMES, ROOMS, type RoomId } from './rooms';

interface Props {
  room: RoomId;
  narrow: boolean;
  strip: ReactNode;
  menuOpen: boolean;
  onRoom(room: RoomId): void;
  onSearch(): void;
  onMenu(): void;
}

export function TitleBar({ room, narrow, strip, menuOpen, onRoom, onSearch, onMenu }: Props) {
  return (
    <header className="case-bar" data-tauri-drag-region>
      {/* The window's own traffic lights sit here, drawn by the system. */}
      <div className="traffic" data-tauri-drag-region />
      <div className="switcher" role="tablist" aria-label="Rooms">
        {ROOMS.map((id, i) => (
          <button
            key={id}
            type="button"
            role="tab"
            className="segment"
            aria-selected={id === room}
            title={`${ROOM_NAMES[id]} (${keys(`⌘${i + 1}`)})`}
            onClick={() => onRoom(id)}
          >
            {ROOM_NAMES[id]}
          </button>
        ))}
      </div>
      <div className="case-gap" data-tauri-drag-region />
      {strip}
      <div className="case-gap" data-tauri-drag-region />
      <button
        type="button"
        className={narrow ? 'magnifier' : 'search-pill'}
        onClick={onSearch}
        aria-label={`Search (${keys('⌘K')})`}
      >
        <Magnifier />
        {!narrow && <span className="search-word">Search</span>}
        {!narrow && <span className="search-keys">{keys('⌘K')}</span>}
      </button>
      <button
        type="button"
        className="astronaut-chip"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        aria-label="You"
        onClick={onMenu}
      >
        <Astronaut />
      </button>
    </header>
  );
}

function Magnifier() {
  return (
    <svg className="glyph" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
      <circle cx="5.75" cy="5.75" r="4.25" fill="none" stroke="currentColor" strokeWidth="1.5" />
      <path d="M9 9l3.75 3.75" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}

// The starter astronaut's helmet until the account carries its own: the
// astronaut stores ids, never art (2.4), and Space draws the real one.
function Astronaut() {
  return (
    <svg className="portrait" width="28" height="28" viewBox="0 0 28 28" aria-hidden="true">
      <circle cx="14" cy="14" r="13" className="portrait-helmet" />
      <rect x="5" y="9" width="18" height="11" rx="5.5" className="portrait-visor" />
      <circle cx="9.5" cy="12" r="1.25" className="portrait-glint" />
    </svg>
  );
}

export interface MenuItem {
  label: string;
  run(): void;
}

/** The astronaut chip's menu: "Your galaxy", "Your shelf", "Settings…", "Export everything…", "Sign out". */
export function AstronautMenu({ items }: { items: MenuItem[] }) {
  return (
    <div className="chip-menu" role="menu" aria-label="You">
      {items.map((item) => (
        <button key={item.label} type="button" role="menuitem" className="menu-item" data-dense onClick={item.run}>
          {item.label}
        </button>
      ))}
    </div>
  );
}
