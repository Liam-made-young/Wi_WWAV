// The shell's lines to the core: its events as React state, and the media
// queries the case answers to.

import { useEffect, useRef, useState } from 'react';
import { call, on } from '../bridge';

/** Hears one core event for as long as the component is mounted. */
export function useCoreEvent<T>(event: string, handler: (payload: T) => void) {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => on(event, (p) => latest.current(p as T)), [event]);
}

export function useMedia(query: string): boolean {
  const [matches, setMatches] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const m = window.matchMedia(query);
    const changed = () => setMatches(m.matches);
    changed();
    m.addEventListener('change', changed);
    return () => m.removeEventListener('change', changed);
  }, [query]);
  return matches;
}

/**
 * Appearance, text size and Reduce Motion (2.13), from Settings and the
 * system, put on the page. Both windows wear them.
 */
export function useAppearance(settings: { appearance: string; textSize: number; reduceMotion: boolean } | null) {
  const systemReduce = useMedia('(prefers-reduced-motion: reduce)');
  const reduce = systemReduce || settings?.reduceMotion === true;
  useEffect(() => {
    const root = document.documentElement;
    if (!settings || settings.appearance === 'system') delete root.dataset.appearance;
    else root.dataset.appearance = settings.appearance;
    root.style.setProperty('--text-size', `${settings?.textSize ?? 13}px`);
    root.dataset.reduceMotion = String(reduce);
  }, [settings, reduce]);
}

/** Below 1180 pt the search pill is a magnifier and the strip 440 pt (2.1). */
export const useNarrow = () => useMedia('(max-width: 1179.98px)');

export interface Status {
  sync: string | null;
  save: string | null;
  upload: string | null;
}

/** The status bar's sentences, one per area, from `status` events (docs/COMMANDS.md). */
export function useStatus(): Status {
  const [status, setStatus] = useState<Status>({ sync: null, save: null, upload: null });
  useCoreEvent<{ area: string; sentence: string }>('status', ({ area, sentence }) => {
    if (area === 'sync' || area === 'save' || area === 'upload') setStatus((s) => ({ ...s, [area]: sentence }));
  });
  return status;
}

export interface Menu {
  undo: string | null;
  redo: string | null;
  cant: string | null;
}

export type UndoRoom = 'heat' | 'space' | 'console' | 'library';
const UNDO_ROOMS: UndoRoom[] = ['heat', 'space', 'console', 'library'];

/** Each view's undo and redo labels ("Undo move clip"), kept fresh by `history` events. */
export function useHistory(): Partial<Record<UndoRoom, Menu>> {
  const [menus, setMenus] = useState<Partial<Record<UndoRoom, Menu>>>({});
  useEffect(() => {
    for (const room of UNDO_ROOMS) {
      call<Menu>('history.get', { room }).then(
        (m) => setMenus((all) => ({ ...all, [room]: m })),
        () => {},
      );
    }
  }, []);
  useCoreEvent<Menu & { room: UndoRoom }>('history', (m) => setMenus((all) => ({ ...all, [m.room]: m })));
  return menus;
}
