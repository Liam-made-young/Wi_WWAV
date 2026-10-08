// Settings → Appearance, the Focus layout's rows (docs/FOCUS.md): which
// layout is on, the hint, and the click. They are this Mac's, kept beside
// first launch (focus/layout.ts), and apply at once in every window.

import { resetHint, setClicks, setLayout, useClicks, useHint, useLayout } from './layout';

export function LayoutSettings() {
  const layout = useLayout();
  const hint = useHint();
  const clicks = useClicks();
  return (
    <>
      <div className="setting">
        <p className="setting-label" data-text="secondary">
          Layout
        </p>
        <div className="radios" role="radiogroup" aria-label="Layout">
          {(
            [
              ['focus', 'Focus layout'],
              ['classic', 'Classic layout'],
            ] as const
          ).map(([value, label]) => (
            <label key={value} className="check" data-dense>
              <input type="radio" name="layout" checked={layout === value} onChange={() => setLayout(value)} />
              {label}
            </label>
          ))}
        </div>
        <p className="why" data-text="secondary">
          Focus shows only what needs you now; every tool is a key away. Classic keeps the tabs, the sidebar and the
          widgets in view.
        </p>
      </div>
      <div className="setting">
        <p className="setting-label" data-text="secondary">
          Focus
        </p>
        <label className="check" data-dense>
          <input type="checkbox" checked={clicks} onChange={(e) => setClicks(e.target.checked)} />
          Click on Done and on starting the timer
        </label>
        <button type="button" className="gel plain" disabled={hint} onClick={() => resetHint()}>
          Show the shortcut hint again
        </button>
        <p className="why" data-text="secondary">
          {hint ? 'The hint is showing under the Now task.' : 'The hint showed for your first week and then went away.'}
        </p>
      </div>
    </>
  );
}
