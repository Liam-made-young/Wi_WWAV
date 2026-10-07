// The shell (docs/SPEC.md 2): everything that stays put when you change
// views. The case (title bar and status bar), the three views inside it,
// and what opens over any view: ⌘K, ⌘⇧N, ⌘L, the player, Settings, Export
// everything and first launch. One keyboard router serves all of it, and the
// app's menus (app/src-tauri) reach it as `menu` events: on Linux the menu
// takes ⌘1 before the page does, so a key is a menu event there.

import { useEffect, useMemo, useRef, useState } from 'react';
import { call } from '../bridge';
import type { HeatHandle } from '../heat/HeatView';
import { idleGesture, stemGesture } from '../shared/stems/gesture';
import { Capture, type CaptureHandle } from './Capture';
import { listenForDrops } from './drop';
import { ExportSheet } from './ExportSheet';
import { FirstLaunch, type FirstLaunchHandle } from './FirstLaunch';
import { GetInfo } from './GetInfo';
import { type UndoRoom, useAppearance, useCoreEvent, useHistory, useNarrow, useStatus } from './hooks';
import { type Command, type KeyContext, type Overlay, route } from './keys';
import { type Clip, type DrawerHandle, LibraryDrawer } from './LibraryDrawer';
import { NowStrip } from './NowStrip';
import { AskBox, type AskHandle, type Item } from '../ask/AskBox';
import { navigate, onNavigate, type Target } from '../ask/nav';
import { IS_MAC, keys } from './platform';
import { PlayerSheet } from './PlayerSheet';
import { ROOM_NAMES, ROOMS, type RoomId } from './rooms';
import { Rooms } from './RoomViews';
import { type Account, PANES, type Pane, Settings, type SettingsValue } from './Settings';
import { type ScreenStatus, StatusBar } from './StatusBar';
import { GalaxyMenu, TitleBar } from './TitleBar';
import { usePlayer } from './usePlayer';
import { useTaskHalf } from './useTaskHalf';

// First launch is this Mac's: once its five steps are through, they stay
// through. Kept in the web view's own storage because the core's settings
// have no key for it (and refuse one they don't know).
const FIRST_LAUNCH = 'wi.firstLaunch';
function firstLaunchDone(): boolean {
  try {
    return localStorage.getItem(FIRST_LAUNCH) === 'done';
  } catch {
    return false;
  }
}

// Sheets that sit over the room behind a backdrop; a click on it closes them.
const MODAL: Overlay[] = ['palette', 'capture', 'settings', 'export', 'player', 'menu'];
const TRANSIENT: Overlay[] = ['palette', 'menu'];

function fieldOf(el: Element | null): KeyContext['field'] {
  if (!el) return null;
  if (el instanceof HTMLTextAreaElement || (el as HTMLElement).isContentEditable) return 'text';
  if (el instanceof HTMLSelectElement) return 'control';
  if (el instanceof HTMLInputElement) {
    return ['checkbox', 'radio', 'range', 'button', 'submit'].includes(el.type) ? 'control' : 'text';
  }
  return null;
}

