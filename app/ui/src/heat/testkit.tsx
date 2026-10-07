// What Heat's component tests share: a seeded fake core at a fixed time, the
// provider and the view mounted on it, a window whose width the test sets,
// and a few helpers to press, click and wait. The fake answers the shapes
// docs/HEAT.md gives, so the views are tested without any Rust.

import { act, createRef, type ReactNode, type RefObject } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import type { ScreenStatus } from '../shell/StatusBar';
import { type HeatClient, heatClient } from './client';
import { createFake, type Fake } from './fake/all';
import { seed, type Seeded } from './fake/seed';
import { setEvents } from './fake/today';
import { type HeatHandle, HeatView } from './HeatView';
import { HeatProvider } from './store';
import { epochOf } from '../shared/time/zone';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

export const NY = 'America/New_York';
/** 7 Oct 2026, 10:00 AM in New York: the morning the founder wants Heat done. */
export const MORNING = epochOf({ year: 2026, month: 10, day: 7, hour: 10, minute: 0 }, NY);

/** A window `width` pt wide, for useMedia: below 1240 the right column folds. */
const listeners = new Set<() => void>();
let width = 1280;
export function setViewport(px: number) {
  width = px;
  window.matchMedia = ((query: string) => {
    const max = /max-width:\s*([\d.]+)px/.exec(query);
    const min = /min-width:\s*([\d.]+)px/.exec(query);
    const list = {
      media: query,
      get matches() {
        return (!max || width <= Number(max[1])) && (!min || width >= Number(min[1]));
      },
      addEventListener: (_: string, fn: () => void) => listeners.add(fn),
      removeEventListener: (_: string, fn: () => void) => listeners.delete(fn),
    };
    return list as unknown as MediaQueryList;
  }) as typeof window.matchMedia;
  for (const fn of [...listeners]) fn();
}

/** Lets a promise chain and the renders it causes settle. */
export async function settle(times = 4) {
  for (let i = 0; i < times; i++) {
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
  }
}

export interface Rig {
  fake: Fake;
  client: HeatClient;
  /** The fake core's own line, for the calls the shell makes (history.undo). */
  call<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  seeded: Seeded;
  host: HTMLElement;
  view: RefObject<HeatHandle | null>;
  /** What Heat last told the status bar. */
  status: () => ScreenStatus;
  opens: { settings: number };
  rerender(ui?: ReactNode): void;
  unmount(): void;
}

interface Options {
  /** Seeded records, or false for an empty library. */
  empty?: boolean;
  now?: number;
  width?: number;
  open?: { id: string; n: number } | null;
}

function boot(options: Options) {
  setViewport(options.width ?? 1280);
  const now = options.now ?? MORNING;
  const seeded = seed(now, NY);
  const { fake, transport } = createFake(options.empty ? {} : seeded.records, { now, zone: NY });
  if (!options.empty) {
    fake.state.currentTaskId = seeded.currentTaskId;
    setEvents(fake, seeded.events);
  }
  return { fake, transport, seeded, client: heatClient(transport) };
}

/** Anything that reads Heat's snapshot (the shell's strip, ⌘⇧N), mounted on a seeded fake core. */
export async function mountWith(children: ReactNode, options: Options = {}): Promise<Rig> {
  const { fake, transport, seeded, client } = boot(options);
  const host = document.createElement('div');
  document.body.append(host);
  const root: Root = createRoot(host);
  const view = createRef<HeatHandle>();
  const opens = { settings: 0 };
  const ui = (
    <HeatProvider client={client} clock={() => fake.now}>
      {children}
    </HeatProvider>
  );
  act(() => root.render(ui));
  await settle();
  return {
    fake,
    client,
    call: transport.call,
    seeded,
    host,
    view,
    status: () => ({ count: null, act: null }),
    opens,
    rerender: (next) => act(() => root.render(next ?? ui)),
    unmount() {
      act(() => root.unmount());
      host.remove();
    },
  };
}

/** Heat mounted on a seeded fake core, with its clock held still at `now` unless a test moves it. */
export async function mountHeat(options: Options = {}): Promise<Rig> {
  const { fake, transport, seeded, client } = boot(options);
  const host = document.createElement('div');
  document.body.append(host);
  const root: Root = createRoot(host);
  const view = createRef<HeatHandle>();
  let status: ScreenStatus = { count: null, act: null };
  const opens = { settings: 0 };
  const ui = (
    <HeatProvider client={client} clock={() => fake.now}>
      <HeatView
        ref={view}
        open={options.open ?? null}
        onStatus={(s) => (status = s)}
        onSettings={() => void opens.settings++}
      />
    </HeatProvider>
  );
  act(() => root.render(ui));
  await settle();
  return {
    fake,
    client,
    call: transport.call,
    seeded,
    host,
    view,
    status: () => status,
    opens,
    rerender: (next) => act(() => root.render(next ?? ui)),
    unmount() {
      act(() => root.unmount());
      host.remove();
    },
  };
}

/** A key press in the keyboard the shell hands Heat. Returns whether Heat took it. */
export async function press(rig: Rig, key: string, init: KeyboardEventInit = {}): Promise<boolean> {
  let took = false;
  await act(async () => {
    const e = new KeyboardEvent('keydown', { key, code: codeOf(key), bubbles: true, cancelable: true, ...init });
    took = rig.view.current!.key(e);
    await new Promise((r) => setTimeout(r, 0));
  });
  await settle();
  return took;
}

const codeOf = (key: string) =>
  key.length === 1 ? (/\d/.test(key) ? `Digit${key}` : `Key${key.toUpperCase()}`) : key === 'Enter' ? 'Enter' : key;

/** Esc, which the shell sends Heat as its own call when nothing is open over the room. */
export async function escape(rig: Rig) {
  await act(async () => {
    rig.view.current!.escape();
    await new Promise((r) => setTimeout(r, 0));
  });
  await settle();
}

export async function click(el: Element | null | undefined) {
  if (!el) throw new Error('Nothing to click.');
  await act(async () => {
    (el as HTMLElement).click();
    await new Promise((r) => setTimeout(r, 0));
  });
  await settle();
}

export const $ = (rig: Rig, selector: string) => rig.host.querySelector<HTMLElement>(selector);
export const $$ = (rig: Rig, selector: string) => [...rig.host.querySelectorAll<HTMLElement>(selector)];
export const byRole = (rig: Rig, role: string, name: string | RegExp) =>
  $$(rig, `[role="${role}"]`).find((el) => matches(el.getAttribute('aria-label') ?? el.textContent ?? '', name));
export const button = (rig: Rig, name: string | RegExp) =>
  $$(rig, 'button').find((el) => matches(el.getAttribute('aria-label') ?? el.textContent ?? '', name));

function matches(text: string, name: string | RegExp) {
  return typeof name === 'string' ? text.trim() === name : name.test(text.trim());
}

/** Types into a field the way a person does: React sees an input event with the new value. */
export async function type(el: Element | null | undefined, value: string) {
  if (!el) throw new Error('Nothing to type into.');
  const proto =
    el instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : el instanceof HTMLSelectElement
        ? HTMLSelectElement.prototype
        : HTMLInputElement.prototype;
  const set = Object.getOwnPropertyDescriptor(proto, 'value')!.set!;
  await act(async () => {
    (el as HTMLElement).focus();
    set.call(el, value);
    el.dispatchEvent(new Event(el instanceof HTMLSelectElement ? 'change' : 'input', { bubbles: true }));
  });
}

/** Lets real time pass, with the renders it causes. */
export const wait = (ms: number) =>
  act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
