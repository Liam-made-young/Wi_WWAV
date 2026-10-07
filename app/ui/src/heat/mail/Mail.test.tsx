import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { mailboxOf, mailTexts } from '../fake/settings';
import { $, $$, button, click, mountHeat, press, type Rig, settle, type } from '../testkit';

// docs/SPEC.md 3.10 and docs/PLAN.md S2.4. What a fail looks like: a thread
// without its sender, time, subject, course chip and state chip; a pane
// without Claude's reason and a link to what it made; Open in Gmail opening
// anything but mail.google.com/mail/u/0/#all/<threadId>; Make a task not
// opening the task sheet with the subject and "From mail:"; Mail that can
// delete; a reply that isn't in the outbox word for word; a sort that puts a
// thread in the wrong section; no sentence for an empty Mail.

let rig: Rig;
afterEach(() => {
  rig?.unmount();
  vi.restoreAllMocks();
});
beforeEach(() => {
  vi.spyOn(window, 'open').mockImplementation(() => null);
});

async function open(options: Parameters<typeof mountHeat>[0] = {}) {
  rig = await mountHeat(options);
  await click(button(rig, 'Mail'));
}
const text = (el: Element | null | undefined) => (el?.textContent ?? '').replace(/\s+/g, ' ').trim();
const panel = () => $(rig, '#heat-panel-mail')!;
const rows = () => $$(panel(), '.heat-thread');

describe('Mail', () => {
  it('lists the threads newest first, each with sender, time, subject and its chips', async () => {
    await open();
    expect(rows().map((r) => text(r))).toEqual([
      'Prof. Tanaka7:00 AMQuiz 4 moved to ThursdayJPN 201Task made',
      'BrightspaceOct 6Grade posted: Kanji quiz 3JPN 201Grade posted',
      'RegistrarOct 5Fall break reminderNothing to do',
    ]);
    // The sidebar holds All and the three states, with counts.
    const side = $$(rig, '.heat-sidebar .heat-side-row').map((r) => text(r));
    expect(side.slice(-4)).toEqual(['All3', 'Grade posted1', 'Task made1', 'Nothing to do1']);
    expect(rig.status().count).toBe('3 threads');
  });

  it('shows what Claude recorded, with its reason and a link to what it made', async () => {
    await open();
    const pane = $(panel(), '.heat-pane')!;
    expect(text($(pane, '.heat-pane-subject'))).toBe('Quiz 4 moved to Thursday');
    expect(text($(pane, '.heat-pane-from'))).toBe('Prof. Tanaka · 7:00 AM');
    expect(text($(pane, '.heat-pane-facts'))).toBe('CourseJPN 201StateTask made');
    expect(text($(pane, '.heat-mail-reason'))).toBe('The email moves the quiz, so the task’s due date matches.');
    // Before Claude has saved the thread's text, the reader says so and offers Gmail.
    expect(text($(pane, '.heat-mail-reader'))).toBe(
      'Claude hasn’t saved this thread’s text yet. Ask Claude to read your mail, or open it in Gmail.',
    );
    await click(button(pane, 'Task: Grammar quiz 4'));
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Tasks');
    expect($(rig, '.heat-info')).toBeTruthy();
  });

  it('links a grade notice to the pending grade it made', async () => {
    await open();
    await click(rows()[1]);
    expect(text($(panel(), '.heat-mail-reason'))).toBe('A grade notice with no score in it.');
    expect(button(panel(), 'Grade: Kanji quiz 3')).toBeTruthy();
    await click(button(panel(), 'Grade: Kanji quiz 3'));
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Grades');
  });

  it('filters by state from the sidebar', async () => {
    await open();
    await click($$(rig, '.heat-sidebar .heat-side-row').find((r) => text(r).startsWith('Nothing to do')));
    expect(rows().map((r) => $(r, '.heat-thread-subject')!.textContent)).toEqual(['Fall break reminder']);
    expect(text($(panel(), '.heat-pane-subject'))).toBe('Fall break reminder');
    expect(rig.status().count).toBe('1 thread');
  });

  it('walks the threads with ↑ ↓', async () => {
    await open();
    await press(rig, 'ArrowDown');
    expect(text($(panel(), '.heat-pane-subject'))).toBe('Grade posted: Kanji quiz 3');
    await press(rig, 'ArrowDown');
    await press(rig, 'ArrowUp');
    expect(text($(panel(), '.heat-pane-subject'))).toBe('Grade posted: Kanji quiz 3');
  });

  it('says "No school mail recorded yet. Ask Claude to read it." when Claude has recorded nothing', async () => {
    await open({ empty: true });
    expect(text($(panel(), '.heat-empty'))).toBe('No school mail recorded yet. Ask Claude to read it.');
    expect($(panel(), '.heat-pane')).toBeNull();
  });

  it('shows a thread Claude records while Mail is open', async () => {
    await open({ empty: true });
    await rig.client.put('mailThread', {
      gmailThreadId: 'g-9',
      subject: 'Lab 5a is due Friday',
      from: 'Prof. Okada <okada@uri.edu>',
      receivedAt: rig.fake.now - 60_000,
      course: 'MTH 142',
      state: 'task',
      reason: 'It names a deadline.',
      recordedBy: 'claude',
    });
    await settle();
    expect(rows().map((r) => $(r, '.heat-thread-subject')!.textContent)).toEqual(['Lab 5a is due Friday']);
  });
});

