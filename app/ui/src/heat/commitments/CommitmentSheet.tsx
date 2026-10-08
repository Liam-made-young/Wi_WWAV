// A commitment (docs/COMMITMENTS.md): a class, a shift, a commute. Its name
// and kind, the weekdays it repeats on (none picked means it happens once,
// on one day), its times and dates, where it is, the travel time before and
// after, whether it is fixed, and for a class its course: one Learn holds,
// or a new code, which makes the course in the same step. An existing one
// also lists the days that differ, with Skip a day and Move a day.
//
// A new commitment is `heat.commitment.create`; a saved one sends only what
// changed, so a rule this sheet can't show is left as it is. Esc keeps what
// was typed for next time; Cancel throws it away. One Save is one undo step.

import { type FormEvent, useEffect, useRef, useState } from 'react';
import { clock, shortMonthDay } from '../../shared/time/format';
import type { DayKey } from '../../shared/time/zone';
import type { Commitment, CommitmentException, CommitmentInput, CommitmentKind, Id } from '../client';
import { courseLabel } from '../fmt';
import { useDraft } from '../frame';
import { useHeat } from '../store';
import * as copy from './copy';
import { fieldMinutes, timeField } from './time';

/** The course picker's value for a code typed in. */
const NEW = 'new';

interface Form {
  title: string;
  kind: CommitmentKind;
  /** Weekday codes; none means once, on `date`. */
  days: string[];
  date: string;
  start: string;
  end: string;
  from: string;
  until: string;
  location: string;
  before: string;
  after: string;
  hardness: Commitment['hardness'];
  /** A course's id, '' for none, or NEW for the code in `code`. */
  course: string;
  code: string;
  /** '' is Automatic: the space grouped by course, for a class. */
  spaceId: string;
}

/** One day skipped or moved, as it is being typed. */
interface Change {
  kind: 'skip' | 'move';
  date: string;
  toDate: string;
  start: string;
  end: string;
}

/** What the core is sent for a form. Times are minutes; an empty last day or course is null. */
function inputOf(f: Form): CommitmentInput {
  return {
    title: f.title.trim(),
    kind: f.kind,
    start: fieldMinutes(f.start) ?? undefined,
    end: fieldMinutes(f.end) ?? undefined,
    days: f.days,
    ...(f.days.length === 0 ? { date: f.date } : { from: f.from, until: f.until || null }),
    location: f.location.trim(),
    bufferBefore: Math.round(Number(f.before) || 0),
    bufferAfter: Math.round(Number(f.after) || 0),
    hardness: f.hardness,
    ...(f.kind === 'class' && f.course === NEW ? { course: f.code.trim() } : { courseId: (f.kind === 'class' && f.course) || null }),
    spaceId: f.spaceId || null,
  };
}

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

interface Props {
  /** The commitment to edit; none makes a new one. */
  commitmentId?: Id;
  /** The day a new one starts on. */
  day?: DayKey;
  onClose(): void;
}

