// The edge reveal (docs/FOCUS.md): rest the cursor on the window's left 8 px
// and the tool list slides out, as a hidden Dock does. It lists every view
// in the registry with its key, and Focus first. It is always in the page,
// only out of sight, so the keyboard and VoiceOver reach it as they reach
// anything else: tabbing into it brings it out.

import { useEffect, useRef } from 'react';
import { type View, useViews } from './registry';

interface Props {
  open: boolean;
  /** The view showing now, or null in Focus. */
  current: string | null;
  onOpen(open: boolean): void;
  onSummon(view: View): void;
  onFocus(): void;
}

/** The token's milliseconds, read off the page, so the rest time is design/tokens.json's. */
function restMs(el: Element): number {
  const n = parseFloat(getComputedStyle(el).getPropertyValue('--prism-motion-edge-rest'));
  return Number.isFinite(n) ? n : 150;
}

export function EdgeReveal({ open, current, onOpen, onSummon, onFocus }: Props) {
  const views = useViews();
  const resting = useRef<ReturnType<typeof setTimeout>>(undefined);
  const nav = useRef<HTMLElement>(null);
  useEffect(() => () => clearTimeout(resting.current), []);

  const rest = (e: React.MouseEvent) => {
    clearTimeout(resting.current);
    resting.current = setTimeout(() => onOpen(true), restMs(e.currentTarget));
  };
  const away = () => clearTimeout(resting.current);
  const go = (run: () => void) => () => {
    run();
    onOpen(false);
    // The list goes back in, and the keyboard goes with what was summoned.
    (document.activeElement as HTMLElement | null)?.blur();
  };

  return (
    <>
      <span className="edge-zone" aria-hidden="true" onMouseEnter={rest} onMouseLeave={away} />
      <nav
        ref={nav}
        className="edge"
        aria-label="Tools"
        data-open={open}
        onMouseLeave={() => onOpen(false)}
        onFocus={() => onOpen(true)}
        onBlur={(e) => {
          if (!nav.current?.contains(e.relatedTarget as Node | null)) onOpen(false);
        }}
      >
        <button type="button" className="edge-row" aria-current={current === null ? 'page' : undefined} onClick={go(onFocus)}>
          <svg className="edge-icon" width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
            <circle cx="10" cy="10" r="2" fill="currentColor" />
            <circle cx="10" cy="10" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
          </svg>
          <span className="edge-name">Focus</span>
          <kbd className="edge-key">Esc</kbd>
        </button>
        {views.map((v) => (
          <button
            key={v.id}
            type="button"
            className="edge-row"
            aria-current={current === v.id ? 'page' : undefined}
            aria-keyshortcuts={v.shortcut === null ? undefined : String(v.shortcut)}
            onClick={go(() => onSummon(v))}
          >
            <svg className="edge-icon" width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
              {v.icon}
            </svg>
            <span className="edge-name">{v.title}</span>
            {v.shortcut !== null && <kbd className="edge-key">{v.shortcut}</kbd>}
          </button>
        ))}
      </nav>
    </>
  );
}
