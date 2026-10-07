// Week and Day (docs/SPEC.md 3.7): the PKM's grid, 44 px an hour for the whole
// day. Other calendars' events sit grey and hatched behind the blocks; a
// deadline is a small heat-coloured flag at its time on the right of its
// column ("due 11:59 PM"), so a block and its deadline read on one line. The
// all-day strip holds due pills and milestone beads. The tray on the left
// lists this week's open tasks that have no block, in heat order, ready to
// drag onto a column (a block) or a day's strip (a scheduled day).

import { type DragEvent, useLayoutEffect, useRef } from 'react';
import { clock, longDay, shortMonthDay } from '../../shared/time/format';
import { addDays, type DayKey, dayKey, keyParts, minuteOfDay, startOfDay } from '../../shared/time/zone';
import { useActions } from '../actions';
import type { HeatLevel, Id, TimeBlock } from '../client';
import { copy, formatMinutes } from '../fmt';
import { useSelection } from '../frame';
import { useHeat, useNow } from '../store';
import { carriesTask, draggedTask, dragTask, Heat } from '../ui';
import { weekDays } from './days';
import type { DueItem } from './items';
import { Pill } from './Pill';

const HOUR_PX = 44;
const GRID_HEIGHT = 24 * HOUR_PX;
const toY = (min: number) => (min / 60) * HOUR_PX;
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const LEVEL: Record<HeatLevel, string> = { Overdue: 'overdue', Hot: 'hot', Warm: 'warm', Cool: 'cool', Done: 'done' };

/** "12 AM", "7 AM", "12 PM", "11 PM" */
const hourLabel = (h: number) => `${h % 12 === 0 ? 12 : h % 12} ${h < 12 ? 'AM' : 'PM'}`;

interface Props {
  days: DayKey[];
  selected: DayKey;
  due: Map<DayKey, DueItem[]>;
  onPick(day: DayKey): void;
  spaceId: Id | null;
}

