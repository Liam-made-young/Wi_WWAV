// Calendar (docs/SPEC.md 3.7): the month grid as Heat has it, and Week and
// Day views on the PKM's 44 px-an-hour grid. M, W and D switch views, ← and →
// page, T jumps to today; ⇧← ⇧→ and ⇧↑ ⇧↓ move the selected day by a day or a
// week, and ↑ ↓ walk the tasks in view. "+" adds a task due 11:59 PM on the
// selected day; the tab's secondary act is Today.
//
// A deadline is a small heat-coloured flag at its time on the right of its
// column; the week's all-day strip holds due pills and milestone beads; an
// unscheduled tray on the left lists this week's open tasks that have no
// block, in heat order, ready to drag in.

import { useEffect, useMemo, useState } from 'react';
import { atMinute, type DayKey } from '../../shared/time/zone';
import type { Id } from '../client';
import { PendingExceptions } from '../commitments/Lines';
import { Schedule } from '../commitments/Schedule';
import { copy, plural } from '../fmt';
import { useFrame, useSelection, useSheets, useSpaceFilter, useTabActs, useTabKeys, useTabScope } from '../frame';
import { useHeat } from '../store';
import { daysOf, type Mode, moveSelected, page, titleOf, weekDays } from './days';
import { dueByDay, type DueItem } from './items';
import { MonthView } from './MonthView';
import { TimeGrid } from './TimeGrid';
import './calendar.css';

const MODES: { mode: Mode; label: string; key: string }[] = [
  { mode: 'month', label: copy.calendar.month, key: 'M' },
  { mode: 'week', label: copy.calendar.week, key: 'W' },
  { mode: 'day', label: copy.calendar.day, key: 'D' },
];

export function Calendar() {
  const { snap, idx, tz, date, setRange } = useHeat();
  const { spaceId } = useSpaceFilter();
  const { taskId, selectTask } = useSelection();
  const { newTask } = useSheets();
  const { focusInfo } = useFrame();
  const { active } = useTabScope();

  const [mode, setMode] = useState<Mode>('month');
  const [anchorDay, setAnchor] = useState<DayKey | null>(null);
  const [pickedDay, setPicked] = useState<DayKey | null>(null);
  const anchor = anchorDay ?? date;
  const selected = pickedDay ?? anchorDay ?? date;
  const days = useMemo(() => daysOf(mode, anchor), [mode, anchor]);

  // The snapshot is asked for the days the view shows, so its blocks and events cover them.
  // (Day asks for its whole week, which the tray reads.)
  useEffect(() => {
    if (!active) return;
    const shown = mode === 'day' ? weekDays(anchor) : days;
    setRange({ from: shown[0], to: shown[shown.length - 1] });
  }, [active, mode, anchor, days, setRange]);

  const inSpace = (id: Id | undefined) => !spaceId || (id !== undefined && idx.task.get(id)?.spaceId === spaceId);
  const due = useMemo(() => {
    if (!snap) return new Map<DayKey, DueItem[]>();
    const all = dueByDay(snap, days);
    return new Map([...all].map(([day, items]) => [day, items.filter((i) => inSpace(i.taskId))]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snap, days, spaceId]);

  const go = (next: Mode, day: DayKey) => {
    setMode(next);
    setAnchor(day);
    setPicked(day);
  };
  const paged = (dir: 1 | -1) => {
    const next = page(mode, anchor, dir);
    setAnchor(next);
    setPicked(null);
  };
  const today = () => {
    setAnchor(date);
    setPicked(date);
  };
  const step = (by: number) => {
    const moved = moveSelected(mode, anchor, selected, by);
    setAnchor(moved.anchor);
    setPicked(moved.selected);
  };

  // What ↑ ↓ walk: the tasks drawn in view, day by day. A month cell draws three.
  const walk = days.flatMap((d) => {
    const items = due.get(d) ?? [];
    return (mode === 'month' ? items.slice(0, 3) : items).map((i) => i.taskId);
  });

  const count = useMemo(
    () => [...due.values()].reduce((n, items) => n + items.filter((i) => !i.done).length, 0),
    [due],
  );
  useTabActs({
    plus: { run: () => newTask({ due: atMinute(selected, 23 * 60 + 59, tz), spaceId: spaceId ?? undefined }) },
    secondary: { run: today },
    count: snap ? `${titleOf(mode, anchor)} · ${plural(count, 'task')} due` : null,
  });
  useTabKeys({
    key(e) {
      if (e.command || e.alt) return false;
      if (e.shift) {
        const by = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 }[e.key];
        if (by === undefined) return false;
        step(by);
        return true;
      }
      switch (e.key.length === 1 ? e.key.toLowerCase() : e.key) {
        case 'm':
          return (setMode('month'), true);
        case 'w':
          return (setMode('week'), true);
        case 'd':
          return (setMode('day'), true);
        case 't':
          return (today(), true);
        case 'ArrowLeft':
          return (paged(-1), true);
        case 'ArrowRight':
          return (paged(1), true);
        default:
          return false;
      }
    },
    move(by) {
      if (walk.length === 0) return;
      const at = taskId ? walk.indexOf(taskId) : -1;
      const next = walk[at < 0 ? (by === 1 ? 0 : walk.length - 1) : Math.min(walk.length - 1, Math.max(0, at + by))];
      selectTask(next);
      const t = idx.task.get(next);
      const d = days.find((day) => due.get(day)?.some((i) => i.taskId === next));
      if (t && d) setPicked(d);
    },
    edit: focusInfo,
  });

  if (!snap) return <div className="heat-cal" />;

  return (
    <div className="heat-cal" data-mode={mode}>
      <header className="heat-cal-head">
        <h1 className="heat-heading">{titleOf(mode, anchor)}</h1>
        <div className="heat-cal-nav">
          <button
            type="button"
            className="heat-nav"
            aria-label={`Previous ${mode}`}
            title={`Previous ${mode} (←)`}
            onClick={() => paged(-1)}
          >
            ‹
          </button>
          <button type="button" className="gel" title="Today (T)" onClick={today}>
            Today
          </button>
          <button
            type="button"
            className="heat-nav"
            aria-label={`Next ${mode}`}
            title={`Next ${mode} (→)`}
            onClick={() => paged(1)}
          >
            ›
          </button>
          <div className="switcher heat-cal-modes" role="radiogroup" aria-label="View">
            {MODES.map((m) => (
              <button
                key={m.mode}
                type="button"
                role="radio"
                className="segment"
                aria-checked={mode === m.mode}
                title={`${m.label} (${m.key})`}
                onClick={() => setMode(m.mode)}
              >
                {m.label}
              </button>
            ))}
          </div>
        </div>
      </header>
      <Schedule day={selected} />
      <PendingExceptions />
      {mode === 'month' ? (
        <MonthView
          days={days}
          anchor={anchor}
          selected={selected}
          due={due}
          onPick={setPicked}
          onMore={(day) => go('day', day)}
        />
      ) : (
        <TimeGrid days={days} selected={selected} due={due} onPick={setPicked} spaceId={spaceId} />
      )}
    </div>
  );
}
