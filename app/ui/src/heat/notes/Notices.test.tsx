import { afterEach, describe, expect, it } from 'vitest';
import { addNotice } from '../fake/notes';
import { register } from '../fake/core';
import { $, $$, button, click, mountHeat, type Rig, settle } from '../testkit';

// The quiet notices (docs/NOTES.md). What a fail looks like: a notice that
// only shows on the Notes tab; more than three at once, or the oldest on
// top; Undo that isn't the entry the core named; a notice that stays after
// it was answered; Open that doesn't open the note.

let rig: Rig;
afterEach(() => rig?.unmount());

const text = (el: Element | null | undefined) => (el?.textContent ?? '').replace(/\s+/g, ' ').trim();
const notices = () => $$(rig, '.notes-notice');
const lines = () => notices().map((n) => text($(n, '.notes-notice-text')));
const called = (cmd: string) => rig.calls.filter((c) => c.cmd === cmd).map((c) => c.args);

async function mount() {
  rig = await mountHeat();
  for (const n of rig.seeded.notes) rig.fake.store.note.set(n.id, n);
  rig.fake.emit(['note']);
  await settle();
}

describe('Notices', () => {
  it('shows the newest three on every tab, in a polite live region', async () => {
    await mount();
    const region = $(rig, '.notes-notices')!;
    expect(region.getAttribute('aria-live')).toBe('polite');
    expect(notices()).toEqual([]);
    for (const line of ['One', 'Two', 'Three', 'Four']) addNotice(rig.fake, { kind: 'capture', text: line });
    await settle();
    // Today is showing: a notice is Learn's, not the Notes tab's.
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Today');
    expect(lines()).toEqual(['Four', 'Three', 'Two']);
    await click(button(rig, 'Grades'));
    expect(lines()).toEqual(['Four', 'Three', 'Two']);
    // Dismissing one makes room for the one that was waiting.
    await click(button(notices()[0], 'Dismiss: Four'));
    expect(called('heat.notice.dismiss')).toEqual([{ id: 'capture:4' }]);
    expect(lines()).toEqual(['Three', 'Two', 'One']);
  });

  it('undoes what it says with the entry the core named, and then is gone', async () => {
    await mount();
    const filed = await rig.client.notes.file('n-page', { courseId: 'c-jpn201' });
    const txnId = rig.fake.journal.at(-1)!.id;
    expect(filed.line).toBe('Filed to JPN 201');
    addNotice(rig.fake, { kind: 'filed', text: 'Filed to JPN 201 · Oct 7', noteId: 'n-page', undo: { txnId } });
    await settle();
    expect(notices().map((n) => $$(n, 'button').map((b) => b.getAttribute('aria-label') ?? b.textContent))).toEqual([
      ['Undo', 'Open', 'Dismiss: Filed to JPN 201 · Oct 7'],
    ]);
    await click(button(notices()[0], 'Undo'));
    expect(called('history.undoEntry')).toEqual([{ txnId }]);
    // The filing is taken back: the page is in the Notes inbox again.
    expect(rig.fake.store.note.get('n-page')).toMatchObject({ inbox: true });
    expect(rig.fake.store.note.get('n-page')!.courseId).toBeUndefined();
    expect(called('heat.notice.dismiss')).toEqual([{ id: 'filed:1' }]);
    expect(notices()).toEqual([]);
  });

  it('keeps a notice whose undo the core refused, and says why', async () => {
    await mount();
    addNotice(rig.fake, { kind: 'filed', text: 'Filed to JPN 201', undo: { txnId: 'txn-nope' } });
    await settle();
    await click(button(notices()[0], 'Undo'));
    expect(rig.status().count).toBe('That change is no longer in the history.');
    expect(lines()).toEqual(['Filed to JPN 201']);
    expect(called('heat.notice.dismiss')).toEqual([]);
  });

  it('runs the one act the core named, as it was given', async () => {
    await mount();
    const ran: unknown[] = [];
    register('heat.test.act', (args) => (ran.push(args), {}));
    addNotice(rig.fake, {
      kind: 'exception',
      text: 'JPN 201 is cancelled on Friday',
      act: { label: 'Skip it', cmd: 'heat.test.act', args: { id: 'com-1' } },
    });
    await settle();
    // Today lists what a mail asked for in a row of its own, so the notice keeps out of its way there.
    expect(notices()).toEqual([]);
    await click(button(rig, 'Tasks')!);
    await click(button(notices()[0], 'Skip it'));
    expect(ran).toEqual([{ id: 'com-1' }]);
    expect(notices()).toEqual([]);
  });

  it('opens the note it is about, in the Notes tab', async () => {
    await mount();
    addNotice(rig.fake, { kind: 'filed', text: 'Filed to JPN 201 · Oct 7', noteId: 'n-drills' });
    await settle();
    await click(button(notices()[0], 'Open'));
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Notes');
    expect(($(rig, '#heat-panel-notes .notes-title') as HTMLInputElement).value).toBe('Te-form drills');
    // Opening it is not answering it: the notice waits for its own dismissal.
    expect(lines()).toEqual(['Filed to JPN 201 · Oct 7']);
  });
});
