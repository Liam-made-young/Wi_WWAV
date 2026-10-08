// Today (docs/SPEC.md 3.5): the sketch's "today's plan". A 300 px time
// column on the left, the plan list on the right, and a 112 px Pomodoro
// panel across the bottom. The header reads "Today, Wednesday, October 7"
// over "4 blocks · 3h 10m planned · 2 due today".
//
// "+" adds a task straight into the plan; Plan my day (⇧Return) fills the
// time between now and the day's end with dashed drafts, each with its
// reason: Return accepts all, a click accepts one, Esc clears them.

import { useMemo } from 'react';
import { longDay } from '../../shared/time/format';
import { useActions } from '../actions';
import type { Id } from '../client';
import { CommitmentLines, PendingExceptions } from '../commitments/Lines';
import { copy, formatMinutes } from '../fmt';
import { useSelection, useSheets, useSpaceFilter, useTabActs, useTabKeys } from '../frame';
import { useHeat } from '../store';
import { Pomodoro } from './Pomodoro';
import { PlanList } from './PlanList';
import { useDailyNote } from './DailyNote';
import { TimeColumn } from './TimeColumn';
import './today.css';

/** A row the keyboard can stand on: a planned block, or a task. */
export type Stop = { kind: 'block'; id: Id } | { kind: 'task'; id: Id };

export function Today() {
  const { snap, idx, date } = useHeat();
  const actions = useActions();
  const { newTask } = useSheets();
  const { spaceId } = useSpaceFilter();
  const { taskId, blockId, selectTask, selectBlock } = useSelection();
  const note = useDailyNote();

  const today = snap?.derived.today;
  const inSpace = (id: Id | undefined) => !spaceId || (id !== undefined && idx.task.get(id)?.spaceId === spaceId);

  // The lists come from the snapshot in the order the core gave them; the space filter only leaves rows out.
  const view = useMemo(() => {
    const blocks = (today?.planned ?? []).flatMap((id) => {
      const b = idx.block.get(id);
      return b && (b.habitId !== undefined || inSpace(b.taskId)) ? [b] : [];
    });
    const dueToday = (today?.dueToday ?? []).filter((id) => inSpace(id));
    const hot = (today?.hotUnplanned ?? []).filter((id) => inSpace(id));
    const recurring = (today?.recurringToday ?? []).filter((r) => r.kind === 'habit' || inSpace(r.id));
    const drafts = (snap?.heatState.planDrafts ?? []).filter((d) => inSpace(d.taskId));
    return { blocks, dueToday, hot, recurring, drafts };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [today, idx, spaceId, snap?.heatState.planDrafts]);

  // The subtitle is the core's; with a space chosen it counts what is shown.
  const subtitle = !today
    ? ''
    : !spaceId
      ? today.header
      : copy.today.subtitle(
          view.blocks.length,
          formatMinutes(view.blocks.reduce((n, b) => n + b.minutes, 0)),
          view.dueToday.length,
        );

  // ↑ ↓ walk the plan list's rows, top to bottom.
  const stops: Stop[] = [
    ...view.blocks.map((b) => ({ kind: 'block' as const, id: b.id })),
    ...view.dueToday.map((id) => ({ kind: 'task' as const, id })),
    ...view.recurring.flatMap((r) => (r.kind === 'task' ? [{ kind: 'task' as const, id: r.id }] : [])),
    ...view.hot.map((id) => ({ kind: 'task' as const, id })),
  ];
  const at = stops.findIndex((s) => (s.kind === 'block' ? s.id === blockId : s.id === taskId));

  useTabActs({
    plus: { run: () => newTask({ intoPlan: true }) },
    secondary: { run: () => void actions.planMyDay() },
    count: snap ? subtitle : null,
  });
  useTabKeys({
    move(by) {
      if (stops.length === 0) return;
      const next = stops[at < 0 ? (by === 1 ? 0 : stops.length - 1) : Math.min(stops.length - 1, Math.max(0, at + by))];
      if (next.kind === 'block') selectBlock(next.id);
      else selectTask(next.id);
    },
    escape: () => note.leave(),
  });

  return (
    <div className="heat-today">
      <header className="heat-today-head">
        <h1 className="heat-heading">{copy.today.header(longDay(date))}</h1>
        <p className="heat-subtitle" data-text="secondary">
          {subtitle}
        </p>
        <CommitmentLines />
        <PendingExceptions />
      </header>
      <div className="heat-today-body">
        <TimeColumn blocks={view.blocks} drafts={view.drafts} />
        <PlanList view={view} note={note} onPlan={() => void actions.planMyDay()} />
      </div>
      <Pomodoro />
    </div>
  );
}
