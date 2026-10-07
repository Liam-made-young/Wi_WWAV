// The time column (docs/SPEC.md 3.5): 44 px an hour, a 15-minute snap, from
// 7 AM to midnight, scrolled so now sits a third of the way down, with a 1 px
// red line at the current minute. Blocks wear a 3 px left border in their
// task's heat colour, the title and the length; the block now is in has the
// Aqua ring and finished ones dim with a check. Events from other calendars
// sit behind them, grey, hatched and read-only, so a clash shows as an
// overlap. A task dropped here makes a block as long as its estimate; a
// block's bottom edge resizes it, its body moves it; P and ⌥↑ ⌥↓ do the
// same from the keyboard.

import { type CSSProperties, type DragEvent, type PointerEvent, useLayoutEffect, useRef, useState } from 'react';
import { addDays, minuteOfDay, startOfDay } from '../../shared/time/zone';
import { useActions } from '../actions';
import type { Draft, HeatLevel, Id, TimeBlock } from '../client';
import { clock, formatMinutes } from '../fmt';
import { useSelection } from '../frame';
import { useHeat, useNow } from '../store';
import { carriesTask, draggedTask } from '../ui';
import {
  COLUMN_END,
  COLUMN_HEIGHT,
  COLUMN_START,
  HOUR_PX,
  initialScroll,
  minutesToY,
  movedStart,
  nowLineY,
  resizedMinutes,
  snap,
  yToMinutes,
} from './gap';

const HOURS = Array.from({ length: (COLUMN_END - COLUMN_START) / 60 }, (_, i) => COLUMN_START / 60 + i);

/** "7 AM", "12 PM", "11 PM" */
const hourLabel = (h: number) => `${h % 12 === 0 ? 12 : h % 12} ${h < 12 ? 'AM' : 'PM'}`;

const LEVEL: Record<HeatLevel, string> = { Overdue: 'overdue', Hot: 'hot', Warm: 'warm', Cool: 'cool', Done: 'done' };

interface Drag {
  id: Id;
  start: number;
  minutes: number;
}

