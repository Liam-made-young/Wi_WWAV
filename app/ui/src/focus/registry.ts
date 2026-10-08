// The view registry (docs/FOCUS.md): every tool Learn can summon, in one
// list. The number keys, the edge reveal, the ⌘ map and ⌘K all read it, so a
// view registered here is in every one of them with nothing else to edit.
//
//   registerView({ id: 'notes', title: 'Notes', icon: <path d="…" />, component: Notes });
//
// A view is mounted inside Learn's frame (heat/HeatView.tsx), so the frame's
// hooks work in it: useTabActs, useTabKeys, useSelection, useSidebarSlot.
// The six first tabs register themselves from TAB_IDS, TAB_TABLE and
// HEAT_TABS (focus/builtin.tsx); a tab added to those tables is registered
// with them.

import { type ComponentType, type ReactNode, useSyncExternalStore } from 'react';

export interface ViewDef {
  /** Unique, and the frame's tab id. */
  id: string;
  title: string;
  /** The number key, 1 to 9. Left out, or taken already, it gets the next free one; past 9 a view has no key. */
  shortcut?: number;
  /** What goes inside a 20 × 20 svg, drawn in currentColor. */
  icon?: ReactNode;
  component: ComponentType;
  /** Whether it uses the left sidebar (the spaces filter, its own sections). The sidebar shows only then. */
  sidebar?: boolean;
  /** What its "+" is called ("New task"), or null for none. */
  plus?: string | null;
  /** Its one secondary act, on ⇧Return, or null for none. */
  secondary?: string | null;
  /** The interrupt sources whose line opens this view when it names no other. */
  interrupts?: readonly string[];
}

export interface View extends Required<Omit<ViewDef, 'shortcut' | 'icon'>> {
  shortcut: number | null;
  icon: ReactNode;
}

const defs = new Map<string, ViewDef>();
const listeners = new Set<() => void>();
let list: readonly View[] = [];

/** Keys go to whoever asks for one first, in the order views registered; the rest take what is free. */
function settle() {
  const taken = new Map<number, string>();
  const wants = [...defs.values()];
  for (const d of wants) {
    const n = d.shortcut;
    if (n !== undefined && Number.isInteger(n) && n >= 1 && n <= 9 && !taken.has(n)) taken.set(n, d.id);
  }
  const keyOf = new Map([...taken].map(([n, id]) => [id, n]));
  let free = 1;
  for (const d of wants) {
    if (keyOf.has(d.id)) continue;
    while (taken.has(free)) free += 1;
    if (free > 9) break;
    taken.set(free, d.id);
    keyOf.set(d.id, free);
  }
  list = wants
    .map<View>((d) => ({
      id: d.id,
      title: d.title,
      shortcut: keyOf.get(d.id) ?? null,
      icon: d.icon ?? null,
      component: d.component,
      sidebar: d.sidebar ?? false,
      plus: d.plus ?? null,
      secondary: d.secondary ?? null,
      interrupts: d.interrupts ?? [],
    }))
    .sort((a, b) => (a.shortcut ?? 99) - (b.shortcut ?? 99));
  for (const l of listeners) l();
}

/** Adds a view, or replaces the one with its id. Answers how to take it out again. */
export function registerView(def: ViewDef): () => void {
  defs.set(def.id, def);
  settle();
  return () => {
    if (defs.get(def.id) !== def) return;
    defs.delete(def.id);
    settle();
  };
}

/** Every view, in the order of their keys. */
export function allViews(): readonly View[] {
  return list;
}

export function viewById(id: string): View | undefined {
  return list.find((v) => v.id === id);
}

export function viewByShortcut(n: number): View | undefined {
  return list.find((v) => v.shortcut === n);
}

/** The view an interrupt from `source` opens. */
export function viewForInterrupt(source: string): View | undefined {
  return list.find((v) => v.interrupts.includes(source));
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The registry as state: a component that lists the tools redraws when one registers. */
export function useViews(): readonly View[] {
  return useSyncExternalStore(subscribe, allViews, allViews);
}
