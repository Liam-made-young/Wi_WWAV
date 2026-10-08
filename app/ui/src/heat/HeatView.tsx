// Heat, the profile view (docs/SPEC.md 3.3): a toolbar with "+", the six
// tabs as one segmented control and Sync; below it the 190 px sidebar with
// the spaces filter, the main view, and the 290 px right column of widgets,
// which folds into a 44 px strip of icons below 1240 pt. Selecting a task or
// a block slides Get Info over the right column; Esc slides it back.
//
// The frame reads `heat.snapshot` through the store and refetches on the
// `heat` event. It holds what every tab shares: the tab, the space, the
// selection, the sheets, and the keyboard the shell hands it. What each tab
// draws is in HEAT_TABS (heat/tabs.tsx), and every tab is a view in the
// registry (focus/registry.ts), which the toolbar and the number keys read.
//
// In the Focus layout (docs/FOCUS.md) the frame has two layers. Focus is the
// default: the Now task, its timer and at most one interrupt line, and
// nothing else. A tab is a tool, summoned over Focus and opened full width
// under a small header; Esc goes back. The toolbar, the tab bar and the
// right column are the Classic layout's.

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
import { registerBuiltins } from '../focus/builtin';
import { focusOf, FocusScreen } from '../focus/FocusScreen';
import type { Layout } from '../focus/layout';
import { useViews, viewById, viewByShortcut } from '../focus/registry';
import { useLearnScreen } from '../ask/screen';
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
  TASK_DRAG,
} from './frame';
import './heat.css';
import { InfoPanel } from './info/InfoPanel';
import { focusKind, type HeatCommand, heatRoute } from './keys';
import { Notices } from './notes/Notices';
import { NewTaskSheet, TookSheet } from './sheets';
import { SpacesFilter } from './SpacesFilter';
import { useHeat } from './store';
import { Widgets } from './widgets/Widgets';

// The tabs in TAB_IDS become views before anything draws.
registerBuiltins();

/** Which of the Focus layout's layers is showing: Focus itself, or a summoned tool. */
export type Layer = 'focus' | 'tool';

/** The Right column folds below this window width (3.3). */
export const FOLD_BELOW = 1240;

export interface HeatProps {
  /** The task the Now strip or ⌘K opened Heat on; `n` counts openings so the same task opens again. */
  open: { id: string; n: number } | null;
  /** What Heat tells the status bar: its count, its one secondary act, its save line. */
  onStatus(status: ScreenStatus): void;
  /** "New space…" goes to Settings → Heat, where spaces are edited. */
  onSettings(): void;
  /** `focus` draws Focus and one summoned tool; `classic`, the default, the toolbar, tabs and right column. */
  layout?: Layout;
  /** Tells the shell which layer shows and which view, so it can put the status bar away in Focus. */
  onLayer?(layer: Layer, view: string): void;
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
  /** Opens a registered view by its id: ⌘K, the edge reveal and the ⌘ map come through here. */
  summon(id: string): void;
  /** Back to Focus (the Focus layout only). */
  toFocus(): void;
  /** ⌥⌘R, for ⌘K: sync the calendars. */
  sync(): void;
}

type Sheet =
  | { kind: 'new'; options: NewTaskOptions }
  | { kind: 'took'; taskId: Id }
  | { kind: 'custom'; node: ReactNode }
  | null;

const NO_SELECTION: Selection = { taskId: null, blockId: null };

