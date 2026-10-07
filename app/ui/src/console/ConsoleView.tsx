// The Console's frame (docs/SPEC.md 5.2): the transport, the browser, the
// arrangement with its OUTPUT row, the inspector and the bottom panel, in
// metal with DMG screens inside. This build is the frame only: the browser
// lists the real library and the engine's state is real, and every control
// the Console stage fills is shown disabled with the sentence that says so,
// so nothing pretends.

import { useEffect, useRef, useState } from 'react';
import { call } from '../bridge';
import { useCoreEvent } from '../shell/hooks';
import { useCurrentView } from '../shell/useCurrentView';
import './console.css';

const LATER = 'Arrives with the Console stage.';

type Browser = 'library' | 'plugins' | 'takes';
type Panel = 'editor' | 'chain' | 'mixer';

interface Clip {
  id: string;
  kind: string;
  title: string;
  artist?: string | null;
  bpm?: number | null;
  key?: string | null;
  duration?: number | null;
}

interface EngineStatus {
  state: 'starting' | 'running' | 'restarting' | 'stopped';
  sampleRate?: number | null;
  block?: number | null;
}

const BARS = 32;

export function ConsoleView({ active: forced }: { active?: boolean }) {
  const root = useRef<HTMLDivElement>(null);
  const seen = useCurrentView(root);
  const active = forced ?? seen;
  const [browser, setBrowser] = useState<Browser>('library');
  const [showBrowser, setShowBrowser] = useState(true);
  const [showInspector, setShowInspector] = useState(true);
  const [panel, setPanel] = useState<Panel | null>('mixer');
  const [clips, setClips] = useState<Clip[] | null>(null);
  const [engine, setEngine] = useState<EngineStatus | null>(null);

  useEffect(() => {
    if (!active) return;
    call<{ clips: Clip[] }>('library.list', { limit: 200 })
      .then((r) => setClips(r.clips))
      .catch(() => setClips([]));
    call<EngineStatus>('engine.status')
      .then(setEngine)
      .catch(() => setEngine(null));
  }, [active]);
  useCoreEvent<unknown>('engine.stopped', () => setEngine((e) => (e ? { ...e, state: 'restarting' } : e)));
  useCoreEvent<unknown>('engine.back', () => call<EngineStatus>('engine.status').then(setEngine).catch(() => {}));

  // ⌥⌘B the browser, ⌘I the inspector, ⌥1–⌥3 the bottom panel; the open
  // one again closes it (5.2). Only while the Console is the view.
  useEffect(() => {
    if (!active) return;
    const keys = (e: KeyboardEvent) => {
      if (e.metaKey && e.altKey && e.code === 'KeyB') {
        setShowBrowser((v) => !v);
        e.preventDefault();
      } else if (e.metaKey && !e.altKey && !e.shiftKey && e.code === 'KeyI') {
        setShowInspector((v) => !v);
        e.preventDefault();
      } else if (e.altKey && !e.metaKey && ['Digit1', 'Digit2', 'Digit3'].includes(e.code)) {
        const next = (['editor', 'chain', 'mixer'] as const)[Number(e.code.slice(5)) - 1];
        setPanel((p) => (p === next ? null : next));
        e.preventDefault();
      }
    };
    window.addEventListener('keydown', keys);
    return () => window.removeEventListener('keydown', keys);
  }, [active]);

  const engineLine = engineText(engine);

  return (
    <div
      ref={root}
      className="console"
      data-browser={showBrowser}
      data-inspector={showInspector}
      data-panel={panel ?? 'none'}
    >
      <header className="console-transport metal" aria-label="Transport">
        <div className="transport-buttons">
          <button type="button" className="metal-button" disabled title={LATER} aria-label="Return to zero">⏮</button>
          <button type="button" className="metal-button" disabled title={LATER} aria-label="Play">▶</button>
          <button type="button" className="metal-button record" disabled title={LATER} aria-label="Record">●</button>
          <button type="button" className="metal-button" disabled title={LATER} aria-label="Loop">⟲</button>
        </div>
        <output className="dmg transport-screen" aria-label="Position, tempo, key and meter">
          <span>BAR 001.1.000</span>
          <span>00:00:00:00</span>
          <span>/ 120.00 BPM</span>
          <span>— KEY</span>
          <span>4/4</span>
        </output>
        <span className="dmg transport-chip" aria-label="Grid off">GRID ○</span>
        <output className="dmg transport-engine" aria-label={`Engine: ${engineLine}`} data-state={engine?.state ?? 'none'}>
          {engineLine}
        </output>
      </header>

      {showBrowser && (
        <nav className="console-browser metal" aria-label="Browser">
          <div className="segmented" role="tablist">
            {(['library', 'plugins', 'takes'] as const).map((b) => (
              <button
                key={b}
                type="button"
                role="tab"
                aria-selected={browser === b}
                className={browser === b ? 'selected' : ''}
                onClick={() => setBrowser(b)}
              >
                {b === 'library' ? 'Library' : b === 'plugins' ? 'Plugins' : 'Takes'}
              </button>
            ))}
          </div>
          <div className="browser-list">
            {browser === 'library' && <LibraryList clips={clips} />}
            {browser === 'plugins' && <p className="empty">Your VST3 and AU plugins appear here once the engine scans them. {LATER}</p>}
            {browser === 'takes' && <p className="empty">No takes yet. Arm a track and play: what you play is a take.</p>}
          </div>
        </nav>
      )}

      <section className="console-arrangement" aria-label="Arrangement">
        <div className="ruler" aria-hidden="true">
          <div className="ruler-head" />
          <div className="ruler-bars">
            {Array.from({ length: BARS }, (_, i) => (
              <span key={i} className="ruler-bar">{i + 1}</span>
            ))}
          </div>
        </div>
        <div className="lanes">
          <p className="empty lanes-empty">Drop a .wwav here to make a stem group, or any audio to make a track. {LATER}</p>
        </div>
        <div className="output-row" role="row" aria-label="OUTPUT: select it and press Return to export">
          <div className="track-head">OUTPUT</div>
          <div className="output-lane dmg">EXPORT · .WWAV · .SWAV · WAV</div>
        </div>
      </section>

      {showInspector && (
        <aside className="console-inspector metal" aria-label="Inspector">
          <h2 className="inspector-title">Inspector</h2>
          <p className="empty">Select a clip, a track or a note to see every field of it here.</p>
        </aside>
      )}

      {panel && (
        <section className="console-panel metal" aria-label="Bottom panel">
          <div className="segmented" role="tablist">
            {(['editor', 'chain', 'mixer'] as const).map((p, i) => (
              <button
                key={p}
                type="button"
                role="tab"
                aria-selected={panel === p}
                className={panel === p ? 'selected' : ''}
                onClick={() => setPanel(p)}
                title={`⌥${i + 1}`}
              >
                {p === 'editor' ? 'Editor' : p === 'chain' ? 'Chain' : 'Mixer'}
              </button>
            ))}
          </div>
          {panel === 'mixer' ? (
            <div className="mixer">
              {(['vocals', 'drums', 'other', 'bass'] as const).map((stem) => (
                <div key={stem} className={`strip stem-${stem}`} aria-label={`${stem} bus`}>
                  <div className="dmg meter" aria-hidden="true" />
                  <span className="strip-name">{stem.toUpperCase()}</span>
                </div>
              ))}
              <div className="strip master" aria-label="Master">
                <div className="dmg meter" aria-hidden="true" />
                <span className="strip-name">MASTER</span>
              </div>
            </div>
          ) : (
            <p className="empty">
              {panel === 'editor'
                ? 'The piano roll for an instrument clip, or the waveform for an audio clip. '
                : "The selected track's devices: plugins and the six built-in effects. "}
              {LATER}
            </p>
          )}
        </section>
      )}
    </div>
  );
}

function LibraryList({ clips }: { clips: Clip[] | null }) {
  if (clips === null) return <p className="empty">Reading the library…</p>;
  if (clips.length === 0) return <p className="empty">Nothing in the library yet. Import a song with File → Import, or drop one on the window.</p>;
  return (
    <ul className="clips">
      {clips.map((c) => (
        <li key={c.id} className="clip" draggable={false} title={LATER}>
          <span className="clip-title">{c.title || 'Untitled'}</span>
          <span className="clip-facts">{[c.kind, c.key, c.bpm ? `${Math.round(c.bpm)} BPM` : null].filter(Boolean).join(' · ')}</span>
        </li>
      ))}
    </ul>
  );
}

function engineText(e: EngineStatus | null): string {
  if (!e) return 'ENGINE —';
  switch (e.state) {
    case 'running':
      return e.sampleRate ? `${Math.round(e.sampleRate / 100) / 10} KHZ · ${e.block ?? '—'} SMP` : 'ENGINE ON';
    case 'starting':
      return 'ENGINE STARTING';
    case 'restarting':
      return 'ENGINE RESTARTING';
    default:
      return 'ENGINE OFF';
  }
}
