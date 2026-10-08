// The case's title bar (docs/SPEC.md 2.1): 52 pt of brushed metal in every
// view, light and dark, holding the traffic lights, the view switcher, the
// Now strip, the prompt box's pill and the galaxy chip. The case never changes, so
// the controls never move.

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
      <div className="switcher" role="tablist" aria-label="Views">
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
        aria-label={`Ask Claude or search (${keys('⌘K')})`}
      >
        <Magnifier />
        {!narrow && <span className="search-word">Ask or search</span>}
        {!narrow && <span className="search-keys">{keys('⌘K')}</span>}
      </button>
      <button
        type="button"
        className="galaxy-chip"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        aria-label="You"
        onClick={onMenu}
      >
        <Galaxy />
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

// A 28 pt miniature of your own galaxy (2.1): the night, a sun, two orbits and
// a few worlds. Until the account has a galaxy to draw (Space isn't in this
// build yet) it is the starter one: the same sky for everyone, in the night
// register's own tokens. Space draws the real one from the galaxy's `skySeed`.
const STARTER_WORLDS = [
  { orbit: 0, angle: 200 },
  { orbit: 1, angle: 40 },
  { orbit: 1, angle: 280 },
] as const;
const ORBITS = [6.5, 10.5];

function Galaxy() {
  return (
    <span className="galaxy" aria-hidden="true">
      <svg width="28" height="28" viewBox="0 0 28 28">
        <circle cx="14" cy="14" r="13.5" className="galaxy-ground" />
        {ORBITS.map((r) => (
          <circle key={r} cx="14" cy="14" r={r} className="galaxy-orbit" />
        ))}
        {STARTER_WORLDS.map(({ orbit, angle }) => {
          const a = (angle * Math.PI) / 180;
          return (
            <circle
              key={`${orbit}-${angle}`}
              cx={(14 + ORBITS[orbit] * Math.cos(a)).toFixed(2)}
              cy={(14 + ORBITS[orbit] * Math.sin(a)).toFixed(2)}
              r="1.4"
              className="galaxy-world"
            />
          );
        })}
      </svg>
      <span className="galaxy-sun" />
    </span>
  );
}

export interface MenuItem {
  label: string;
  run(): void;
}

/** The galaxy chip's menu: "Your galaxy", "Your public Heat view", "Settings…", "Export everything…", "Sign out". */
export function GalaxyMenu({ items }: { items: MenuItem[] }) {
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
