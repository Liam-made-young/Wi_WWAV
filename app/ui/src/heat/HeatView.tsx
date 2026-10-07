// Heat, the profile view (docs/SPEC.md 3.3): a toolbar with "+", the six
// tabs as one segmented control and Sync; below it the 190 px sidebar with
// the spaces filter, the main view, and the 290 px right column of widgets,
// which folds into a 44 px strip of icons below 1240 pt. Selecting a task or
// a block slides Get Info over the right column; Esc slides it back.
//
// The frame reads `heat.snapshot` through the store and refetches on the
// `heat` event. It holds what every tab shares: the tab, the space, the
// selection, the sheets, and the keyboard the shell hands it. What each tab
// draws is in HEAT_TABS (heat/tabs.tsx).

import {
  forwardRef,
  type MutableRefObject,
  type ReactNode,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useReducer,
  useRef,
  useState,
} from 'react';
import { useMedia } from '../shell/hooks';
import { IS_MAC, keys } from '../shell/platform';
import type { ScreenStatus } from '../shell/StatusBar';
import { useActions } from './actions';
import type { Id } from './client';
import {
  type FrameApi,
  FrameContext,
  type HeatKey,
  type NewTaskOptions,
  type Selection,
  Scope,
  type TabActs,
  type TabId,
  type TabKeys,
  TAB_IDS,
  TAB_TABLE,
  TASK_DRAG,
} from './frame';
import './heat.css';
import { InfoPanel } from './info/InfoPanel';
import { focusKind, type HeatCommand, heatRoute } from './keys';
import { NewTaskSheet, TookSheet } from './sheets';
import { SpacesFilter } from './SpacesFilter';
import { useHeat } from './store';
import { HEAT_TABS } from './tabs';
import { Widgets } from './widgets/Widgets';

/** The Right column folds below this window width (3.3). */
export const FOLD_BELOW = 1240;

export interface HeatProps {
  /** The task the Now strip or ⌘K opened Heat on; `n` counts openings so the same task opens again. */
  open: { id: string; n: number } | null;
  /** What Heat tells the status bar: its count, its one secondary act, its save line. */
  onStatus(status: ScreenStatus): void;
  /** "New space…" goes to Settings → Heat, where spaces are edited. */
  onSettings(): void;
}

export interface HeatHandle {
  /** The shell's keyboard hands Heat each key its router didn't claim; true when Heat used it. */
  key(e: KeyboardEvent): boolean;
  /** Esc, with nothing open over the room. */
  escape(): void;
  /** ⇧Return: the current tab's one secondary act. */
  secondary(): void;
  /** ⌘I: Get Info for the selection. */
  getInfo(): void;
}

type Sheet =
  | { kind: 'new'; options: NewTaskOptions }
  | { kind: 'took'; taskId: Id }
  | { kind: 'custom'; node: ReactNode }
  | null;

const NO_SELECTION: Selection = { taskId: null, blockId: null };

