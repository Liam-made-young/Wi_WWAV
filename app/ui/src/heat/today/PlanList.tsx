// The plan list (docs/SPEC.md 3.5): four sections, any empty one hidden,
// and the daily note below them. Planned comes in time order with start
// times, then Due today, not planned, Recurring today ↻, and Hot, not
// planned (up to 5). While Plan my day's drafts wait they sit on top, each
// with its reason. The order is the snapshot's; this only draws it.

import { type ReactNode, useEffect, useRef } from 'react';
import { useActions } from '../actions';
import type { Draft, Id, TimeBlock } from '../client';
import { clock, copy, dueText, effectiveDue, formatMinutes } from '../fmt';
import { useSelection } from '../frame';
import { useHeat } from '../store';
import { dragTask, Heat } from '../ui';
import { minuteOfDay } from '../../shared/time/zone';
import { type DailyNote, DailyNoteLine } from './DailyNote';

export interface PlanView {
  blocks: TimeBlock[];
  dueToday: Id[];
  hot: Id[];
  recurring: { kind: 'task' | 'habit'; id: Id }[];
  drafts: Draft[];
}

export function PlanList({ view, note, onPlan }: { view: PlanView; note: DailyNote; onPlan(): void }) {
  const { snap, idx, tz, now, date, client, act } = useHeat();
  const actions = useActions();
  const { taskId, blockId, selectTask, selectBlock } = useSelection();
  const nowMin = minuteOfDay(now(), tz);
  const draftsRef = useRef<HTMLDivElement>(null);

  // Plan my day puts the keyboard on its drafts, where Return and Esc are for them.
  const had = useRef(0);
  useEffect(() => {
    if (view.drafts.length > 0 && had.current === 0) draftsRef.current?.focus({ preventScroll: true });
    had.current = view.drafts.length;
  }, [view.drafts.length]);

  // A selected row stays in view as ↑ ↓ walk the list.
  useEffect(() => {
    const id = blockId ? `heat-row-b-${blockId}` : taskId ? `heat-row-t-${taskId}` : null;
    if (id) document.getElementById(id)?.scrollIntoView?.({ block: 'nearest' });
  }, [taskId, blockId]);

  if (!snap) return <div className="heat-plan" />;
  const title = (b: TimeBlock) =>
    (b.habitId ? idx.habit.get(b.habitId)?.title : idx.task.get(b.taskId ?? '')?.title) ?? '';
  const empty = view.blocks.length === 0 && view.drafts.length === 0;
  const tickedToday = (id: Id) => idx.ticked.get(id)?.has(date) ?? false;
  const habitDone = (id: Id) => snap.derived.habits[id]?.today ?? idx.habit.get(id)?.log[date] === true;

  const toggleHabit = (id: Id) => {
    const h = idx.habit.get(id);
    if (!h) return;
    const log = { ...h.log };
    if (log[date]) delete log[date];
    else log[date] = true;
    void act(client.patch('habit', id, { log }));
  };

  return (
    <div className="heat-plan">
      <div className="heat-plan-bar">
        <button type="button" className="gel" onClick={onPlan} title="Plan my day (⇧Return)">
          {copy.tabs.secondary.Today}
        </button>
      </div>
      <div role="listbox" aria-label="Today’s plan" className="heat-plan-rows">
        {view.drafts.length > 0 && (
          <Section title="Plan my day" hint="Return accepts all. A click accepts one. Esc clears them.">
            <div ref={draftsRef} tabIndex={-1} className="heat-drafts" aria-label="Drafts">
              {view.drafts.map((d) => {
                const t = idx.task.get(d.taskId);
                return (
                  <div
                    key={d.taskId}
                    role="option"
                    aria-selected={false}
                    className="heat-row heat-row-draft"
                    data-dense
                    data-draft={d.taskId}
                    onClick={() => void actions.acceptDrafts([d.taskId])}
                  >
                    <span className="heat-row-time">{clock(d.start)}</span>
                    <span className="heat-row-title">{t?.title}</span>
                    <span className="heat-row-sub" data-text="secondary">
                      {formatMinutes(d.minutes)}
                    </span>
                    <span className="heat-row-why" data-text="secondary">
                      {d.reason}
                      {d.leftLine ? ` ${d.leftLine}.` : ''}
                    </span>
                  </div>
                );
              })}
            </div>
          </Section>
        )}

        {empty && <p className="heat-empty">{copy.today.empty}</p>}

        {view.blocks.length > 0 && (
          <Section title={copy.today.planned}>
            {view.blocks.map((b) => {
              const end = b.start + b.minutes;
              const current = b.date === date && b.start <= nowMin && nowMin < end;
              const finished = b.date === date && end <= nowMin;
              return (
                <div
                  key={b.id}
                  id={`heat-row-b-${b.id}`}
                  role="option"
                  className="heat-row"
                  data-dense
                  data-current={current}
                  data-finished={finished}
                  aria-selected={blockId === b.id}
                  onClick={() => selectBlock(b.id)}
                >
                  <span className="heat-row-time">{clock(b.start)}</span>
                  <span className="heat-row-title">
                    {finished && <span aria-label="finished">✓ </span>}
                    {title(b)}
                  </span>
                  <span className="heat-row-sub" data-text="secondary">
                    {formatMinutes(b.minutes)}
                  </span>
                </div>
              );
            })}
          </Section>
        )}

        {view.dueToday.length > 0 && (
          <Section title={copy.today.dueToday}>
            {view.dueToday.map((id) => (
              <TaskLine key={id} id={id} selected={taskId === id} onSelect={() => selectTask(id)} />
            ))}
          </Section>
        )}

        {view.recurring.length > 0 && (
          <Section title={copy.today.recurring}>
            {view.recurring.map((r) =>
              r.kind === 'task' ? (
                <TaskLine
                  key={`t-${r.id}`}
                  id={r.id}
                  selected={taskId === r.id}
                  onSelect={() => selectTask(r.id)}
                  tick={tickedToday(r.id)}
                />
              ) : (
                <div
                  key={`h-${r.id}`}
                  role="option"
                  aria-selected={false}
                  className="heat-row"
                  data-dense
                  data-done={habitDone(r.id)}
                >
                  <Tick
                    on={habitDone(r.id)}
                    label={`${idx.habit.get(r.id)?.title}, today`}
                    onToggle={() => toggleHabit(r.id)}
                  />
                  <span className="heat-row-title">{idx.habit.get(r.id)?.title}</span>
                  <span className="heat-row-sub" data-text="secondary">
                    {formatMinutes(idx.habit.get(r.id)?.minutes ?? 30)}
                  </span>
                </div>
              ),
            )}
          </Section>
        )}

        {view.hot.length > 0 && (
          <Section title={copy.today.hot}>
            {view.hot.map((id) => (
              <TaskLine key={id} id={id} selected={taskId === id} onSelect={() => selectTask(id)} />
            ))}
          </Section>
        )}
      </div>
      <DailyNoteLine note={note} />
    </div>
  );
}

