// A habit (docs/SPEC.md 3.9): a name, an optional length ("Practise kanji,
// 20m") that puts it under Recurring in Today, and the per-habit streak
// counter, off by default. Esc keeps what was typed; Cancel throws it away.

import { type FormEvent, useEffect, useRef, useState } from 'react';
import type { Habit } from '../client';
import { copy } from '../fmt';
import { useDraft } from '../frame';
import { useHeat } from '../store';

interface Form {
  title: string;
  minutes: string;
  counter: boolean;
}

export function HabitSheet({ habit, onClose }: { habit?: Habit; onClose(): void }) {
  const { client, act, say } = useHeat();
  const [initial] = useState<Form>(() => ({
    title: habit?.title ?? '',
    minutes: habit?.minutes === undefined ? '' : String(habit.minutes),
    counter: habit?.showCounter ?? false,
  }));
  const [form, setForm, clear] = useDraft<Form>(`habit:${habit?.id ?? 'new'}`, initial);
  const [why, setWhy] = useState<string | null>(null);
  const title = useRef<HTMLInputElement>(null);
  useEffect(() => title.current?.focus(), []);
  const set = (change: Partial<Form>) => {
    setForm({ ...form, ...change });
    setWhy(null);
  };

  const save = async (e: FormEvent) => {
    e.preventDefault();
    const name = form.title.trim();
    if (!name) {
      setWhy(copy.habits.nameFirst);
      title.current?.focus();
      return;
    }
    const minutes = form.minutes.trim() === '' ? undefined : Math.round(Number(form.minutes));
    if (minutes !== undefined && !(minutes >= 1 && minutes <= 600)) return setWhy(copy.habitsUi.lengthRange);
    const r = habit
      ? await act(
          client.patch('habit', habit.id, {
            title: name,
            minutes: minutes ?? (null as never),
            showCounter: form.counter,
          }),
        )
      : await act(
          client.put('habit', {
            title: name,
            ...(minutes === undefined ? {} : { minutes }),
            log: {},
            showCounter: form.counter,
          }),
        );
    if (!r) return;
    clear();
    say(habit ? `Saved ${name}.` : `Added ${name}.`);
    onClose();
  };

  const remove = async () => {
    if (!habit || !(await act(client.delete('habit', habit.id)))) return;
    clear();
    say(`Deleted ${habit.title}.`);
    onClose();
  };

  return (
    <form
      noValidate
      className="sheet heat-sheet"
      role="dialog"
      aria-label={habit ? `Edit habit ${habit.title}` : 'New habit'}
      onSubmit={(e) => void save(e)}
    >
      <h2 className="sheet-title">{habit ? 'Edit habit' : 'New habit'}</h2>
      <label className="field">
        <span data-text="secondary">Habit</span>
        <input
          ref={title}
          value={form.title}
          placeholder="Practise kanji"
          onChange={(e) => set({ title: e.target.value })}
        />
      </label>
      <label className="field">
        <span data-text="secondary">Length in minutes, if it has one</span>
        <input
          type="number"
          inputMode="numeric"
          min={1}
          max={600}
          value={form.minutes}
          onChange={(e) => set({ minutes: e.target.value })}
        />
      </label>
      <p className="why" data-text="secondary">
        {copy.habits.advice}
      </p>
      <label className="check" data-dense>
        <input type="checkbox" checked={form.counter} onChange={(e) => set({ counter: e.target.checked })} />
        {copy.habits.showCounter}
      </label>
      <p className="why" data-text="secondary">
        {copy.habitsUi.counterHint}
      </p>
      <p className="why heat-sheet-why" data-text="secondary" role="status">
        {why}
      </p>
      <div className="note-actions">
        <button type="submit" className="gel">
          {habit ? 'Save' : 'Add habit'}
        </button>
        <button
          type="button"
          className="gel plain"
          onClick={() => {
            clear();
            onClose();
          }}
        >
          Cancel
        </button>
        {habit && (
          <button type="button" className="gel plain" onClick={() => void remove()}>
            Delete habit
          </button>
        )}
      </div>
    </form>
  );
}