export const HeatView = forwardRef<HeatHandle, HeatProps>(function HeatView({ open, onStatus, onSettings }, ref) {
  const heat = useHeat();
  const { snap, message } = heat;
  const folded = useMedia(`(max-width: ${FOLD_BELOW - 0.02}px)`);

  const [tab, setTab] = useState<TabId>('today');
  const [spaceId, setSpace] = useState<Id | null>(null);
  const [selection, setSelection] = useState<Selection>(NO_SELECTION);
  const [sidebar, setSidebar] = useState<HTMLElement | null>(null);
  const [focusLength, setFocusLength] = useState(25);
  const [sheet, setSheet] = useState<Sheet>(null);
  const [popover, setPopover] = useState<string | null>(null);
  // A new task's title stays typed until it is saved: Esc on a sheet keeps the draft (2.7).
  const [draft, setDraft] = useState('');
  // What the tabs' own sheets held when Esc closed them.
  const drafts = useRef(new Map<string, unknown>()).current;

  const actsRef = useRef<Partial<Record<TabId, MutableRefObject<TabActs>>>>({});
  const keysRef = useRef<Partial<Record<TabId, MutableRefObject<TabKeys>>>>({});
  const [touched, touch] = useReducer((n: number) => n + 1, 0);

  const registerActs = useCallback((id: TabId, acts: MutableRefObject<TabActs>) => {
    actsRef.current[id] = acts;
    touch();
    return () => {
      if (actsRef.current[id] === acts) delete actsRef.current[id];
    };
  }, []);
  const registerKeys = useCallback((id: TabId, k: MutableRefObject<TabKeys>) => {
    keysRef.current[id] = k;
    return () => {
      if (keysRef.current[id] === k) delete keysRef.current[id];
    };
  }, []);

  const selectTask = useCallback(
    (id: Id | null) => setSelection(id ? { taskId: id, blockId: null } : NO_SELECTION),
    [],
  );
  const selectBlock = useCallback(
    (id: Id | null) => setSelection(id ? { taskId: null, blockId: id } : NO_SELECTION),
    [],
  );

  const frame = useMemo<FrameApi>(
    () => ({
      tab,
      setTab,
      spaceId,
      setSpace,
      selection,
      selectTask,
      selectBlock,
      sidebar,
      newTask: (options = {}) => {
        if (options.title) setDraft(options.title);
        setSheet({ kind: 'new', options });
      },
      askTook: (taskId) => setSheet({ kind: 'took', taskId }),
      openSheet: (node) => setSheet({ kind: 'custom', node }),
      closeSheet: () => setSheet(null),
      drafts,
      focusInfo: () => document.querySelector<HTMLElement>('.heat-info [data-info-first]')?.focus(),
      focusLength,
      setFocusLength,
      registerActs,
      registerKeys,
      touch,
    }),
    [tab, spaceId, selection, selectTask, selectBlock, sidebar, focusLength, registerActs, registerKeys, drafts],
  );

  // The frame's acts and keys live where the tabs are, so the tabs read them from the frame.
  return (
    <FrameContext.Provider value={frame}>
      <Frame
        {...{ frame, folded, sheet, setSheet, popover, setPopover, draft, setDraft, touched }}
        actsRef={actsRef}
        keysRef={keysRef}
        open={open}
        onStatus={onStatus}
        onSettings={onSettings}
        setSidebar={setSidebar}
        handleRef={ref}
        message={message?.text ?? null}
        status={snap?.derived.status ?? null}
      />
    </FrameContext.Provider>
  );
});

interface FrameProps {
  frame: FrameApi;
  folded: boolean;
  sheet: Sheet;
  setSheet(s: Sheet): void;
  popover: string | null;
  setPopover(p: string | null): void;
  draft: string;
  setDraft(d: string): void;
  touched: number;
  actsRef: MutableRefObject<Partial<Record<TabId, MutableRefObject<TabActs>>>>;
  keysRef: MutableRefObject<Partial<Record<TabId, MutableRefObject<TabKeys>>>>;
  open: HeatProps['open'];
  onStatus(status: ScreenStatus): void;
  onSettings(): void;
  setSidebar(el: HTMLElement | null): void;
  handleRef: React.ForwardedRef<HeatHandle>;
  message: string | null;
  status: string | null;
}