export function TimeColumn({ blocks, drafts }: { blocks: TimeBlock[]; drafts: Draft[] }) {
  const { snap: s, idx, tz, date } = useHeat();
  const actions = useActions();
  const { blockId, selectBlock } = useSelection();
  const now = useNow(60_000);
  const nowMin = minuteOfDay(now, tz);
  const scroller = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const dragRef = useRef<Drag | null>(null);
  const [guide, setGuide] = useState<number | null>(null);

  // Scrolled so now sits a third of the way down, once, when there is something to scroll.
  const placed = useRef(false);
  useLayoutEffect(() => {
    if (placed.current || !s || !scroller.current) return;
    placed.current = true;
    scroller.current.scrollTop = initialScroll(nowMin, scroller.current.clientHeight);
  }, [s, nowMin]);

  // Other calendars' timed events that fall on today, as the minutes they cover.
  const dayStart = startOfDay(date, tz);
  const dayEnd = startOfDay(addDays(date, 1), tz);
  const events = (s?.events ?? []).flatMap((e) => {
    if (e.allDay || e.end <= dayStart || e.start >= dayEnd) return [];
    const from = Math.max(COLUMN_START, e.start <= dayStart ? 0 : minuteOfDay(e.start, tz));
    const to = Math.min(COLUMN_END, e.end >= dayEnd ? 1440 : minuteOfDay(e.end, tz));
    return to > from ? [{ id: e.id, title: e.title, from, to }] : [];
  });

  const yOf = (clientY: number) => clientY - (inner.current?.getBoundingClientRect().top ?? 0);

  // A block's body moves it and its bottom edge resizes it; a press that doesn't move selects it.
  const begin = (e: PointerEvent, b: TimeBlock, mode: 'move' | 'resize') => {
    if (e.button !== 0) return;
    e.stopPropagation();
    const y0 = e.clientY;
    let moved = false;
    const onMove = (m: globalThis.PointerEvent) => {
      const dy = m.clientY - y0;
      if (Math.abs(dy) > 3) moved = true;
      if (!moved) return;
      const next: Drag =
        mode === 'move'
          ? { id: b.id, start: movedStart(b.minutes, minutesToY(b.start) + dy), minutes: b.minutes }
          : { id: b.id, start: b.start, minutes: resizedMinutes(b.start, minutesToY(b.start + b.minutes) + dy) };
      dragRef.current = next;
      setDrag(next);
    };
    const onUp = () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onUp);
      const final = dragRef.current;
      dragRef.current = null;
      setDrag(null);
      if (!moved || !final) return selectBlock(b.id);
      void actions.setBlock(b, mode === 'move' ? { start: final.start } : { minutes: final.minutes });
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onUp);
  };

  const over = (e: DragEvent) => {
    if (!carriesTask(e)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
    setGuide(minutesToY(snap(yToMinutes(yOf(e.clientY)))));
  };
  const drop = (e: DragEvent) => {
    const id = draggedTask(e);
    setGuide(null);
    if (!id) return;
    e.preventDefault();
    void actions.dropOnColumn(id, yOf(e.clientY), date);
  };

  const line = nowLineY(nowMin);
  const title = (b: TimeBlock) =>
    (b.habitId ? idx.habit.get(b.habitId)?.title : idx.task.get(b.taskId ?? '')?.title) ?? '';

  return (
    <div className="heat-column" ref={scroller} aria-label="Today’s time column">
      <div
        className="heat-column-inner"
        ref={inner}
        style={{ height: COLUMN_HEIGHT }}
        data-column
        onDragOver={over}
        onDragLeave={() => setGuide(null)}
        onDrop={drop}
      >
        {HOURS.map((h) => (
          <div key={h} className="heat-hour" style={{ top: minutesToY(h * 60), height: HOUR_PX }}>
            <span className="heat-hour-label" data-text="secondary">
              {hourLabel(h)}
            </span>
          </div>
        ))}

        {events.map((e) => (
          <div
            key={e.id}
            className="heat-event"
            data-event={e.id}
            style={{ top: minutesToY(e.from), height: Math.max(16, minutesToY(e.to) - minutesToY(e.from)) }}
          >
            <span className="heat-event-title">{e.title}</span>
          </div>
        ))}

        {blocks.map((b) => {
          const shown = drag?.id === b.id ? drag : b;
          const end = b.start + b.minutes;
          const level = b.taskId ? s?.derived.tasks[b.taskId]?.heat.level : undefined;
          return (
            <div
              key={b.id}
              className="heat-block"
              data-block={b.id}
              data-level={level ? LEVEL[level] : 'none'}
              data-selected={blockId === b.id}
              data-current={b.date === date && b.start <= nowMin && nowMin < end}
              data-finished={b.date === date && end <= nowMin}
              data-dragging={drag?.id === b.id}
              style={
                { top: minutesToY(shown.start), height: Math.max(16, (shown.minutes / 60) * HOUR_PX) } as CSSProperties
              }
              onPointerDown={(e) => begin(e, b, 'move')}
              title={`${title(b)}, ${clock(shown.start)}, ${formatMinutes(shown.minutes)}`}
            >
              <span className="heat-block-title">
                {b.date === date && end <= nowMin ? '✓ ' : ''}
                {title(b)}
              </span>
              <span className="heat-block-length">{formatMinutes(shown.minutes)}</span>
              <span className="heat-block-grip" data-grip onPointerDown={(e) => begin(e, b, 'resize')} />
            </div>
          );
        })}

        {drafts.map((d) => (
          <div
            key={d.taskId}
            className="heat-block heat-block-draft"
            data-draft-block={d.taskId}
            style={{ top: minutesToY(d.start), height: Math.max(16, (d.minutes / 60) * HOUR_PX) }}
            onClick={() => void actions.acceptDrafts([d.taskId])}
            title={`Draft: ${idx.task.get(d.taskId)?.title}. ${d.reason} Click to accept.`}
          >
            <span className="heat-block-title">{idx.task.get(d.taskId)?.title}</span>
            <span className="heat-block-length">{formatMinutes(d.minutes)}</span>
          </div>
        ))}

        {guide !== null && (
          <div className="heat-guide" style={{ top: guide }} aria-hidden="true">
            <span>{clock(snap(yToMinutes(guide + 0.01)))}</span>
          </div>
        )}
        {line !== null && <div className="heat-now-mark" data-now-line style={{ top: line }} aria-hidden="true" />}
      </div>
    </div>
  );
}
