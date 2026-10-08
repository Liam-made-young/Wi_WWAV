// What this Mac keeps about the Focus layout (docs/FOCUS.md): which layout
// is on, whether the hint still shows, and whether a click sounds. They are
// kept in the web view's own storage, as first launch is: they belong to
// this Mac, and the Settings window (another window on the same storage)
// changes them and is heard here.

import { useSyncExternalStore } from 'react';
import { CONFIG } from './model';

export type Layout = 'focus' | 'classic';

const LAYOUT = 'wi.layout';
const HINT = 'wi.focusHint';
const CLICKS = 'wi.clicks';

const listeners = new Set<() => void>();

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Private storage refused: the choice lasts until the window closes.
  }
  for (const l of listeners) l();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  window.addEventListener('storage', listener);
  return () => {
    listeners.delete(listener);
    window.removeEventListener('storage', listener);
  };
}

/** Focus unless Classic was chosen. */
export function getLayout(): Layout {
  return read(LAYOUT) === 'classic' ? 'classic' : 'focus';
}

export function setLayout(layout: Layout) {
  write(LAYOUT, layout);
}

export function useLayout(): Layout {
  return useSyncExternalStore(subscribe, getLayout, getLayout);
}

// ---- the hint ---------------------------------------------------------------

interface Hint {
  /** When the layout was first seen, epoch ms. */
  first: number;
  launches: number;
}

function readHint(): Hint | null {
  try {
    const h = JSON.parse(read(HINT) ?? 'null') as Hint | null;
    return h && typeof h.first === 'number' && typeof h.launches === 'number' ? h : null;
  } catch {
    return null;
  }
}

let counted = false;

/** Counts this launch, once per page. */
export function countLaunch(now = Date.now()) {
  if (counted) return;
  counted = true;
  const h = readHint() ?? { first: now, launches: 0 };
  write(HINT, JSON.stringify({ first: h.first, launches: h.launches + 1 }));
}

/** Whether the hint still shows: for the first days or launches, whichever ends first. */
export function hintShows(now = Date.now(), cfg = CONFIG): boolean {
  const h = readHint();
  if (!h) return true;
  return h.launches <= cfg.hintLaunches && now - h.first < cfg.hintDays * 24 * 60 * 60 * 1000;
}

let hintText = '';
function hintSnapshot(): string {
  // A string, so the store's value is the same until it changes.
  hintText = hintShows() ? 'on' : 'off';
  return hintText;
}

export function useHint(): boolean {
  return useSyncExternalStore(subscribe, hintSnapshot, hintSnapshot) === 'on';
}

/** Settings → Appearance: the hint shows again, for as long as it did the first time. */
export function resetHint(now = Date.now()) {
  write(HINT, JSON.stringify({ first: now, launches: 1 }));
}

// ---- click sounds -----------------------------------------------------------

/** Off until switched on. */
export function clicksOn(): boolean {
  return read(CLICKS) === 'on';
}

export function setClicks(on: boolean) {
  write(CLICKS, on ? 'on' : null);
}

export function useClicks(): boolean {
  return useSyncExternalStore(subscribe, clicksOn, clicksOn);
}
