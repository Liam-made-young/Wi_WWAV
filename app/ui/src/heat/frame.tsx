// What a Heat tab can ask of Heat's frame (docs/SPEC.md 3.3), and the one
// table of the six tabs. A tab is a plain component in HEAT_TABS
// (heat/tabs.tsx) that reads the snapshot with useHeat() and tells the
// frame what it needs through these hooks:
//
//   useTabActs({ plus, secondary, count })   the tab's one "+" and one secondary act
//   useTabKeys({ move, edit, escape, key })  keys the tab answers while it is the current one
//   useSelection()                           the selected task or block (Get Info follows it)
//   useSpaceFilter()                         the sidebar's space, or null for All
//   useSidebarSlot()                         where the tab may portal its own sidebar sections
//   useSheets()                              the frame's sheets: a new task, "Time it took", and a tab's own
//   useDraft(key, initial)                   a field's text that Esc keeps for the next time its sheet opens
//
// Every tab stays mounted while another is showing, so its scroll, selection
// and half-typed text are where you left them (2.3 applies to Heat's tabs
// as it does to its rooms).

import {
  createContext,
  type MutableRefObject,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useReducer,
  useRef,
} from 'react';
import type { DayKey } from '../shared/time/zone';
import type { Id } from './client';

/** What a dragged task carries in its dataTransfer: its id. */
export const TASK_DRAG = 'application/x-heat-task';

export type TabId = 'today' | 'tasks' | 'calendar' | 'grades' | 'habits' | 'mail' | 'database' | 'wiki';

/**
 * The tabs in order, with their keys (3.17: 1-6, with no field focused). The
 * Database and the Wiki follow the first six, on 7 and 8 (docs/ASK.md).
 */
export const TAB_IDS: readonly TabId[] = ['today', 'tasks', 'calendar', 'grades', 'habits', 'mail', 'database', 'wiki'];

/**
 * 3.3's table: what "+" adds (its tooltip is "New task", never "Add", 7.5)
 * and the tab's one secondary act, which ⇧Return runs and the status bar
 * names. Mail has no "+".
 */
export const TAB_TABLE: Record<TabId, { name: string; plus: string | null; adds: string | null; secondary: string }> = {
  today: { name: 'Today', plus: 'New task', adds: 'A task straight into the plan', secondary: 'Plan my day' },
  tasks: { name: 'Tasks', plus: 'New task', adds: 'A task', secondary: 'Triage inbox' },
  calendar: {
    name: 'Calendar',
    plus: 'New task',
    adds: 'A task due 11:59 PM on the selected day',
    secondary: 'Today',
  },
  grades: { name: 'Grades', plus: 'New grade', adds: 'A grade', secondary: 'Add course' },
  habits: { name: 'Habits', plus: 'New habit', adds: 'A habit', secondary: 'Show the year' },
  mail: { name: 'Mail', plus: null, adds: null, secondary: 'Open in Gmail' },
  database: { name: 'Database', plus: 'New row', adds: 'A row in this table', secondary: 'Fill down' },
  wiki: { name: 'Wiki', plus: null, adds: null, secondary: 'Search Wikipedia' },
};

export interface Act {
  run(): void;
  /** Why it can't run now. The control is disabled and says so: "Habit limit reached". */
  disabled?: string | null;
  /** Leaves the act out altogether, as Tasks does with "Triage inbox" while the inbox is empty. */
  hidden?: boolean;
}

export interface TabActs {
  plus?: Act;
  secondary?: Act;
  /** The status bar's count for this tab: "12 tasks". */
  count?: string | null;
}

/** A key as the tab hears it. */
export interface HeatKey {
  key: string;
  code: string;
  shift: boolean;
  /** ⌘ on a Mac, Ctrl elsewhere. */
  command: boolean;
  alt: boolean;
  repeat: boolean;
}

export interface TabKeys {
  /** ↑ ↓ */
  move?(by: 1 | -1): void;
  /** Return on the selection. */
  edit?(): void;
  /** Esc, first: say whether it did anything (closing a popover, leaving a field). */
  escape?(): boolean;
  /** ⌘F: put the keyboard in the tab's filter. */
  filter?(): void;
  /** Any key the frame didn't take: say whether the tab did. */
  key?(e: HeatKey): boolean;
}

export interface Selection {
  taskId: Id | null;
  blockId: Id | null;
}

export interface NewTaskOptions {
  title?: string;
  notes?: string;
  due?: number;
  scheduledDate?: DayKey;
  /** Today's "+": the new task goes straight into the plan, in the next free gap. */
  intoPlan?: boolean;
  spaceId?: Id;
}