describe('Open in Gmail and Make a task', () => {
  it('opens the thread at mail.google.com, from the button and from ⇧Return', async () => {
    await open();
    await click(button(panel(), 'Open in Gmail'));
    expect(window.open).toHaveBeenLastCalledWith(
      'https://mail.google.com/mail/u/0/#all/g-1',
      '_blank',
      'noopener,noreferrer',
    );
    await click(rows()[2]);
    rig.view.current!.secondary();
    expect(window.open).toHaveBeenLastCalledWith(
      'https://mail.google.com/mail/u/0/#all/g-3',
      '_blank',
      'noopener,noreferrer',
    );
    expect(rig.status().act).toBe('Open in Gmail');
  });

  it('T opens the task sheet with the subject, and the task’s notes start "From mail:" with the link back', async () => {
    await open();
    expect(await press(rig, 't')).toBe(true);
    const sheet = $(rig, '[role="dialog"][aria-label="New task"]')!;
    expect(($(sheet, 'input') as HTMLInputElement).value).toBe('Quiz 4 moved to Thursday');
    await click(button(sheet, 'Add task'));
    const made = [...rig.fake.store.task.values()].find((t) => t.title === 'Quiz 4 moved to Thursday')!;
    expect(made.notes).toBe('From mail: https://mail.google.com/mail/u/0/#all/g-1');
  });
});