function Section({ title, hint, children }: { title: string; hint?: string; children: ReactNode }) {
  return (
    <div role="group" aria-label={title} className="heat-section">
      <h3 className="heat-section-title" data-text="secondary">
        {title}
      </h3>
      {hint && (
        <p className="heat-section-hint" data-text="secondary">
          {hint}
        </p>
      )}
      {children}
    </div>
  );
}

/** The tick: a mouse's way to check a task off; the keyboard's is ⌘↩. */
export function Tick({ on, label, onToggle }: { on: boolean; label: string; onToggle(): void }) {
  return (
    <span
      role="checkbox"
      aria-checked={on}
      aria-label={label}
      tabIndex={-1}
      className="heat-tick"
      data-on={on}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
    >
      {on ? '✓' : ''}
    </span>
  );
}

/** A task in the plan list: tick, title, due, and its heat; drag it onto the column to give it a block. */
function TaskLine({ id, selected, onSelect, tick }: { id: Id; selected: boolean; onSelect(): void; tick?: boolean }) {
  const { snap, idx, tz, now } = useHeat();
  const actions = useActions();
  const t = idx.task.get(id);
  const d = snap?.derived.tasks[id];
  if (!t || !d) return null;
  const due = effectiveDue(t, d, tz);
  const on = tick ?? t.done;
  return (
    <div
      id={`heat-row-t-${id}`}
      role="option"
      className="heat-row"
      data-dense
      data-done={on}
      aria-selected={selected}
      {...dragTask(id)}
      onClick={onSelect}
    >
      <Tick on={on} label={`${t.title}, done`} onToggle={() => void actions.toggleDone(id)} />
      <span className="heat-row-title">
        {t.title}
        {t.rrule && <span aria-label="repeats"> ↻</span>}
      </span>
      <span className="heat-row-sub" data-text="secondary">
        {due !== null ? dueText(due, now(), tz) : ''}
      </span>
      <Heat level={d.heat.level} v={d.heat.v} word={false} />
    </div>
  );
}
