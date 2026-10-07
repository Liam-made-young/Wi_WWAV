// The fake core's School sheet and calendars (docs/HEAT.md, docs/SPEC.md
// 3.11): the school's name, Brightspace host, course-code pattern and term
// dates, and the calendars that come in as private iCal addresses. An address
// goes to the Keychain and never comes back: a calendar keeps a reference,
// and the school only says whether a link is saved.

import type { Calendar, Id, MailAccount, MailAction, MailMessage, MailPlace, School } from '../client';
import { derive, type Fake, refuse, register } from './core';

const schools = new WeakMap<Fake, School>();

const mailAccounts = new WeakMap<Fake, MailAccount[]>();

/** The text Claude saved for each thread, by Gmail's thread id. A test puts it here. */
export const mailTexts = new WeakMap<Fake, Map<string, MailMessage[]>>();

/** The fake's mailbox: each thread's place, the outbox, and whether mail is read on its own. */
interface Mailbox {
  places: Map<string, MailPlace>;
  actions: MailAction[];
  background: boolean;
  line: string | null;
}
const mailboxes = new WeakMap<Fake, Mailbox>();
export const mailboxOf = (fake: Fake): Mailbox =>
  mailboxes.get(fake) ??
  mailboxes.set(fake, { places: new Map(), actions: [], background: true, line: null }).get(fake)!;

derive((snap, fake) => {
  const box = mailboxOf(fake);
  snap.school = schools.get(fake) ?? null;
  snap.mailAccounts = mailAccounts.get(fake) ?? [];
  snap.mailState = Object.fromEntries(box.places);
  snap.mailSync = {
    line: box.line,
    at: null,
    doing: null,
    background: box.background,
    queued: box.actions.filter((a) => a.status === 'queued').length,
    failed: box.actions.filter((a) => a.status === 'failed').length,
  };
});

const threadOf = (fake: Fake, threadId: unknown) =>
  [...fake.store.mailThread.values()].find((m) => m.gmailThreadId === threadId) ?? refuse('That thread isn’t in Mail.');

function queue(fake: Fake, action: Omit<MailAction, 'id' | 'status' | 'createdAt'>): { action: MailAction } {
  const box = mailboxOf(fake);
  const made: MailAction = { ...action, id: fake.newId(), status: 'queued', createdAt: fake.now };
  box.actions.push(made);
  fake.emit([]);
  return { action: made };
}

register('heat.mail.send', (args, fake) => {
  const [to, subject, body] = [String(args.to ?? '').trim(), String(args.subject ?? '').trim(), String(args.body ?? '')];
  if (!body.trim()) refuse('Write the mail first.');
  if (!to) refuse('Say who the mail is to.');
  if (!to.includes('@')) refuse(`"${to}" isn’t a mail address (To).`);
  if (!subject) refuse('Give the mail a subject.');
  return queue(fake, { kind: 'send', to, subject, body: body.trimEnd(), ...(args.cc ? { cc: String(args.cc) } : {}) });
});

register('heat.mail.reply', (args, fake) => {
  const thread = threadOf(fake, args.threadId);
  const body = String(args.body ?? '');
  if (!body.trim()) refuse('Write the mail first.');
  return queue(fake, { kind: 'reply', threadId: thread.gmailThreadId, body: body.trimEnd(), ...(args.to ? { to: String(args.to) } : {}) });
});

function place(fake: Fake, threadId: string, change: Partial<MailPlace>) {
  const box = mailboxOf(fake);
  box.places.set(threadId, { unread: false, archived: false, ...box.places.get(threadId), ...change });
}

register('heat.mail.archive', (args, fake) => {
  const thread = threadOf(fake, args.threadId);
  const archived = args.archived !== false;
  place(fake, thread.gmailThreadId, { archived });
  return queue(fake, { kind: archived ? 'archive' : 'unarchive', threadId: thread.gmailThreadId });
});

register('heat.mail.mark', (args, fake) => {
  const thread = threadOf(fake, args.threadId);
  const unread = args.unread === true;
  place(fake, thread.gmailThreadId, { unread });
  return queue(fake, { kind: unread ? 'markUnread' : 'markRead', threadId: thread.gmailThreadId });
});

register('heat.mail.search', (args, fake) => {
  const words = String(args.q ?? '').toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return { threadIds: [] };
  const texts = mailTexts.get(fake);
  return {
    threadIds: [...fake.store.mailThread.values()]
      .filter((m) => {
        const hay = [m.subject, m.from, m.reason, ...(texts?.get(m.gmailThreadId) ?? []).map((t) => t.text)].join('\n').toLowerCase();
        return words.every((w) => hay.includes(w));
      })
      .map((m) => m.gmailThreadId),
  };
});

