// Settings → Learn's School sheet and calendars (docs/SPEC.md 2.13, 3.11):
// the school's name, Brightspace host, course-code pattern and term dates,
// the Brightspace calendar link, and the other calendars below it. Calendars
// come in only as private iCal addresses. An address goes to the Keychain
// and never comes back: the sheet only says whether a link is saved.

import { type FormEvent, useCallback, useEffect, useState } from 'react';
import { call } from '../bridge';

const DEFAULT_PATTERN = '^([A-Z]{3})\\s?(\\d{3})';

interface Sheet {
  name: string;
  host: string;
  codePattern: string;
  termStart: string;
  termEnd: string;
}

interface CalendarRow {
  id: string;
  name: string;
  kind: 'brightspace' | 'ical';
  lastSyncedAt: number | null;
}

interface Read {
  school?: (Partial<Sheet> & { icalSaved?: boolean }) | null;
  records: { calendar: CalendarRow[] };
}

const EMPTY: Sheet = { name: '', host: '', codePattern: DEFAULT_PATTERN, termStart: '', termEnd: '' };

function today(): string {
  const d = new Date();
  const two = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}`;
}

/** "Read today at 3:41 PM", "Read Oct 6 at 3:41 PM", or "Not read yet". */
function readLine(at: number | null): string {
  if (at === null) return 'Not read yet';
  const d = new Date(at);
  const time = d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  const sameDay = d.toDateString() === new Date().toDateString();
  return sameDay ? `Read today at ${time}` : `Read ${d.toLocaleDateString([], { month: 'short', day: 'numeric' })} at ${time}`;
}

const sentence = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** `ask` is the core's `call`; a test passes its fake core's. */
export function SchoolSheet({ ask = call }: { ask?: typeof call }) {
  const [sheet, setSheet] = useState<Sheet>(EMPTY);
  const [calendars, setCalendars] = useState<CalendarRow[]>([]);
  const [link, setLink] = useState('');
  const [added, setAdded] = useState({ name: '', url: '' });
  const [said, setSaid] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const read = useCallback(
    () =>
      ask<Read>('heat.snapshot', { date: today() }).then((snap) => {
        setCalendars(snap.records.calendar);
        return snap;
      }),
    [ask],
  );

  useEffect(() => {
    read().then(
      (snap) => {
        const s = snap.school;
        if (s) {
          setSheet({
            name: s.name ?? '',
            host: s.host ?? '',
            codePattern: s.codePattern || DEFAULT_PATTERN,
            termStart: s.termStart ?? '',
            termEnd: s.termEnd ?? '',
          });
        }
      },
      (e) => setSaid(sentence(e)),
    );
  }, [read]);

  const brightspace = calendars.find((c) => c.kind === 'brightspace');
  const others = calendars.filter((c) => c.kind === 'ical');

  // One act at a time; whatever it says, or why it couldn't, is the status line.
  const run = (act: () => Promise<string | null>) => {
    setBusy(true);
    act()
      .then(setSaid, (e) => setSaid(sentence(e)))
      .then(read)
      .catch((e) => setSaid(sentence(e)))
      .finally(() => setBusy(false));
  };

  const sync = () => ask<{ line: string }>('heat.calendars.sync', {}).then((r) => r.line);

  const saveSchool = (e: FormEvent) => {
    e.preventDefault();
    const url = link.trim();
    run(async () => {
      await ask('heat.school.set', { ...sheet, ...(url ? { icalUrl: url } : {}) });
      setLink('');
      return url ? sync() : 'Saved.';
    });
  };

  const addCalendar = (e: FormEvent) => {
    e.preventDefault();
    run(async () => {
      await ask('heat.calendars.add', { name: added.name, kind: 'ical', url: added.url });
      setAdded({ name: '', url: '' });
      return sync();
    });
  };

  const remove = (c: CalendarRow) =>
    run(async () => {
      await ask('heat.calendars.remove', { id: c.id });
      return c.kind === 'brightspace'
        ? 'Removed the Brightspace calendar. The tasks it made stay.'
        : `Removed ${c.name}. Its events are gone from the time column.`;
    });

  const field = (key: keyof Sheet) => ({
    value: sheet[key],
    onChange: (e: { target: { value: string } }) => setSheet({ ...sheet, [key]: e.target.value }),
  });

  return (
    <>
      <form className="setting school" onSubmit={saveSchool}>
        <p className="setting-label" data-text="secondary">
          School
        </p>
        <label className="field">
          Name
          <input aria-label="School name" {...field('name')} />
        </label>
        <label className="field">
          Brightspace host
          <input aria-label="Brightspace host" placeholder="brightspace.uri.edu" {...field('host')} />
        </label>
        <label className="field">
          Course-code pattern
          <input aria-label="Course-code pattern" className="path" spellCheck={false} {...field('codePattern')} />
        </label>
        <div className="school-term">
          <label className="field">
            Term starts
            <input aria-label="Term starts" type="date" {...field('termStart')} />
          </label>
          <label className="field">
            Term ends
            <input aria-label="Term ends" type="date" {...field('termEnd')} />
          </label>
        </div>
        <label className="field">
          Brightspace calendar link
          <input
            aria-label="Brightspace calendar link"
            type="password"
            autoComplete="off"
            spellCheck={false}
            placeholder={brightspace ? 'Saved in the keychain. Paste a new link to replace it.' : 'Paste the iCal link, which starts with https:// or webcal://'}
            value={link}
            onChange={(e) => setLink(e.target.value)}
          />
        </label>
        <p className="why" data-text="secondary">
          In Brightspace, open Calendar, then Subscribe, and copy the link. Anyone who holds it can read your calendar,
          so it is kept in the keychain on this Mac and nowhere else.
        </p>
        <div className="school-acts">
          <button type="submit" className="gel" disabled={busy}>
            Save
          </button>
          {brightspace && (
            <>
              <span data-text="secondary">{readLine(brightspace.lastSyncedAt)}</span>
              <button type="button" className="gel plain" disabled={busy} onClick={() => remove(brightspace)}>
                Remove the link
              </button>
            </>
          )}
        </div>
      </form>

      <div className="setting">
        <p className="setting-label" data-text="secondary">
          Other calendars
        </p>
        {others.length === 0 ? (
          <p className="why" data-text="secondary">
            None yet. Their events show grey in the time column and never make tasks.
          </p>
        ) : (
          <ul className="calendars">
            {others.map((c) => (
              <li key={c.id}>
                <span className="calendar-name">{c.name}</span>
                <span data-text="secondary">{readLine(c.lastSyncedAt)}</span>
                <button type="button" className="gel plain" disabled={busy} aria-label={`Remove ${c.name}`} onClick={() => remove(c)}>
                  Remove
                </button>
              </li>
            ))}
          </ul>
        )}
        <form className="calendar-add" onSubmit={addCalendar}>
          <input
            aria-label="Calendar name"
            placeholder="Name"
            value={added.name}
            onChange={(e) => setAdded({ ...added, name: e.target.value })}
          />
          <input
            aria-label="Calendar iCal address"
            type="password"
            autoComplete="off"
            spellCheck={false}
            placeholder="Its secret address in iCal format"
            value={added.url}
            onChange={(e) => setAdded({ ...added, url: e.target.value })}
          />
          <button type="submit" className="gel" disabled={busy || !added.name.trim() || !added.url.trim()}>
            Add
          </button>
        </form>
        <p className="why" data-text="secondary">
          Google Calendar gives one in each calendar’s settings, under “Secret address in iCal format”.
        </p>
      </div>

      <div className="setting">
        <button type="button" className="gel" disabled={busy || calendars.length === 0} onClick={() => run(sync)}>
          Read the calendars now
        </button>
        {said && (
          <p role="status" className="calendar-said">
            {said}
          </p>
        )}
      </div>
    </>
  );
}
