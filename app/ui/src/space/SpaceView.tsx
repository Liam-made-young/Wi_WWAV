// Space's frame (docs/SPEC.md 4.3): the sky fills the view, the breadcrumb
// sits top left, the selection's info card at the right edge, Newest bottom
// left, and − · + · fit bottom right. This build draws the universe tier in
// real 3D from a sample catalogue, marked as such, until Space reads
// everyone's galaxies from mi-wwav.com; diving in, the planet player and
// Open in Console arrive with the Space stage.

import { type MouseEvent, useEffect, useRef, useState } from 'react';
import type { Galaxy } from './model/catalogue';
import { GALAXIES } from './model/fixtures/catalogue';
import { useCurrentView } from '../shell/useCurrentView';
import { createSky, ZOOM_STEP, type Projected, type Sky } from './sky';
import './space.css';

const LATER = 'Arrives with the Space stage.';

export function SpaceView({ active: forced, galaxies = GALAXIES }: { active?: boolean; galaxies?: Galaxy[] }) {
  const holder = useRef<HTMLDivElement>(null);
  const seen = useCurrentView(holder);
  const active = forced ?? seen;
  const canvas = useRef<HTMLCanvasElement>(null);
  const sky = useRef<Sky | null>(null);
  const [labels, setLabels] = useState<Projected[]>([]);
  const [selected, setSelected] = useState<Galaxy | null>(null);
  const [failed, setFailed] = useState(false);

  // The sky is made the first time Space is shown, and kept; while Space is
  // the view, it follows the window's size.
  useEffect(() => {
    if (!active || !canvas.current || !holder.current) return;
    if (!sky.current) {
      try {
        sky.current = createSky(canvas.current, galaxies);
      } catch {
        setFailed(true);
        return;
      }
    }
    const h = holder.current;
    const fit = () => {
      sky.current?.resize(h.clientWidth, h.clientHeight);
      setLabels(sky.current?.project() ?? []);
    };
    fit();
    if (typeof ResizeObserver === 'undefined') return;
    const watch = new ResizeObserver(fit);
    watch.observe(h);
    return () => watch.disconnect();
  }, [active, galaxies]);

  useEffect(
    () => () => {
      sky.current?.dispose();
      sky.current = null;
    },
    [],
  );

  const zoom = (factor: number) => {
    sky.current?.zoom(factor);
    setLabels(sky.current?.project() ?? []);
  };
  const fitAll = () => {
    sky.current?.fit();
    setLabels(sky.current?.project() ?? []);
  };

  // ⌥⌘+ and ⌥⌘− zoom (⌘+ stays text size), Esc deselects (4.4).
  useEffect(() => {
    if (!active) return;
    const keys = (e: KeyboardEvent) => {
      if (e.metaKey && e.altKey && (e.key === '=' || e.key === '+' || e.code === 'Equal')) {
        zoom(ZOOM_STEP);
        e.preventDefault();
      } else if (e.metaKey && e.altKey && (e.key === '-' || e.code === 'Minus')) {
        zoom(1 / ZOOM_STEP);
        e.preventDefault();
      } else if (e.key === 'Escape') {
        setSelected(null);
      }
    };
    window.addEventListener('keydown', keys);
    return () => window.removeEventListener('keydown', keys);
  }, [active]);

  const click = (e: MouseEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    setSelected(sky.current?.pick(e.clientX - r.left, e.clientY - r.top) ?? null);
  };

  return (
    <div className="space" ref={holder}>
      <div className="space-sky" onClick={click} role="img" aria-label="The universe: everyone's galaxies">
        <canvas ref={canvas} className="space-canvas" />
        {labels
          .filter((l) => l.visible)
          .map((l) => (
            <span key={l.id} className="space-label" style={{ transform: `translate(${l.x}px, ${l.y + 14}px)` }}>
              {l.name}
            </span>
          ))}
      </div>
      {failed && <p className="space-failed">This Mac couldn't draw the sky in 3D. Newest still lists every work.</p>}

      <nav className="space-breadcrumb" aria-label="Where you are">
        <span className="crumb">Everyone</span>
        <span className="space-sample" title="Space reads everyone's galaxies from mi-wwav.com in the Space stage.">
          Sample sky
        </span>
      </nav>

      <aside className="space-card" aria-label="Selection">
        {selected ? (
          <>
            <div className="card-orb" aria-hidden="true" />
            <h2 className="card-title">{selected.displayName}</h2>
            <p className="card-line">
              {selected.systems.length === 1 ? '1 solar system' : `${selected.systems.length} solar systems`}
            </p>
            <button type="button" className="night-button" disabled title={LATER}>
              Open
            </button>
          </>
        ) : (
          <p className="card-empty">Select a galaxy to see whose it is.</p>
        )}
      </aside>

      <button type="button" className="night-button space-newest" disabled title={LATER}>
        Newest
      </button>

      <div className="space-zoom" role="group" aria-label="Zoom">
        <button type="button" className="night-button" onClick={() => zoom(1 / ZOOM_STEP)} aria-label="Zoom out">
          −
        </button>
        <button type="button" className="night-button" onClick={() => zoom(ZOOM_STEP)} aria-label="Zoom in">
          +
        </button>
        <button type="button" className="night-button" onClick={fitAll}>
          fit
        </button>
      </div>
    </div>
  );
}
