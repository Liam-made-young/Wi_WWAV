// The Now strip (docs/SPEC.md 2.2): Heat's olive LCD grown to 520 × 44 pt
// (440 below 1180 pt), split by an etched divider. The task on the left
// answers "what am I doing", the track on the right "what am I hearing".
// Its lights stand in the 44 pt squares stem/geometry.ts gives them; the
// rest of the track half expands the player.

import { type CSSProperties, type RefObject, useEffect, useRef, useState } from 'react';
import { LIGHT_HIT, NOW_STRIP, NOW_STRIP_NARROW_W, stemLights, trackHalfX } from '../shared/stems/geometry';
import { glowTone, keyColor, parseKey } from '../styles/keyColor';
import { type Anchor, cursorAt } from './cursor';
import type { RoomId } from './rooms';
import { StemLight, useStemPointer } from './StemLights';
import { type ConsoleTransport, consoleLine, EMPTY_TRACK, elapsed, pausedLine, trackTime } from './strip';
import type { Player } from './usePlayer';
import type { TaskHalf } from './useTaskHalf';

interface Props {
  narrow: boolean;
  task: TaskHalf;
  player: Player;
  room: RoomId;
  /** The Console's transport while it has a session open; the Console sets it. */
  consoleTransport: ConsoleTransport | null;
  onTask(taskId: string | null): void;
  onExpand(): void;
}

const LEVEL_FILL = { Overdue: 'overdue', Hot: 'hot', Warm: 'warm', Cool: 'cool', Done: 'cool' } as const;

export function NowStrip({ narrow, task, player, room, consoleTransport, onTask, onExpand }: Props) {
  const width = narrow ? NOW_STRIP_NARROW_W : NOW_STRIP.w;
  const trackX = trackHalfX(width);
  return (
    <div className="strip" style={{ width, height: NOW_STRIP.h }} data-narrow={narrow}>
      <button
        type="button"
        className="strip-half strip-task"
        style={{ width: trackX - 1 }}
        onClick={() => onTask(task.taskId)}
      >
        <span className="strip-line1" data-text="body">
          {task.line1}
        </span>
        <span className="strip-line2" data-text="secondary">
          {task.line2}
        </span>
        {task.level && (
          <span className="strip-meter" aria-hidden="true">
            <span
              className="strip-fill strip-heat"
              data-level={LEVEL_FILL[task.level]}
              style={{ width: `${Math.round(task.meter * 100)}%` }}
            />
          </span>
        )}
      </button>
      <span className="strip-divider" style={{ left: trackX - 1 }} aria-hidden="true" />
      <div className="strip-half strip-track" style={{ left: trackX, width: width - trackX }}>
        {room === 'console' && consoleTransport ? (
          <span className="strip-line1 strip-dmg" data-text="body">
            {consoleLine(consoleTransport)}
          </span>
        ) : player.state.clip === null ? (
          <>
            <span className="strip-line1" data-text="body">
              {EMPTY_TRACK.line1}
            </span>
            <span className="strip-line2" data-text="secondary">
              {EMPTY_TRACK.line2}
            </span>
          </>
        ) : (
          <Track player={player} narrow={narrow} width={width} trackX={trackX} onExpand={onExpand} />
        )}
      </div>
    </div>
  );
}

function Track({
  player,
  narrow,
  width,
  trackX,
  onExpand,
}: {
  player: Player;
  narrow: boolean;
  width: number;
  trackX: number;
  onExpand(): void;
}) {
  const { state, mix } = player;
  const [seconds, meter] = useCursor(player.anchor, state.duration);
  const handlers = useStemPointer(player);
  const key = parseKey(state.key);
  const planet = { '--key': keyColor(key), '--glow': glowTone(key) } as CSSProperties;
  const lights = mix ? stemLights(width) : [];
  const timeLeft = lights.length ? lights[lights.length - 1].hit.x + LIGHT_HIT - trackX : 0;
  const title = pausedLine(state.pausedFor) ?? state.title ?? '';
  return (
    <>
      <button
        type="button"
        className="strip-expand"
        aria-label={`Show the player: ${state.title ?? ''}, ${elapsed(seconds)} of ${elapsed(state.duration)}`}
        onClick={onExpand}
      />
      <span className="strip-line1" data-text="body" aria-hidden="true">
        <span className="planet" style={planet} />
        <span className="strip-title">{title}</span>
      </span>
      {mix &&
        lights.map(({ stem, hit }) => (
          <StemLight
            key={stem}
            stem={stem}
            mix={mix}
            size={8}
            handlers={handlers(stem)}
            style={{ left: hit.x - trackX, top: hit.y, width: hit.w, height: hit.h }}
          />
        ))}
      <span className="strip-line2 strip-time" data-text="secondary" style={{ left: timeLeft }} aria-hidden="true">
        {trackTime(seconds, state.duration, narrow)}
      </span>
      <span className="strip-meter" aria-hidden="true">
        <span ref={meter} className="strip-fill strip-playhead" />
      </span>
    </>
  );
}

/**
 * The cursor, moved on from the clock's last anchor each frame while it
 * plays, and still otherwise: no frame is drawn while nothing plays. The
 * meter moves every frame; the time re-renders only when its second turns.
 */
function useCursor(anchor: Anchor, duration: number): [number, RefObject<HTMLSpanElement | null>] {
  const meter = useRef<HTMLSpanElement>(null);
  const [second, setSecond] = useState(() => Math.floor(cursorAt(anchor, performance.now(), duration)));
  useEffect(() => {
    let frame = 0;
    const draw = () => {
      const s = cursorAt(anchor, performance.now(), duration);
      if (meter.current) meter.current.style.width = `${duration > 0 ? (s / duration) * 100 : 0}%`;
      setSecond(Math.floor(s));
      if (anchor.speed > 0) frame = requestAnimationFrame(draw);
    };
    draw();
    return () => cancelAnimationFrame(frame);
  }, [anchor, duration]);
  return [second, meter];
}
