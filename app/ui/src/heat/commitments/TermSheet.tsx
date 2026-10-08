// The academic calendar (docs/COMMITMENTS.md): the term's dates, which are
// set in Settings and only read here, and the breaks, the days classes don't
// meet. A break is added by hand (`heat.put` of a `termBreak`), deleted, or
// read from the school's calendar through Import schedule. A class skips
// every day inside one.

import { type FormEvent, useEffect, useRef, useState } from 'react';
import { shortMonthDay } from '../../shared/time/format';
import type { TermBreak } from '../client';
import { useSheets } from '../frame';
import { useHeat } from '../store';
import * as copy from './copy';
import { ImportSheet } from './ImportSheet';

export function TermSheet({ onClose }: { onClose(): void }) {
  const { snap, client, act, say } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const term = snap?.commitments?.term;
  const breaks = [...(snap?.records.termBreak ?? [])].sort((a, b) => a.from.localeCompare(b.from));
  const [title, setTitle] = useState('');
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [why, setWhy] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const name = useRef<HTMLInputElement>(null);
  useEffect(() => name.current?.focus(), []);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    const named = title.trim();
    if (!named) {
      setWhy(copy.term.nameFirst);
      name.current?.focus();
      return;
    }
    if (!from) return setWhy(copy.term.datesFirst);
    // A break with no last day is one day long.
    const last = to || from;
    if (last < from) return setWhy(copy.term.order);
    setBusy(true);
    const r = await act(client.put('termBreak', { title: named, from, to: last, source: 'you' }));
    setBusy(false);
    if (!r) return;
    say(copy.term.added(named));
    setTitle('');
    setFrom('');
    setTo('');
    setWhy(null);
  };

  const remove = async (b: TermBreak) => {
    if (await act(client.delete('termBreak', b.id))) say(copy.term.deleted(b.title));
  };

  return (
    <form noValidate className="sheet heat-sheet heat-sheet-wide" role="dialog" aria-label={copy.term.title} onSubmit={(e) => void add(e)}>
      <h2 className="sheet-title">{copy.term.title}</h2>
      <p className="heat-term-dates">
        {term?.start && term.end ? copy.term.dates(shortMonthDay(term.start), shortMonthDay(term.end)) : copy.term.noDates}{' '}
        <span data-text="secondary">{copy.term.setIn}</span>
      </p>

      <fieldset className="heat-set">
        <legend data-text="secondary">{copy.term.breaks}</legend>
        {breaks.length === 0 ? (
          <p className="why" data-text="secondary">
            {copy.term.none}
          </p>
        ) : (
          <ul className="heat-commit-exceptions">
            {breaks.map((b) => (
              <li key={b.id} data-break={b.id}>
                <span>{b.title}</span>
                <span data-text="secondary">
                  {b.from === b.to ? shortMonthDay(b.from) : `${shortMonthDay(b.from)} to ${shortMonthDay(b.to)}`}
                </span>
                <button type="button" className="gel plain" aria-label={copy.term.delete(b.title)} onClick={() => void remove(b)}>
                  Delete
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="heat-term-add">
          <label className="field">
            <span data-text="secondary">Break</span>
            <input
              ref={name}
              value={title}
              placeholder="Thanksgiving recess"
              aria-describedby="heat-term-why"
              onChange={(e) => {
                setTitle(e.target.value);
                setWhy(null);
              }}
            />
          </label>
          <label className="field">
            <span data-text="secondary">First day</span>
            <input type="date" value={from} onChange={(e) => setFrom(e.target.value)} />
          </label>
          <label className="field">
            <span data-text="secondary">Last day</span>
            <input type="date" value={to} onChange={(e) => setTo(e.target.value)} />
          </label>
          <button type="submit" className="gel" disabled={busy}>
            {copy.term.add}
          </button>
        </div>
        <p id="heat-term-why" className="why" data-text="secondary" role="status">
          {why}
        </p>
      </fieldset>

      <div className="note-actions">
        <button type="button" className="gel" onClick={onClose}>
          {copy.term.done}
        </button>
        <button type="button" className="gel plain" onClick={() => openSheet(<ImportSheet mode="breaks" onClose={closeSheet} />)}>
          {copy.term.import}
        </button>
      </div>
    </form>
  );
}