export function TimeGrid({ days, selected, due, onPick, spaceId }: Props) {
  const { snap, idx, tz, date } = useHeat();
  const { taskId, blockId, selectTask, selectBlock } = useSelection();
  const actions = useActions();
  const now = useNow(60_000);
  const nowMin = minuteOfDay(now, tz);
  const scroller = useRef<HTMLDivElement>(null);

  const inSpace = (id: Id | undefined) => !spaceId || (id !== undefined && idx.task.get(id)?.spaceId === spaceId);
  const cols = days.length;
  const key = days[0];

  // Scrolled to now a third of the way down when today is in view, else to 7 AM.
  const placed = useRef<string | null>(null);
  useLayoutEffect(() => {
    const el = scroller.current;
    if (!el || !snap || placed.current === `${cols}${key}`) return;
    placed.current = `${cols}${key}`;
    const top = days.includes(date) ? toY(nowMin) - el.clientHeight / 3 : toY(7 * 60);
    el.scrollTop = Math.max(0, Math.min(GRID_HEIGHT - el.clientHeight, top));
  }, [snap, cols, key, days, date, nowMin]);

  if (!snap) return null;

  const week = weekDays(key);
  const blocksOn = (day: DayKey) =>
    snap.records.timeBlock.filter((b) => b.date === day && (b.habitId !== undefined || inSpace(b.taskId)));
  const titleOf = (b: TimeBlock) =>
    (b.habitId ? idx.habit.get(b.habitId)?.title : idx.task.get(b.taskId ?? '')?.title) ?? '';

  // The tray: this week's open tasks with no block this week, in the snapshot's heat order.
  const blocked = new Set(
    snap.records.timeBlock.filter((b) => b.date >= week[0] && b.date <= week[6]).map((b) => b.taskId),
  );
  const inWeek = (d: DayKey | undefined) => d !== undefined && d >= week[0] && d <= week[6];
  const tray = snap.derived.lists.allOpen.filter((id) => {
    const t = idx.task.get(id);
    if (!t || !inSpace(id) || blocked.has(id)) return false;
    const dueDay = t.rrule
      ? (snap.derived.tasks[id]?.next ?? undefined)
      : t.due !== null
        ? dayKey(t.due, tz)
        : undefined;
    return inWeek(dueDay ?? undefined) || inWeek(t.scheduledDate);
  });

  const dropOnColumn = (e: DragEvent, day: DayKey) => {
    const id = draggedTask(e);
    if (!id) return;
    e.preventDefault();
    const top = (e.currentTarget as HTMLElement).getBoundingClientRect().top;
    void actions.dropAt(id, ((e.clientY - top) / HOUR_PX) * 60, day);
  };
  const dropOnDay = (e: DragEvent, day: DayKey) => {
    const id = draggedTask(e);
    if (!id) return;
    e.preventDefault();
    void actions.schedule(id, day);
  };
  const over = (e: DragEvent) => {
    if (carriesTask(e)) e.preventDefault();
  };

  return (
    <div className="heat-week" style={{ ['--cols' as string]: cols }}>
      <aside className="heat-tray" aria-label={copy.calendar.unscheduled}>
        <h2 className="heat-section-title" data-text="secondary">
          {copy.calendar.unscheduled}
        </h2>
        <div role="listbox" aria-label={copy.calendar.unscheduled}>
          {tray.map((id) => {
            const t = idx.task.get(id)!;
            const d = snap.derived.tasks[id];
            return (
              <div
                key={id}
                role="option"
                className="heat-tray-row"
                data-dense
                aria-selected={taskId === id}
                {...dragTask(id)}
                onClick={() => selectTask(id)}
              >
                <span className="heat-tray-title">{t.title}</span>
                <span className="heat-tray-meta">
                  {d && <Heat level={d.heat.level} v={d.heat.v} word={false} />}
                  <span className="heat-tray-sub" data-text="secondary">
                    {formatMinutes(d?.estimate.min ?? 30)}
                  </span>
                </span>
              </div>
            );
          })}
        </div>
        {tray.length === 0 && (
          <p className="heat-tray-empty" data-text="secondary">
            {copy.status.empty}
          </p>
        )}
      </aside>

      <div className="heat-cal-main">
        <div className="heat-cal-heads">
          <span className="heat-cal-gutter" />
          {days.map((day) => (
            <button
              key={day}
              type="button"
              className="heat-cal-dayhead"
              data-dense
              data-today={day === date}
              data-selected={day === selected}
              aria-label={longDay(day)}
              aria-pressed={day === selected}
              onClick={() => onPick(day)}
            >
              <span data-text="secondary">
                {
                  WEEKDAYS[
                    new Date(Date.UTC(keyParts(day).year, keyParts(day).month - 1, keyParts(day).day)).getUTCDay()
                  ]
                }
              </span>
              <span className="heat-cal-num">{keyParts(day).day}</span>
            </button>
          ))}
        </div>

        <div className="heat-cal-allday">
          <span className="heat-cal-gutter" data-text="secondary">
            All day
          </span>
          {days.map((day) => {
            const items = (due.get(day) ?? []).filter((i) => !i.done);
            const beads = snap.records.milestone.filter((m) => m.date === day && (!spaceId || m.spaceId === spaceId));
            const dayStart = startOfDay(day, tz);
            const dayEnd = startOfDay(addDays(day, 1), tz);
            const events = snap.events.filter((e) => e.allDay && e.end > dayStart && e.start < dayEnd);
            return (
              <div
                key={day}
                className="heat-cal-strip"
                data-day={day}
                onDragOver={over}
                onDrop={(e) => dropOnDay(e, day)}
              >
                {events.map((e) => (
                  <span key={e.id} className="heat-allday-event" title={e.title}>
                    {e.title}
                  </span>
                ))}
                {beads.map((m) => (
                  <span
                    key={m.id}
                    className="heat-milestone"
                    data-done={m.done}
                    title={`${m.title}, ${shortMonthDay(m.date)}`}
                  >
                    <span className="heat-bead-glass" aria-hidden="true" />
                    {m.title}
                  </span>
                ))}
                {items.map((i) => (
                  <Pill
                    key={`${i.taskId}-${i.day}`}
                    item={i}
                    selected={taskId === i.taskId}
                    onSelect={() => selectTask(i.taskId)}
                  />
                ))}
              </div>
            );
          })}
        </div>

        <div className="heat-cal-scroll" ref={scroller}>
          <div className="heat-cal-grid" style={{ height: GRID_HEIGHT }}>
            <div className="heat-cal-hours" aria-hidden="true">
              {Array.from({ length: 24 }, (_, h) => (
                <span key={h} className="heat-hour-label" data-text="secondary" style={{ top: toY(h * 60) + 2 }}>
                  {h === 0 ? '' : hourLabel(h)}
                </span>
              ))}
            </div>
            {days.map((day) => {
              const dayStart = startOfDay(day, tz);
              const dayEnd = startOfDay(addDays(day, 1), tz);
              const events = snap.events.filter((e) => !e.allDay && e.end > dayStart && e.start < dayEnd);
              const flags = (due.get(day) ?? []).filter((i) => !i.done).sort((a, b) => a.minute - b.minute);
              let last = -Infinity;
              return (
                <div
                  key={day}
                  className="heat-cal-col"
                  data-day={day}
                  data-today={day === date}
                  onDragOver={over}
                  onDrop={(e) => dropOnColumn(e, day)}
                >
                  {events.map((e) => {
                    const from = e.start <= dayStart ? 0 : minuteOfDay(e.start, tz);
                    const to = e.end >= dayEnd ? 1440 : minuteOfDay(e.end, tz);
                    return (
                      <div
                        key={e.id}
                        className="heat-event"
                        data-event={e.id}
                        style={{ top: toY(from), height: Math.max(16, toY(to) - toY(from)) }}
                      >
                        <span className="heat-event-title">{e.title}</span>
                      </div>
                    );
                  })}
                  {blocksOn(day).map((b) => {
                    const end = b.start + b.minutes;
                    const level = b.taskId ? snap.derived.tasks[b.taskId]?.heat.level : undefined;
                    return (
                      <div
                        key={b.id}
                        className="heat-block"
                        data-block={b.id}
                        data-level={level ? LEVEL[level] : 'none'}
                        data-selected={blockId === b.id}
                        data-current={day === date && b.start <= nowMin && nowMin < end}
                        data-finished={day === date && end <= nowMin}
                        style={{ top: toY(b.start), height: Math.max(16, toY(b.minutes)) }}
                        title={`${titleOf(b)}, ${clock(b.start)}, ${formatMinutes(b.minutes)}`}
                        onClick={() => selectBlock(b.id)}
                      >
                        <span className="heat-block-title">{titleOf(b)}</span>
                      </div>
                    );
                  })}
                  {flags.map((i) => {
                    const top = Math.max(toY(i.minute), last + 16);
                    last = top;
                    return (
                      <div
                        key={`${i.taskId}-${i.day}`}
                        className="heat-flag"
                        data-flag={i.taskId}
                        data-level={i.level ? LEVEL[i.level] : 'none'}
                        style={{ top }}
                        title={`${i.title}, due ${clock(i.minute)}`}
                        onClick={() => selectTask(i.taskId)}
                      >
                        {copy.calendar.dueFlag(clock(i.minute))}
                      </div>
                    );
                  })}
                  {day === date && (
                    <div
                      className="heat-now-mark"
                      data-now-line
                      style={{ top: toY(nowMin), left: 0 }}
                      aria-hidden="true"
                    />
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
