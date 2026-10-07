// The right column (docs/SPEC.md 3.5): the sketch's five widgets in its
// order, each a brushed-metal panel with a small heading. Below 1240 pt the
// column folds into a 44 px strip of icons, each opening its widget as a
// popover. Habits, Mail and Grades only read the snapshot; Now and Hot
// tasks act on tasks.

import { type DragEvent, type ReactNode, useState } from 'react';
import { minuteOfDay } from '../../shared/time/zone';
import { useActions } from '../actions';
import type { Id } from '../client';
import { clock, copy, dueText, effectiveDue } from '../fmt';
import { useSelection, useTabs } from '../frame';
import { useHeat, useNow } from '../store';
import { lcdLine, stripText, timerView } from '../timer';
import { carriesTask, draggedTask, dragTask, Heat, SpaceDot } from '../ui';

type WidgetId = 'now' | 'habits' | 'hot' | 'mail' | 'grades';

const TITLES: Record<WidgetId, string> = {
  now: copy.widgets.now,
  habits: copy.widgets.habits,
  hot: copy.widgets.hotTasks,
  mail: copy.widgets.mail,
  grades: copy.widgets.grades,
};

const ICONS: Record<WidgetId, ReactNode> = {
  now: <circle cx="10" cy="10" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.6" />,
  habits: (
    <>
      <circle cx="6" cy="10" r="3.5" fill="currentColor" />
      <circle cx="14.5" cy="10" r="3" fill="none" stroke="currentColor" strokeWidth="1.6" />
    </>
  ),
  hot: (
    <path
      d="M10 2.5c1 3-2.5 4-2.5 7.2a2.7 2.7 0 0 0 5.4 0c0-1-.5-1.8-1-2.5 2.4 1 4 3 4 5.3A5.9 5.9 0 0 1 10 18.5a5.9 5.9 0 0 1-5.9-5.9C4.1 7.7 8 6.2 10 2.5z"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinejoin="round"
    />
  ),
  mail: (
    <>
      <rect x="2.5" y="4.5" width="15" height="11" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.6" />
      <path d="M3 5.5l7 5.5 7-5.5" fill="none" stroke="currentColor" strokeWidth="1.6" />
    </>
  ),
  grades: (
    <path
      d="M4 16.5 10 3.5l6 13M6.2 12.5h7.6"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  ),
};

export function Widgets({
  folded,
  popover,
  setPopover,
}: {
  folded: boolean;
  popover: string | null;
  setPopover(id: string | null): void;
}) {
  const { snap } = useHeat();
  // Grades stays hidden until a course exists (3.5).
  const shown: WidgetId[] = [
    'now',
    'habits',
    'hot',
    'mail',
    ...((snap?.records.course.length ?? 0) > 0 ? (['grades'] as const) : []),
  ];
  const body = (id: WidgetId, close?: () => void): ReactNode => {
    switch (id) {
      case 'now':
        return <NowWidget />;
      case 'habits':
        return <HabitsWidget />;
      case 'hot':
        return <HotWidget />;
      case 'mail':
        return <MailWidget close={close} />;
      case 'grades':
        return <GradesWidget close={close} />;
    }
  };

  if (!folded) {
    return (
      <aside className="heat-right" aria-label="Widgets">
        {shown.map((id) => (
          <section key={id} className="heat-widget" aria-labelledby={`heat-widget-${id}`}>
            <h2 id={`heat-widget-${id}`} className="heat-widget-title" data-text="secondary">
              {TITLES[id]}
            </h2>
            <div className="heat-widget-well">{body(id)}</div>
          </section>
        ))}
      </aside>
    );
  }
  return (
    <aside className="heat-right heat-strip" aria-label="Widgets">
      {shown.map((id) => (
        <button
          key={id}
          type="button"
          className="heat-widget-icon"
          aria-label={TITLES[id]}
          aria-haspopup="dialog"
          aria-expanded={popover === id}
          title={TITLES[id]}
          onClick={() => setPopover(popover === id ? null : id)}
        >
          <svg width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
            {ICONS[id]}
          </svg>
        </button>
      ))}
      {popover && (shown as string[]).includes(popover) && (
        <section className="heat-popover" role="dialog" aria-label={TITLES[popover as WidgetId]}>
          <h2 className="heat-widget-title" data-text="secondary">
            {TITLES[popover as WidgetId]}
          </h2>
          <div className="heat-widget-well">{body(popover as WidgetId, () => setPopover(null))}</div>
        </section>
      )}
    </aside>
  );
}

