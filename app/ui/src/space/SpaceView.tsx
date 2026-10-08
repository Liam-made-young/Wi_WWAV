// Space (docs/SPACE.md): you are an astronaut seen from your own eyes, every
// link is a body, and a body's page is at its core. Fly at one and its real
// page grows out of it until it fills the view; back away and it falls into
// the sky again. This build flies a proving ground of real sites, marked as
// one, until home and search fill the sky (docs/SPACE.md 10).
//
// The strip along the top is Space's own: where you are, and the way out of
// a page. Everything under it is sky, and the live page is laid over the
// sky by the app's shell (pages.ts), so nothing drawn here can sit on top
// of a page.

import { type KeyboardEvent, type PointerEvent, type WheelEvent, useEffect, useMemo, useRef, useState } from 'react';
import { Matrix4, Quaternion, Vector3 } from 'three';
import { useCoreEvent } from '../shell/hooks';
import { useCurrentView } from '../shell/useCurrentView';
import { provingGround, start } from './flight/ground';
import {
  type Body,
  type Controls,
  type Pilot,
  type View,
  LIVE_FROM,
  LIVE_UNTIL,
  PULLED_IN,
  STILL,
  aim,
  approach,
  inside,
  look,
  nudge,
  sight,
  step,
} from './flight/model';
import { type Sky, createSky } from './flight/scene';
import { type Heard, IN_APP, pages } from './pages';
import './space.css';

/** What is flying you, if anything: to a body until its page covers so much of the view, and then in or not. */
interface Course {
  id: string;
  cover: number;
  enter: boolean;
}

/** Keys that move you, by the place of the key, so they are the same on every layout. */
const KEYS: Record<string, Partial<Controls>> = {
  KeyW: { forward: 1 },
  KeyS: { forward: -1 },
  KeyA: { right: -1 },
  KeyD: { right: 1 },
  KeyR: { up: 1 },
  KeyF: { up: -1 },
  KeyQ: { roll: 1 },
  KeyE: { roll: -1 },
  ArrowLeft: { yaw: 1 },
  ArrowRight: { yaw: -1 },
  ArrowUp: { pitch: 1 },
  ArrowDown: { pitch: -1 },
};

function held(codes: ReadonlySet<string>): Controls {
  const c: Controls = { ...STILL };
  for (const code of codes) {
    const adds = KEYS[code];
    if (adds) for (const k of Object.keys(adds) as (keyof Controls)[]) c[k] += adds[k] ?? 0;
  }
  return c;
}

/** Whether something of the app's is drawn over the sky: at its middle or near a corner, the topmost thing isn't the sky's. */
function hidden(box: HTMLElement, at: DOMRect): boolean {
  if (typeof document.elementFromPoint !== 'function') return false;
  const inset = 24;
  const spots: [number, number][] = [
    [at.left + at.width / 2, at.top + at.height / 2],
    [at.left + inset, at.top + inset],
    [at.right - inset, at.top + inset],
    [at.left + inset, at.bottom - inset],
    [at.right - inset, at.bottom - inset],
  ];
  return spots.some(([x, y]) => {
    const top = document.elementFromPoint(x, y);
    return !!top && !box.contains(top);
  });
}

function pilotAt(bodies: readonly Body[]): Pilot {
  const { at, toward } = start(bodies);
  const facing = new Quaternion().setFromRotationMatrix(new Matrix4().lookAt(at, toward, new Vector3(0, 1, 0)));
  return { at, facing, velocity: new Vector3() };
}

