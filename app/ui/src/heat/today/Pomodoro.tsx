// The Pomodoro timer (docs/SPEC.md 3.5): an olive LCD panel with 32 px
// digits ("24:59"), a line such as "Focus 1 of 4 · Mix the second verse" and
// a meter that drains. F starts or pauses, ⇧F stops and logs, I marks
// "Pulled away". Nothing starts without a press, and a break waits for one.
// The timer lives in the core and the store, so it keeps running when you
// switch views; the Now strip reads it as "focus 18:42 left".

import { useActions } from '../actions';
import { copy } from '../fmt';
import { useFrame } from '../frame';
import { useHeat, useNow } from '../store';
import { lcdLine, timerView } from '../timer';

const LENGTHS = [25, 50];

export function Pomodoro() {
  const { snap, idx } = useHeat();
  const { focusLength, setFocusLength } = useFrame();
  const actions = useActions();
  const timer = snap?.heatState.timer;
  const running = timer ? (timer.running ?? timer.endsAt !== null) : false;
  const now = useNow(running ? 1000 : 30_000);
  if (!snap || !timer) return <section className="heat-pomo" aria-label="Pomodoro timer" />;

  const v = timerView(timer, now, focusLength);
  const target = idx.task.get(timer.taskId ?? snap.heatState.currentTaskId ?? '');
  const idle = v.phase === 'idle';
  const verb = idle ? copy.widgets.startFocus : v.waiting ? 'Start break' : v.running ? 'Pause' : 'Resume';
  const canStart = !idle || target !== undefined;
  const custom = !LENGTHS.includes(focusLength);

  return (
    <section className="heat-pomo" aria-label="Pomodoro timer" data-phase={v.phase} data-running={v.running}>
      <div className="heat-lcd" role="timer" aria-label="Focus timer">
        <span className="heat-lcd-digits" aria-live="off">
          {v.digits}
        </span>
        <span className="heat-lcd-line" data-text="secondary">
          {lcdLine(v, target)}
        </span>
        <span className="heat-lcd-note" data-text="secondary" role="status">
          {v.note}
        </span>
        <span className="heat-lcd-meter" aria-hidden="true">
          <span className="heat-lcd-fill" style={{ width: `${Math.round(v.meter * 100)}%` }} />
        </span>
      </div>
      <div className="heat-pomo-keys">
        <button
          type="button"
          className="gel"
          disabled={!canStart}
          onClick={() => void actions.focusKey()}
          title={`${verb} (F)`}
        >
          {verb}
        </button>
        <button
          type="button"
          className="gel"
          disabled={idle}
          onClick={() => void actions.stopFocus()}
          title="Stop and log (⇧F)"
        >
          Stop and log
        </button>
        <button
          type="button"
          className="gel"
          disabled={!(v.phase === 'focus' && v.running)}
          onClick={() => void actions.pulledAway()}
          title="Pulled away (I)"
        >
          {copy.focus.pulledAway}
        </button>
        <div className="heat-lengths" role="radiogroup" aria-label="Focus length">
          {LENGTHS.map((n) => (
            <button
              key={n}
              type="button"
              role="radio"
              aria-checked={focusLength === n}
              className="heat-length"
              disabled={!idle}
              onClick={() => setFocusLength(n)}
            >
              {n}
            </button>
          ))}
          <label className="check heat-length-custom" data-dense>
            <input
              type="number"
              min={10}
              max={90}
              aria-label="Custom focus length in minutes"
              disabled={!idle}
              value={custom ? focusLength : ''}
              placeholder="10–90"
              onChange={(e) => {
                const n = Math.round(Number(e.target.value));
                if (n >= 10 && n <= 90) setFocusLength(n);
              }}
            />
          </label>
        </div>
      </div>
    </section>
  );
}