export const HeatView = forwardRef<HeatHandle, HeatProps>(function HeatView(
  { open, onStatus, onSettings, layout = 'classic', onLayer },
  ref,
) {
  const heat = useHeat();
  const { snap, message } = heat;
  const folded = useMedia(`(max-width: ${FOLD_BELOW - 0.02}px)`);

  const [tab, showTab] = useState<TabId>('today');
  // Showing a tab summons it: in the Focus layout that leaves Focus for the tool.
  const [layer, setLayer] = useState<Layer>('focus');
  const setTab = useCallback((id: TabId) => {
    showTab(id);
    setLayer('tool');
  }, []);
  const toFocus = useCallback(() => setLayer('focus'), []);
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
      sheetOpen: sheet !== null,
      drafts,
      focusInfo: () => document.querySelector<HTMLElement>('.heat-info [data-info-first]')?.focus(),
      focusLength,
      setFocusLength,
      registerActs,
      registerKeys,
      touch,
    }),
    [tab, setTab, spaceId, selection, selectTask, selectBlock, sidebar, sheet, focusLength, registerActs, registerKeys, drafts],
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
        layout={layout}
        layer={layout === 'focus' ? layer : 'tool'}
        toFocus={toFocus}
        onLayer={onLayer}
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
  layout: Layout;
  /** Always `tool` in the Classic layout, which has no Focus. */
  layer: Layer;
  toFocus(): void;
  onLayer?(layer: Layer, view: string): void;
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
  // The prompt box is told what is on screen, so "these" means these.
  useLearnScreen(tab, selection, frame.spaceId);

  const views = useViews();
  const view = viewById(tab);
  const classic = p.layout === 'classic';
  const inFocus = p.layer === 'focus';
  const { onLayer, layer } = p;
  useEffect(() => onLayer?.(layer, tab), [onLayer, layer, tab]);

  const acts = p.actsRef.current[tab]?.current;
  const plus = !view?.plus ? null : (acts?.plus ?? null);
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
  const secondaryName = secondary ? (view?.secondary ?? null) : null;
  const count = acts?.count ?? null;
  const { onStatus } = p;
  useEffect(() => {
    // Focus has no count and no secondary act: the status bar is put away there.
    if (inFocus) onStatus({ count: null, act: null, save: p.status });
    else onStatus({ count: p.message ?? count, act: secondaryName, save: p.status });
  }, [onStatus, inFocus, p.message, count, secondaryName, p.status]);

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
          if (!view?.plus || !now) return;
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

  // Sync ends in a fresh snapshot whether or not the core had anything to announce.
  const sync = async () => {
    const r = await act(client.calendars.sync());
    await heat.refetch();
    if (r) say(r.line);
  };

  const focusInfo = () => {
    panel.current?.querySelector<HTMLElement>('[data-info-first]')?.focus();
  };

  // Focus's own keys: they act on the Now task, never on a tool that is out of sight.
  const focusKey = (k: HeatKey): boolean => {
    const key = k.key.length === 1 ? k.key.toLowerCase() : k.key;
    const nowTask = focusOf(snap)?.now.taskId ?? null;
    if (k.command) {
      if (key !== 'Enter' || k.shift || k.alt || !nowTask) return false;
      void actions.toggleDone(nowTask);
      return true;
    }
    if (k.alt || k.repeat) return false;
    if (k.shift) {
      if (key !== 'f') return false;
      void actions.stopFocus();
      return true;
    }
    switch (key) {
      case 'f':
        if (snap?.heatState.timer.phase === 'idle' && nowTask) {
          void act(client.focus('start', { taskId: nowTask, length: frame.focusLength }));
        } else void actions.focusKey();
        return true;
      case 'i':
        void actions.pulledAway();
        return true;
      case 'n':
        frame.newTask();
        return true;
      default:
        return false;
    }
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
      if (!inFocus && p.keysRef.current[tab]?.current.key?.(hk)) return true;
      // A number is a view's key, read from the registry, so a view that registers has its key at once.
      if (!hk.command && !hk.alt && !hk.shift && !hk.repeat && /^[1-9]$/.test(hk.key)) {
        const to = viewByShortcut(Number(hk.key));
        if (to) setTab(to.id as TabId);
        return to !== undefined;
      }
      if (inFocus) return focusKey(hk);
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
      if (inFocus) return;
      if (p.keysRef.current[tab]?.current.escape?.()) return;
      if (drafts.length > 0) return void actions.clearDrafts();
      const inInfo = panel.current?.contains(document.activeElement);
      if (inInfo) {
        (document.activeElement as HTMLElement).blur();
        return;
      }
      if (selection.taskId || selection.blockId) frame.selectTask(null);
      // With nothing left to close, Esc is the way back to Focus.
      if (!classic) p.toFocus();
    },
    secondary() {
      const now = nowSecondary();
      if (now && !now.hidden && !now.disabled) now.run();
    },
    getInfo() {
      if (selection.taskId || selection.blockId) focusInfo();
      else say('Select a task first.');
    },
    summon(id) {
      if (viewById(id)) setTab(id as TabId);
    },
    toFocus: p.toFocus,
    sync() {
      void sync();
    },
  }));

  const info = !inFocus && (selection.taskId !== null || selection.blockId !== null);
  const reason = plus?.disabled ?? (plus === null && view?.plus ? `${view.title} isn’t built yet.` : null);

  return (
    <div
      className="heat register-desk"
      data-tab={tab}
      data-folded={folded}
      data-info={info}
      data-layout={p.layout}
      data-layer={classic ? undefined : p.layer}
      data-sidebar={classic ? undefined : (view?.sidebar ?? false)}
    >
      {classic ? (
        <header className="heat-toolbar" role="toolbar" aria-label="Learn">
          {view?.plus && (
            <button
              type="button"
              className="gel heat-plus"
              aria-label={view.plus}
              title={`${view.plus} (N)`}
              disabled={!plus || !!plus.disabled}
              onClick={() => nowPlus()?.run()}
            >
              <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
                <path d="M7 1.5v11M1.5 7h11" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            </button>
          )}
          <div className="switcher heat-tabs" role="tablist" aria-label="Learn tabs">
            {views.map((v) => (
              <button
                key={v.id}
                type="button"
                role="tab"
                id={`heat-tab-${v.id}`}
                aria-controls={`heat-panel-${v.id}`}
                className="segment"
                aria-selected={v.id === tab}
                title={v.shortcut === null ? v.title : `${v.title} (${v.shortcut})`}
                onClick={() => setTab(v.id as TabId)}
                onDragEnter={(e) => springLoad(e, () => setTab(v.id as TabId), v.id === tab)}
                onDragLeave={cancelSpring}
                onDrop={cancelSpring}
              >
                {v.title}
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
      ) : (
        !inFocus && (
          <header className="tool-head" aria-label={view?.title ?? 'Learn'}>
            <h1 className="tool-name" id={`heat-tab-${tab}`}>
              {view?.title}
            </h1>
            {reason && (
              <span className="heat-why" role="status">
                {reason}
              </span>
            )}
            <span className="heat-toolbar-gap" />
            {view?.plus && (
              <button
                type="button"
                className="prism-plain"
                title={`${view.plus} (N)`}
                disabled={!plus || !!plus.disabled}
                onClick={() => nowPlus()?.run()}
              >
                {view.plus}
              </button>
            )}
            <button type="button" className="prism-plain tool-esc" onClick={p.toFocus}>
              Esc to focus
            </button>
          </header>
        )
      )}

      <aside className="heat-sidebar" aria-label="Sidebar" inert={inFocus}>
        <SpacesFilter onNewSpace={p.onSettings} />
        <div className="heat-sidebar-slot" ref={p.setSidebar} />
      </aside>

      <main
        className="heat-main"
        inert={inFocus}
        onClickCapture={(e) => {
          // ⇧-click is the other way to ⇧Return: the tab's one secondary act (2.7).
          const act = e.shiftKey ? nowSecondary() : undefined;
          if (!act || act.hidden || act.disabled) return;
          e.preventDefault();
          e.stopPropagation();
          act.run();
        }}
      >
        {views.map((v) => {
          const Tab = v.component;
          const id = v.id as TabId;
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
              <Scope tab={id} active={id === tab && !inFocus}>
                <Tab />
              </Scope>
            </section>
          );
        })}
      </main>

      {inFocus && <FocusScreen summon={(id) => setTab(id as TabId)} />}

      {classic && <Widgets folded={folded} popover={popover} setPopover={setPopover} />}

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
      <Notices />
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
