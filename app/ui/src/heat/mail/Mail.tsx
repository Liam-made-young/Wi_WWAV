// Mail (docs/SPEC.md 3.10): the threads Claude has recorded from each of
// your mail accounts, sorted by how pressing it found them, and each read in a reader view (Reader.tsx). Learn reads no Gmail itself: the
// text is what Claude saved, plain, so there is no HTML and no remote image
// to block. A 320 px thread list and a
// pane for the one selected; the sidebar switches between accounts, then
// holds All and the three states with counts. Each thread has one action bar: Open in Gmail (the tab's secondary
// act) and Make a task (T). Mail has no reply, send or delete: Gmail already
// does those, and Wi_WWAV never touches your mailbox.

import { Fragment, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { clockAt, shortMonthDay } from '../../shared/time/format';
import { dayKey } from '../../shared/time/zone';
import type { Id, MailAccount, MailThread } from '../client';
import { openExternal } from '../external';
import { copy, plural } from '../fmt';
import { useFrame, useSelection, useSheets, useSidebarSlot, useSpaceFilter, useTabActs, useTabKeys } from '../frame';
import { useHeat } from '../store';
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

const senderOf = (from: string) => from.replace(/<.*>/, '').trim() || from;

type Priority = NonNullable<MailThread['priority']>;
/** The list's sections, most pressing first. "Everything else" is normal and low. */
const SECTIONS: { id: 'urgent' | 'high' | 'rest'; label: string }[] = [
  { id: 'urgent', label: 'Urgent' },
  { id: 'high', label: 'High' },
  { id: 'rest', label: 'Everything else' },
];
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
const sectionOf = (m: MailThread) => (m.priority === 'urgent' || m.priority === 'high' ? m.priority : 'rest');

export function Mail() {
  const { snap, idx, tz, date } = useHeat();
  const { spaceId } = useSpaceFilter();
  const { selectTask } = useSelection();
  const { setTab } = useFrame();
  const { newTask } = useSheets();
  const slot = useSidebarSlot();
  const [state, setState] = useState<Filter>('all');
  const [account, setAccount] = useState<string | null>(null);
  const [picked, setPicked] = useState<Id | null>(null);
  const [filter, setFilter] = useState('');
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
  const needle = filter.trim().toLowerCase();
  const matching = here.filter(
    (m) =>
      (state === 'all' || m.state === state) &&
      (!needle || `${m.subject} ${m.from} ${m.course ?? ''} ${m.category ?? ''}`.toLowerCase().includes(needle)),
  );
  // Once Claude has marked anything urgent or high, the list reads in sections, most pressing
  // first and newest first inside each. Until then it is one list, as before.
  const sorted = matching.some((m) => sectionOf(m) !== 'rest');
  const threads = sorted ? SECTIONS.flatMap((s) => matching.filter((m) => sectionOf(m) === s.id)) : matching;
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

  useTabActs({
    secondary: { run: openInGmail, hidden: !current },
    count: snap ? plural(threads.length, 'thread') : null,
  });
  useTabKeys({
    move(by) {
      if (threads.length === 0) return;
      const at = current ? threads.findIndex((m) => m.id === current.id) : -1;
      setPicked(threads[Math.min(threads.length - 1, Math.max(0, at + by))].id);
    },
    key(e) {
      // Mail has no delete: ⌫ does nothing here, whatever is selected in another tab.
      if (e.key === 'Backspace' || e.key === 'Delete') return true;
      if (!e.command && !e.alt && !e.shift && e.key.toLowerCase() === 't') {
        makeTask();
        return true;
      }
      return false;
    },
    escape() {
      if (document.activeElement !== filterField.current) return false;
      filterField.current?.blur();
      return true;
    },
    filter: () => filterField.current?.focus(),
  });

  if (!snap) return <div className="heat-mailtab" />;
  const stamp = (ms: number) => (dayKey(ms, tz) === date ? clockAt(ms, tz) : shortMonthDay(dayKey(ms, tz)));
  const count = (s: Filter) => here.filter((m) => s === 'all' || m.state === s).length;

  return (
    <div className="heat-mailtab">
      {slot &&
        createPortal(
          <>
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
        <input
          ref={filterField}
          type="search"
          className="heat-filter"
          aria-label="Filter the threads"
          placeholder="Filter"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </header>
      {here.length === 0 ? (
        <p className="heat-empty">
          {picking ? `Nothing recorded from ${picking} yet. Ask Claude to read it.` : copy.mailUi.empty}
        </p>
      ) : (
        <div className="heat-mail-body">
          <div role="listbox" aria-label="Threads" className="heat-threads">
            {threads.length === 0 && <p className="heat-empty">{copy.status.empty}</p>}
            {threads.map((m, n) => (
              <Fragment key={m.id}>
                {sorted && (n === 0 || sectionOf(threads[n - 1]) !== sectionOf(m)) && (
                  <p className="heat-mail-section" role="presentation" data-text="secondary">
                    {SECTIONS.find((s) => s.id === sectionOf(m))!.label}
                  </p>
                )}
              <div
                role="option"
                className="heat-thread"
                data-dense
                data-priority={sectionOf(m) === 'rest' ? undefined : m.priority}
                aria-selected={current?.id === m.id}
                tabIndex={-1}
                onClick={() => setPicked(m.id)}
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
                <button type="button" className="gel" onClick={openInGmail} title="Opens the thread in Gmail (⇧Return)">
                  {copy.mail.openInGmail}
                </button>
                <button type="button" className="gel" onClick={makeTask} title="Make a task (T)">
                  {copy.mail.makeTask}
                </button>
              </div>
              <Reader key={current.gmailThreadId} threadId={current.gmailThreadId} onOpenInGmail={openInGmail} />
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
