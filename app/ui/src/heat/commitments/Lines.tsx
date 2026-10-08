// The lines commitments add to Today and Calendar (docs/COMMITMENTS.md).
// Under Today's subtitle: what is next and when to leave ("NEXT JPN 101
// 10:00 · LEAVE 9:35"), and what the day has left ("3h 20m free today.
// Planned 4h. Move 40m?"). Both are the core's sentences, shown as given.
// And each change a mail asked for, one quiet row with its one tap: nothing
// is ever applied without it.

import { useState } from 'react';
import type { Id, PendingException } from '../client';
import { useHeat } from '../store';
import * as copy from './copy';
import './commitments.css';

export function CommitmentLines() {
  const { snap } = useHeat();
  const c = snap?.commitments;
  if (!c) return null;
  return (
    <>
      {c.next && <p className="heat-next-line">{c.next.line}</p>}
      {c.free.line && (
        <p className="heat-free-line" data-over={c.free.overMin > 0} data-text="secondary">
          {c.free.line}
        </p>
      )}
    </>
  );
}

export function PendingExceptions() {
  const { snap, client, act, say, refetch } = useHeat();
  const [busy, setBusy] = useState<Id | null>(null);
  const list = snap?.commitments?.pending ?? [];
  if (list.length === 0) return null;

  const confirm = async (p: PendingException) => {
    setBusy(p.id);
    const r = await act(client.commitments.confirm(p.id));
    setBusy(null);
    if (!r) return;
    say(r.line);
    await refetch();
  };
  const dismiss = async (p: PendingException) => {
    setBusy(p.id);
    const r = await act(client.commitments.dismiss(p.id));
    setBusy(null);
    if (r) await refetch();
  };

  return (
    <div className="heat-pending" role="group" aria-label={copy.pending.label}>
      {list.map((p) => (
        <p key={p.id} className="heat-pending-row" data-pending={p.id}>
          <span className="heat-pending-line">{p.line}</span>
          <button type="button" className="gel" disabled={busy === p.id} onClick={() => void confirm(p)}>
            {p.act}
          </button>
          <button type="button" className="gel plain" disabled={busy === p.id} onClick={() => void dismiss(p)}>
            {copy.pending.later}
          </button>
        </p>
      ))}
    </div>
  );
}
