// Calendar's sidebar section, "Schedule" (docs/COMMITMENTS.md): every
// commitment with its colour, when it meets and its dates, as the snapshot's
// `commitments.list` words them; a click opens it. Below: a new commitment,
// Import schedule and the academic calendar; the schedules read and waiting
// for a look; the calendars kept in step, each with a remove; and when the
// person sleeps, which nothing is planned into.

import { type CSSProperties, useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import type { DayKey } from '../../shared/time/zone';
import type { Id } from '../client';
import { useSheets, useSidebarSlot } from '../frame';
import { useHeat } from '../store';
import { CommitmentSheet } from './CommitmentSheet';
import * as copy from './copy';
import { ImportSheet } from './ImportSheet';
import { TermSheet } from './TermSheet';
import { fieldMinutes, timeField } from './time';
import './commitments.css';

/** `day` is the day Calendar has selected: a new commitment starts there. */
export function Schedule({ day }: { day?: DayKey }) {
  const slot = useSidebarSlot();
  return slot ? createPortal(<ScheduleSection day={day} />, slot) : null;
}

function ScheduleSection({ day }: { day?: DayKey }) {
  const { snap, client, act, say, refetch } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const c = snap?.commitments;
  const listed = new Map((c?.list ?? []).map((l) => [l.id, l]));
  const drafts = c?.drafts ?? [];
  const count = (state: string) => drafts.filter((d) => d.state === state).length;
  const review = (state: string) => {
    const draft = drafts.find((d) => d.state === state);
    if (draft) openSheet(<ImportSheet draftId={draft.id} onClose={closeSheet} />);
  };

  const removeFeed = async (id: Id, name: string) => {
    if (!(await act(client.commitments.removeFeed(id)))) return;
    say(copy.schedule.feedRemoved(name));
    await refetch();
  };

  return (
    <nav className="heat-schedule-side" aria-label={copy.schedule.heading}>
      <h2 className="heat-side-heading" data-text="secondary">
        {copy.schedule.heading}
      </h2>
      {(snap?.records.commitment ?? []).map((k) => {
        const l = listed.get(k.id);
        const hue = l?.hue ?? null;
        return (
          <button
            key={k.id}
            type="button"
            className="heat-side-row heat-schedule-row"
            data-dense
            data-commitment-row={k.id}
            title={[k.title, l?.when, l?.range, l?.label].filter(Boolean).join(', ')}
            onClick={() => openSheet(<CommitmentSheet commitmentId={k.id} onClose={closeSheet} />)}
          >
            <span
              className="heat-commit-dot"
              data-hue={hue !== null}
              style={hue === null ? undefined : ({ '--hue': hue } as CSSProperties)}
              aria-hidden="true"
            />
            <span className="heat-schedule-words">
              <span className="heat-side-name">{k.title}</span>
              {l && (
                <>
                  <span className="heat-schedule-when" data-text="secondary">
                    {l.when}
                  </span>
                  <span className="heat-schedule-when" data-text="secondary">
                    {l.range}
                  </span>
                </>
              )}
            </span>
          </button>
        );
      })}
      {snap && snap.records.commitment.length === 0 && (
        <p className="heat-side-why" data-text="secondary">
          {copy.schedule.none}
        </p>
      )}

      {count('ready') > 0 && (
        <button type="button" className="heat-side-row heat-schedule-waiting" data-dense onClick={() => review('ready')}>
          {copy.schedule.toReview(count('ready'))}
        </button>
      )}
      {count('reading') > 0 && (
        <p className="heat-side-why" data-text="secondary" role="status">
          {copy.schedule.reading(count('reading'))}
        </p>
      )}
      {count('failed') > 0 && (
        <button type="button" className="heat-side-row heat-schedule-waiting" data-dense onClick={() => review('failed')}>
          {copy.schedule.failed(count('failed'))}
        </button>
      )}

      <button
        type="button"
        className="heat-side-row heat-side-new"
        data-dense
        onClick={() => openSheet(<CommitmentSheet day={day} onClose={closeSheet} />)}
      >
        {copy.schedule.new}
      </button>
      <button
        type="button"
        className="heat-side-row heat-side-new"
        data-dense
        onClick={() => openSheet(<ImportSheet onClose={closeSheet} />)}
      >
        {copy.schedule.import}
      </button>
      <button
        type="button"
        className="heat-side-row heat-side-new"
        data-dense
        onClick={() => openSheet(<TermSheet onClose={closeSheet} />)}
      >
        {copy.schedule.calendar}
      </button>

      {(c?.feeds.length ?? 0) > 0 && (
        <>
          <h2 className="heat-side-heading" data-text="secondary">
            {copy.schedule.feeds}
          </h2>
          {c?.feeds.map((f) => (
            <div key={f.id} className="heat-side-row heat-schedule-feed" data-feed={f.id}>
              <span className="heat-side-name">{f.name}</span>
              <button
                type="button"
                className="heat-schedule-remove"
                data-dense
                aria-label={copy.schedule.removeFeed(f.name)}
                title={copy.schedule.removeFeed(f.name)}
                onClick={() => void removeFeed(f.id, f.name)}
              >
                ×
              </button>
            </div>
          ))}
        </>
      )}

      {c && <Sleep from={c.sleep.from} to={c.sleep.to} />}
    </nav>
  );
}

/** To bed and up: two times of day, saved as soon as both read as times. */
function Sleep({ from, to }: { from: number; to: number }) {
  const { client, act, refetch } = useHeat();
  const [bed, setBed] = useState(timeField(from));
  const [up, setUp] = useState(timeField(to));
  // The core's answer is what the fields show, whoever set it.
  useEffect(() => {
    setBed(timeField(from));
    setUp(timeField(to));
  }, [from, to]);

  const save = async (nextBed: string, nextUp: string) => {
    const [a, b] = [fieldMinutes(nextBed), fieldMinutes(nextUp)];
    if (a === null || b === null || (a === from && b === to)) return;
    if (await act(client.commitments.setSleep(a, b))) await refetch();
  };

  return (
    <>
      <h2 className="heat-side-heading" data-text="secondary">
        {copy.schedule.sleep}
      </h2>
      <div className="heat-schedule-sleep">
        <label className="field">
          <span data-text="secondary">{copy.schedule.toBed}</span>
          <input
            type="time"
            value={bed}
            onChange={(e) => {
              setBed(e.target.value);
              void save(e.target.value, up);
            }}
          />
        </label>
        <label className="field">
          <span data-text="secondary">{copy.schedule.up}</span>
          <input
            type="time"
            value={up}
            onChange={(e) => {
              setUp(e.target.value);
              void save(bed, e.target.value);
            }}
          />
        </label>
      </div>
    </>
  );
}
