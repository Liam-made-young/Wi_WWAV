// Settings → Learn's mail accounts (docs/SPEC.md 3.10): the addresses whose
// mail Claude reads and records, and how each reaches the mailbox Claude's
// Gmail connector is signed in to. Wi_WWAV reads no mail and holds no
// password: an account here is only an address, and where it is forwarded.

import { type FormEvent, useCallback, useEffect, useState } from 'react';
import { call } from '../bridge';

interface Account {
  address: string;
  name: string;
  via: 'connector' | 'forward';
  forwardTo?: string;
}

const sentence = (e: unknown) => (e instanceof Error ? e.message : String(e));
const BLANK = { address: '', name: '', via: 'connector' as Account['via'], forwardTo: '' };

function today(): string {
  const d = new Date();
  const two = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}`;
}

/** `ask` is the core's `call`; a test passes its fake core's. */
export function MailAccounts({ ask = call }: { ask?: typeof call }) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [added, setAdded] = useState(BLANK);
  const [said, setSaid] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const read = useCallback(
    () =>
      ask<{ mailAccounts?: Account[] }>('heat.snapshot', { date: today() }).then(
        (snap) => setAccounts(snap.mailAccounts ?? []),
        (e) => setSaid(sentence(e)),
      ),
    [ask],
  );
  useEffect(() => void read(), [read]);

  // The whole list goes each time; what comes back is what was kept.
  const save = (list: Account[], done: string, after?: () => void) => {
    setBusy(true);
    ask<{ accounts: Account[] }>('heat.mail.accounts.set', { accounts: list })
      .then(
        (r) => {
          setAccounts(r.accounts);
          setSaid(done);
          after?.();
        },
        (e) => setSaid(sentence(e)),
      )
      .finally(() => setBusy(false));
  };

  const add = (e: FormEvent) => {
    e.preventDefault();
    const address = added.address.trim();
    const next: Account = {
      address,
      name: added.name.trim(),
      via: added.via,
      ...(added.via === 'forward' ? { forwardTo: added.forwardTo.trim() } : {}),
    };
    save([...accounts, next], `Added ${address.toLowerCase()}. Ask Claude to read your mail.`, () => setAdded(BLANK));
  };

  return (
    <div className="setting">
      <p className="setting-label" data-text="secondary">
        Mail accounts
      </p>
      <p className="why" data-text="secondary">
        Wi_WWAV never reads your mail and holds no password. Claude reads it with its own Gmail connector, then records
        each thread in Mail under the account it came from.
      </p>
      {accounts.length > 0 && (
        <ul className="calendars mail-accounts">
          {accounts.map((a) => (
            <li key={a.address}>
              <span className="calendar-name">{a.name}</span>
              <span data-text="secondary">
                {a.name === a.address ? '' : `${a.address} · `}
                {a.via === 'forward' ? `forwarded to ${a.forwardTo}` : 'the mailbox Claude’s Gmail connector reads'}
              </span>
              <button
                type="button"
                className="gel plain"
                disabled={busy}
                aria-label={`Remove ${a.address}`}
                onClick={() =>
                  save(
                    accounts.filter((x) => x.address !== a.address),
                    `Removed ${a.address}. What was recorded from it stays in Mail.`,
                  )
                }
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
      <form className="mail-add" onSubmit={add}>
        <div className="calendar-add">
          <input
            aria-label="Mail address"
            type="email"
            placeholder="name@school.edu"
            value={added.address}
            onChange={(e) => setAdded({ ...added, address: e.target.value })}
          />
          <input
            aria-label="Account name"
            placeholder="Name, such as School"
            value={added.name}
            onChange={(e) => setAdded({ ...added, name: e.target.value })}
          />
        </div>
        <div className="calendar-add">
          <select
            aria-label="How its mail arrives"
            value={added.via}
            onChange={(e) => setAdded({ ...added, via: e.target.value as Account['via'] })}
          >
            <option value="connector">Claude’s Gmail connector is signed in to it</option>
            <option value="forward">It is forwarded into that mailbox</option>
          </select>
          {added.via === 'forward' && (
            <input
              aria-label="Forwarded to"
              type="email"
              placeholder="you+school@gmail.com"
              value={added.forwardTo}
              onChange={(e) => setAdded({ ...added, forwardTo: e.target.value })}
            />
          )}
          <button
            type="submit"
            className="gel"
            disabled={busy || !added.address.trim() || (added.via === 'forward' && !added.forwardTo.trim())}
          >
            Add
          </button>
        </div>
      </form>
      {added.via === 'forward' && (
        <p className="why" data-text="secondary">
          Forward it to your Gmail address with a plus tag, such as you+school@gmail.com. Gmail delivers that to your
          own inbox, and the tag is how Claude tells this account’s mail from the rest.
        </p>
      )}
      {said && (
        <p role="status" className="calendar-said">
          {said}
        </p>
      )}
    </div>
  );
}
