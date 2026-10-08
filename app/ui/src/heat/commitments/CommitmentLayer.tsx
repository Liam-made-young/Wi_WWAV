// The layer Calendar and Today draw (docs/COMMITMENTS.md): each of a day's
// commitments as a solid, quiet block in its space's colour, behind the task
// blocks, with its travel time as a faint extension above and below. A task
// block has a 3 px heat border and another calendar's event is grey and
// hatched, so the three read apart. A day an exception moved says "moved".
// A click opens the commitment; nothing here drags.
//
// Every day's list comes from `snap.commitments.days`, with exceptions,
// breaks and the term's end already applied: this draws what it is given.
// The month shows the same days as a row of dots, and a block that overlaps
// a commitment or its travel wears the core's line for it as a small flag.

import type { CSSProperties } from 'react';
import { clock, shortMonthDay } from '../../shared/time/format';
import type { DayKey } from '../../shared/time/zone';
import type { CommitmentOccurrence, Id, Snapshot } from '../client';
import { useSheets } from '../frame';
import { useHeat } from '../store';
import { CommitmentSheet } from './CommitmentSheet';
import * as copy from './copy';
import './commitments.css';

/** A line of text in a block. */
const LINE_PX = 16;
/** The least a commitment is drawn: it is a control, and a dense one is 24 px (3.18). */
const MIN_PX = 24;
/** How many dots a month cell draws before it says "+2". */
const DOTS = 4;

const hueOf = (o: CommitmentOccurrence) => (o.hue === null ? {} : ({ '--hue': o.hue } as CSSProperties));

/** "JPN 101, 10:00 AM to 10:50 AM, Swan Hall 201. Moved from Oct 9." */
function words(o: CommitmentOccurrence): string {
  const parts = [o.title, copy.layer.span(clock(o.start), clock(o.end)), ...(o.location ? [o.location] : [])];
  return `${parts.join(', ')}${o.movedFrom ? `. ${copy.layer.movedFrom(shortMonthDay(o.movedFrom))}.` : ''}`;
}

interface Props {
  day: DayKey;
  /** Where a minute after midnight sits in the grid, in px. */
  toY(min: number): number;
  /** The grid's first and last minute: Today's column starts at 7 AM. */
  from?: number;
  to?: number;
}

export function CommitmentLayer({ day, toY, from = 0, to = 1440 }: Props) {
  const { snap } = useHeat();
  const { openSheet, closeSheet } = useSheets();
  const list = snap?.commitments?.days[day] ?? [];

  return (
    <>
      {list.map((o) => {
        const top = Math.max(from, o.start);
        const bottom = Math.min(to, o.end);
        if (bottom <= top) return null;
        const before = Math.max(from, o.start - o.bufferBefore);
        const after = Math.min(to, o.end + o.bufferAfter);
        const height = toY(bottom) - toY(top);
        const rows = Math.floor(height / LINE_PX);
        const key = `${o.commitmentId}-${o.start}`;
        return [
          before < top && (
            <div
              key={`${key}-before`}
              className="heat-commit-buffer"
              data-buffer="before"
              data-hue={o.hue !== null}
              style={{ top: toY(before), height: toY(top) - toY(before), ...hueOf(o) }}
              aria-hidden="true"
            />
          ),
          <button
            key={key}
            type="button"
            className="heat-commit"
            data-dense
            data-commitment={o.commitmentId}
            data-kind={o.kind}
            data-hue={o.hue !== null}
            data-moved={o.movedFrom !== undefined}
            style={{ top: toY(top), height: Math.max(MIN_PX, height), ...hueOf(o) }}
            title={words(o)}
            aria-label={words(o)}
            onClick={() => openSheet(<CommitmentSheet commitmentId={o.commitmentId} onClose={closeSheet} />)}
          >
            <span className="heat-commit-title">
              {o.movedFrom && <span className="heat-commit-moved">{copy.layer.moved}</span>}
              <span className="heat-commit-name">{o.title}</span>
            </span>
            {rows >= 2 && <span className="heat-commit-sub">{copy.layer.span(clock(o.start), clock(o.end))}</span>}
            {rows >= 3 && o.location && <span className="heat-commit-sub">{o.location}</span>}
          </button>,
          after > bottom && (
            <div
              key={`${key}-after`}
              className="heat-commit-buffer"
              data-buffer="after"
              data-hue={o.hue !== null}
              style={{ top: toY(bottom), height: toY(after) - toY(bottom), ...hueOf(o) }}
              aria-hidden="true"
            />
          ),
        ];
      })}
    </>
  );
}

/** A month cell's commitments: a dot each in its colour, four at most, with the rest as a number. */
export function CommitmentDots({ day }: { day: DayKey }) {
  const { snap } = useHeat();
  const list = snap?.commitments?.days[day] ?? [];
  if (list.length === 0) return null;
  const said = list.map((o) => `${o.title} ${clock(o.start)}`).join(', ');
  return (
    <span className="heat-commit-dots" role="img" aria-label={said} title={said}>
      {list.slice(0, DOTS).map((o) => (
        <span
          key={`${o.commitmentId}-${o.start}`}
          className="heat-commit-dot"
          data-hue={o.hue !== null}
          style={hueOf(o)}
        />
      ))}
      {list.length > DOTS && (
        <span className="heat-commit-more" data-text="secondary">
          {copy.layer.more(list.length - DOTS)}
        </span>
      )}
    </span>
  );
}

/** The blocks that overlap a commitment or its travel, each with the core's sentence for it. */
export function conflictLines(snap: Snapshot | null): Map<Id, string> {
  const out = new Map<Id, string>();
  for (const c of snap?.commitments?.conflicts ?? []) {
    const said = out.get(c.blockId);
    out.set(c.blockId, said ? `${said} ${c.line}` : c.line);
  }
  return out;
}

/** The small flag a block wears when it overlaps a commitment: the sentence is its title. */
export function ConflictFlag({ line }: { line: string | undefined }) {
  if (!line) return null;
  return (
    <span className="heat-conflict" data-conflict-flag title={line}>
      <span className="heat-sr">{line}</span>
    </span>
  );
}
