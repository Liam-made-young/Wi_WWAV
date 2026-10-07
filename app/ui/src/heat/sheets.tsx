// The frame's two sheets (docs/SPEC.md 3.1, 3.3, 3.5). A sheet drops from the
// top, validates in one line ("Give the task a name first."), and Esc closes
// it keeping what was typed for next time (2.7).
//
//   New task      "+" on Today, Tasks and Calendar; N
//   Time it took  checking off a task that has no logged time

import { type FormEvent, useEffect, useRef, useState } from 'react';
import { atMinute, dayKey, minuteOfDay } from '../shared/time/zone';
import { useActions } from './actions';
import type { Id } from './client';
import { copy } from './fmt';
import type { NewTaskOptions } from './frame';
import { useHeat } from './store';

const pad = (n: number) => String(n).padStart(2, '0');

/** "2026-10-07" and "23:59" for an instant, as date and time fields read them. */
export function fieldsOf(ms: number | null, tz: string): { date: string; time: string } {
  if (ms === null) return { date: '', time: '' };
  const m = minuteOfDay(ms, tz);
  return { date: dayKey(ms, tz), time: `${pad(Math.floor(m / 60))}:${pad(m % 60)}` };
}

/** The instant a date and time field name; a date with no time is due at 11:59 PM (3.7). */
export function instantOf(date: string, time: string, tz: string): number | null {
  if (!date) return null;
  const [h, m] = time ? time.split(':').map(Number) : [23, 59];
  return atMinute(date, h * 60 + m, tz);
}

interface NewProps {
  options: NewTaskOptions;
  draft: string;
  onDraft(title: string): void;
  onClose(): void;
  onSaved(): void;
}

export function NewTaskSheet({ options, draft, onDraft, onClose, onSaved }: NewProps) {
  const { snap, tz, idx } = useHeat();
  const actions = useActions();
  const spaces = snap?.records.space ?? [];
  const [spaceId, setSpaceId] = useState<Id>(options.spaceId ?? spaces[0]?.id ?? '');
  const space = idx.space.get(spaceId);
  const [type, setType] = useState('');
  const [group, setGroup] = useState('');
  const [difficulty, setDifficulty] = useState(3);
  const start = fieldsOf(options.due ?? null, tz);
  const [date, setDate] = useState(start.date);
  const [time, setTime] = useState(start.time);
  const [why, setWhy] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const title = useRef<HTMLInputElement>(null);

  useEffect(() => title.current?.focus(), []);
  useEffect(() => {
    setType(space?.types[0] ?? 'Other');
    setGroup('');
  }, [space]);

  const save = async (e: FormEvent) => {
    e.preventDefault();
    const name = draft.trim();
    if (!name) {
      setWhy(copy.tasks.nameFirst);
      title.current?.focus();
      return;
    }
    if (!space) return;
    setBusy(true);
    const kind = space.groupKind;
    const fields = {
      spaceId,
      title: name,
      type,
      difficulty,
      due: instantOf(date, time, tz),
      ...(options.notes ? { notes: options.notes } : {}),
      ...(options.scheduledDate ? { scheduledDate: options.scheduledDate } : {}),
      ...(kind === 'course' && group ? { courseId: group } : {}),
      ...(kind === 'milestone' && group ? { milestoneId: group } : {}),
      ...(kind === 'free' && group.trim() ? { group: group.trim() } : {}),
    };
    const record = await actions.makeTask(fields);
    setBusy(false);
    if (!record) return;
    onSaved();
    onClose();
    if (options.intoPlan) await actions.placeNew(record.id);
  };

  const groups =
    space?.groupKind === 'course'
      ? (snap?.records.course ?? []).map((c) => ({ id: c.id, name: c.code }))
      : space?.groupKind === 'milestone'
        ? (snap?.records.milestone ?? []).filter((m) => m.spaceId === spaceId).map((m) => ({ id: m.id, name: m.title }))
        : [];

  return (
    <form className="sheet heat-sheet" role="dialog" aria-label="New task" onSubmit={save}>
      <h2 className="sheet-title">New task</h2>
      <label className="field">
        <span data-text="secondary">Task</span>
        <input
          ref={title}
          value={draft}
          onChange={(e) => {
            onDraft(e.target.value);
            setWhy(null);
          }}
          placeholder="What needs doing?"
          aria-describedby="heat-new-why"
        />
      </label>
      <p id="heat-new-why" className="why" data-text="secondary" role="status">
        {why ?? copy.tasks.rankIt}
      </p>
      <div className="heat-sheet-grid">
        <label className="field">
          <span data-text="secondary">Space</span>
          <select value={spaceId} onChange={(e) => setSpaceId(e.target.value)}>
            {spaces.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span data-text="secondary">Type</span>
          <select value={type} onChange={(e) => setType(e.target.value)}>
            {(space?.types ?? ['Other']).map((t) => (
              <option key={t}>{t}</option>
            ))}
          </select>
        </label>
        {space?.groupKind === 'free' ? (
          <label className="field">
            <span data-text="secondary">{space.groupLabel}</span>
            <input value={group} onChange={(e) => setGroup(e.target.value)} />
          </label>
        ) : (
          groups.length > 0 && (
            <label className="field">
              <span data-text="secondary">{space?.groupLabel}</span>
              <select value={group} onChange={(e) => setGroup(e.target.value)}>
                <option value="">None</option>
                {groups.map((g) => (
                  <option key={g.id} value={g.id}>
                    {g.name}
                  </option>
                ))}
              </select>
            </label>
          )
        )}
        <label className="field">
          <span data-text="secondary">Difficulty</span>
          <select value={difficulty} onChange={(e) => setDifficulty(Number(e.target.value))}>
            {[1, 2, 3, 4, 5].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span data-text="secondary">Due</span>
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
        </label>
        <label className="field">
          <span data-text="secondary">Time</span>
          <input type="time" value={time} onChange={(e) => setTime(e.target.value)} disabled={!date} />
        </label>
      </div>
      <div className="note-actions">
        <button type="submit" className="gel" disabled={busy}>
          Add task
        </button>
        <button type="button" className="gel plain" onClick={onClose}>
          Cancel
        </button>
      </div>
    </form>
  );
}

/** "Time it took": the hint says what the answer trains; no answer still checks the task off. */
export function TookSheet({ taskId, onClose }: { taskId: Id; onClose(): void }) {
  const { idx } = useHeat();
  const actions = useActions();
  const task = idx.task.get(taskId);
  const [minutes, setMinutes] = useState('');
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => field.current?.focus(), []);

  const done = async (e: FormEvent, skip = false) => {
    e.preventDefault();
    const n = Math.round(Number(minutes));
    onClose();
    await actions.finish(taskId, !skip && n > 0 ? n : null);
  };

  return (
    <form className="sheet heat-sheet" role="dialog" aria-label={copy.check.title} onSubmit={(e) => void done(e)}>
      <h2 className="sheet-title">{copy.check.title}</h2>
      <p>{task?.title}</p>
      <label className="field">
        <span data-text="secondary">Minutes</span>
        <input
          ref={field}
          type="number"
          inputMode="numeric"
          min={1}
          max={600}
          value={minutes}
          onChange={(e) => setMinutes(e.target.value)}
        />
      </label>
      <p className="why" data-text="secondary">
        {copy.check.hint}
      </p>
      <div className="note-actions">
        <button type="submit" className="gel">
          Done
        </button>
        <button type="button" className="gel plain" onClick={(e) => void done(e, true)}>
          Skip
        </button>
        <button type="button" className="gel plain" onClick={onClose}>
          Cancel
        </button>
      </div>
    </form>
  );
}
