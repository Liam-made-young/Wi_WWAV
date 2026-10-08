// "Open this note", asked from outside the Notes tab: a notice's Open button
// sits in Learn's frame, and the tab that holds the note is somewhere else.
// The asker names the note here and switches to Notes; the tab hears it and
// opens it. One request at a time, taken once.

import { useEffect } from 'react';
import type { Id } from '../client';

let asked: Id | null = null;
const hearers = new Set<() => void>();

/** Asks the Notes tab to open a note. */
export function openNote(id: Id) {
  asked = id;
  for (const hear of [...hearers]) hear();
}

/** The Notes tab's side: `open` is called with each note asked for, once. */
export function useOpenRequests(open: (id: Id) => void) {
  useEffect(() => {
    const hear = () => {
      const id = asked;
      asked = null;
      if (id) open(id);
    };
    hearers.add(hear);
    // One asked for before the tab was there.
    hear();
    return () => void hearers.delete(hear);
  }, [open]);
}
