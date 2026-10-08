// Focus, the default screen (docs/FOCUS.md): only what needs you now. The
// Now task, large and centred, with its space's band of light; the focus
// timer under it; the first step, if it has steps; Done; and at most one
// interrupt line. With entropy high it shows the fix instead of the mess,
// and with every loop closed it shows nothing but "All clear".
//
// It is drawn inside Learn's frame, so it acts through the same actions
// Today and Tasks do. What it shows is `snapshot.focus`, which the core
// works out; nothing here ranks, counts or decides.

import { type CSSProperties, useMemo } from 'react';
import { clockAt } from '../shared/time/format';
import { useActions } from '../heat/actions';
import type { Id, Snapshot, Task } from '../heat/client';
import { copy, dueText, effectiveDue } from '../heat/fmt';
import { useFrame } from '../heat/frame';
import { type Index, useHeat, useNow } from '../heat/store';
import { lcdLine, timerView } from '../heat/timer';
import { keys } from '../shell/platform';
import { bandOf, intensityOf } from './band';
import { click } from './click';
import { useHint } from './layout';
import { type Action, emptyMemory, type FocusState, focusState, type Interrupt } from './model';
import { allViews, viewForInterrupt } from './registry';

const LENGTHS = [25, 50];

/** `snapshot.focus`, or the same worked out here for a core that doesn't send it yet. */
export function focusOf(snap: Snapshot | null): FocusState | null {
  if (!snap) return null;
  return snap.focus ?? focusState(snap, emptyMemory());
}

/** A task's first open step: its first open subtask, else the first unticked box in its notes. */
export function firstStep(task: Task, snap: Snapshot): string | null {
  const child = snap.records.task.find((t) => t.parentTaskId === task.id && !t.done);
  if (child) return child.title;
  const box = /^\s*[-*]\s+\[ \]\s+(.+)$/m.exec(task.notes ?? '');
  return box ? box[1].trim() : null;
}

/** Where a task belongs, in a word or two: its course's code, its project, or its space. */
function placeOf(task: Task, idx: Index): string {
  return (
    (task.courseId && idx.course.get(task.courseId)?.code) ||
    (task.projectId && idx.project.get(task.projectId)?.title) ||
    idx.space.get(task.spaceId)?.name ||
    ''
  );
}