describe('what Mail can’t do', () => {
  // Mail answers, files and searches; it still never deletes.
  it('has no delete, trash or spam, anywhere in the tab', async () => {
    await open();
    const names = $$(panel(), 'button, a, [role="button"], [role="menuitem"]').map((el) =>
      (el.getAttribute('aria-label') ?? el.textContent ?? '').trim(),
    );
    expect(names).toEqual(expect.arrayContaining(['Compose', 'Reply', 'Archive', 'Mark unread', 'Open in Gmail', 'Make a task']));
    for (const name of names) expect(name).not.toMatch(/delete|trash|spam|remove/i);
  });

  it('writes a reply and a new mail into the outbox, word for word', async () => {
    await open();
    const top = [...rig.fake.store.mailThread.values()].sort((a, b) => b.receivedAt - a.receivedAt)[0];
    await click(button(panel(), 'Reply'));
    const form = $(panel(), '.heat-compose')!;
    expect(form.getAttribute('aria-label')).toBe(`Reply to ${top.subject}`);
    expect(button(form, 'Send')!.hasAttribute('disabled')).toBe(true);
    await type($(form, 'textarea'), 'Thank you, せんせい.\nLiam');
    await click(button(form, 'Send'));
    await settle();
    expect($(panel(), '.heat-compose')).toBeNull();
    expect(text($(panel(), '.heat-mail-sync [role="status"]'))).toBe('Reply is in the outbox, on its way.');
    expect(mailboxOf(rig.fake).actions).toMatchObject([
      { kind: 'reply', threadId: top.gmailThreadId, body: 'Thank you, せんせい.\nLiam', status: 'queued' },
    ]);

    await click(button(panel(), 'Compose'));
    const fresh = $(panel(), '.heat-compose')!;
    expect(fresh.getAttribute('aria-label')).toBe('New mail');
    await type($(fresh, '[aria-label="Subject"]'), 'Lab 6');
    await type($(fresh, 'textarea'), 'When is it due?');
    await click(button(fresh, 'Send'));
    await settle();
    // The core's own sentence, and what was written stays to be fixed.
    expect(text($(fresh, '[role="alert"]'))).toBe('Say who the mail is to.');
    expect(($(fresh, 'textarea') as HTMLTextAreaElement).value).toBe('When is it due?');
    await type($(fresh, '[aria-label="To"]'), 'ta@uri.edu');
    await click(button(fresh, 'Send'));
    await settle();
    expect(mailboxOf(rig.fake).actions[1]).toMatchObject({ kind: 'send', to: 'ta@uri.edu', subject: 'Lab 6', body: 'When is it due?' });

    // The outbox lists both, newest first, and one can be thrown away before it goes.
    const boxes = () => $$(rig, '.heat-mail-box').map((b) => text(b));
    expect(boxes()).toEqual(['Inbox3', 'Unread0', 'Archived0', 'Outbox2']);
    await click($$(rig, '.heat-mail-box')[3]);
    await settle();
    const out = $$(panel(), '.heat-outbox-row');
    expect(out.map((r) => text($(r, '.heat-outbox-subject')))).toEqual(['Lab 6', top.subject]);
    expect(text(out[0])).toContain('Waiting');
    await click(button(out[0], 'Discard Lab 6'));
    await settle();
    expect(mailboxOf(rig.fake).actions.map((a) => a.kind)).toEqual(['reply']);
    expect($$(panel(), '.heat-outbox-row')).toHaveLength(1);
  });

  it('archives, marks unread and reads a thread by choosing it', async () => {
    await open();
    const [a, b] = [...rig.fake.store.mailThread.values()].sort((x, y) => y.receivedAt - x.receivedAt);
    const boxes = () => $$(rig, '.heat-mail-box').map((x) => text(x));
    await click(button(panel(), 'Mark unread'));
    await settle();
    expect(rows()[0].hasAttribute('data-unread')).toBe(true);
    expect(boxes()).toEqual(['Inbox3', 'Unread1', 'Archived0', 'Outbox1']);
    expect(button(panel(), 'Mark read')).toBeTruthy();
    // Choosing another thread and coming back reads it.
    await click(rows()[1]);
    await click(rows()[0]);
    await settle();
    expect(rows()[0].hasAttribute('data-unread')).toBe(false);
    expect(rig.calls.filter((c) => c.cmd === 'heat.mail.mark').map((c) => c.args)).toEqual([
      { threadId: a.gmailThreadId, unread: true },
      { threadId: a.gmailThreadId, unread: false },
    ]);

    await click(button(panel(), 'Archive'));
    await settle();
    expect(rows().map((r) => text($(r, '.heat-thread-subject')))).not.toContain(a.subject);
    expect(boxes().slice(0, 3)).toEqual(['Inbox2', 'Unread0', 'Archived1']);
    await click($$(rig, '.heat-mail-box')[2]);
    expect(rows().map((r) => text($(r, '.heat-thread-subject')))).toEqual([a.subject]);
    await click(button(panel(), 'Move to the inbox'));
    await settle();
    expect(rows()).toHaveLength(0);
    await click($$(rig, '.heat-mail-box')[0]);
    expect(rows()).toHaveLength(3);
    // E and U do the same from the keyboard.
    await click(rows()[1]);
    await press(rig, 'e');
    await settle();
    expect(rows().map((r) => text($(r, '.heat-thread-subject')))).not.toContain(b.subject);
  });

  it('sorts by priority, date, unread or tag', async () => {
    await open();
    const [newest, middle, oldest] = [...rig.fake.store.mailThread.values()].sort((a, b) => b.receivedAt - a.receivedAt);
    Object.assign(newest, { priority: 'low', category: 'promotions' });
    Object.assign(middle, { priority: 'urgent', category: 'school' });
    Object.assign(oldest, { priority: 'normal' });
    mailboxOf(rig.fake).places.set(oldest.gmailThreadId, { unread: true, archived: false });
    rig.fake.emit(['mailThread']);
    await settle();
    const subjects = () => rows().map((r) => text($(r, '.heat-thread-subject')));
    const heads = () => $$(panel(), '.heat-mail-section').map((h) => text(h));
    const sortBy = async (value: string) => {
      const select = $(panel(), 'select[aria-label="Sort the threads"]') as unknown as HTMLSelectElement;
      select.value = value;
      select.dispatchEvent(new Event('change', { bubbles: true }));
      await settle();
    };
    expect(heads()).toEqual(['Urgent', 'Everything else']);
    expect(subjects()).toEqual([middle.subject, newest.subject, oldest.subject]);
    await sortBy('date');
    expect(heads()).toEqual([]);
    expect(subjects()).toEqual([newest.subject, middle.subject, oldest.subject]);
    await sortBy('unread');
    expect(heads()).toEqual(['Unread', 'Read']);
    expect(subjects()).toEqual([oldest.subject, newest.subject, middle.subject]);
    await sortBy('tag');
    expect(heads()).toEqual(['School', 'Promotions', 'No tag']);
    expect(subjects()).toEqual([middle.subject, newest.subject, oldest.subject]);
  });

  it('searches inside the saved text, not only what a row shows', async () => {
    await open();
    const [, , oldest] = [...rig.fake.store.mailThread.values()].sort((a, b) => b.receivedAt - a.receivedAt);
    mailTexts.set(rig.fake, new Map([[oldest.gmailThreadId, [{ from: 'Registrar', sentAt: oldest.receivedAt, text: 'Offices close for the long weekend.' }]]]));
    await type($(panel(), 'input[type="search"]'), 'long weekend');
    await settle();
    expect(rows().map((r) => text($(r, '.heat-thread-subject')))).toEqual([oldest.subject]);
    await type($(panel(), 'input[type="search"]'), 'nothing like it');
    await settle();
    expect(rows()).toHaveLength(0);
  });

  it('says how mail is kept in step, reads it when asked, and can stop reading on its own', async () => {
    await open();
    const line = () => text($(panel(), '.heat-mail-sync [role="status"]'));
    expect(line()).toBe('Mail hasn’t been read from here yet.');
    await click(button(panel(), 'Read mail now'));
    await settle();
    expect(line()).toBe('Mail read 10:00 AM: nothing new');
    const own = $(panel(), '.heat-mail-sync input[type="checkbox"]') as HTMLInputElement;
    expect(own.checked).toBe(true);
    await click(own);
    await settle();
    expect(mailboxOf(rig.fake).background).toBe(false);
  });

  it('does nothing on ⌫ or Delete, even with a task selected in another tab', async () => {
    rig = await mountHeat();
    await click(button(rig, 'Tasks'));
    await click($$(rig, '.heat-trow').find((r) => text(r).includes('Order PCBs')));
    await click(button(rig, 'Mail'));
    const before = rig.fake.store.task.size;
    const threads = rig.fake.store.mailThread.size;
    expect(await press(rig, 'Backspace')).toBe(true);
    expect(await press(rig, 'Delete')).toBe(true);
    expect(rig.calls.filter((c) => c.cmd === 'heat.delete')).toEqual([]);
    expect(rig.fake.store.task.size).toBe(before);
    expect(rig.fake.store.mailThread.size).toBe(threads);
  });

  it('writes nothing of its own: opening, selecting, filtering and Open in Gmail change no record', async () => {
    await open();
    await click(rows()[1]);
    await click($$(rig, '.heat-sidebar .heat-side-row').find((r) => text(r).startsWith('Grade posted')));
    await click(button(panel(), 'Open in Gmail'));
    expect(rig.calls.filter((c) => /^heat\.(put|patch|delete|public)/.test(c.cmd))).toEqual([]);
    expect(rig.fake.journal).toEqual([]);
  });

  it('switches between accounts and reads in sections once Claude has sorted by priority', async () => {
    await open();
    // Before any account or priority, Mail is one list with no switcher.
    expect($$(rig, '.heat-mail-account')).toEqual([]);
    expect($$(panel(), '.heat-mail-section')).toEqual([]);

    await rig.call('heat.mail.accounts.set', {
      accounts: [
        { address: 'liam.young@uri.edu', name: 'URI', via: 'forward', forwardTo: 'me+uri@gmail.com' },
        { address: 'me@gmail.com', name: 'Personal', via: 'connector' },
      ],
    });
    const [newest, middle, oldest] = [...rig.fake.store.mailThread.values()].sort((a, b) => b.receivedAt - a.receivedAt);
    Object.assign(newest, { account: 'liam.young@uri.edu', priority: 'normal', category: 'school' });
    Object.assign(middle, { account: 'liam.young@uri.edu', priority: 'urgent', category: 'school' });
    Object.assign(oldest, { account: 'me@gmail.com', priority: 'high', category: 'money' });
    rig.fake.emit(['mailThread']);
    await settle();

    // Most pressing first, whatever arrived last, each under its section.
    expect($$(panel(), '.heat-mail-section').map((s) => text(s))).toEqual(['Urgent', 'High', 'Everything else']);
    expect(rows().map((r) => r.querySelector('.heat-thread-subject')!.textContent)).toEqual([
      middle.subject,
      oldest.subject,
      newest.subject,
    ]);
    expect(rows().map((r) => r.getAttribute('data-priority'))).toEqual(['urgent', 'high', null]);
    expect(text(rows()[1])).toContain('Money');
    const accounts = () => $$(rig, '.heat-mail-account').map((a) => text(a));
    expect(accounts()).toEqual(['All accounts3', 'URI2', 'Personal1']);
    // The states are still the sidebar's last four rows.
    expect($$(rig, '.heat-sidebar .heat-side-row').map((r) => text(r)).slice(-4)[0]).toBe('All3');

    await click($$(rig, '.heat-mail-account')[2]);
    expect(rows().map((r) => r.querySelector('.heat-thread-subject')!.textContent)).toEqual([oldest.subject]);
    expect($$(rig, '.heat-sidebar .heat-side-row').map((r) => text(r)).slice(-4)[0]).toBe('All1');
    expect(text($(panel(), '.heat-pane-facts'))).toContain('PriorityHigh');
    expect(text($(panel(), '.heat-pane-facts'))).toContain('Accountme@gmail.com');
    await click($$(rig, '.heat-mail-account')[0]);
    expect(rows()).toHaveLength(3);
  });

  it('reads a thread in the reader: each message as plain text, with links and quotes, and no HTML', async () => {
    await open();
    const first = [...rig.fake.store.mailThread.values()].sort((a, b) => b.receivedAt - a.receivedAt)[0];
    const texts = new Map([
      [
        first.gmailThreadId,
        [
          { id: 'm1', from: 'Liam Young <liam.young@uri.edu>', sentAt: first.receivedAt - 3_600_000, text: 'Could the quiz move?' },
          {
            id: 'm2',
            from: 'Prof. Tanaka <tanaka@uri.edu>',
            to: 'liam.young@uri.edu',
            sentAt: first.receivedAt,
            text: 'Yes.\nIt is Thursday now.\n\nSee https://brightspace.uri.edu/d2l/home.\n\n> Could the quiz move?\n\n<img src=x onerror=alert(1)>',
          },
        ],
      ],
    ]);
    mailTexts.set(rig.fake, texts);
    rig.fake.emit(['mailThread']);
    await settle();

    const messages = $$(panel(), '.heat-mail-message');
    expect(messages.map((m) => text($(m, '.heat-mail-message-from')))).toEqual(['Liam Young', 'Prof. Tanaka']);
    expect(text($(messages[1], '.heat-mail-message-to'))).toBe('To liam.young@uri.edu');
    const body = $(messages[1], '.heat-mail-text')!;
    expect([...body.children].map((c) => c.tagName)).toEqual(['P', 'P', 'BLOCKQUOTE', 'P']);
    expect(body.children[0].querySelectorAll('br')).toHaveLength(1);
    expect(text(body.children[2])).toBe('Could the quiz move?');
    // An address is a link that opens in the browser, without the sentence's full stop.
    const link = $(body, 'a.heat-mail-address')!;
    expect(link.textContent).toBe('https://brightspace.uri.edu/d2l/home');
    await click(link);
    expect(window.open).toHaveBeenCalledWith('https://brightspace.uri.edu/d2l/home', expect.anything(), expect.anything());
    // Markup in a message is its words, never an element.
    expect(body.querySelector('img')).toBeNull();
    expect(text(body.children[3])).toBe('<img src=x onerror=alert(1)>');
    // Another thread has none saved yet.
    await click(rows()[1]);
    await settle();
    expect(text($(panel(), '.heat-mail-reader'))).toContain('Claude hasn’t saved this thread’s text yet.');
  });
});