/** Now: the current task, its space dot and heat tube, the block's end, the timer, and what to do next. */
function NowWidget() {
  const { snap, idx, tz, date, client, act } = useHeat();
  const actions = useActions();
  const { selectTask } = useSelection();
  const running = (snap?.heatState.timer.running ?? snap?.heatState.timer.endsAt != null) === true;
  const now = useNow(running ? 1000 : 30_000);
  const [over, setOver] = useState(false);
  const task = snap?.heatState.currentTaskId ? idx.task.get(snap.heatState.currentTaskId) : undefined;

  const drop = {
    onDragOver: (e: DragEvent) => {
      if (carriesTask(e)) {
        e.preventDefault();
        setOver(true);
      }
    },
    onDragLeave: () => setOver(false),
    onDrop: (e: DragEvent) => {
      setOver(false);
      const id = draggedTask(e);
      if (id) {
        e.preventDefault();
        void act(client.setCurrent(id));
      }
    },
  };

  if (!snap || !task) {
    return (
      <p className="heat-widget-empty" data-drop-now data-over={over} {...drop}>
        {copy.widgets.nowEmpty}
      </p>
    );
  }
  const d = snap.derived.tasks[task.id];
  const nowMin = minuteOfDay(now, tz);
  const block = snap.records.timeBlock.find(
    (b) => b.taskId === task.id && b.date === date && b.start <= nowMin && nowMin < b.start + b.minutes,
  );
  const timer = timerView(snap.heatState.timer, now);
  const onThis = timer.phase !== 'idle';
  const verb = timer.phase === 'idle' ? copy.widgets.startFocus : timer.running ? 'Pause' : 'Resume';
  const due = effectiveDue(task, d, tz);

  return (
    <div className="heat-now" data-drop-now data-over={over} {...drop}>
      <button type="button" className="heat-now-title" data-dense onClick={() => selectTask(task.id)}>
        <SpaceDot hue={idx.space.get(task.spaceId)?.hue} />
        <span className="heat-now-name">{task.title}</span>
      </button>
      <p className="heat-now-line heat-now-heat">
        {d && <Heat level={d.heat.level} v={d.heat.v} />}
        {due !== null && <span>{dueText(due, now, tz)}</span>}
      </p>
      {block && <p className="heat-now-line">{copy.today.blockEnds(clock(block.start + block.minutes))}</p>}
      {onThis && (
        <p className="heat-now-line" data-text="secondary">
          {stripText(snap.heatState.timer, now) ?? lcdLine(timer, task)}
        </p>
      )}
      <div className="heat-now-actions">
        <button type="button" className="gel" onClick={() => void actions.focusKey()} title={`${verb} (F)`}>
          {verb}
        </button>
        <button type="button" className="gel" onClick={() => void actions.toggleDone(task.id)} title="Mark done (⌘↩)">
          {copy.widgets.done}
        </button>
        {task.link && (
          <button type="button" className="gel plain" disabled title="The Console isn’t in this build yet.">
            {task.link.kind === 'session' ? copy.widgets.openSession : copy.widgets.openLink}
          </button>
        )}
      </div>
    </div>
  );
}

/** Habits: today's orbs, and "3 of 5 done". Clicking an orb toggles today. */
function HabitsWidget() {
  const { snap, date, client, act } = useHeat();
  const habits = snap?.records.habit ?? [];
  if (!snap || habits.length === 0) return <p className="heat-widget-empty">{copy.widgets.habitsEmpty}</p>;
  const doneToday = (id: Id, log: Record<string, true>) => snap.derived.habits[id]?.today ?? log[date] === true;
  const done = habits.filter((h) => doneToday(h.id, h.log)).length;
  const toggle = (id: Id, log: Record<string, true>) => {
    const next = { ...log };
    if (next[date]) delete next[date];
    else next[date] = true;
    void act(client.patch('habit', id, { log: next }));
  };
  return (
    <>
      <div className="heat-orbs">
        {habits.map((h) => {
          const on = doneToday(h.id, h.log);
          return (
            <button
              key={h.id}
              type="button"
              className="heat-orb"
              data-done={on}
              aria-pressed={on}
              aria-label={`${h.title}, ${on ? 'done' : 'not done'} today`}
              title={h.title}
              onClick={() => toggle(h.id, h.log)}
            >
              <span className="heat-orb-disc" aria-hidden="true">
                {on ? '✓' : ''}
              </span>
            </button>
          );
        })}
      </div>
      <p className="heat-widget-line">{copy.habits.ofDone(done, habits.length)}</p>
    </>
  );
}

