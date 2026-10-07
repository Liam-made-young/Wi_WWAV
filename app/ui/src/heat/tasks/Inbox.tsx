// The inbox (docs/SPEC.md 2.7, 3.14): captures wait here until they are
// triaged "→ task", "→ note", "→ project" or "→ upload". Return on the row
// under the cursor makes it a task and opens it for editing.

import { useActions } from '../actions';
import type { Id } from '../client';
import { copy } from '../fmt';
import { useHeat } from '../store';

const ACTS = [
  ['task', copy.capture.toTask, 'Make a task'],
  ['note', copy.capture.toNote, 'Make a note'],
  ['project', copy.capture.toProject, 'Make a project'],
  ['upload', copy.capture.toUpload, 'Make an upload'],
] as const;

export function Inbox({ ids, cursor, onCursor }: { ids: Id[]; cursor: Id | null; onCursor(id: Id): void }) {
  const { idx } = useHeat();
  const actions = useActions();

  if (ids.length === 0) return <p className="heat-empty">{copy.capture.inboxZero}</p>;
  return (
    <div role="listbox" aria-label="Inbox" className="heat-inbox">
      {ids.map((id) => {
        const c = idx.capture.get(id);
        if (!c) return null;
        return (
          <div
            key={id}
            id={`heat-row-c-${id}`}
            role="option"
            className="heat-row heat-inbox-row"
            data-dense
            aria-selected={cursor === id}
            onClick={() => onCursor(id)}
          >
            <span className="heat-row-title">{c.text}</span>
            <span className="heat-inbox-acts">
              {ACTS.map(([to, label, long]) => (
                <button
                  key={to}
                  type="button"
                  className="heat-inbox-act"
                  data-dense
                  aria-label={`${long}: ${c.text}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    void actions.triage(id, to);
                  }}
                >
                  {label}
                </button>
              ))}
            </span>
          </div>
        );
      })}
    </div>
  );
}