// The part of the frame that needs the actions, which need the frame's context.
function Frame(p: FrameProps) {
  const { frame, folded, sheet, setSheet, popover, setPopover, draft, setDraft, touched } = p;
  const { tab, setTab, selection, selectTask } = frame;
  const heat = useHeat();
  const { snap, idx, client, act, say } = heat;
  const actions = useActions();
  const panel = useRef<HTMLDivElement>(null);

  const acts = p.actsRef.current[tab]?.current;
  const plus = TAB_TABLE[tab].plus === null ? null : (acts?.plus ?? null);
  const secondary = acts?.secondary && !acts.secondary.hidden && !acts.secondary.disabled ? acts.secondary : null;
  const drafts = snap?.heatState.planDrafts ?? [];
  void touched;

  // The strip's click and ⌘K open Heat on a task: Tasks, All, the task selected.
  const opened = useRef(0);
  useEffect(() => {
    if (!p.open || p.open.n === opened.current) return;
    opened.current = p.open.n;
    frame.setSpace(null);
    setTab('tasks');
    selectTask(p.open.id);
  }, [p.open, frame, setTab, selectTask]);

  // The status bar names the tab's count and its one secondary act (2.1, 2.7).
  // What a click or a key runs is read when it happens, not when the frame last drew.
  const nowPlus = () => p.actsRef.current[tab]?.current.plus;
  const nowSecondary = () => p.actsRef.current[tab]?.current.secondary;
  const secondaryName = secondary ? TAB_TABLE[tab].secondary : null;
  const count = acts?.count ?? null;
  const { onStatus } = p;
  useEffect(() => {
    onStatus({ count: p.message ?? count, act: secondaryName, save: p.status });
  }, [onStatus, p.message, count, secondaryName, p.status]);

  const selectedTask =
    selection.taskId ?? (selection.blockId ? idx.block.get(selection.blockId)?.taskId : undefined) ?? null;

  const run = (cmd: HeatCommand) => {
    const tabKeys = p.keysRef.current[tab]?.current;
    switch (cmd.type) {
      case 'tab':
        return setTab(cmd.tab);
      case 'new':
        {
          const now = nowPlus();
          if (TAB_TABLE[tab].plus === null || !now) return;
          if (now.disabled) say(now.disabled);
          else now.run();
        }
        return;
      case 'move':
        return tabKeys?.move?.(cmd.by);
      case 'edit':
        return tabKeys?.edit ? tabKeys.edit() : focusInfo();
      case 'done':
        return void (selectedTask && actions.toggleDone(selectedTask));
      case 'delete':
        if (selection.blockId) return void actions.removeBlock(selection.blockId);
        return void (selectedTask && actions.remove(selectedTask));
      case 'current':
        return void (selectedTask && actions.makeCurrent(selectedTask));
      case 'place':
        return void (selectedTask && actions.place(selectedTask));
      case 'focus':
        return void actions.focusKey();
      case 'stopFocus':
        return void actions.stopFocus();
      case 'pulledAway':
        return void actions.pulledAway();
      case 'acceptDrafts':
        return void actions.acceptDrafts();
      case 'nudge': {
        const block = selection.blockId ? idx.block.get(selection.blockId) : undefined;
        if (!block) return;
        const step = cmd.by * 15;
        return void (cmd.resize
          ? actions.setBlock(block, { minutes: Math.max(15, block.minutes + step) })
          : actions.setBlock(block, { start: Math.min(1440 - block.minutes, Math.max(420, block.start + step)) }));
      }
      case 'filter':
        return tabKeys?.filter?.();
      case 'sync':
        return void sync();
    }
  };

  const sync = async () => {
    const r = await act(client.calendars.sync());
    if (r) say(r.line);
  };

  const focusInfo = () => {
    panel.current?.querySelector<HTMLElement>('[data-info-first]')?.focus();
  };

  useImperativeHandle(p.handleRef, () => ({
    key(e) {
      const hk: HeatKey = {
        key: e.key,
        code: e.code,
        shift: e.shiftKey,
        command: IS_MAC ? e.metaKey : e.ctrlKey,
        alt: e.altKey,
        repeat: e.repeat,
      };
      const focus = focusKind(document.activeElement);
      if (focus === 'text' || sheet) return false;
      if (p.keysRef.current[tab]?.current.key?.(hk)) return true;
      const cmd = heatRoute(hk, {
        tab,
        focus,
        selected: selection.taskId !== null || selection.blockId !== null,
        drafts: drafts.length > 0,
      });
      if (!cmd) return false;
      run(cmd);
      return true;
    },
    escape() {
      if (popover) return setPopover(null);
      if (sheet) return setSheet(null);
      if (p.keysRef.current[tab]?.current.escape?.()) return;
      if (drafts.length > 0) return void actions.clearDrafts();
      const inInfo = panel.current?.contains(document.activeElement);
      if (inInfo) {
        (document.activeElement as HTMLElement).blur();
        return;
      }
      if (selection.taskId || selection.blockId) frame.selectTask(null);
    },
    secondary() {
      const now = nowSecondary();
      if (now && !now.hidden && !now.disabled) now.run();
    },
    getInfo() {
      if (selection.taskId || selection.blockId) focusInfo();
      else say('Select a task first.');
    },
  }));

  const info = selection.taskId !== null || selection.blockId !== null;
  const reason =
    plus?.disabled ??
    (plus === null && TAB_TABLE[tab].plus !== null ? `${TAB_TABLE[tab].name} isn’t built yet.` : null);

  return (
    <div className="heat register-desk" data-tab={tab} data-folded={folded} data-info={info}>
      <header className="heat-toolbar" role="toolbar" aria-label="Heat">
        {TAB_TABLE[tab].plus !== null && (
          <button
            type="button"
            className="gel heat-plus"
            aria-label={TAB_TABLE[tab].plus!}
            title={`${TAB_TABLE[tab].plus} (N)`}
            disabled={!plus || !!plus.disabled}
            onClick={() => nowPlus()?.run()}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
              <path d="M7 1.5v11M1.5 7h11" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
          </button>
        )}
        <div className="switcher heat-tabs" role="tablist" aria-label="Heat tabs">
          {TAB_IDS.map((id, i) => (
            <button
              key={id}
              type="button"
              role="tab"
              id={`heat-tab-${id}`}
              aria-controls={`heat-panel-${id}`}
              className="segment"
              aria-selected={id === tab}
              title={`${TAB_TABLE[id].name} (${i + 1})`}
              onClick={() => setTab(id)}
              onDragEnter={(e) => springLoad(e, () => setTab(id), id === tab)}
              onDragLeave={cancelSpring}
              onDrop={cancelSpring}
            >
              {TAB_TABLE[id].name}
            </button>
          ))}
        </div>
        <span className="heat-toolbar-gap" />
        {reason && (
          <span className="heat-why" data-text="secondary" role="status">
            {reason}
          </span>
        )}
        <button type="button" className="gel" title={`Sync calendars (${keys('⌥⌘R')})`} onClick={() => void sync()}>
          Sync
        </button>
      </header>

      <aside className="heat-sidebar" aria-label="Sidebar">
        <SpacesFilter onNewSpace={p.onSettings} />
        <div className="heat-sidebar-slot" ref={p.setSidebar} />
      </aside>

      <main
        className="heat-main"
        onClickCapture={(e) => {
          // ⇧-click is the other way to ⇧Return: the tab's one secondary act (2.7).
          const act = e.shiftKey ? nowSecondary() : undefined;
          if (!act || act.hidden || act.disabled) return;
          e.preventDefault();
          e.stopPropagation();
          act.run();
        }}
      >
        {TAB_IDS.map((id) => {
          const Tab = HEAT_TABS[id];
          return (
            <section
              key={id}
              id={`heat-panel-${id}`}
              role="tabpanel"
              aria-labelledby={`heat-tab-${id}`}
              className="heat-tabpanel"
              data-current={id === tab}
              inert={id !== tab}
            >
              <Scope tab={id} active={id === tab}>
                <Tab />
              </Scope>
            </section>
          );
        })}
      </main>

      <Widgets folded={folded} popover={popover} setPopover={setPopover} />

      {info && (
        <div className="heat-info-slot" ref={panel}>
          <InfoPanel />
        </div>
      )}

      {sheet && (
        <>
          <div className="heat-backdrop" onClick={() => setSheet(null)} />
          {sheet.kind === 'new' && (
            <NewTaskSheet
              options={sheet.options}
              draft={draft}
              onDraft={setDraft}
              onClose={() => setSheet(null)}
              onSaved={() => setDraft('')}
            />
          )}
          {sheet.kind === 'took' && <TookSheet taskId={sheet.taskId} onClose={() => setSheet(null)} />}
          {sheet.kind === 'custom' && sheet.node}
        </>
      )}
      <p className="heat-sr" aria-live="polite">
        {p.message}
      </p>
    </div>
  );
}

// Dragging a task over a tab for half a second opens it, so a row from Tasks
// can be carried to Today's column or to a Calendar day (3.6).
let spring: ReturnType<typeof setTimeout> | undefined;
function springLoad(e: React.DragEvent, open: () => void, here: boolean) {
  if (here || !e.dataTransfer.types.includes(TASK_DRAG)) return;
  clearTimeout(spring);
  spring = setTimeout(open, 500);
}
function cancelSpring() {
  clearTimeout(spring);
}
