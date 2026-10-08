// Mail's outbox (docs/SPEC.md 3.10): what was asked for in Mail and is
// waiting for Claude to do in Gmail, what failed and why, and the last few
// that went. A failed one can be tried again or thrown away; nothing in it
// is ever sent twice.

import { useEffect, useState } from 'react';
import { clockAt, shortMonthDay } from '../../shared/time/format';
import { dayKey } from '../../shared/time/zone';
import type { MailAction, MailThread } from '../client';
import { copy, sentenceOf } from '../fmt';
import { useHeat } from '../store';

const KIND: Record<MailAction['kind'], string> = {
  send: 'New mail',
  reply: 'Reply',
  archive: 'Archive',
  unarchive: 'Move to the inbox',
  markRead: 'Mark read',
  markUnread: 'Mark unread',
};
const STATUS: Record<MailAction['status'], string> = { queued: 'Waiting', done: 'Done', failed: 'Didn’t go' };

export function Outbox({ threads }: { threads: MailThread[] }) {
  const { client, snap, tz, date } = useHeat();
  const [actions, setActions] = useState<MailAction[] | null>(null);
  const [said, setSaid] = useState<string | null>(null);

  // Read again whenever the snapshot moves: a run finishing an action moves it.
  useEffect(() => {
    let live = true;
    client.mail.outbox().then(
      (r) => live && setActions(r.actions),
      (e) => live && setSaid(sentenceOf(e)),
    );
    return () => {
      live = false;
    };
  }, [client, snap]);

  if (!actions) return <div className="heat-outbox" aria-busy="true" />;
  const subjectOf = (a: MailAction) =>
    a.subject ?? threads.find((m) => m.gmailThreadId === a.threadId)?.subject ?? 'A thread no longer in Mail';
  const stamp = (ms: number) => (dayKey(ms, tz) === date ? clockAt(ms, tz) : shortMonthDay(dayKey(ms, tz)));
  const act = (going: Promise<unknown>) => going.catch((e) => setSaid(sentenceOf(e)));
  // Newest first; what still needs you is at the top either way.
  const rows = [...actions].reverse().sort((a, b) => Number(b.status !== 'done') - Number(a.status !== 'done'));

  return (
    <div className="heat-outbox">
      {rows.length === 0 && <p className="heat-empty">{copy.status.empty}</p>}
      {rows.map((a) => (
        <div key={a.id} className="heat-outbox-row" data-status={a.status}>
          <div className="heat-outbox-what">
            <span className="heat-tag">{KIND[a.kind]}</span>
            <span className="heat-outbox-subject">{subjectOf(a)}</span>
            {a.to && <span data-text="secondary">to {a.to}</span>}
          </div>
          {a.body && <p className="heat-outbox-body">{a.body}</p>}
          <div className="heat-outbox-foot">
            <span className="heat-tag">{STATUS[a.status]}</span>
            <span data-text="secondary">{stamp(a.doneAt ?? a.createdAt)}</span>
            {a.error && <span role="alert">{a.error}</span>}
            {a.status === 'failed' && (
              <button type="button" className="gel plain" aria-label={`Try ${subjectOf(a)} again`} onClick={() => void act(client.mail.retry(a.id))}>
                Try again
              </button>
            )}
            {a.status !== 'done' && (
              <button type="button" className="gel plain" aria-label={`Discard ${subjectOf(a)}`} onClick={() => void act(client.mail.discard(a.id))}>
                Discard
              </button>
            )}
          </div>
        </div>
      ))}
      {said && <p role="status">{said}</p>}
    </div>
  );
}