export function FocusScreen({ summon }: { summon(id: string): void }) {
  const { snap, idx, tz, client, act, message } = useHeat();
  const actions = useActions();
  const frame = useFrame();
  const hint = useHint();
  const timer = snap?.heatState.timer;
  const running = timer ? (timer.running ?? timer.endsAt !== null) : false;
  const now = useNow(running ? 1000 : 30_000);
  const focus = useMemo(() => focusOf(snap), [snap]);
  if (!snap || !focus || !timer) return <section className="focus" aria-label="Focus" aria-busy="true" />;

  const task = focus.now.kind === 'task' ? idx.task.get(focus.now.taskId) : undefined;
  const v = timerView(timer, now, frame.focusLength);
  const idle = v.phase === 'idle';

  const run = async (action: Action, interrupt?: Interrupt) => {
    switch (action.do) {
      case 'current':
        if (action.taskId) await act(client.setCurrent(action.taskId));
        break;
      case 'task':
        if (action.taskId) {
          frame.setSpace(null);
          frame.selectTask(action.taskId);
          summon('tasks');
        }
        break;
      case 'view': {
        const id = action.view ?? (interrupt && viewForInterrupt(interrupt.source)?.id);
        if (id) summon(id);
        break;
      }
      case 'break':
        await act(client.focus('resume'));
        break;
      case 'command':
        if (action.cmd) await act(client.attention.run(action.cmd, action.args));
        break;
      case 'plan':
        // The drafts wait in Today for Return; the fix has been answered for today either way.
        await actions.planMyDay();
        await act(client.attention.snooze());
        summon('today');
        break;
    }
    if (interrupt) await act(client.attention.dismiss(interrupt.id));
  };

  const start = async () => {
    if (idle) {
      if (!task) return;
      click();
      await act(client.focus('start', { taskId: task.id, length: frame.focusLength }));
      return;
    }
    await act(client.focus(v.running ? 'pause' : 'resume'));
  };

  const done = (id: Id) => {
    click();
    void actions.toggleDone(id);
  };

  const shortcuts = allViews().filter((x) => x.shortcut !== null);
  const lastKey = shortcuts.length ? Math.max(...shortcuts.map((x) => x.shortcut!)) : 0;
  const timerTask = idx.task.get(timer.taskId ?? task?.id ?? '');
  const habit = timer.habitId ? idx.habit.get(timer.habitId) : undefined;
  const target = habit ? ({ title: habit.title } as Task) : timerTask;
  // A line raised with no action of its own opens the view registered for its source.
  const home = focus.interrupt && !focus.interrupt.action ? viewForInterrupt(focus.interrupt.source) : undefined;
  const interruptAction: Action | null =
    focus.interrupt?.action ?? (home ? { label: `Open ${home.title}`, do: 'view', view: home.id } : null);
  const verb = idle ? copy.widgets.startFocus : v.waiting ? 'Start break' : v.running ? 'Pause' : 'Resume';

  return (
    <section className="focus" aria-label="Focus" data-now={focus.now.kind} data-entropy={focus.entropy.level}>
      <div className="focus-stage">
        {task ? (
          <NowTask key={task.id} task={task} snap={snap} idx={idx} tz={tz} at={now} why={focus.now.why} />
        ) : focus.now.kind === 'fix' ? (
          <div className="focus-now" key={focus.now.fix.kind}>
            <h1 className="focus-title">{focus.now.fix.line}</h1>
            <p className="focus-meta">{focus.now.fix.ask}</p>
          </div>
        ) : (
          <div className="focus-now" key="clear">
            <h1 className="focus-title">All clear</h1>
            <p className="focus-meta">
              {focus.next
                ? `Next: ${focus.next.title} at ${clockAt(focus.next.start, tz)}.`
                : 'Nothing needs you right now.'}
            </p>
          </div>
        )}

        {(task || !idle) && (
          <div className="focus-timer" data-phase={v.phase} data-running={v.running}>
            <p className="focus-digits" role="timer" aria-label={`Focus timer, ${v.digits} left`}>
              {v.digits}
            </p>
            <span className="focus-meter" aria-hidden="true">
              <span className="focus-meter-fill" style={{ '--left': v.meter } as CSSProperties} />
            </span>
            {!idle && <p className="focus-line">{lcdLine(v, target)}</p>}
            {v.note && (
              <p className="focus-line" role="status">
                {v.note}
              </p>
            )}
          </div>
        )}

        <div className="focus-actions">
          {focus.now.kind === 'fix' ? (
            <>
              <button type="button" className="prism-fill" onClick={() => void run(focus.now.fix!.action)}>
                {focus.now.fix.action.label}
              </button>
              <button type="button" className="prism-plain" onClick={() => void act(client.attention.snooze())}>
                Not now
              </button>
            </>
          ) : (
            (task || !idle) && (
              <>
                <button
                  type="button"
                  className={idle ? 'prism-fill' : 'prism-plain'}
                  disabled={idle && !task}
                  title={`${verb} (F)`}
                  onClick={() => void start()}
                >
                  {verb}
                </button>
                <div className="focus-lengths" role="radiogroup" aria-label="Focus length in minutes">
                  {LENGTHS.map((n) => (
                    <button
                      key={n}
                      type="button"
                      role="radio"
                      aria-checked={frame.focusLength === n}
                      className="prism-plain"
                      disabled={!idle}
                      onClick={() => frame.setFocusLength(n)}
                    >
                      {n}
                    </button>
                  ))}
                </div>
                <button
                  type="button"
                  className="prism-plain"
                  disabled={!(v.phase === 'focus' && v.running)}
                  title={`${copy.focus.pulledAway} (I)`}
                  onClick={() => void actions.pulledAway()}
                >
                  {copy.focus.pulledAway}
                </button>
                {!idle && (
                  <button
                    type="button"
                    className="prism-plain"
                    title={`Stop and log (${keys('⇧')}F)`}
                    onClick={() => void actions.stopFocus()}
                  >
                    Stop
                  </button>
                )}
                {task && (
                  <button
                    type="button"
                    className={idle ? 'prism-plain' : 'prism-fill'}
                    title={`Mark done (${keys('⌘')}Return)`}
                    onClick={() => done(task.id)}
                  >
                    {copy.widgets.done}
                  </button>
                )}
              </>
            )
          )}
        </div>

        {focus.interrupt && (
          <p
            className="focus-interrupt"
            role="status"
            aria-label="Interrupt"
            data-source={focus.interrupt.source}
            data-priority={focus.interrupt.priority}
          >
            <span className="focus-interrupt-line">{focus.interrupt.line}</span>
            {interruptAction && (
              <button type="button" className="prism-plain" onClick={() => void run(interruptAction, focus.interrupt!)}>
                {interruptAction.label}
              </button>
            )}
            <button
              type="button"
              className="prism-plain focus-dismiss"
              onClick={() => void act(client.attention.dismiss(focus.interrupt!.id))}
            >
              Dismiss
            </button>
          </p>
        )}
      </div>

      <p className="focus-said" role="status">
        {message?.text}
      </p>
      {hint && (
        <p className="focus-hint">
          {keys('⌘K')} for anything · 1–{lastKey || 6} for tools · hold {keys('⌘')} for the map
        </p>
      )}
    </section>
  );
}

function NowTask({
  task,
  snap,
  idx,
  tz,
  at,
  why,
}: {
  task: Task;
  snap: Snapshot;
  idx: Index;
  tz: string;
  at: number;
  why: 'current' | 'heat' | null;
}) {
  const d = snap.derived.tasks[task.id];
  const due = effectiveDue(task, d, tz);
  const level = d?.heat.level ?? 'Cool';
  const overdue = level === 'Overdue';
  const place = placeOf(task, idx);
  const step = firstStep(task, snap);
  const light = { '--band': bandOf(idx.space.get(task.spaceId)?.hue), '--heat': intensityOf(d?.heat.v) } as CSSProperties;
  return (
    <div className="focus-now" data-why={why ?? undefined} data-level={level}>
      <span className="focus-band" style={light} aria-hidden="true" />
      <h1 className="focus-title">{task.title}</h1>
      <p className="focus-meta">
        {place && <span>{place}</span>}
        <span className="focus-due" data-risk={overdue || undefined}>
          {due === null ? 'No due date' : dueText(due, at, tz)}
        </span>
        {level === 'Hot' && <span>Hot</span>}
      </p>
      {step && (
        <p className="focus-step">
          <span className="focus-step-label">First</span> {step}
        </p>
      )}
    </div>
  );
}
