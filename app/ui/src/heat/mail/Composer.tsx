// Writing a mail (docs/SPEC.md 3.10): a new one, or a reply to the thread in
// the pane. Send puts it in the outbox as written and asks for it to go at
// once. Wi_WWAV holds no password: Claude sends it, word for word, with the
// person's own Gmail connector.

import { type FormEvent, useEffect, useRef, useState } from 'react';
import { sentenceOf } from '../fmt';
import { useHeat } from '../store';

interface Props {
  /** The thread being answered; none for a new mail. */
  reply?: { threadId: string; subject: string; to: string };
  onDone(said: string): void;
  onCancel(): void;
}

export function Composer({ reply, onDone, onCancel }: Props) {
  const { client } = useHeat();
  const [to, setTo] = useState(reply?.to ?? '');
  const [cc, setCc] = useState('');
  const [subject, setSubject] = useState('');
  const [body, setBody] = useState('');
  const [said, setSaid] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const first = useRef<HTMLTextAreaElement & HTMLInputElement>(null);
  useEffect(() => first.current?.focus(), []);

  const send = (e?: FormEvent) => {
    e?.preventDefault();
    if (busy) return;
    setBusy(true);
    const going = reply
      ? client.mail.reply(reply.threadId, body, to.trim() || undefined, cc.trim() || undefined)
      : client.mail.send({ to, subject, body, ...(cc.trim() ? { cc } : {}) });
    going.then(
      () => onDone(reply ? 'Reply is in the outbox, on its way.' : 'Mail is in the outbox, on its way.'),
      (err) => {
        setSaid(sentenceOf(err));
        setBusy(false);
      },
    );
  };

  return (
    <form
      className="heat-compose"
      aria-label={reply ? `Reply to ${reply.subject}` : 'New mail'}
      onSubmit={send}
      onKeyDown={(e) => {
        // ⌘Return sends; Esc puts the mail away only when nothing is written.
        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) send();
        if (e.key === 'Escape' && !body.trim()) onCancel();
        e.stopPropagation();
      }}
    >
      <label className="heat-compose-row">
        <span data-text="secondary">To</span>
        <input
          ref={reply ? undefined : first}
          aria-label="To"
          value={to}
          placeholder={reply ? 'The sender, unless you say otherwise' : 'name@example.com'}
          onChange={(e) => setTo(e.target.value)}
        />
      </label>
      <label className="heat-compose-row">
        <span data-text="secondary">Cc</span>
        <input aria-label="Cc" value={cc} onChange={(e) => setCc(e.target.value)} />
      </label>
      {!reply && (
        <label className="heat-compose-row">
          <span data-text="secondary">Subject</span>
          <input aria-label="Subject" value={subject} onChange={(e) => setSubject(e.target.value)} />
        </label>
      )}
      <textarea
        ref={reply ? first : undefined}
        className="heat-compose-body"
        aria-label="The mail"
        rows={reply ? 8 : 14}
        value={body}
        onChange={(e) => setBody(e.target.value)}
      />
      {said && <p role="alert">{said}</p>}
      <div className="heat-pane-acts">
        <button type="submit" className="gel" disabled={busy || !body.trim()} title="Send (⌘Return)">
          Send
        </button>
        <button type="button" className="gel plain" onClick={onCancel}>
          Cancel
        </button>
        <span className="why" data-text="secondary">
          It goes exactly as written, from the mailbox Claude’s Gmail connector is signed in to.
        </span>
      </div>
    </form>
  );
}
