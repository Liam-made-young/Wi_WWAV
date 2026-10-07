// Habits (docs/SPEC.md 3.9): global, so the space filter doesn't apply. Up to
// 6, each with an orb for today, a 14-day grid, and a record that only grows
// ("Done 41 days since August 26"). A streak counter is a per-habit setting,
// off by default. Today still counts until midnight: today's square is open,
// never a miss. "Show the year" opens a 53 × 7 grid under each habit. "+" adds
// a habit, and at 6 says "Habit limit reached". A habit with a length can be
// focused on, and ticks itself when the session reaches that length.

import { useState } from 'react';
import { addDays, type DayKey, weekdayOf } from '../../shared/time/zone';
import { shortMonthDay } from '../../shared/time/format';
import type { Habit } from '../client';
import { copy, formatMinutes } from '../fmt';
import { useSheets, useTabActs } from '../frame';
import { PublicSwitch } from '../public/PublicSwitch';
import { useHeat } from '../store';
import { HabitSheet } from './HabitSheet';
import './habits.css';

const LIMIT = 6;

export function Habits() {
  const { snap } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const [year, setYear] = useState(false);
  const habits = snap?.records.habit ?? [];
  const done = habits.filter((h) => snap?.derived.habits[h.id]?.today ?? false).length;

  useTabActs({
    plus: {
      run: () => openSheet(<HabitSheet onClose={closeSheet} />),
      disabled: habits.length >= LIMIT ? copy.habits.limit : null,
    },
    secondary: { run: () => setYear((on) => !on) },
    count: snap && habits.length > 0 ? copy.habits.ofDone(done, habits.length) : null,
  });

  if (!snap) return <div className="heat-habits" />;
  return (
    <div className="heat-habits">
      <header className="heat-habits-head">
        <div>
          <h1 className="heat-heading">Habits</h1>
          <p className="heat-subtitle" data-text="secondary">
            {copy.habits.advice}
          </p>
        </div>
        <button
          type="button"
          className="gel"
          aria-pressed={year}
          onClick={() => setYear(!year)}
          title={`${copy.tabs.secondary.Habits} (⇧Return)`}
        >
          {year ? copy.habitsUi.hideYear : copy.habitsUi.showYear}
        </button>
      </header>
      <div className="heat-habits-body">
        {habits.length === 0 && <p className="heat-empty">{copy.habitsUi.noHabits}</p>}
        {habits.map((h) => (
          <HabitCard key={h.id} habit={h} year={year} />
        ))}
      </div>
    </div>
  );
}

function HabitCard({ habit, year }: { habit: Habit; year: boolean }) {
  const { snap, date, client, act } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const d = snap?.derived.habits[habit.id];
  const today = d?.today ?? habit.log[date] === true;

  const tick = (day: DayKey) => {
    const log = { ...habit.log };
    if (log[day]) delete log[day];
    else log[day] = true;
    void act(client.patch('habit', habit.id, { log }));
  };
  const days = Array.from({ length: 14 }, (_, i) => addDays(date, i - 13));
  const title = habit.minutes === undefined ? habit.title : `${habit.title}, ${formatMinutes(habit.minutes)}`;

  return (
    <article className="heat-habit" aria-label={habit.title}>
      <button
        type="button"
        className="heat-orb heat-habit-orb"
        data-done={today}
        aria-pressed={today}
        aria-label={`${habit.title}, ${today ? 'done' : 'not done'} today`}
        onClick={() => tick(date)}
      >
        <span className="heat-orb-disc" aria-hidden="true">
          {today ? '✓' : ''}
        </span>
      </button>
      <div className="heat-habit-main">
        <h2 className="heat-habit-title">{title}</h2>
        <p className="heat-habit-record" data-text="secondary">
          {d?.counter ?? d?.record}
        </p>
        <div className="heat-grid14" role="group" aria-label={`${habit.title}, the last 14 days`}>
          {days.map((day) => (
            <button
              key={day}
              type="button"
              className="heat-cell"
              data-dense
              data-done={habit.log[day] === true}
              data-today={day === date}
              aria-pressed={habit.log[day] === true}
              aria-label={`${habit.title}, ${shortMonthDay(day)}${day === date ? ', today' : ''}, ${habit.log[day] ? 'done' : 'not done'}`}
              title={day === date ? `${shortMonthDay(day)}. ${copy.habitsUi.today}` : shortMonthDay(day)}
              onClick={() => tick(day)}
            >
              <span className="heat-cell-square" />
            </button>
          ))}
        </div>
        {year && <YearGrid habit={habit} today={date} />}
        <div className="heat-habit-acts">
          {habit.minutes !== undefined && (
            <button
              type="button"
              className="gel"
              onClick={() => void act(client.focus('start', { habitId: habit.id }))}
              title="Starts a focus session on this habit. It ticks when the session reaches its length."
            >
              Start focus
            </button>
          )}
          <button
            type="button"
            className="gel plain"
            onClick={() => openSheet(<HabitSheet habit={habit} onClose={closeSheet} />)}
          >
            Edit habit…
          </button>
        </div>
      </div>
      <PublicSwitch kind="habit" id={habit.id} on={habit.public === true} name={habit.title} />
    </article>
  );
}

/** "Show the year": 53 weeks of 7 days from Sunday, ending with this week. A picture, so it is not a control. */
function YearGrid({ habit, today }: { habit: Habit; today: DayKey }) {
  const first = addDays(today, -weekdayOf(today) - 52 * 7);
  const cells = Array.from({ length: 53 * 7 }, (_, i) => addDays(first, i));
  return (
    <div className="heat-year" role="img" aria-label={`${habit.title}, the year`}>
      <div className="heat-year-grid">
        {cells.map((day) => (
          <span
            key={day}
            className="heat-year-cell"
            data-done={habit.log[day] === true}
            data-future={day > today}
            title={day > today ? undefined : `${shortMonthDay(day)}${habit.log[day] ? ', done' : ''}`}
          />
        ))}
      </div>
    </div>
  );
}
