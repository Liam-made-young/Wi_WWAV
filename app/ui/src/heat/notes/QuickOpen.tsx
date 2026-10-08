// Quick open (docs/NOTES.md): a small field in the middle of the tab that
// lists notes by title as you type, the ones whose title starts with the
// words first. ↑ ↓ move, Return opens, Esc closes.

import { useEffect, useId, useRef, useState } from 'react';
import type { Id } from '../client';

const LISTED = 10;

export function QuickOpen({ notes, onOpen, onClose }: { notes: { id: Id; title: string }[]; onOpen(id: Id): void; onClose(): void }) {
  const [text, setText] = useState('');
  const [row, setRow] = useState(0);
  const field = useRef<HTMLInputElement>(null);
  const listId = useId();
  useEffect(() => field.current?.focus(), []);

  const needle = text.trim().toLowerCase();
  const has = notes.filter((n) => n.title.toLowerCase().includes(needle));
  const starts = (n: { title: string }) => n.title.toLowerCase().startsWith(needle);
  const rows = [...has.filter(starts), ...has.filter((n) => !starts(n))].slice(0, LISTED);
  const at = Math.min(row, Math.max(0, rows.length - 1));

  return (
    <div className="notes-quick-back" onClick={onClose}>
      <div className="notes-quick" role="dialog" aria-label="Quick open" onClick={(e) => e.stopPropagation()}>
        <input
          ref={field}
          type="text"
          role="combobox"
          className="notes-quick-field"
          aria-label="Open a note by its title"
          aria-expanded={rows.length > 0}
          aria-controls={listId}
          aria-activedescendant={rows.length > 0 ? `${listId}-${at}` : undefined}
          aria-autocomplete="list"
          placeholder="Open a note…"
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setRow(0);
          }}
          onKeyDown={(e) => {
            if (e.nativeEvent.isComposing) return;
            if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
              e.preventDefault();
              if (rows.length > 0) setRow((at + (e.key === 'ArrowDown' ? 1 : -1) + rows.length) % rows.length);
            } else if (e.key === 'Enter') {
              e.preventDefault();
              if (rows[at]) onOpen(rows[at].id);
            } else if (e.key === 'Escape') {
              e.preventDefault();
              onClose();
            }
          }}
        />
        <ul id={listId} role="listbox" aria-label="Notes" className="notes-quick-list">
          {rows.map((n, i) => (
            <li
              key={n.id}
              id={`${listId}-${i}`}
              role="option"
              className="notes-link-row"
              data-dense
              aria-selected={i === at}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => onOpen(n.id)}
            >
              {n.title}
            </li>
          ))}
        </ul>
        {rows.length === 0 && (
          <p className="why" data-text="secondary">
            {notes.length === 0 ? 'There are no notes yet.' : 'No note has that in its title.'}
          </p>
        )}
      </div>
    </div>
  );
}