export function CommitmentSheet({ commitmentId, day, onClose }: Props) {
  const { snap, client, act, say, date } = useHeat();
  const commitment = commitmentId ? snap?.records.commitment.find((c) => c.id === commitmentId) : undefined;
  const listed = snap?.commitments?.list.find((l) => l.id === commitmentId);
  const courses = snap?.records.course ?? [];
  const spaces = snap?.records.space ?? [];

  // Built once, so what Save sends is what changed since the sheet opened.
  const [initial] = useState<Form>(() => {
    const c = commitment;
    const first = day ?? date;
    return {
      title: c?.title ?? '',
      kind: c?.kind ?? 'class',
      days: c ? (listed?.days ?? []) : [],
      date: c ? c.from : first,
      start: c ? timeField(c.start) : '',
      end: c ? timeField(c.end) : '',
      from: c ? c.from : first,
      until: c?.until ?? '',
      location: c?.location ?? '',
      before: String(c?.bufferBefore ?? 0),
      after: String(c?.bufferAfter ?? 0),
      hardness: c?.hardness ?? 'fixed',
      course: c?.courseId ?? '',
      code: '',
      spaceId: c?.spaceId ?? '',
    };
  });
  const [form, setForm, clear] = useDraft<Form>(`commitment:${commitmentId ?? 'new'}`, initial);
  const [why, setWhy] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [change, setChange] = useState<Change | null>(null);
  const title = useRef<HTMLInputElement>(null);
  useEffect(() => title.current?.focus(), []);
  // Deleted somewhere else: there is nothing left to edit.
  const gone = commitmentId !== undefined && snap !== null && !commitment;
  useEffect(() => {
    if (gone) onClose();
  }, [gone, onClose]);
  if (gone) return null;

  const set = (next: Partial<Form>) => {
    setForm({ ...form, ...next });
    setWhy(null);
  };
  const toggle = (code: string) =>
    set({ days: form.days.includes(code) ? form.days.filter((d) => d !== code) : [...form.days, code] });
  const repeats = form.days.length > 0;

  const save = async (e: FormEvent) => {
    e.preventDefault();
    const name = form.title.trim();
    if (!name) {
      setWhy(copy.sheet.nameFirst);
      title.current?.focus();
      return;
    }
    const start = fieldMinutes(form.start);
    const end = fieldMinutes(form.end);
    if (start === null || end === null) return setWhy(copy.sheet.timesFirst);
    if (end <= start) return setWhy(copy.sheet.endsBefore);
    if (repeats ? !form.from : !form.date) return setWhy(copy.sheet.dayFirst);
    if ([form.before, form.after].some((b) => !(Number(b) >= 0 && Number(b) <= 240))) return setWhy(copy.sheet.travelRange);
    if (form.kind === 'class' && form.course === NEW && !form.code.trim()) return setWhy(copy.sheet.codeFirst);

    const next = Object.entries(inputOf(form));
    const was = inputOf(initial) as Record<string, unknown>;
    const changed = Object.fromEntries(next.filter(([k, v]) => !same(v, was[k]))) as CommitmentInput;
    if (commitment && Object.keys(changed).length === 0) {
      clear();
      return onClose();
    }
    setBusy(true);
    const r = commitment
      ? await act(client.commitments.update(commitment.id, changed))
      : // A new one sends what is set; what is left out is the core's to fill in.
        await act(client.commitments.create(Object.fromEntries(next.filter(([, v]) => v !== null)) as CommitmentInput));
    setBusy(false);
    if (!r) return;
    clear();
    say(r.line || (commitment ? copy.sheet.saved(name) : copy.sheet.added(name)));
    onClose();
  };

  const remove = async () => {
    if (!commitment || !(await act(client.commitments.remove(commitment.id)))) return;
    clear();
    say(copy.sheet.deleted(commitment.title));
    onClose();
  };

  const restore = async (ex: CommitmentException) => {
    if (commitment) await act(client.commitments.removeException(commitment.id, ex.date));
  };

  const except = async () => {
    if (!commitment || !change) return;
    if (!change.date) return setWhy(copy.sheet.whichDay);
    const start = fieldMinutes(change.start);
    const end = fieldMinutes(change.end);
    const r = await act(
      client.commitments.addException(commitment.id, {
        date: change.date,
        kind: change.kind,
        ...(change.kind === 'move' && change.toDate ? { toDate: change.toDate } : {}),
        ...(change.kind === 'move' && start !== null ? { start } : {}),
        ...(change.kind === 'move' && end !== null ? { end } : {}),
      }),
    );
    if (!r) return;
    say(r.line);
    setChange(null);
  };
  const asking = (kind: Change['kind']) =>
    setChange(change?.kind === kind ? null : { kind, date: '', toDate: '', start: '', end: '' });

  const heading = commitment ? copy.sheet.edit(commitment.title) : copy.sheet.new;
  return (
    <form noValidate className="sheet heat-sheet heat-sheet-wide" role="dialog" aria-label={heading} onSubmit={(e) => void save(e)}>
      <h2 className="sheet-title">{heading}</h2>
      {listed && (
        <p className="why" data-text="secondary">
          {listed.when} · {listed.range}
        </p>
      )}
      <div className="heat-sheet-grid">
        <label className="field">
          <span data-text="secondary">Name</span>
          <input
            ref={title}
            value={form.title}
            placeholder="JPN 101"
            aria-describedby="heat-commit-why"
            onChange={(e) => set({ title: e.target.value })}
          />
        </label>
        <label className="field">
          <span data-text="secondary">Kind</span>
          <select value={form.kind} onChange={(e) => set({ kind: e.target.value as CommitmentKind })}>
            {(Object.keys(copy.kinds) as CommitmentKind[]).map((k) => (
              <option key={k} value={k}>
                {copy.kinds[k]}
              </option>
            ))}
          </select>
        </label>
      </div>

      <fieldset className="heat-set">
        <legend data-text="secondary">Repeats on</legend>
        <div className="heat-commit-days">
          {copy.WEEKDAYS.map((d) => (
            <button
              key={d.code}
              type="button"
              className="heat-day-toggle"
              data-dense
              data-day={d.code}
              aria-pressed={form.days.includes(d.code)}
              aria-label={d.name}
              title={d.name}
              onClick={() => toggle(d.code)}
            >
              {d.letter}
            </button>
          ))}
        </div>
        {!repeats && (
          <p className="why" data-text="secondary">
            {copy.sheet.oneDay}
          </p>
        )}
        <div className="heat-sheet-grid">
          {repeats ? (
            <>
              <label className="field">
                <span data-text="secondary">From</span>
                <input type="date" value={form.from} onChange={(e) => set({ from: e.target.value })} />
              </label>
              <label className="field">
                <span data-text="secondary">Until</span>
                <input type="date" value={form.until} onChange={(e) => set({ until: e.target.value })} />
              </label>
            </>
          ) : (
            <label className="field">
              <span data-text="secondary">Day</span>
              <input type="date" value={form.date} onChange={(e) => set({ date: e.target.value })} />
            </label>
          )}
        </div>
        {repeats && form.kind === 'class' && !form.until && (
          <p className="why" data-text="secondary">
            {copy.sheet.untilTerm}
          </p>
        )}
      </fieldset>

      <div className="heat-sheet-grid">
        <label className="field">
          <span data-text="secondary">Starts</span>
          <input type="time" value={form.start} onChange={(e) => set({ start: e.target.value })} />
        </label>
        <label className="field">
          <span data-text="secondary">Ends</span>
          <input type="time" value={form.end} onChange={(e) => set({ end: e.target.value })} />
        </label>
        <label className="field">
          <span data-text="secondary">Where</span>
          <input value={form.location} placeholder="Swan Hall 201" onChange={(e) => set({ location: e.target.value })} />
        </label>
        <label className="field">
          <span data-text="secondary">Fixed or flexible</span>
          <select value={form.hardness} onChange={(e) => set({ hardness: e.target.value as Commitment['hardness'] })}>
            <option value="fixed">{copy.sheet.fixed}</option>
            <option value="flexible">{copy.sheet.flexible}</option>
          </select>
        </label>
        <label className="field">
          <span data-text="secondary">Travel before, in minutes</span>
          <input
            type="number"
            inputMode="numeric"
            min={0}
            max={240}
            value={form.before}
            onChange={(e) => set({ before: e.target.value })}
          />
        </label>
        <label className="field">
          <span data-text="secondary">Travel after, in minutes</span>
          <input
            type="number"
            inputMode="numeric"
            min={0}
            max={240}
            value={form.after}
            onChange={(e) => set({ after: e.target.value })}
          />
        </label>
        {form.kind === 'class' && (
          <label className="field">
            <span data-text="secondary">Course</span>
            <select value={form.course} onChange={(e) => set({ course: e.target.value })}>
              <option value="">{copy.sheet.noCourse}</option>
              {courses.map((c) => (
                <option key={c.id} value={c.id}>
                  {snap?.derived.courses[c.id]?.label ?? courseLabel(c)}
                </option>
              ))}
              <option value={NEW}>{copy.sheet.newCourse}</option>
            </select>
          </label>
        )}
        {form.kind === 'class' && form.course === NEW && (
          <label className="field">
            <span data-text="secondary">New course’s code</span>
            <input value={form.code} placeholder="JPN 101" onChange={(e) => set({ code: e.target.value })} />
          </label>
        )}
        <label className="field">
          <span data-text="secondary">Space</span>
          <select value={form.spaceId} onChange={(e) => set({ spaceId: e.target.value })}>
            <option value="">{copy.sheet.automatic}</option>
            {spaces.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </label>
      </div>
      <p id="heat-commit-why" className="why" data-text="secondary" role="status">
        {why ?? copy.sheet.travelHint}
      </p>

      {commitment && (
        <fieldset className="heat-set">
          <legend data-text="secondary">{copy.sheet.exceptions}</legend>
          {commitment.exceptions.length > 0 && (
            <ul className="heat-commit-exceptions">
              {commitment.exceptions.map((ex) => (
                <li key={ex.date} data-exception={ex.date}>
                  <span>{shortMonthDay(ex.date)}</span>
                  <span data-text="secondary">
                    {ex.kind === 'skip'
                      ? copy.sheet.skipped
                      : copy.sheet.movedTo(
                          shortMonthDay(ex.toDate ?? ex.date),
                          ex.start === undefined ? null : clock(ex.start),
                        )}
                    {ex.note ? ` · ${ex.note}` : ''}
                  </span>
                  <button
                    type="button"
                    className="gel plain"
                    aria-label={`${copy.sheet.restore} ${shortMonthDay(ex.date)}`}
                    onClick={() => void restore(ex)}
                  >
                    {copy.sheet.restore}
                  </button>
                </li>
              ))}
            </ul>
          )}
          <div className="note-actions">
            <button type="button" className="gel plain" aria-expanded={change?.kind === 'skip'} onClick={() => asking('skip')}>
              {copy.sheet.skipDay}
            </button>
            <button type="button" className="gel plain" aria-expanded={change?.kind === 'move'} onClick={() => asking('move')}>
              {copy.sheet.moveDay}
            </button>
          </div>
          {change && (
            <div className="heat-commit-change" data-change={change.kind}>
              <label className="field">
                <span data-text="secondary">The day</span>
                <input type="date" value={change.date} onChange={(e) => setChange({ ...change, date: e.target.value })} />
              </label>
              {change.kind === 'move' && (
                <>
                  <label className="field">
                    <span data-text="secondary">To</span>
                    <input
                      type="date"
                      value={change.toDate}
                      onChange={(e) => setChange({ ...change, toDate: e.target.value })}
                    />
                  </label>
                  <label className="field">
                    <span data-text="secondary">Starts</span>
                    <input type="time" value={change.start} onChange={(e) => setChange({ ...change, start: e.target.value })} />
                  </label>
                  <label className="field">
                    <span data-text="secondary">Ends</span>
                    <input type="time" value={change.end} onChange={(e) => setChange({ ...change, end: e.target.value })} />
                  </label>
                </>
              )}
              <button type="button" className="gel" onClick={() => void except()}>
                {change.kind === 'skip' ? copy.sheet.skipIt : copy.sheet.moveIt}
              </button>
            </div>
          )}
        </fieldset>
      )}

      <div className="note-actions">
        <button type="submit" className="gel" disabled={busy}>
          {commitment ? copy.sheet.save : copy.sheet.add}
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
        {commitment && (
          <button type="button" className="gel plain" onClick={() => void remove()}>
            Delete
          </button>
        )}
      </div>
    </form>
  );
}
