// The quiet notices (docs/NOTES.md): one line each for what Learn just did
// on its own, with its undo or its one act. "Filed to JPN 101 · Oct 7" has
// Undo; "Class moved to room 204 on Friday" has the act the core named. They
// sit in a corner of Learn's frame whichever tab is showing, newest first,
// three at most, and stay until they are answered or dismissed.

import type { Notice } from '../client';
import { useTabs } from '../frame';
import { useHeat } from '../store';
import { openNote } from './open';
import './notes.css';

const SHOWN = 3;

export function Notices() {
  const { snap, client, act, refetch } = useHeat();
  const { setTab } = useTabs();
  const notices = (snap?.notices ?? []).slice(0, SHOWN);

  /** Does what a notice offered; once it is done the notice has nothing left to say. */
  const answer = async (notice: Notice, going: Promise<unknown>) => {
    if ((await act(going)) !== null) await act(client.notices.dismiss(notice.id));
    await refetch();
  };
  const dismiss = async (notice: Notice) => {
    await act(client.notices.dismiss(notice.id));
    await refetch();
  };
  const open = async (noteId: string) => {
    openNote(noteId);
    setTab('notes');
    await refetch();
  };

  return (
    <div className="notes-notices" role="region" aria-label="Notices" aria-live="polite">
      {notices.map((n) => (
        <div key={n.id} className="notes-notice" data-kind={n.kind}>
          <span className="notes-notice-text">{n.text}</span>
          <span className="notes-notice-acts">
            {n.undo && (
              <button type="button" className="heat-link" data-dense onClick={() => void answer(n, client.notices.undo(n.undo!.txnId))}>
                Undo
              </button>
            )}
            {n.act && (
              <button type="button" className="heat-link" data-dense onClick={() => void answer(n, client.notices.run(n.act!.cmd, n.act!.args))}>
                {n.act.label}
              </button>
            )}
            {n.noteId && (
              <button type="button" className="heat-link" data-dense onClick={() => void open(n.noteId!)}>
                Open
              </button>
            )}
            <button type="button" className="notes-notice-x" data-dense aria-label={`Dismiss: ${n.text}`} onClick={() => void dismiss(n)}>
              ×
            </button>
          </span>
        </div>
      ))}
    </div>
  );
}