export function Shell() {
  const [library, setLibrary] = useState('');
  const [settings, setSettings] = useState<SettingsValue | null>(null);
  const [account, setAccount] = useState<Account>({ signedIn: false });
  const [room, showRoom] = useState<RoomId>('heat');
  const [stack, setStack] = useState<Overlay[]>(() => (firstLaunchDone() ? [] : ['first']));
  const [heatTask, setHeatTask] = useState<string | null>(null);
  // Each opening of Heat on a task counts, so the same task opens again.
  const [heatOpen, setHeatOpen] = useState<{ id: string; n: number } | null>(null);
  const [heatStatus, setHeatStatus] = useState<ScreenStatus>({ count: null, act: null });
  const [infoId, setInfoId] = useState<string | null>(null);
  const [pane, setPane] = useState<Pane>('account');
  const [toast, setToast] = useState<{ text: string; n: number } | null>(null);
  const [drawerStatus, setDrawerStatus] = useState<ScreenStatus>({ count: null, act: null });

  const narrow = useNarrow();
  const status = useStatus();
  const menus = useHistory();
  const player = usePlayer();
  const { half, tasks } = useTaskHalf();

  const palette = useRef<AskHandle>(null);
  const capture = useRef<CaptureHandle>(null);
  const drawer = useRef<DrawerHandle>(null);
  const heat = useRef<HeatHandle>(null);
  const first = useRef<FirstLaunchHandle>(null);
  const keyedUndo = useRef(0);

  // Leaving a view takes the keyboard out of it first, and coming back
  // returns it where it was. A browser scrolls a focused field into view
  // when its view is hidden around it, which would lose the view's scroll.
  const focusIn = useRef<Partial<Record<RoomId, HTMLElement>>>({});
  const setRoom = (next: RoomId) => {
    if (next === room) return;
    const active = document.activeElement as HTMLElement | null;
    if (active && document.querySelector(`[data-room="${room}"]`)?.contains(active)) {
      focusIn.current[room] = active;
      active.blur();
    }
    showRoom(next);
  };
  useEffect(() => {
    focusIn.current[room]?.focus({ preventScroll: true });
  }, [room]);

  const top = stack.at(-1) ?? null;
  const shown = (o: Overlay) => stack.includes(o);
  // ⌘Z acts on the view you are in; with the library drawer open, that is the library.
  const undoRoom: UndoRoom = shown('drawer') ? 'library' : room;

  // The app's Edit menu names that view's undo, and the View menu ticks it.
  // (In a browser, on the dev bridge, there is no shell to tell: the core
  // answers that it has no such command, and nothing is lost.)
  useEffect(() => {
    call('shell.room', { room: undoRoom }).catch(() => {});
  }, [undoRoom]);

  useEffect(() => {
    call<{ library: string }>('app.hello').then(
      (h) => setLibrary(h.library),
      () => {},
    );
    call<SettingsValue>('app.settings.get').then(setSettings, () => {});
    call<Account>('account.status').then(setAccount, () => {});
  }, []);
  useCoreEvent<SettingsValue>('settings', setSettings);
  useCoreEvent<Account>('account', (a) => setAccount((was) => ({ ...was, ...a })));

  useAppearance(settings);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 2600);
    return () => clearTimeout(t);
  }, [toast]);

  const say = (text: string) => setToast({ text, n: Date.now() });
  const failed = (e: unknown) => say(e instanceof Error ? e.message : String(e));
  // Opening anything closes the palette and the menu: they are ways to it.
  const open = (o: Overlay) => setStack((s) => [...s.filter((x) => x !== o && !TRANSIENT.includes(x)), o]);
  const close = (o: Overlay) => setStack((s) => s.filter((x) => x !== o));
  const toggle = (o: Overlay) => (top === o ? close(o) : open(o));

  const patch = (p: Record<string, unknown>) =>
    call<SettingsValue>('app.settings.set', { patch: p }).then(setSettings, failed);
  const signIn = () => call<Account>('account.signIn').then((a) => setAccount((was) => ({ ...was, ...a })));
  const signOut = () => call<Account>('account.signOut').then(setAccount, failed);

  const showTask = (id: string | null) => {
    setRoom('heat');
    setHeatTask(id);
    if (id) setHeatOpen((was) => ({ id, n: (was?.n ?? 0) + 1 }));
  };

  // A link in an answer, a search result, or a link in the Wiki tab: Learn's
  // own tabs show what is theirs (ask/nav.ts), a task opens as it always
  // has, and a web address goes to the system browser.
  const openTarget = (target: Target) => {
    if (target.what === 'web') return void call('wiki.open', { url: target.url }).catch(failed);
    setRoom('heat');
    if (target.what === 'row' && target.table === 'task') return showTask(target.id);
    navigate(target);
  };
  // A web link clicked inside a tab comes here too.
  useEffect(
    () =>
      onNavigate((t) => {
        if (t.what === 'web') void call('wiki.open', { url: t.url }).catch(failed);
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  const isSong = (c: Clip) => c.kind === 'wwav' || c.kind === 'audio';
  const notInConsole = (c: Clip | null) =>
    say(
      c
        ? `The Console isn’t in this build yet, so it can’t open ‘${c.title}’.`
        : 'Select a work to open in the Console.',
    );
  const notInSpace = (what: string) => say(`${what} comes with Space, which isn’t in this build yet.`);

  const undo = (redo: boolean) =>
    call<{ label: string }>(redo ? 'history.redo' : 'history.undo', { room: undoRoom }).then(
      (r) => say(`${redo ? 'Redone' : 'Undone'} — ${r.label}`),
      failed,
    );

  const playPause = () => {
    const picked = shown('drawer') ? drawer.current?.current() : null;
    if (picked && isSong(picked) && picked.id !== player.state.clip) player.load(picked.id).catch(failed);
    else player.playPause().catch(failed);
  };

  const escape = () => {
    if (top === 'first') return;
    // Nothing open over the room: Esc is the room's own (Heat closes a sheet or Get Info, clears drafts).
    if (top === null) return room === 'heat' ? heat.current?.escape() : undefined;
    if (top === 'drawer' && drawer.current?.deselect()) return;
    close(top);
  };

  const run = (c: Command) => {
    switch (c.type) {
      case 'room':
        return setRoom(c.room);
      case 'playPause':
        return playPause();
      case 'palette':
      case 'drawer':
        return toggle(c.type);
      case 'capture':
        return open('capture');
      case 'undo':
      case 'redo':
        keyedUndo.current = Date.now();
        return void undo(c.type === 'redo');
      case 'escape':
        return escape();
      case 'secondary':
        // The screen's one secondary act: the library's adds the row under the cursor, Heat's tab runs its own.
        if (top === 'drawer') drawer.current?.addToSelection();
        else if (top === null && room === 'heat') heat.current?.secondary();
        return;
      case 'openInConsole':
        return notInConsole(shown('drawer') ? (drawer.current?.current() ?? null) : null);
      case 'exportEverything':
        return open('export');
      case 'textSize':
        return void patch({ textSize: Math.min(20, Math.max(13, (settings?.textSize ?? 13) + c.by)) });
      case 'getInfo': {
        const clip = shown('drawer') ? drawer.current?.current() : null;
        if (clip) {
          setInfoId(clip.id);
          open('info');
        } else if (top === null && room === 'heat') heat.current?.getInfo();
        return;
      }
      case 'move':
        return top === 'palette' ? palette.current?.move(c.by) : drawer.current?.move(c.by);
      case 'open':
        if (top === 'palette') return palette.current?.open(c.other);
        return drawer.current?.open();
      case 'save':
        return capture.current?.save();
      case 'delete':
        return drawer.current?.remove();
      case 'stemKey': {
        if (!player.mix) return;
        const input = { type: 'key' as const, key: c.key, shift: c.shift, command: false, stem: c.stem };
        const { actions } = stemGesture(idleGesture(), input, { mix: player.mix, stage: null });
        return void player.stems(actions).catch(failed);
      }
    }
  };

  // What the app's menus ask of the page (app/src-tauri/README.md): the view
  // keys, Undo and Redo, the library, Export everything and New session.
  useCoreEvent<{ action: string }>('menu', ({ action }) => {
    const view = /^room\.(heat|space|console)$/.exec(action);
    if (view) return setRoom(view[1] as RoomId);
    switch (action) {
      case 'history.undo':
      case 'history.redo': {
        // On a Mac the page may take the same keys first: that one is this one.
        if (Date.now() - keyedUndo.current < 150) return;
        const redo = action === 'history.redo';
        // The menu's Undo replaces the system's, so a text field keeps its own typing undo.
        if (fieldOf(document.activeElement) === 'text') return void document.execCommand(redo ? 'redo' : 'undo');
        return void undo(redo);
      }
      case 'library.toggle':
        return toggle('drawer');
      case 'export.everything':
        return open('export');
      case 'session.new':
        return say('The Console isn’t in this build yet, so there is no new session.');
    }
  });

  // Files opened from outside (a double-click, File → Open…, a second launch):
  // songs and films are already in the library, and the drawer hears of them
  // through `library`. Sessions are the Console's.
  useCoreEvent<{ clips?: { title: string }[]; sessions?: string[]; error?: { message: string } }>('open', (o) => {
    if (o.error) return say(o.error.message);
    const said: string[] = [];
    const n = o.clips?.length ?? 0;
    if (n) said.push(n === 1 ? `Opened ‘${o.clips![0].title}’.` : `Opened ${n} files.`);
    if (o.sessions?.length) said.push('The Console isn’t in this build yet, so it can’t open that session.');
    if (said.length) say(said.join(' '));
  });

  // The one keyboard listener. Everything else learns of keys through `run`.
  const latest = useRef({ run, top, room });
  latest.current = { run, top, room };
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // A field in a sheet that has just closed still holds focus for a
      // moment; it no longer has the keyboard.
      const active = document.activeElement;
      const focused = active?.checkVisibility() ? active : null;
      const stem = focused?.getAttribute('data-stem') as KeyContext['stem'];
      const command = route(e, { mac: IS_MAC, field: fieldOf(focused), stem, overlay: latest.current.top });
      if (!command) {
        // What the router leaves is the current room's: Heat's tabs and lists have their own keys (3.17).
        const { room: here, top: over } = latest.current;
        if (here === 'heat' && over === null && heat.current?.key(e)) {
          e.preventDefault();
          e.stopPropagation();
        }
        return;
      }
      e.preventDefault();
      e.stopPropagation();
      latest.current.run(command);
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  useEffect(
    () =>
      listenForDrops((target, paths) => {
        if (target === 'capture') capture.current?.drop(paths);
        else if (target === 'drawer') drawer.current?.drop(paths);
        else first.current?.drop(paths);
      }),
    [],
  );

  const undoLabel = menus[undoRoom]?.undo;
  const actions: Item[] = [
    ...ROOMS.map((r, i) => ({ label: `Go to ${ROOM_NAMES[r]}`, hint: keys(`⌘${i + 1}`), run: () => setRoom(r) })),
    { label: 'Quick capture', hint: keys('⌘⇧N'), run: () => open('capture') },
    { label: 'Library', hint: keys('⌘L'), run: () => open('drawer') },
    { label: 'Export everything…', hint: keys('⌘⇧E'), run: () => open('export') },
    { label: 'Settings…', run: () => open('settings') },
    ...(undoLabel ? [{ label: undoLabel, hint: keys('⌘Z'), run: () => void undo(false) }] : []),
    account.signedIn
      ? { label: 'Sign out', run: () => void signOut() }
      : { label: 'Sign in or create an account', run: () => void signIn().catch(failed) },
  ];

  const settingsItems = useMemo<Item[]>(() => {
    const go = (p: Pane) => () => {
      setPane(p);
      open('settings');
    };
    return [
      ...PANES.map((p) => ({ label: p.label, run: go(p.id) })),
      { label: 'Text size', run: go('appearance') },
      { label: 'Reduce motion', run: go('appearance') },
      { label: 'Time zone', run: go('heat') },
      { label: 'Buffer size', run: go('audio') },
      { label: 'Empty trash', run: go('library') },
    ];
  }, []);

  const screen: ScreenStatus =
    top === 'drawer' ? drawerStatus : room === 'heat' ? heatStatus : { count: null, act: null };
  const menu = menus[undoRoom];

  return (
    <div className="case">
      <TitleBar
        room={room}
        narrow={narrow}
        menuOpen={shown('menu')}
        onRoom={setRoom}
        onSearch={() => open('palette')}
        onMenu={() => toggle('menu')}
        strip={
          <NowStrip
            narrow={narrow}
            task={half}
            player={player}
            room={room}
            consoleTransport={null}
            onTask={showTask}
            onExpand={() => open('player')}
          />
        }
      />
      <div className="room-area">
        <Rooms
          current={room}
          heatTask={heatTask}
          heat={{
            handle: heat,
            open: heatOpen,
            onStatus: setHeatStatus,
            onSettings: () => {
              setPane('heat');
              open('settings');
            },
          }}
        />
        {top && MODAL.includes(top) && <div className="backdrop" data-overlay={top} onClick={() => close(top)} />}
        <LibraryDrawer
          ref={drawer}
          shown={shown('drawer')}
          onStatus={setDrawerStatus}
          onInfo={(id) => {
            setInfoId(id);
            open('info');
          }}
          onError={say}
        />
        <GetInfo id={infoId} shown={shown('info')} />
        <PlayerSheet player={player} shown={shown('player')} />
        <AskBox
          ref={palette}
          shown={shown('palette')}
          actions={actions}
          settings={settingsItems}
          tasks={tasks}
          onTask={showTask}
          onClip={(clip, other) => {
            if (other) notInConsole(clip);
            else if (isSong(clip)) player.load(clip.id).catch(failed);
            else {
              setInfoId(clip.id);
              open('drawer');
              open('info');
            }
          }}
          onOpen={(target) => openTarget(target)}
          onSaid={say}
          onClose={() => close('palette')}
        />
        <Capture ref={capture} shown={shown('capture')} onError={say} />
        <Settings
          shown={shown('settings')}
          pane={pane}
          settings={settings}
          account={account}
          library={library}
          onPane={setPane}
          onPatch={(p) => void patch(p)}
          onSignIn={() => void signIn().catch(failed)}
          onSignOut={() => void signOut()}
        />
        <ExportSheet shown={shown('export')} library={library} />
        <FirstLaunch
          ref={first}
          shown={shown('first')}
          signedIn={account.signedIn}
          onSignIn={() => signIn().then(() => undefined)}
          onSaid={say}
          onDone={() => {
            try {
              localStorage.setItem(FIRST_LAUNCH, 'done');
            } catch {
              // Private storage refused: first launch shows again next time, nothing worse.
            }
            setRoom('heat');
            close('first');
          }}
        />
        {shown('menu') && (
          <GalaxyMenu
            items={[
              { label: 'Your galaxy', run: () => (setRoom('space'), close('menu')) },
              {
                label: 'Your public Learn view',
                run: () => (setRoom('space'), close('menu'), notInSpace('Your public Learn view, behind your sun,')),
              },
              { label: 'Settings…', run: () => (close('menu'), open('settings')) },
              { label: 'Export everything…', run: () => (close('menu'), open('export')) },
              account.signedIn
                ? { label: 'Sign out', run: () => (close('menu'), void signOut()) }
                : { label: 'Sign in or create an account', run: () => (close('menu'), void signIn().catch(failed)) },
            ]}
          />
        )}
        {toast && (
          <p key={toast.n} className="toast" role="status">
            {toast.text}
          </p>
        )}
      </div>
      <StatusBar screen={screen} undo={menu?.undo ?? menu?.cant ?? null} status={status} />
    </div>
  );
}
