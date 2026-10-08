// The live page (docs/SPACE.md 11): one web view of its own, which the app's
// shell lays over the sky where a body's page is (app/src-tauri
// space_pages.rs). Only the Mac app has it; in a browser the sky still flies
// and a body's page stays a dark screen.

import { call, on } from '../bridge';
import type { Rect } from './flight/model';

/** Whether pages can open here: the UI is running inside the app. */
export const IN_APP: boolean = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** Where the page goes: a rectangle in the window, and how much its content is scaled. */
export interface Place extends Rect {
  zoom: number;
}

/** What comes back about the page. `said` is whatever the page itself wrote; trust none of it. */
export type Heard =
  | { what: 'loading' | 'loaded'; url: string }
  | { what: 'said'; said: Record<string, unknown> };

const quiet = (p: Promise<unknown>) => void p.catch(() => {});

let moving = false;
let next: Record<string, unknown> | null = null;

function move() {
  if (moving || !next) return;
  const args = next;
  next = null;
  moving = true;
  call('space.page.place', args)
    .catch(() => {})
    .finally(() => {
      moving = false;
      move();
    });
}

export const pages = {
  open(url: string, place: Place): void {
    if (IN_APP) quiet(call('space.page.open', { url, ...place }));
  },
  /** Follows the body. Asked every frame, sent as fast as the shell answers, always the newest. */
  place(place: Place | null): void {
    if (!IN_APP) return;
    next = place ? { ...place, shown: true } : { shown: false };
    move();
  },
  dock(docked: boolean): void {
    if (IN_APP) quiet(call('space.page.dock', { docked }));
  },
  close(): void {
    if (!IN_APP) return;
    next = null;
    quiet(call('space.page.close'));
  },
  back: () => IN_APP && quiet(call('space.page.back')),
  forward: () => IN_APP && quiet(call('space.page.forward')),
  reload: () => IN_APP && quiet(call('space.page.reload')),
  heard(handler: (heard: Heard) => void): () => void {
    return on('space.page', (payload) => handler(payload as Heard));
  },
};
