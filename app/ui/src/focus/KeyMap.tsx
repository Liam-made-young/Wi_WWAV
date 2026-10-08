// The ⌘ map (docs/FOCUS.md): hold the command key a little over half a
// second and a faint overlay names every tool and its key; let go and it is
// gone. It reads the registry, so a new view is on the map when it
// registers. Nothing here acts on a key: the shell's one router still does
// that, and any other key pressed with ⌘ is a shortcut, not a hold.

import { useEffect, useState } from 'react';
import { IS_MAC, keys } from '../shell/platform';
import { ROOM_NAMES, ROOMS } from '../shell/rooms';
import { useViews } from './registry';

function holdMs(): number {
  const n = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--prism-motion-map-hold'));
  return Number.isFinite(n) ? n : 600;
}

/** True while the command key has been held alone for long enough. */
export function useHeldCommand(on: boolean): boolean {
  const [held, setHeld] = useState(false);
  useEffect(() => {
    if (!on) return;
    const mod = IS_MAC ? 'Meta' : 'Control';
    let timer: ReturnType<typeof setTimeout> | undefined;
    const stop = () => {
      clearTimeout(timer);
      timer = undefined;
      setHeld(false);
    };
    const down = (e: KeyboardEvent) => {
      if (e.key !== mod || e.altKey || e.shiftKey) return stop();
      if (e.repeat || timer !== undefined) return;
      timer = setTimeout(() => setHeld(true), holdMs());
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === mod) stop();
    };
    window.addEventListener('keydown', down, true);
    window.addEventListener('keyup', up, true);
    window.addEventListener('blur', stop);
    return () => {
      stop();
      window.removeEventListener('keydown', down, true);
      window.removeEventListener('keyup', up, true);
      window.removeEventListener('blur', stop);
    };
  }, [on]);
  return held;
}

const ALWAYS: readonly [string, string][] = [
  ['⌘K', 'Anything'],
  ['Esc', 'Focus'],
  ['F', 'Start or pause focus'],
  ['⌘Return', 'Done'],
  ['N', 'New task'],
  ['⌘⇧N', 'Quick capture'],
  ['⌘L', 'Library'],
  ['⌘Z', 'Undo'],
];

export function KeyMap({ shown }: { shown: boolean }) {
  const views = useViews();
  if (!shown) return null;
  return (
    <div className="keymap" role="note" aria-label="Map of tools and shortcuts">
      <div className="keymap-sheet">
        <section aria-label="Tools">
          <h2 className="keymap-heading">Tools</h2>
          <dl className="keymap-list">
            {views.map((v) => (
              <div key={v.id} className="keymap-row">
                <dt>{v.shortcut ?? keys('⌘K')}</dt>
                <dd>{v.title}</dd>
              </div>
            ))}
          </dl>
        </section>
        <section aria-label="Views">
          <h2 className="keymap-heading">Views</h2>
          <dl className="keymap-list">
            {ROOMS.map((r, i) => (
              <div key={r} className="keymap-row">
                <dt>{keys(`⌥${i + 1}`)}</dt>
                <dd>{ROOM_NAMES[r]}</dd>
              </div>
            ))}
          </dl>
        </section>
        <section aria-label="Everywhere">
          <h2 className="keymap-heading">Everywhere</h2>
          <dl className="keymap-list">
            {ALWAYS.map(([k, does]) => (
              <div key={k} className="keymap-row">
                <dt>{keys(k)}</dt>
                <dd>{does}</dd>
              </div>
            ))}
          </dl>
        </section>
      </div>
    </div>
  );
}