export interface FrameApi {
  tab: TabId;
  setTab(tab: TabId): void;
  spaceId: Id | null;
  setSpace(id: Id | null): void;
  selection: Selection;
  selectTask(id: Id | null): void;
  selectBlock(id: Id | null): void;
  sidebar: HTMLElement | null;
  /** The frame's own sheets. */
  newTask(options?: NewTaskOptions): void;
  /** Checking off a task with no logged time asks "Time it took" first (3.5). */
  askTook(taskId: Id): void;
  /** Return and ⌘I: the keyboard goes to Get Info's first field, if something is selected. */
  focusInfo(): void;
  /** A tab's own sheet: the frame drops it from the top over its backdrop, and Esc closes it (2.7). */
  openSheet(sheet: ReactNode): void;
  closeSheet(): void;
  /** Whether a sheet is down now, so nothing drops a second one over what is being typed. */
  sheetOpen: boolean;
  /** What sheets' fields held when Esc closed them (see useDraft). */
  drafts: Map<string, unknown>;
  /** The focus length the LCD offers when idle: 25, 50 or a custom 10-90. */
  focusLength: number;
  setFocusLength(minutes: number): void;
  /** Frame-level state the toolbar draws, refreshed when a tab's acts change. */
  registerActs(tab: TabId, acts: MutableRefObject<TabActs>): () => void;
  registerKeys(tab: TabId, keys: MutableRefObject<TabKeys>): () => void;
  touch(): void;
}

export const FrameContext = createContext<FrameApi | null>(null);
export const ScopeContext = createContext<{ tab: TabId; active: boolean } | null>(null);

export function useFrame(): FrameApi {
  const frame = useContext(FrameContext);
  if (!frame) throw new Error('A Learn tab lives inside HeatView.');
  return frame;
}

/** Which tab this component is in, and whether that tab is the one showing. */
export function useTabScope(): { tab: TabId; active: boolean } {
  const scope = useContext(ScopeContext);
  if (!scope) throw new Error('A Learn tab lives inside HeatView.');
  return scope;
}

/** Tells the frame this tab's "+", secondary act and status-bar count. */
export function useTabActs(acts: TabActs): void {
  const frame = useFrame();
  const { tab } = useTabScope();
  const ref = useRef(acts);
  ref.current = acts;
  useLayoutEffect(() => frame.registerActs(tab, ref), [frame.registerActs, tab]);
  // The frame redraws its toolbar only when what it shows changes, not when a handler's identity does.
  const shown = [
    acts.plus ? `+${acts.plus.disabled ?? ''}${acts.plus.hidden ? 'h' : ''}` : '-',
    acts.secondary ? `s${acts.secondary.disabled ?? ''}${acts.secondary.hidden ? 'h' : ''}` : '-',
    acts.count ?? '',
  ].join('|');
  useEffect(() => frame.touch(), [shown, frame.touch]);
}

/** The keys this tab answers while it is the one showing. */
export function useTabKeys(keys: TabKeys): void {
  const frame = useFrame();
  const { tab } = useTabScope();
  const ref = useRef(keys);
  ref.current = keys;
  useLayoutEffect(() => frame.registerKeys(tab, ref), [frame.registerKeys, tab]);
}

export function useSelection() {
  const { selection, selectTask, selectBlock } = useFrame();
  return { ...selection, selectTask, selectBlock };
}

export function useSpaceFilter(): { spaceId: Id | null; setSpace(id: Id | null): void } {
  const { spaceId, setSpace } = useFrame();
  return { spaceId, setSpace };
}

/** The element under the spaces filter where the showing tab may portal its sidebar sections. */
export function useSidebarSlot(): HTMLElement | null {
  const { sidebar } = useFrame();
  const { active } = useTabScope();
  return active ? sidebar : null;
}

export function useSheets() {
  const { newTask, askTook, openSheet, closeSheet, sheetOpen } = useFrame();
  return { newTask, askTook, openSheet, closeSheet, sheetOpen };
}

/**
 * Text that outlives its sheet: Esc on a sheet keeps what was typed for the
 * next time it opens (2.7). `clear` once it is saved.
 */
export function useDraft<T>(key: string, initial: T): [T, (value: T) => void, () => void] {
  const { drafts } = useFrame();
  const [, redraw] = useReducer((n: number) => n + 1, 0);
  const set = useCallback(
    (value: T) => {
      drafts.set(key, value);
      redraw();
    },
    [drafts, key],
  );
  const clear = useCallback(() => {
    drafts.delete(key);
    redraw();
  }, [drafts, key]);
  return [(drafts.has(key) ? drafts.get(key) : initial) as T, set, clear];
}

/** The tab the other tabs can jump to: Mail's widget opens Mail. */
export function useTabs() {
  const { tab, setTab } = useFrame();
  return { tab, setTab };
}

export function Scope({ tab, active, children }: { tab: TabId; active: boolean; children: ReactNode }) {
  return <ScopeContext.Provider value={{ tab, active }}>{children}</ScopeContext.Provider>;
}
