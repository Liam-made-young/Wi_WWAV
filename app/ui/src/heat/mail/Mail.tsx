// Mail (docs/SPEC.md 3.10): your mail, from each of your accounts, as Claude
// recorded and sorted it, read in a reader view (Reader.tsx) and answered
// from here (Composer.tsx). Learn holds no password and reads no Gmail
// itself: Claude reads and sends with the person's own Gmail connector, in
// two jobs that share no tool (crates/wi-core/src/mail_cmd.rs).
//
// A 320 px thread list and a pane for the one selected. The sidebar holds
// the mailboxes (Inbox, Unread, Archived, Outbox), the accounts, then All
// and the three states with counts. The list sorts by priority, date,
// unread or tag. A thread's acts are Reply (R), Archive (E), Mark unread
// (U), Open in Gmail and Make a task (T). Mail has no delete.

import { Fragment, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { clockAt, shortMonthDay } from '../../shared/time/format';
import { dayKey } from '../../shared/time/zone';
import type { Id, MailAccount, MailPlace, MailThread } from '../client';
import { openExternal } from '../external';
import { copy, plural, sentenceOf } from '../fmt';
import { useFrame, useSelection, useSheets, useSidebarSlot, useSpaceFilter, useTabActs, useTabKeys } from '../frame';
import { useHeat } from '../store';
import { Composer } from './Composer';
import { Outbox } from './Outbox';
import { Reader } from './Reader';
import './mail.css';

type Filter = 'all' | MailThread['state'];

const STATES: { id: Filter; label: string }[] = [
  { id: 'all', label: copy.mailUi.all },
  { id: 'grade', label: copy.mail.gradePosted },
  { id: 'task', label: copy.mail.taskMade },
  { id: 'nothing', label: copy.mail.nothingToDo },
];
const WORD = { grade: copy.mail.gradePosted, task: copy.mail.taskMade, nothing: copy.mail.nothingToDo } as const;

type Box = 'inbox' | 'unread' | 'archived' | 'outbox';
const BOXES: { id: Box; label: string }[] = [
  { id: 'inbox', label: 'Inbox' },
  { id: 'unread', label: 'Unread' },
  { id: 'archived', label: 'Archived' },
  { id: 'outbox', label: 'Outbox' },
];

type Sort = 'priority' | 'date' | 'unread' | 'tag';
const SORTS: { id: Sort; label: string }[] = [
  { id: 'priority', label: 'Priority' },
  { id: 'date', label: 'Date' },
  { id: 'unread', label: 'Unread first' },
  { id: 'tag', label: 'Tag' },
];

const senderOf = (from: string) => from.replace(/<.*>/, '').trim() || from;

type Priority = NonNullable<MailThread['priority']>;
const PRIORITY: Record<Priority, string> = { urgent: 'Urgent', high: 'High', normal: 'Normal', low: 'Low' };
const CATEGORY: Record<NonNullable<MailThread['category']>, string> = {
  school: 'School',
  work: 'Work',
  money: 'Money',
  people: 'People',
  updates: 'Updates',
  promotions: 'Promotions',
  other: 'Other',
};
const NO_PLACE: MailPlace = { unread: false, archived: false };

/** The list in sections for a sort: each section's name (none for a list that has only one) and its threads, newest first. */
function sections(list: MailThread[], sort: Sort, placeOf: (m: MailThread) => MailPlace) {
  const named = (groups: [string, (m: MailThread) => boolean][]) =>
    groups.map(([label, has]) => ({ label: label as string | null, items: list.filter(has) })).filter((g) => g.items.length > 0);
  const pressing = (m: MailThread) => m.priority === 'urgent' || m.priority === 'high';
  if (sort === 'priority' && list.some(pressing)) {
    // "Everything else" is normal and low.
    return named([
      ['Urgent', (m) => m.priority === 'urgent'],
      ['High', (m) => m.priority === 'high'],
      ['Everything else', (m) => !pressing(m)],
    ]);
  }
  if (sort === 'unread' && list.some((m) => placeOf(m).unread)) {
    return named([
      ['Unread', (m) => placeOf(m).unread],
      ['Read', (m) => !placeOf(m).unread],
    ]);
  }
  if (sort === 'tag' && list.some((m) => m.category)) {
    return named([
      ...Object.entries(CATEGORY).map(([id, label]): [string, (m: MailThread) => boolean] => [label, (m) => m.category === id]),
      ['No tag', (m) => !m.category],
    ]);
  }
  return [{ label: null as string | null, items: list }];
}

export function Mail() {
  const { snap, idx, tz, date, client } = useHeat();
  const { spaceId } = useSpaceFilter();
  const { selectTask } = useSelection();
  const { setTab } = useFrame();
  const { newTask } = useSheets();
  const slot = useSidebarSlot();
  const [state, setState] = useState<Filter>('all');
  const [account, setAccount] = useState<string | null>(null);
  const [box, setBox] = useState<Box>('inbox');
  const [sort, setSort] = useState<Sort>('priority');
  const [picked, setPicked] = useState<Id | null>(null);
  const [filter, setFilter] = useState('');
  const [found, setFound] = useState<{ for: string; ids: Set<string> } | null>(null);
  const [writing, setWriting] = useState<'new' | 'reply' | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  const filterField = useRef<HTMLInputElement>(null);

  // A thread belongs to its task's space; one with no task is school mail, which is Classes' kind of
  // work, unless Claude sorted it into another category.
  const inSpace = (m: MailThread) => {
    if (!spaceId) return true;
    const task = m.taskId ? idx.task.get(m.taskId) : undefined;
    if (task) return task.spaceId === spaceId;
    return idx.space.get(spaceId)?.groupKind === 'course' && (!m.category || m.category === 'school');
  };
  // Newest first, as the Mail widget has it.
  const inSpaceNow = useMemo(
    () => [...(snap?.records.mailThread ?? [])].filter(inSpace).sort((a, b) => b.receivedAt - a.receivedAt),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [snap, spaceId, idx],
  );
  // The switcher: the accounts from Settings → Learn, then any address a thread carries that
  // has since left the list, so no thread is ever out of reach.
  const accounts = useMemo(() => {
    const listed: MailAccount[] = snap?.mailAccounts ?? [];
    const stray = [...new Set(inSpaceNow.map((m) => m.account).filter((a): a is string => !!a))]
      .filter((a) => !listed.some((l) => l.address === a))
      .map((a): MailAccount => ({ address: a, name: a, via: 'connector' }));
    return [...listed, ...stray];
  }, [snap, inSpaceNow]);
  const picking = account !== null && accounts.some((a) => a.address === account) ? account : null;
  const here = picking ? inSpaceNow.filter((m) => m.account === picking) : inSpaceNow;

  const placeOf = (m: MailThread) => snap?.mailState?.[m.gmailThreadId] ?? NO_PLACE;
  const inBox = (m: MailThread, b: Box) => {
    const p = placeOf(m);
    return b === 'archived' ? p.archived : b === 'unread' ? p.unread && !p.archived : !p.archived;
  };
  const boxed = box === 'outbox' ? [] : here.filter((m) => inBox(m, box));

  // The filter reads what a row shows at once; the core then adds the threads whose saved text
  // holds the words, so a search reaches inside the mail.
  const needle = filter.trim().toLowerCase();
  useEffect(() => {
    if (needle.length < 2) return setFound(null);
    let live = true;
    client.mail.search(needle).then(
      (r) => live && setFound({ for: needle, ids: new Set(r.threadIds) }),
      () => {},
    );
    return () => {
      live = false;
    };
  }, [client, needle, snap]);
  const matching = boxed.filter(
    (m) =>
      (state === 'all' || m.state === state) &&
      (!needle ||
        `${m.subject} ${m.from} ${m.course ?? ''} ${m.category ?? ''}`.toLowerCase().includes(needle) ||
        (found?.for === needle && found.ids.has(m.gmailThreadId))),
  );
  const groups = sections(matching, sort, placeOf);
  const threads = groups.flatMap((g) => g.items);
  const heading = new Map(groups.filter((g) => g.label).map((g) => [g.items[0].id, g.label!]));
  const current = threads.find((m) => m.id === picked) ?? threads[0] ?? null;

  // Every thread lives in the one mailbox Claude's connector reads, forwarded or not, so that is
  // the mailbox Gmail is asked to open. With no accounts set up, it is the browser's first one.
  const mailbox = snap?.mailAccounts?.find((a) => a.via === 'connector')?.address;
  const gmailUrl = (threadId: string) =>
    mailbox
      ? `https://mail.google.com/mail/?authuser=${encodeURIComponent(mailbox)}#all/${threadId}`
      : copy.mailUi.gmailUrl(threadId);
  const openInGmail = () => current && openExternal(gmailUrl(current.gmailThreadId));
  const makeTask = () =>
    current &&
    newTask({
      title: current.subject,
      notes: `${copy.mail.fromMail} ${gmailUrl(current.gmailThreadId)}`,
      ...(spaceId ? { spaceId } : {}),
    });

  const doing = (going: Promise<unknown>) => void going.catch((e) => setSaid(sentenceOf(e)));
  // Choosing a thread reads it: here at once, and in Gmail within the minute.
  const pick = (m: MailThread) => {
    setPicked(m.id);
    setWriting(null);
    if (placeOf(m).unread) doing(client.mail.mark(m.gmailThreadId, false));
  };
  const archive = () => current && doing(client.mail.archive(current.gmailThreadId, !placeOf(current).archived));
  const toggleUnread = () => current && doing(client.mail.mark(current.gmailThreadId, !placeOf(current).unread));
  const reply = () => current && setWriting('reply');
  const compose = () => setWriting('new');

  useTabActs({
    plus: { run: compose },
    secondary: { run: openInGmail, hidden: !current },
    count: snap ? plural(threads.length, 'thread') : null,
  });
  useTabKeys({
    move(by) {
      if (threads.length === 0) return;
      const at = current ? threads.findIndex((m) => m.id === current.id) : -1;
      pick(threads[Math.min(threads.length - 1, Math.max(0, at + by))]);
    },
    key(e) {
      // Mail has no delete: ⌫ does nothing here, whatever is selected in another tab.
      if (e.key === 'Backspace' || e.key === 'Delete') return true;
      if (e.command || e.alt || e.shift) return false;
      const act = { t: makeTask, r: reply, e: archive, u: toggleUnread }[e.key.toLowerCase()];
      if (!act) return false;
      act();
      return true;
    },
    escape() {
      if (writing) {
        setWriting(null);
        return true;
      }
      if (document.activeElement !== filterField.current) return false;
      filterField.current?.blur();
      return true;
    },
    filter: () => filterField.current?.focus(),
  });

  if (!snap) return <div className="heat-mailtab" />;
  const stamp = (ms: number) => (dayKey(ms, tz) === date ? clockAt(ms, tz) : shortMonthDay(dayKey(ms, tz)));
  const count = (s: Filter) => boxed.filter((m) => s === 'all' || m.state === s).length;
  const sync = snap.mailSync;
  const waiting = (sync?.queued ?? 0) + (sync?.failed ?? 0);
  const boxCount = (b: Box) => (b === 'outbox' ? waiting : here.filter((m) => inBox(m, b)).length);
  const syncLine =
    sync?.doing === 'reading'
      ? 'Reading your mail…'
      : sync?.doing === 'sending'
        ? 'Sending…'
        : (sync?.line ?? 'Mail hasn’t been read from here yet.');

  return (
    <div className="heat-mailtab">
      {slot &&
        createPortal(
          <>
            <h2 className="heat-side-heading" data-text="secondary">
              Mailboxes
            </h2>
            {BOXES.map((b) => (
              <button
                key={b.id}
                type="button"
                className="heat-side-row heat-mail-box"
                data-dense
                aria-current={box === b.id ? 'true' : undefined}
                onClick={() => (setBox(b.id), setWriting(null))}
              >
                <span className="heat-side-name">{b.label}</span>
                <span className="heat-side-count" data-text="secondary">
                  {boxCount(b.id)}
                </span>
              </button>
            ))}
            {accounts.length > 0 && (
              <>
                <h2 className="heat-side-heading" data-text="secondary">
                  Accounts
                </h2>
                {[{ address: null as string | null, name: 'All accounts' }, ...accounts].map((a) => (
                  <button
                    key={a.address ?? 'all'}
                    type="button"
                    className="heat-side-row heat-mail-account"
                    data-dense
                    title={a.address ?? undefined}
                    aria-current={picking === a.address ? 'true' : undefined}
                    onClick={() => setAccount(a.address)}
                  >
                    <span className="heat-side-name">{a.name}</span>
                    <span className="heat-side-count" data-text="secondary">
                      {a.address ? inSpaceNow.filter((m) => m.account === a.address).length : inSpaceNow.length}
                    </span>
                  </button>
                ))}
              </>
            )}
            <h2 className="heat-side-heading" data-text="secondary">
              {copy.widgets.mail}
            </h2>
            {STATES.map((s) => (
              <button
                key={s.id}
                type="button"
                className="heat-side-row"
                data-dense
                aria-current={state === s.id ? 'true' : undefined}
                onClick={() => setState(s.id)}
              >
                <span className="heat-side-name">{s.label}</span>
                <span className="heat-side-count" data-text="secondary" aria-label={`${count(s.id)} threads`}>
                  {count(s.id)}
                </span>
              </button>
            ))}
          </>,
          slot,
        )}
      <header className="heat-mail-head">
        <h1 className="heat-heading">Mail</h1>
        <div className="heat-mail-tools">
          <button type="button" className="gel" onClick={compose} title="Write a mail">
            Compose
          </button>
          <label className="heat-mail-sort" data-text="secondary">
            Sort
            <select aria-label="Sort the threads" value={sort} onChange={(e) => setSort(e.target.value as Sort)}>
              {SORTS.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
          <input
            ref={filterField}
            type="search"
            className="heat-filter"
            aria-label="Filter the threads"
            placeholder="Search"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
      </header>
      <div className="heat-mail-sync">
        <span role="status" data-text="secondary">
          {said ?? syncLine}
        </span>
        <label className="check" data-dense title="While Wi_WWAV is open, Claude reads new mail every half hour.">
          <input
            type="checkbox"
            checked={sync?.background ?? true}
            onChange={(e) => doing(client.mail.setBackground(e.target.checked))}
          />
          Read on its own
        </label>
        <button type="button" className="gel plain" disabled={!!sync?.doing} onClick={() => (setSaid(null), doing(client.mail.sync()))}>
          Read mail now
        </button>
      </div>
      {box === 'outbox' ? (
        <Outbox threads={inSpaceNow} />
      ) : writing === 'new' ? (
        <div className="heat-mail-writing">
          <Composer onDone={(line) => (setWriting(null), setSaid(line))} onCancel={() => setWriting(null)} />
        </div>
      ) : here.length === 0 ? (
        <p className="heat-empty">
          {picking ? `Nothing recorded from ${picking} yet. Ask Claude to read it.` : copy.mailUi.empty}
        </p>
      ) : (
        <div className="heat-mail-body">
          <div role="listbox" aria-label="Threads" className="heat-threads">
            {threads.length === 0 && <p className="heat-empty">{copy.status.empty}</p>}
            {threads.map((m) => (
              <Fragment key={m.id}>
                {heading.has(m.id) && (
                  <p className="heat-mail-section" role="presentation" data-text="secondary">
                    {heading.get(m.id)}
                  </p>
                )}
                <div
                  role="option"
                  className="heat-thread"
                  data-dense
                  data-priority={m.priority === 'urgent' || m.priority === 'high' ? m.priority : undefined}
                  data-unread={placeOf(m).unread ? '' : undefined}
                  aria-selected={current?.id === m.id}
                  tabIndex={-1}
                  onClick={() => pick(m)}
                >
                  <span className="heat-thread-from">{senderOf(m.from)}</span>
                  <span className="heat-thread-time">{stamp(m.receivedAt)}</span>
                  <span className="heat-thread-subject">{m.subject}</span>
                  <span className="heat-thread-chips">
                    {m.course && <span className="heat-tag">{m.course}</span>}
                    {m.category && m.category !== 'school' && <span className="heat-tag">{CATEGORY[m.category]}</span>}
                    <span className="heat-tag">{WORD[m.state]}</span>
                  </span>
                </div>
              </Fragment>
            ))}
          </div>
          {current && (
            <section className="heat-pane" aria-label={current.subject}>
              <h2 className="heat-pane-subject">{current.subject}</h2>
              <p className="heat-pane-from" data-text="secondary">
                {senderOf(current.from)} · {stamp(current.receivedAt)}
              </p>
              <dl className="heat-pane-facts">
                {current.course && (
                  <div>
                    <dt data-text="secondary">Course</dt>
                    <dd>{current.course}</dd>
                  </div>
                )}
                <div>
                  <dt data-text="secondary">State</dt>
                  <dd>{WORD[current.state]}</dd>
                </div>
                {current.priority && (
                  <div>
                    <dt data-text="secondary">Priority</dt>
                    <dd>{PRIORITY[current.priority]}</dd>
                  </div>
                )}
                {current.category && (
                  <div>
                    <dt data-text="secondary">About</dt>
                    <dd>{CATEGORY[current.category]}</dd>
                  </div>
                )}
                {current.account && (
                  <div>
                    <dt data-text="secondary">Account</dt>
                    <dd>{current.account}</dd>
                  </div>
                )}
              </dl>
              <p className="heat-pane-reason-label" data-text="secondary">
                {copy.mailUi.claudeReason}
              </p>
              <p className="heat-mail-reason">{current.reason}</p>
              <Made
                thread={current}
                onTask={(id) => (setTab('tasks'), selectTask(id))}
                onGrades={() => setTab('grades')}
              />
              <div className="heat-pane-acts">
                <button type="button" className="gel" onClick={reply} title="Reply (R)">
                  Reply
                </button>
                <button type="button" className="gel" onClick={archive} title="Archive (E)">
                  {placeOf(current).archived ? 'Move to the inbox' : 'Archive'}
                </button>
                <button type="button" className="gel" onClick={toggleUnread} title="Mark unread (U)">
                  {placeOf(current).unread ? 'Mark read' : 'Mark unread'}
                </button>
                <button type="button" className="gel" onClick={openInGmail} title="Opens the thread in Gmail (⇧Return)">
                  {copy.mail.openInGmail}
                </button>
                <button type="button" className="gel" onClick={makeTask} title="Make a task (T)">
                  {copy.mail.makeTask}
                </button>
              </div>
              {writing === 'reply' && (
                <Composer
                  key={`reply-${current.gmailThreadId}`}
                  reply={{ threadId: current.gmailThreadId, subject: current.subject, to: '' }}
                  onDone={(line) => (setWriting(null), setSaid(line))}
                  onCancel={() => setWriting(null)}
                />
              )}
              <Reader key={`read-${current.gmailThreadId}`} threadId={current.gmailThreadId} onOpenInGmail={openInGmail} />
            </section>
          )}
        </div>
      )}
    </div>
  );
}

/** A link to what Claude made from the thread: the task, or the pending grade. */
function Made({ thread, onTask, onGrades }: { thread: MailThread; onTask(id: Id): void; onGrades(): void }) {
  const { snap, idx } = useHeat();
  if (thread.state === 'task') {
    const task = thread.taskId ? idx.task.get(thread.taskId) : undefined;
    return task ? (
      <p className="heat-pane-made">
        <span data-text="secondary">{copy.mailUi.made}: </span>
        <button type="button" className="heat-link" data-dense onClick={() => onTask(task.id)}>
          Task: {task.title}
        </button>
      </p>
    ) : null;
  }
  if (thread.state === 'grade') {
    const grade = snap?.records.grade.find((g) => g.mailThreadId === thread.id);
    return (
      <p className="heat-pane-made">
        <span data-text="secondary">{copy.mailUi.made}: </span>
        <button type="button" className="heat-link" data-dense onClick={onGrades}>
          {grade ? `Grade: ${grade.title}` : copy.mailUi.openGrades}
        </button>
      </p>
    );
  }
  return null;
}