export function SpaceView({ active: forced, bodies: given }: { active?: boolean; bodies?: Body[] }) {
  const holder = useRef<HTMLDivElement>(null);
  const seen = useCurrentView(holder);
  const active = forced ?? seen;
  const bodies = useMemo(() => given ?? provingGround(), [given]);

  const skyBox = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const names = useRef(new Map<string, HTMLSpanElement>());
  const sky = useRef<Sky | null>(null);
  const [failed, setFailed] = useState(false);

  // Flight is not React's: it changes every frame. React hears only what the strip says.
  const pilot = useRef<Pilot>(pilotAt(bodies));
  const view = useRef<View>({ width: 1, height: 1 });
  const keys = useRef(new Set<string>());
  const course = useRef<Course | null>(null);
  const inPage = useRef<string | null>(null);
  const live = useRef<string | null>(null);
  const drag = useRef<{ x: number; y: number; moved: number } | null>(null);

  const [looking, setLooking] = useState<Body | null>(null);
  const [docked, setDocked] = useState<Body | null>(null);
  const [page, setPage] = useState<{ title: string; url: string } | null>(null);

  const byId = (id: string | null) => bodies.find((b) => b.id === id) ?? null;

  const goTo = (body: Body, enter: boolean) => {
    if (inPage.current) return;
    course.current = { id: body.id, cover: enter ? 1 : 0.55, enter };
  };

  const leave = () => {
    const id = inPage.current;
    if (!id) return;
    inPage.current = null;
    setDocked(null);
    pages.dock(false);
    course.current = { id, cover: 0.5, enter: false };
    skyBox.current?.focus();
  };

  // "Search moves you": the app can fly you to a link, and into its page.
  useCoreEvent<{ url: string; enter: boolean }>('space.fly', ({ url, enter }) => {
    const body = bodies.find((b) => b.url === url);
    if (!body) return;
    if (inPage.current) {
      inPage.current = null;
      setDocked(null);
      pages.dock(false);
    }
    goTo(body, enter === true);
  });

  // What the page says: scrolling and pinching over it still move you, a click goes in, Esc comes out.
  const hear = (heard: Heard) => {
    if (heard.what !== 'said') return;
    const said = heard.said;
    if (Array.isArray(said.wheel) && !inPage.current) {
      course.current = null;
      nudge(pilot.current, bodies, view.current, -Number(said.wheel[1]) || 0);
    }
    if (typeof said.pinch === 'number' && said.pinch > 0 && !inPage.current) {
      course.current = null;
      nudge(pilot.current, bodies, view.current, Math.log(said.pinch) * 900);
    }
    if (said.enter && !inPage.current) {
      const body = byId(live.current);
      if (body) goTo(body, true);
    }
    if (said.leave) leave();
    // The page that holds YouTube's player lives on this Mac; its own address is nobody's business.
    if (typeof said.title === 'string' && typeof said.url === 'string' && !said.url.startsWith('http://127.0.0.1')) {
      setPage({ title: said.title, url: said.url });
    }
  };
  const hearRef = useRef(hear);
  hearRef.current = hear;
  useEffect(() => pages.heard((h) => hearRef.current(h)), []);

  // ⌘2 while you are in a page brings you back out: the menu answers whichever web view has the keyboard.
  useCoreEvent<{ action: string }>('menu', ({ action }) => {
    if (action === 'room.space' && inPage.current) leave();
  });

  // The sky is made the first time Space is shown, and kept.
  useEffect(() => {
    if (!active || !canvas.current || !skyBox.current) return;
    if (!sky.current) {
      try {
        sky.current = createSky(canvas.current, bodies);
      } catch {
        setFailed(true);
        return;
      }
    }
    const box = skyBox.current;
    const fit = () => {
      view.current = { width: Math.max(1, box.clientWidth), height: Math.max(1, box.clientHeight) };
      sky.current?.resize(view.current.width, view.current.height);
    };
    fit();
    box.focus();
    const watch = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(fit);
    watch?.observe(box);

    let frame = 0;
    let last = performance.now();
    let aimed: string | null = null;
    const tick = (now: number) => {
      const dt = Math.min(0.05, Math.max(0, (now - last) / 1000));
      last = now;
      const me = pilot.current;
      const v = view.current;

      const flying = course.current;
      const to = flying ? byId(flying.id) : null;
      if (flying && to) {
        if (approach(me, to, v, flying.cover, dt)) {
          course.current = null;
          if (flying.enter) {
            inPage.current = to.id;
            setDocked(to);
            pages.dock(true);
          }
        }
      } else if (!inPage.current) {
        step(me, held(keys.current), bodies, dt);
      }

      const target = byId(inPage.current) ?? to ?? aim(me, bodies, v);
      if ((target?.id ?? null) !== aimed) {
        aimed = target?.id ?? null;
        setLooking(target);
      }

      // One page is live at a time: the one you are looking at, once it is big enough to read.
      const shown = byId(live.current);
      const shownSight = shown ? sight(me, shown, v) : null;
      if (shown && shown.id !== inPage.current && (!shownSight || shownSight.cover < LIVE_UNTIL || shown.id !== target?.id)) {
        live.current = null;
        setPage(null);
        pages.close();
      }
      const s = target ? sight(me, target, v) : null;
      const box0 = box.getBoundingClientRect();
      if (IN_APP && target && s && !live.current && s.cover >= LIVE_FROM) {
        live.current = target.id;
        setPage({ title: target.name, url: target.url });
        pages.open(target.url, { x: box0.left + s.rect.x, y: box0.top + s.rect.y, width: s.rect.width, height: s.rect.height, zoom: s.cover });
      }
      const current = byId(live.current);
      const at = current ? sight(me, current, v) : null;
      // Anything the app draws over the sky (⌘K, a sheet, the drawer) would be under the page, so the page steps aside for it.
      const covered = hidden(box, box0);
      if (current && at) {
        if (inPage.current === current.id) {
          pages.place(covered ? null : { x: box0.left, y: box0.top, width: v.width, height: v.height, zoom: 1 });
        } else if (!covered && inside(at.rect, v)) {
          pages.place({ x: box0.left + at.rect.x, y: box0.top + at.rect.y, width: at.rect.width, height: at.rect.height, zoom: at.cover });
        } else {
          pages.place(null);
        }
        // Flying free, a page that all but fills the view takes you the rest of the way in.
        if (!inPage.current && !course.current && at.cover >= PULLED_IN && at.off < 0.08) goTo(current, true);
      }

      sky.current?.draw(me, now / 1000, inPage.current ?? (current && at && inside(at.rect, v) ? current.id : null));

      for (const body of bodies) {
        const el = names.current.get(body.id);
        if (!el) continue;
        const b = sight(me, body, v);
        const on = !!b && !inPage.current && b.rect.x + b.rect.width / 2 > -200 && b.rect.x + b.rect.width / 2 < v.width + 200;
        el.style.display = on ? '' : 'none';
        if (b && on) {
          const below = Math.max(b.radius, 10) + 8;
          el.style.transform = `translate(${b.rect.x + b.rect.width / 2}px, ${b.rect.y + b.rect.height / 2 + below}px)`;
          el.style.opacity = String(Math.min(1, 0.45 + body.magnitude / 14));
        }
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      watch?.disconnect();
      keys.current.clear();
      // Another view is showing: the page waits out of sight, where you left it.
      pages.place(null);
    };
  }, [active, bodies]);

  useEffect(
    () => () => {
      pages.close();
      sky.current?.dispose();
      sky.current = null;
    },
    [],
  );

  const pointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.focus();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, y: e.clientY, moved: 0 };
  };
  const pointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d || inPage.current) return;
    const dx = e.clientX - d.x;
    const dy = e.clientY - d.y;
    d.x = e.clientX;
    d.y = e.clientY;
    d.moved += Math.abs(dx) + Math.abs(dy);
    if (d.moved > 4) {
      course.current = null;
      // You take hold of the sky and pull it: dragging right turns you left.
      look(pilot.current, dx * 0.0032, dy * 0.0032);
    }
  };
  const pointerUp = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    drag.current = null;
    if (!d || d.moved > 4) return;
    const box = e.currentTarget.getBoundingClientRect();
    const body = sky.current?.pick(pilot.current, e.clientX - box.left, e.clientY - box.top);
    if (body) goTo(body, e.detail >= 2);
  };
  const wheel = (e: WheelEvent<HTMLDivElement>) => {
    if (inPage.current) return;
    course.current = null;
    // A pinch on a trackpad arrives as a wheel with ctrl held, in small numbers.
    nudge(pilot.current, bodies, view.current, -e.deltaY * (e.ctrlKey ? 6 : 1));
  };

  // The sky's own keys, heard only while the sky has the keyboard. Esc is the shell's, and reaches here as its command.
  const keyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === 'Enter') {
      const body = byId(live.current) ?? looking;
      if (body) goTo(body, true);
      e.preventDefault();
      return;
    }
    if (e.key === 'Escape') {
      course.current = null;
      return;
    }
    if (KEYS[e.code]) {
      course.current = null;
      keys.current.add(e.code);
      e.preventDefault();
      e.stopPropagation();
    }
  };
  const keyUp = (e: KeyboardEvent<HTMLDivElement>) => {
    keys.current.delete(e.code);
  };

  const here = docked ?? looking;
  return (
    <div className="space" ref={holder}>
      <header className="space-strip">
        <span className="crumb">Space</span>
        <span className="space-sample" title="Six real sites and four links on them, placed by hand until home and search fill the sky.">
          Proving ground
        </span>
        <span className="space-where" aria-live="polite">
          {here ? (
            <>
              <strong>{docked && page ? page.title || here.name : here.name}</strong>
              <span className="space-url">{docked && page ? page.url : here.url}</span>
            </>
          ) : (
            <span className="space-url">Nothing ahead. Turn to find a light.</span>
          )}
        </span>
        {docked ? (
          <span className="space-acts">
            <button type="button" className="night-button" onClick={() => pages.back()} aria-label="Back">
              ‹
            </button>
            <button type="button" className="night-button" onClick={() => pages.forward()} aria-label="Forward">
              ›
            </button>
            <button type="button" className="night-button" onClick={leave}>
              Back out
            </button>
          </span>
        ) : (
          here && (
            <span className="space-acts">
              <button type="button" className="night-button" onClick={() => goTo(here, true)}>
                Go in
              </button>
            </span>
          )
        )}
      </header>

      <div
        className="space-sky"
        ref={skyBox}
        tabIndex={0}
        role="application"
        aria-label="The sky. W A S D to move, drag or the arrow keys to look, scroll to fly toward what you face, Return to go in, Esc to come out."
        onPointerDown={pointerDown}
        onPointerMove={pointerMove}
        onPointerUp={pointerUp}
        onWheel={wheel}
        onKeyDown={keyDown}
        onKeyUp={keyUp}
        onBlur={() => keys.current.clear()}
      >
        <canvas ref={canvas} className="space-canvas" />
        {bodies.map((b) => (
          <span
            key={b.id}
            className="space-label"
            ref={(el) => {
              if (el) names.current.set(b.id, el);
              else names.current.delete(b.id);
            }}
          >
            {b.name}
          </span>
        ))}
        {failed && <p className="space-failed">This Mac couldn't draw the sky in 3D.</p>}
        {!docked && (
          <p className="space-hint" data-text="secondary">
            {IN_APP
              ? 'W A S D move · drag to look · scroll to fly in · click a light to go to it · Return to go in · Esc to come out'
              : 'W A S D move · drag to look · scroll to fly in · click a light to go to it. Pages open in the Mac app.'}
          </p>
        )}
      </div>
    </div>
  );
}
