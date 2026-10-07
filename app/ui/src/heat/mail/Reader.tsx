// Mail's reader (docs/SPEC.md 3.10): a thread's messages as plain text, the
// way a browser's reader view shows a page. The text is what Claude saved
// with `save_mail_text`, kept on this Mac. Nothing here is HTML, so there is
// no remote image to load and no script to run; an address in the text is a
// link that opens in the browser.

import { Fragment, useEffect, useState } from 'react';
import { clockAt, shortMonthDay } from '../../shared/time/format';
import { dayKey } from '../../shared/time/zone';
import type { MailMessage } from '../client';
import { openExternal } from '../external';
import { useHeat } from '../store';

const senderOf = (from: string) => from.replace(/<.*>/, '').trim() || from;

// An address ends before the punctuation that closes the sentence it sits in.
const ADDRESS = /https?:\/\/[^\s<>"')\]]+[^\s<>"')\].,;:!?]/g;

/** A line of mail with its addresses made links. */
function Line({ text }: { text: string }) {
  const parts: (string | { url: string })[] = [];
  let at = 0;
  for (const m of text.matchAll(ADDRESS)) {
    if (m.index > at) parts.push(text.slice(at, m.index));
    parts.push({ url: m[0] });
    at = m.index + m[0].length;
  }
  if (at < text.length) parts.push(text.slice(at));
  return (
    <>
      {parts.map((p, n) =>
        typeof p === 'string' ? (
          <Fragment key={n}>{p}</Fragment>
        ) : (
          <a
            key={n}
            className="heat-mail-address"
            href={p.url}
            onClick={(e) => {
              e.preventDefault();
              openExternal(p.url);
            }}
          >
            {p.url}
          </a>
        ),
      )}
    </>
  );
}

/** Paragraphs are split on blank lines; a paragraph of quoted lines ("> …") reads as a quote. */
function Body({ text }: { text: string }) {
  return (
    <>
      {text.split(/\n{2,}/).map((para, n) => {
        const lines = para.split('\n');
        const quoted = lines.every((l) => l.trimStart().startsWith('>'));
        const shown = quoted ? lines.map((l) => l.replace(/^\s*>+\s?/, '')) : lines;
        const Tag = quoted ? 'blockquote' : 'p';
        return (
          <Tag key={n} className={quoted ? 'heat-mail-quote' : undefined}>
            {shown.map((l, i) => (
              <Fragment key={i}>
                {i > 0 && <br />}
                <Line text={l} />
              </Fragment>
            ))}
          </Tag>
        );
      })}
    </>
  );
}

type Saved = { for: string; messages: MailMessage[] | null } | null;

export function Reader({ threadId, onOpenInGmail }: { threadId: string; onOpenInGmail(): void }) {
  const { client, snap, tz, date } = useHeat();
  const [saved, setSaved] = useState<Saved>(null);

  // Read again when the snapshot moves: Claude may have just saved this thread's text.
  useEffect(() => {
    let live = true;
    client.mail.text(threadId).then(
      (r) => live && setSaved({ for: threadId, messages: r.messages }),
      () => live && setSaved({ for: threadId, messages: null }),
    );
    return () => {
      live = false;
    };
  }, [client, threadId, snap]);

  if (!saved || saved.for !== threadId) return <div className="heat-mail-reader" aria-busy="true" />;
  if (!saved.messages) {
    return (
      <div className="heat-mail-reader">
        <p className="why" data-text="secondary">
          Claude hasn’t saved this thread’s text yet. Ask Claude to read your mail, or{' '}
          <button type="button" className="heat-link" data-dense onClick={onOpenInGmail}>
            open it in Gmail
          </button>
          .
        </p>
      </div>
    );
  }
  const stamp = (ms: number) =>
    dayKey(ms, tz) === date ? clockAt(ms, tz) : `${shortMonthDay(dayKey(ms, tz))}, ${clockAt(ms, tz)}`;
  return (
    <div className="heat-mail-reader">
      {saved.messages.map((m, n) => (
        <article key={m.id ?? n} className="heat-mail-message" aria-label={`From ${senderOf(m.from)}`}>
          <header className="heat-mail-message-head">
            <span className="heat-mail-message-from">{senderOf(m.from)}</span>
            <span data-text="secondary">{stamp(m.sentAt)}</span>
          </header>
          {m.to && (
            <p className="heat-mail-message-to" data-text="secondary">
              To {m.to}
            </p>
          )}
          <div className="heat-mail-text">
            <Body text={m.text} />
          </div>
        </article>
      ))}
    </div>
  );
}
