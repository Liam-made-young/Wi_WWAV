// A stem light (2.2, 8.3): a disc in PRANA's stem colour whose state is
// its shape, never its colour alone. Filled is audible; a stem-colour ring
// with an open centre is muted; an outer ring is soloed; a faint fill in a
// dashed ring is silent under someone else's solo. A 1 pt ring in the
// register's ink carries the contrast on any ground. A click works like a
// moon's, through the one stem gesture: it mutes at once, and a second
// click within 250 ms reverts that and solos instead.

import { type CSSProperties, type PointerEvent, useRef } from 'react';
import { idleGesture, type GestureInput, stemGesture } from '../shared/stems/gesture';
import { type Mix, stemShape, voiceLabel } from '../shared/stems/mix';
import type { Stem } from '../shared/stems/stems';
import { IS_MAC } from './platform';
import type { Player } from './usePlayer';

/** The pointer half of the gesture for a surface of lights; keys come through the router. */
export function useStemPointer(player: Player) {
  const gesture = useRef(idleGesture());
  const send = (input: GestureInput) => {
    if (!player.mix) return;
    const step = stemGesture(gesture.current, input, { mix: player.mix, stage: null });
    gesture.current = step.state;
    if (step.actions.length) void player.stems(step.actions);
  };
  return (stem: Stem) => ({
    onPointerDown(e: PointerEvent) {
      if (e.button !== 0) return;
      e.currentTarget.setPointerCapture(e.pointerId);
      const command = IS_MAC ? e.metaKey : e.ctrlKey;
      const at = { x: e.clientX, y: e.clientY };
      send({
        type: 'down',
        pointer: e.pointerId,
        command,
        target: { kind: 'stem', stem },
        at,
        time: performance.now(),
      });
    },
    onPointerMove(e: PointerEvent) {
      send({ type: 'move', pointer: e.pointerId, at: { x: e.clientX, y: e.clientY }, time: performance.now() });
    },
    onPointerUp(e: PointerEvent) {
      send({ type: 'up', pointer: e.pointerId, time: performance.now() });
    },
    onPointerCancel(e: PointerEvent) {
      send({ type: 'cancel', pointer: e.pointerId });
    },
  });
}

interface Props {
  stem: Stem;
  mix: Mix;
  /** The disc's diameter in pt: 8 in the strip. */
  size: number;
  style?: CSSProperties;
  handlers: ReturnType<ReturnType<typeof useStemPointer>>;
}

export function StemLight({ stem, mix, size, style, handlers }: Props) {
  const shape = stemShape(mix, stem, false, 'desk');
  const r = size / 2;
  const box = size * 2 + 2;
  const colour = `var(--stem-${stem})`;
  return (
    <button
      type="button"
      className="stem-light"
      data-stem={stem}
      aria-label={voiceLabel(mix, stem)}
      style={style}
      {...handlers}
    >
      <svg width={box} height={box} viewBox={`${-box / 2} ${-box / 2} ${box} ${box}`} aria-hidden="true">
        {shape.outerRing && <circle r={r + 3} fill="none" stroke="currentColor" strokeWidth={2} />}
        <circle r={r} fill={colour} fillOpacity={shape.fill} />
        {shape.stemRing > 0 && <circle r={r - 1} fill="none" stroke={colour} strokeWidth={2} />}
        <circle
          r={r + 0.5}
          fill="none"
          stroke="currentColor"
          strokeWidth={1}
          strokeDasharray={shape.inkRing === 'dashed' ? '1.5 1.5' : undefined}
        />
      </svg>
    </button>
  );
}
