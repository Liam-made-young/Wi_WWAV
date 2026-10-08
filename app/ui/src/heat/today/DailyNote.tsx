// The daily note (docs/SPEC.md 3.5): collapsed to one line below the plan
// until clicked, "How's the day going? Markdown + [[wikilinks]] welcome."
// It saves when you leave it, and Esc leaves it with what you typed kept.

import { useEffect, useRef, useState } from 'react';
import { copy } from '../fmt';
import { useHeat } from '../store';

export function useDailyNote() {
  const { snap, client, act, date } = useHeat();
  // The day's note is the note titled with the day (docs/NOTES.md); a `dailyNote` record from before stands in until it is moved over.
  const noteId = snap?.notes?.daily ?? null;
  const note = noteId ? snap?.records.note.find((n) => n.id === noteId) : undefined;
  const saved = note?.markdown ?? snap?.records.dailyNote.find((n) => n.date === date)?.markdown ?? '';
  const [open, setOpen] = useState(false);
  const [text, setText] = useState(saved);
  const latest = useRef({ text, saved, open });
  latest.current = { text, saved, open };

  useEffect(() => {
    if (!open) setText(saved);
  }, [saved, open]);

  return {
    open,
    text,
    saved,
    setText,
    begin: () => setOpen(true),
    /** Leaves the note, saving what changed. Says whether there was a note open to leave. */
    leave(): boolean {
      const { text: t, saved: s, open: o } = latest.current;
      if (!o) return false;
      setOpen(false);
      if (t !== s) void act(client.notes.daily(date, t));
      return true;
    },
  };
}

export type DailyNote = ReturnType<typeof useDailyNote>;

export function DailyNoteLine({ note }: { note: DailyNote }) {
  const field = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    if (note.open) field.current?.focus();
  }, [note.open]);

  if (note.open) {
    return (
      <div className="heat-note">
        <textarea
          ref={field}
          className="heat-note-field"
          aria-label="Daily note"
          rows={4}
          placeholder={copy.today.dailyNote}
          value={note.text}
          onChange={(e) => note.setText(e.target.value)}
          onBlur={() => note.leave()}
        />
      </div>
    );
  }
  const first = note.saved.split('\n')[0];
  return (
    <div className="heat-note">
      <button type="button" className="heat-note-line" data-dense onClick={note.begin} aria-label="Daily note">
        {first || copy.today.dailyNote}
      </button>
    </div>
  );
}