register('heat.mail.outbox', (_args, fake) => ({ actions: mailboxOf(fake).actions }));

register('heat.mail.outbox.retry', (args, fake) => {
  const action = mailboxOf(fake).actions.find((a) => a.id === args.id) ?? refuse('No action in the outbox has that id.');
  action.status = 'queued';
  delete action.error;
  fake.emit([]);
  return {};
});

register('heat.mail.outbox.discard', (args, fake) => {
  const box = mailboxOf(fake);
  if (!box.actions.some((a) => a.id === args.id)) refuse('No action in the outbox has that id.');
  box.actions = box.actions.filter((a) => a.id !== args.id);
  fake.emit([]);
  return {};
});

register('heat.mail.sync', (_args, fake) => {
  mailboxOf(fake).line = 'Mail read 10:00 AM: nothing new';
  fake.emit([]);
  return { started: true };
});

register('heat.mail.background.set', (args, fake) => {
  mailboxOf(fake).background = args.on !== false;
  fake.emit([]);
  return {};
});

register('heat.mail.accounts.set', (args, fake) => {
  const list = (Array.isArray(args.accounts) ? args.accounts : []) as Partial<MailAccount>[];
  const kept: MailAccount[] = [];
  for (const a of list) {
    const address = String(a.address ?? '').trim().toLowerCase();
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(address)) refuse('A mail account needs its address, such as name@school.edu.');
    if (kept.some((k) => k.address === address)) refuse(`${address} is in the list twice.`);
    const name = String(a.name ?? '').trim() || address;
    if (a.via === 'forward') {
      const forwardTo = String(a.forwardTo ?? '').trim().toLowerCase();
      if (!forwardTo.includes('@')) refuse(`Say where ${address} is forwarded to, such as you+school@gmail.com.`);
      kept.push({ address, name, via: 'forward', forwardTo });
    } else kept.push({ address, name, via: 'connector' });
  }
  mailAccounts.set(fake, kept);
  fake.emit([]);
  return { accounts: kept };
});

register('heat.school.set', (args, fake) => {
  const name = String(args.name ?? '').trim();
  const pattern = String(args.codePattern ?? '');
  if (!name) refuse('Give the school a name first.');
  try {
    new RegExp(pattern.replace(/^\/|\/[a-z]*$/g, ''));
  } catch {
    refuse('That course-code pattern isn’t a pattern Learn can read.');
  }
  const was = schools.get(fake);
  const url = typeof args.icalUrl === 'string' ? args.icalUrl.trim() : '';
  schools.set(fake, {
    name,
    host: String(args.host ?? '').trim(),
    icalSaved: url !== '' || (was?.icalSaved ?? false),
    codePattern: pattern,
    termStart: String(args.termStart ?? ''),
    termEnd: String(args.termEnd ?? ''),
  });
  if (url !== '' && ![...fake.store.calendar.values()].some((c) => c.kind === 'brightspace')) {
    const calendar: Calendar = {
      id: fake.newId(),
      name: 'Brightspace',
      kind: 'brightspace',
      keychainRef: 'keychain:brightspace',
      lastSyncedAt: null,
    };
    fake.write('add calendar', ['calendar'], () => fake.store.calendar.set(calendar.id, calendar));
  }
  fake.emit([]);
  return {};
});

register('heat.calendars.add', (args, fake) => {
  const name = String(args.name ?? '').trim();
  if (!name) refuse('Give the calendar a name first.');
  if (!/^(https?|webcal):\/\//i.test(String(args.url ?? '')))
    refuse('An iCal address starts with https:// or webcal://.');
  const id = fake.newId();
  const calendar: Calendar = {
    id,
    name,
    kind: args.kind === 'brightspace' ? 'brightspace' : 'ical',
    keychainRef: `keychain:${id}`,
    lastSyncedAt: null,
  };
  fake.write('add calendar', ['calendar'], () => fake.store.calendar.set(id, calendar));
  return { calendar };
});

register('heat.calendars.remove', (args, fake) => {
  const id = args.id as Id;
  if (!fake.store.calendar.has(id)) refuse('No calendar has that id.');
  fake.write('delete calendar', ['calendar'], () => fake.store.calendar.delete(id));
  return {};
});


register('heat.mail.text', (args, fake) => ({
  messages: mailTexts.get(fake)?.get(String(args.threadId)) ?? null,
}));