/** Hot tasks: up to 5 Hot or Overdue tasks from all spaces; select one, or drag it onto the time column. */
function HotWidget() {
  const { snap, idx, tz, now } = useHeat();
  const { taskId, selectTask } = useSelection();
  const ids = (snap?.derived.hotTasks ?? []).slice(0, 5);
  if (!snap || ids.length === 0) return <p className="heat-widget-empty">{copy.widgets.hotEmpty}</p>;
  return (
    <div role="listbox" aria-label="Hot tasks" className="heat-lines">
      {ids.map((id) => {
        const t = idx.task.get(id);
        const d = snap.derived.tasks[id];
        if (!t || !d) return null;
        const due = effectiveDue(t, d, tz);
        return (
          <div
            key={id}
            role="option"
            className="heat-line"
            data-dense
            aria-selected={taskId === id}
            {...dragTask(id)}
            onClick={() => selectTask(id)}
          >
            <span className="heat-line-title">{t.title}</span>
            <Heat level={d.heat.level} v={d.heat.v} word={false} />
            {due !== null && (
              <span className="heat-line-sub" data-text="secondary">
                {dueText(due, now(), tz)}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}

const STATE_WORD = { grade: copy.mail.gradePosted, task: copy.mail.taskMade, nothing: copy.mail.nothingToDo } as const;

/** Mail: the 3 newest school threads Claude recorded. */
function MailWidget({ close }: { close?: () => void }) {
  const { snap } = useHeat();
  const { setTab } = useTabs();
  const threads = [...(snap?.records.mailThread ?? [])].sort((a, b) => b.receivedAt - a.receivedAt).slice(0, 3);
  if (threads.length === 0) return <p className="heat-widget-empty">No school mail recorded.</p>;
  return (
    <>
      <ul className="heat-lines heat-plain">
        {threads.map((m) => (
          <li key={m.id} className="heat-mail">
            <span className="heat-line-title">{m.subject}</span>
            <span className="heat-line-sub" data-text="secondary">
              {m.from.replace(/<.*>/, '').trim()} · {STATE_WORD[m.state]}
            </span>
          </li>
        ))}
      </ul>
      <button
        type="button"
        className="gel plain"
        onClick={() => {
          setTab('mail');
          close?.();
        }}
      >
        {copy.widgets.openInMail}
      </button>
    </>
  );
}

/** Grades: the lowest course and its letter, and how many new grades wait for a score. */
function GradesWidget({ close }: { close?: () => void }) {
  const { snap } = useHeat();
  const { setTab } = useTabs();
  if (!snap) return null;
  const graded = snap.records.course.flatMap((c) => {
    const d = snap.derived.courses[c.id];
    return d && d.currentPct !== null ? [{ code: c.code, pct: d.currentPct, letter: d.letter }] : [];
  });
  const lowest = graded.reduce<(typeof graded)[number] | null>(
    (low, c) => (low === null || c.pct < low.pct ? c : low),
    null,
  );
  const pending = snap.records.grade.filter((g) => g.pending).length;
  return (
    <>
      {lowest ? (
        <p className="heat-widget-line">
          {lowest.code} <span className="heat-letter">{lowest.letter}</span>
        </p>
      ) : (
        <p className="heat-widget-empty">{snap.records.course.map((c) => c.code).join(', ')}</p>
      )}
      {pending > 0 && <p className="heat-widget-line">{copy.grades.toEnter(pending)}</p>}
      <button
        type="button"
        className="gel plain"
        onClick={() => {
          setTab('grades');
          close?.();
        }}
      >
        {copy.widgets.openGrades}
      </button>
    </>
  );
}
