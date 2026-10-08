import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { revealedBy, setAttachment, setCapture, suggest } from '../fake/notes';
import { $, $$, button, click, escape, mountHeat, press, type Rig, settle, type, wait } from '../testkit';
import { forgetImages } from './images';

// docs/NOTES.md against the fake core. What a fail looks like: a note whose
// typed text is replaced by a snapshot, or saved on every key; a block that
// can't be clicked into, or that shows its markers when the caret is
// elsewhere; `[[` without its list, or a pick that doesn't write the link; a
// link that goes nowhere; a checkbox that isn't the task it became; a page
// in the inbox that takes more than one choice to file; a rename that leaves
// a link behind.

let rig: Rig;
afterEach(() => {
  rig?.unmount();
  vi.restoreAllMocks();
  forgetImages();
});
beforeEach(() => {
  vi.spyOn(window, 'open').mockImplementation(() => null);
});

/** Learn on the Notes tab, with the sample notes in the library. `first` runs before the notes are there. */
async function open(options: Parameters<typeof mountHeat>[0] = {}, first?: (rig: Rig) => void) {
  rig = await mountHeat(options);
  first?.(rig);
  if (!options.empty) {
    for (const n of rig.seeded.notes) rig.fake.store.note.set(n.id, n);
    rig.fake.emit(['note']);
    await settle();
  }
  await click(button(rig, 'Notes'));
}

const text = (el: Element | null | undefined) => (el?.textContent ?? '').replace(/\s+/g, ' ').trim();
const panel = () => $(rig, '#heat-panel-notes')!;
const rows = () => $$(panel(), '.notes-row');
const titles = () => rows().map((r) => $(r, '.notes-row-title')!.textContent);
const pane = () => $(panel(), '.notes-pane')!;
const titleField = () => $(pane(), '.notes-title') as HTMLInputElement;
const field = () => $(pane(), '.notes-field') as HTMLTextAreaElement | null;
const blocks = () => $$(pane(), '.notes-block');
const side = (label: string) => $$(rig, '.heat-sidebar .heat-side-row').find((r) => text(r).startsWith(label));
const stored = (id: string) => rig.fake.store.note.get(id)!;
const called = (cmd: string) => rig.calls.filter((c) => c.cmd === cmd).map((c) => c.args);
const said = () => rig.status().count;
const pick = (title: string) => click(rows().find((r) => $(r, '.notes-row-title')!.textContent === title));
const end = () => button(pane(), 'Write at the end of the note');
const options = () => $$(pane(), '.notes-links [role="option"]').map((o) => text(o));

async function keyOn(el: Element | null, key: string, init: KeyboardEventInit = {}) {
  if (!el) throw new Error('Nothing has the keyboard.');
  await act(async () => {
    el.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init }));
  });
  await settle(2);
}

/** Puts the caret somewhere in the field, as a click or an arrow key would. */
async function caret(el: HTMLTextAreaElement, at: number) {
  await act(async () => {
    el.setSelectionRange(at, at);
    el.dispatchEvent(new Event('select', { bubbles: true }));
  });
}

describe('the list and the sidebar', () => {
  it('lists the notes newest first, each with its title, date, first words and where it is filed', async () => {
    await open();
    expect(titles()).toEqual(['IMG_2211', 'Mix notes', 'Te-form drills', 'Verb groups']);
    expect(text(rows()[0])).toBe('IMG_22119:00 AMTe-form: 食べる becomes 食べて.Inbox');
    expect(text(rows()[1])).toBe(
      'Mix notes4:00 AMVocals sit too far back after the bridge. See Mix the second verse. Less is more. #mixingWWAV',
    );
    expect(text($(rows()[3], '.notes-row-label'))).toBe('JPN 201 · Intermediate Japanese');
    expect($$(rig, '.heat-sidebar .heat-side-row').map((r) => text(r)).slice(5)).toEqual([
      'Inbox1',
      'All notes4',
      'Today’s note',
      'Send to Wi-WWAV…',
      'JPN 2012',
      '#grammar2',
      '#mixing1',
      '#practice1',
    ]);
    expect(said()).toBe('4 notes');
    // The newest is open.
    expect(titleField().value).toBe('IMG_2211');
  });

  it('narrows the list by a course, a tag and the space, and walks it with ↑ ↓', async () => {
    await open();
    await click(side('JPN 201'));
    expect(titles()).toEqual(['Te-form drills', 'Verb groups']);
    expect(titleField().value).toBe('Te-form drills');
    await press(rig, 'ArrowDown');
    expect(titleField().value).toBe('Verb groups');
    await click(side('#mixing'));
    expect(titles()).toEqual(['Mix notes']);
    expect(titleField().value).toBe('Mix notes');
    await click(side('All notes'));
    await click(button(rig, /^Classes/));
    expect(titles()).toEqual(['Te-form drills', 'Verb groups']);
    // The inbox is every page waiting, whichever space is chosen.
    expect(text(side('Inbox'))).toBe('Inbox1');
    await click(side('Inbox'));
    expect(titles()).toEqual(['IMG_2211']);
  });

  it('filters by a tag when its chip is clicked in a note', async () => {
    await open();
    await pick('Verb groups');
    await click($(pane(), 'button.notes-tag'));
    expect(titles()).toEqual(['Te-form drills', 'Verb groups']);
    expect(side('#grammar')!.getAttribute('aria-current')).toBe('true');
    // The note the chip was in is still the one open.
    expect(titleField().value).toBe('Verb groups');
  });

  it('says so when there are no notes', async () => {
    await open({ empty: true });
    expect(text($(panel(), '.notes-list .heat-empty'))).toBe('No notes yet. Press + to write one, or drop a photo of a page here.');
    expect($(panel(), '.notes-pane')).toBeNull();
  });
});

describe('the editor', () => {
  it('draws every block, and shows the one clicked as it is written', async () => {
    await open();
    await pick('Mix notes');
    expect(blocks().map((b) => b.firstElementChild!.tagName)).toEqual(['P', 'PRE', 'BLOCKQUOTE']);
    expect($(blocks()[0], 'strong')!.textContent).toBe('too far back');
    expect(field()).toBeNull();
    await click($(blocks()[0], 'p'));
    expect(field()!.value).toBe('Vocals sit **too far back** after the bridge. See [[Mix the second verse]].');
    expect(document.activeElement).toBe(field());
    expect(blocks().length).toBe(2);
    await escape(rig);
    expect(field()).toBeNull();
    expect(blocks().length).toBe(3);
  });

  it('saves a moment after the typing stops, not on every key', async () => {
    await open();
    await pick('Mix notes');
    await click($(blocks()[2], 'p'));
    await type(field(), '> Less is more, mostly. #mixing');
    await settle();
    expect(called('heat.note.save')).toEqual([]);
    await wait(700);
    await settle();
    expect(called('heat.note.save')).toEqual([
      {
        id: 'n-mix',
        markdown:
          'Vocals sit **too far back** after the bridge. See [[Mix the second verse]].\n\n```\nEQ: -2 dB at 300 Hz\n```\n\n> Less is more, mostly. #mixing',
      },
    ]);
    expect((await rig.call<{ undo: string }>('history.get', {})).undo).toBe('Undo edit note');
    // The caret is still in the block.
    expect(field()!.value).toBe('> Less is more, mostly. #mixing');
  });

  it('saves at once when the block is left', async () => {
    await open();
    await pick('Mix notes');
    await click($(blocks()[0], 'p'));
    await type(field(), 'Vocals are fine now.');
    await escape(rig);
    expect(stored('n-mix').markdown.split('\n')[0]).toBe('Vocals are fine now.');
    expect(text(blocks()[0])).toBe('Vocals are fine now.');
  });

  it('keeps what is typed when a snapshot arrives, even one that changed the note', async () => {
    await open();
    await pick('Mix notes');
    await click($(blocks()[0], 'p'));
    await type(field(), 'Half a thought');
    // Something else changes, and the views read the snapshot again.
    await act(async () => void (await rig.client.capture('a stray thought')));
    await settle();
    expect(field()!.value).toBe('Half a thought');
    // Even the note itself changing somewhere else doesn't take the words from under the caret.
    rig.fake.store.note.set('n-mix', { ...stored('n-mix'), markdown: 'Written somewhere else.' });
    await act(async () => rig.fake.emit(['note']));
    await settle();
    expect(field()!.value).toBe('Half a thought');
    await wait(700);
    await settle();
    expect(stored('n-mix').markdown.split('\n')[0]).toBe('Half a thought');
  });

  it('takes the core’s text when nothing typed is waiting', async () => {
    await open();
    await pick('Mix notes');
    rig.fake.store.note.set('n-mix', { ...stored('n-mix'), markdown: '# Written by Claude\n\nA new line.' });
    await act(async () => rig.fake.emit(['note']));
    await settle();
    expect(blocks().map((b) => text(b))).toEqual(['Written by Claude', 'A new line.']);
  });

  it('starts a block after a blank line, and one under the last with a click', async () => {
    await open();
    await pick('Mix notes');
    await click($(blocks()[0], 'p'));
    await type(field(), 'First.\n\n');
    await settle();
    // The first block is drawn again, and the caret is in a new one after it.
    expect(text(blocks()[0])).toBe('First.');
    expect(field()!.value).toBe('');
    await type(field(), 'Second.');
    await escape(rig);
    expect(stored('n-mix').markdown.startsWith('First.\n\nSecond.\n\n```')).toBe(true);

    await click(end());
    expect(field()!.value).toBe('');
    await type(field(), 'Last.');
    await escape(rig);
    expect(stored('n-mix').markdown.endsWith('> Less is more. #mixing\n\nLast.')).toBe(true);
    // A block left empty leaves nothing behind.
    await click(end());
    await escape(rig);
    expect(stored('n-mix').markdown.endsWith('\n\nLast.')).toBe(true);
    expect(blocks().length).toBe(5);
  });

  it('moves between blocks with ↑ and ↓, and joins two with ⌫ at the start', async () => {
    await open();
    await pick('Mix notes');
    await click($(blocks()[2], 'p'));
    expect(field()!.value).toBe('> Less is more. #mixing');
    await caret(field()!, 3);
    await keyOn(field(), 'ArrowUp');
    expect(field()!.value).toBe('```\nEQ: -2 dB at 300 Hz\n```');
    // The caret arrives at the end, which is not the first line: ↑ now moves within the block.
    expect(field()!.selectionStart).toBe(field()!.value.length);
    await keyOn(field(), 'ArrowUp');
    expect(field()!.value).toContain('EQ');
    await keyOn(field(), 'ArrowDown');
    expect(field()!.value).toBe('> Less is more. #mixing');
    expect(field()!.selectionStart).toBe(0);

    await keyOn(field(), 'Backspace');
    expect(field()!.value).toBe('```\nEQ: -2 dB at 300 Hz\n```\n> Less is more. #mixing');
    // The caret is where the two met: the start of the line that was the second block.
    expect(field()!.selectionStart).toBe(28);
    await escape(rig);
    expect(stored('n-mix').markdown.endsWith('```\nEQ: -2 dB at 300 Hz\n```\n> Less is more. #mixing')).toBe(true);
  });

  it('goes on with a list on Return, ends it on an empty item, and moves an item with Tab', async () => {
    await open();
    await pick('Te-form drills');
    await click($(blocks()[1], 'li'));
    const list = '- [ ] Do worksheet 4\n- [x] Read section 3.2\n- [ ] Ask about [[Grammar quiz 4]] in [[JPN 201]]';
    expect(field()!.value).toBe(list);
    await caret(field()!, list.length);
    await keyOn(field(), 'Enter');
    expect(field()!.value).toBe(`${list}\n- [ ] `);
    await type(field(), `${list}\n- [ ] Review`);
    await keyOn(field(), 'Tab');
    expect(field()!.value).toBe(`${list}\n    - [ ] Review`);
    await keyOn(field(), 'Tab', { shiftKey: true });
    expect(field()!.value).toBe(`${list}\n- [ ] Review`);
    await keyOn(field(), 'Enter');
    // Return on the empty item ends the list: the caret is in a new block under it.
    await keyOn(field(), 'Enter');
    expect(field()!.value).toBe('');
    expect($$(blocks()[1], 'li').length).toBe(4);
    await escape(rig);
    expect(stored('n-drills').markdown).toContain(`${list}\n- [ ] Review\n\nMore at`);
  });

  it('renames a note from its title, and every link to it follows', async () => {
    await open();
    await pick('Verb groups');
    await type(titleField(), 'Verb classes');
    await act(async () => titleField().blur());
    await settle();
    expect(stored('n-verbs').title).toBe('Verb classes');
    expect(stored('n-drills').markdown).toContain('Drills for the rule in [[Verb classes]].');
    expect(said()).toBe('Renamed. Links in 1 other note follow it.');
    expect(titles()).toContain('Verb classes');
  });

  it('says why a name another note has is refused, and keeps the note’s own', async () => {
    await open();
    await pick('Verb groups');
    await type(titleField(), 'Mix notes');
    await keyOn(titleField(), 'Enter');
    await settle();
    expect(said()).toBe('A note is already called Mix notes.');
    expect(titleField().value).toBe('Verb groups');
    expect(stored('n-verbs').title).toBe('Verb groups');
  });
});

describe('links', () => {
  it('lists notes, courses and tasks after [[, and a pick writes the link', async () => {
    await open();
    await pick('Mix notes');
    await click(end());
    await type(field(), 'See [[');
    // The other notes, then the courses, then open tasks, eight at most.
    expect(options().slice(0, 5)).toEqual(['IMG_2211', 'Te-form drills', 'Verb groups', 'JPN 201course', 'MTH 142course']);
    expect(options().length).toBe(8);
    expect(options().slice(5).every((o) => o.endsWith('task'))).toBe(true);
    await type(field(), 'See [[ver');
    expect(options()).toEqual(['Verb groups', 'Mix the second versetask', 'Finish the cover arttask', 'New note “ver”']);
    const list = $(pane(), '.notes-links')!;
    expect(list.getAttribute('role')).toBe('listbox');
    expect(field()!.getAttribute('aria-activedescendant')).toBe($$(list, '[role="option"]')[0].id);
    await keyOn(field(), 'ArrowDown');
    expect(field()!.getAttribute('aria-activedescendant')).toBe($$(list, '[role="option"]')[1].id);
    expect($$(list, '[aria-selected="true"]').map((o) => text(o))).toEqual(['Mix the second versetask']);
    await keyOn(field(), 'ArrowUp');
    await keyOn(field(), 'Enter');
    expect(field()!.value).toBe('See [[Verb groups]]');
    expect(field()!.selectionStart).toBe(19);
    expect($(pane(), '.notes-links')).toBeNull();

    // Tab picks too, and a course is offered by its code.
    await type(field(), 'See [[Verb groups]] for [[jpn');
    expect(options()).toEqual(['JPN 201course', 'New note “jpn”']);
    await keyOn(field(), 'Tab');
    expect(field()!.value).toBe('See [[Verb groups]] for [[JPN 201]]');
    await escape(rig);
    expect(stored('n-mix').markdown.endsWith('\n\nSee [[Verb groups]] for [[JPN 201]]')).toBe(true);
    expect($$(blocks().at(-1), '.notes-wikilink').map((l) => [l.textContent, l.dataset.kind])).toEqual([
      ['Verb groups', 'note'],
      ['JPN 201', 'course'],
    ]);
  });

  it('closes the list on Esc and leaves the block on the next', async () => {
    await open();
    await pick('Mix notes');
    await click(end());
    await type(field(), '[[ver');
    expect(options().length).toBe(4);
    await escape(rig);
    expect($(pane(), '.notes-links')).toBeNull();
    expect(field()!.value).toBe('[[ver');
    await escape(rig);
    expect(field()).toBeNull();
  });

  it('makes the note when “New note” is picked', async () => {
    await open();
    await pick('Mix notes');
    await click(end());
    await type(field(), 'Ask about [[Keigo forms');
    expect(options()).toEqual(['New note “Keigo forms”']);
    await keyOn(field(), 'Enter');
    await settle();
    expect(field()!.value).toBe('Ask about [[Keigo forms]]');
    expect(called('heat.note.create')).toEqual([{ title: 'Keigo forms', ifMissing: true }]);
    expect([...rig.fake.store.note.values()].some((n) => n.title === 'Keigo forms')).toBe(true);
    await escape(rig);
    expect($(blocks().at(-1), '.notes-wikilink')!.dataset.kind).toBe('note');
  });

  it('opens what a link names: a note here, a course in Grades, a task in Tasks', async () => {
    await open();
    await pick('Te-form drills');
    const link = (name: string) => $$(pane(), '.notes-wikilink').find((l) => l.textContent === name)!;
    expect($$(pane(), '.notes-wikilink').map((l) => [l.textContent, l.dataset.kind])).toEqual([
      ['Verb groups', 'note'],
      ['Grammar quiz 4', 'task'],
      ['JPN 201', 'course'],
      ['Keigo', 'missing'],
    ]);
    await click(link('Verb groups'));
    expect(titleField().value).toBe('Verb groups');
    // A click on a link is not a click into its block.
    expect(field()).toBeNull();

    await pick('Te-form drills');
    await click(link('JPN 201'));
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Grades');

    await click(button(rig, 'Notes'));
    await click(link('Grammar quiz 4'));
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Tasks');
    expect(($(rig, '.heat-info [data-info-first]') as HTMLInputElement).value).toBe('Grammar quiz 4');
  });

  it('makes the note a link to nothing names, and opens it', async () => {
    await open();
    await pick('Te-form drills');
    await click($$(pane(), '.notes-wikilink').find((l) => l.textContent === 'Keigo'));
    expect(called('heat.note.create')).toEqual([{ title: 'Keigo', ifMissing: true }]);
    expect(titleField().value).toBe('Keigo');
    expect(titles()[0]).toBe('Keigo');
    // And the note it came from links here.
    expect($$(pane(), '.notes-backlink').map((b) => text($(b, '.notes-backlink-title')))).toEqual(['Te-form drills']);
  });

  it('shows the notes that link to this one, each with its line, and opens one', async () => {
    await open();
    await pick('Mix notes');
    expect(text($(pane(), '.notes-backlinks'))).toBe('BacklinksNo notes link here yet.');
    await pick('Verb groups');
    const back = $$(pane(), '.notes-backlink');
    expect(back.map((b) => text(b))).toEqual(['Te-form drillsDrills for the rule in Verb groups. #grammar #practice']);
    await click(back[0]);
    expect(titleField().value).toBe('Te-form drills');
  });
});

describe('checkboxes', () => {
  const boxLine = (words: string) => $$(pane(), 'li[data-box]').find((li) => text($(li, '.notes-item')).startsWith(words))!;

  it('makes a checkbox a task, and shows its chip from then on', async () => {
    await open();
    await pick('Te-form drills');
    expect($(boxLine('Do worksheet 4'), '.notes-task-chip')).toBeNull();
    await click(button(boxLine('Do worksheet 4'), 'Make task'));
    expect(called('heat.note.taskFromLine')).toEqual([{ id: 'n-drills', line: 2 }]);
    const task = [...rig.fake.store.task.values()].find((t) => t.title === 'Do worksheet 4')!;
    expect(task).toMatchObject({ noteId: 'n-drills', courseId: 'c-jpn201', done: false });
    expect(stored('n-drills').markdown).toContain(`- [ ] Do worksheet 4 ^t-${task.id}`);
    expect(said()).toBe('Task added.');
    // The marker is the core's; the line reads as it did, with a chip in place of the button.
    expect(text($(boxLine('Do worksheet 4'), '.notes-item'))).toBe('Do worksheet 4');
    expect(button(boxLine('Do worksheet 4'), 'Make task')).toBeUndefined();
    await click($(boxLine('Do worksheet 4'), '.notes-task-chip'));
    expect(($(rig, '.heat-info [data-info-first]') as HTMLInputElement).value).toBe('Do worksheet 4');
  });

  it('marks the task when a linked box is ticked, and follows the task when it is marked elsewhere', async () => {
    await open();
    await pick('Te-form drills');
    await click(button(boxLine('Do worksheet 4'), 'Make task'));
    const id = [...rig.fake.store.task.values()].find((t) => t.title === 'Do worksheet 4')!.id;
    const box = () => $(boxLine('Do worksheet 4'), 'input') as HTMLInputElement;
    expect(box().checked).toBe(false);
    await click(box());
    expect(called('heat.note.toggleBox')).toEqual([{ id: 'n-drills', line: 2, done: true }]);
    expect(rig.fake.store.task.get(id)!.done).toBe(true);
    expect(box().checked).toBe(true);
    // The task unmarked from Tasks: the box shows the task's state, whatever the line says.
    await act(async () => void (await rig.client.done(id, false)));
    await settle();
    expect(stored('n-drills').markdown).toContain('- [x] Do worksheet 4');
    expect(box().checked).toBe(false);
  });

  it('ticks a plain box in the note’s own text', async () => {
    await open();
    await pick('Te-form drills');
    const box = () => $(boxLine('Read section 3.2'), 'input') as HTMLInputElement;
    expect(box().checked).toBe(true);
    await click(box());
    expect(called('heat.note.toggleBox')).toEqual([{ id: 'n-drills', line: 3, done: false }]);
    expect(stored('n-drills').markdown).toContain('- [ ] Read section 3.2');
    expect(box().checked).toBe(false);
    expect(field()).toBeNull();
  });

  it('sends what was typed before it names a line', async () => {
    await open();
    await pick('Te-form drills');
    await click($(blocks()[0], 'p'));
    await type(field(), 'Drills.\nSecond line.');
    await click(button(boxLine('Do worksheet 4'), 'Make task'));
    // The list is one line lower now, in the note as the core holds it.
    expect(called('heat.note.taskFromLine')).toEqual([{ id: 'n-drills', line: 3 }]);
    expect(stored('n-drills').markdown.split('\n')[3]).toMatch(/^- \[ \] Do worksheet 4 \^t-/);
  });
});

describe('search and quick open', () => {
  it('shows the notes that hold the words, with the line they are in', async () => {
    await open();
    await press(rig, 'f', { metaKey: true, ctrlKey: true });
    const search = $(panel(), '.heat-filter') as HTMLInputElement;
    expect(document.activeElement).toBe(search);
    await type(search, 'worksheet');
    await wait(260);
    await settle();
    expect(called('heat.note.search').at(-1)).toMatchObject({ q: 'worksheet' });
    expect(rows().map((r) => text(r))).toEqual(['Te-form drillsOct 6Do worksheet 4JPN 201 · Intermediate Japanese']);
    await click(rows()[0]);
    expect(titleField().value).toBe('Te-form drills');
    await type(search, 'zzz');
    await wait(260);
    await settle();
    expect(text($(panel(), '.notes-list .heat-empty'))).toBe('No note holds those words.');
    await type(search, '');
    await settle();
    expect(titles()).toEqual(['IMG_2211', 'Mix notes', 'Te-form drills', 'Verb groups']);
  });

  it('opens a note by its title from Quick open: O, or the tab’s secondary act', async () => {
    await open();
    expect(rig.status().act).toBe('Quick open');
    await press(rig, 'o');
    const quick = () => $(panel(), '.notes-quick');
    const input = () => $(quick(), 'input') as HTMLInputElement;
    expect(document.activeElement).toBe(input());
    await type(input(), 'e');
    // A title that starts with the letters comes before one that only holds them.
    expect($$(quick(), '[role="option"]').map((o) => text(o))).toEqual(['Mix notes', 'Te-form drills', 'Verb groups']);
    await type(input(), 'te');
    expect($$(quick(), '[role="option"]').map((o) => text(o))).toEqual(['Te-form drills', 'Mix notes']);
    await keyOn(input(), 'ArrowDown');
    await keyOn(input(), 'ArrowUp');
    await keyOn(input(), 'Enter');
    expect(quick()).toBeNull();
    expect(titleField().value).toBe('Te-form drills');

    await act(async () => rig.view.current!.secondary());
    expect(quick()).toBeTruthy();
    await escape(rig);
    expect(quick()).toBeNull();
  });
});

describe('making and filing', () => {
  it('makes a note with “+”, in the course the list is showing, with the caret in its title', async () => {
    await open();
    await click(side('JPN 201'));
    await click(button(rig, 'New note'));
    expect(called('heat.note.create')).toEqual([{ courseId: 'c-jpn201' }]);
    expect(titleField().value).toBe('Untitled');
    expect(document.activeElement).toBe(titleField());
    expect(titles()).toEqual(['Untitled', 'Te-form drills', 'Verb groups']);
    await type(titleField(), 'Particles');
    await keyOn(titleField(), 'Enter');
    await settle();
    expect([...rig.fake.store.note.values()].find((n) => n.title === 'Particles')).toMatchObject({ courseId: 'c-jpn201' });
    // Return in the title goes on to the text.
    expect(document.activeElement).toBe(field());
  });

  it('files a page from the inbox with one choice', async () => {
    await open();
    await click(side('Inbox'));
    expect(titleField().value).toBe('IMG_2211');
    const menu = () => $(pane(), '.notes-file select') as HTMLSelectElement;
    expect($(pane(), '.notes-meta')!.hasAttribute('data-inbox')).toBe(true);
    expect(menu().getAttribute('aria-label')).toBe('File to');
    expect([...menu().options].map((o) => o.textContent)).toEqual([
      'File to…',
      'JPN 201 · Intermediate Japanese',
      'MTH 142 · Calculus II',
      'Classes',
      'WWAV',
      'Personal',
      'No course or space',
    ]);
    await type(menu(), 'course:c-jpn201');
    await settle();
    expect(called('heat.note.file')).toEqual([{ id: 'n-page', courseId: 'c-jpn201' }]);
    expect(stored('n-page')).toMatchObject({ courseId: 'c-jpn201' });
    expect(stored('n-page').inbox).toBeUndefined();
    expect(said()).toBe('Filed to JPN 201');
    expect(text(side('Inbox'))).toBe('Inbox0');
    expect(text($(panel(), '.notes-list .heat-empty'))).toBe('Nothing is waiting to be filed.');
    // The page is still open, and says where it went; it can be moved again from the same menu.
    expect(menu().value).toBe('course:c-jpn201');
    expect(menu().getAttribute('aria-label')).toBe('Filed to');
    await type(menu(), 'none');
    await settle();
    expect(called('heat.note.file').at(-1)).toEqual({ id: 'n-page', none: true });
    expect(stored('n-page').courseId).toBeUndefined();
  });

  it('deletes a note, and shows one in the Finder', async () => {
    await open();
    await pick('Mix notes');
    await click(button(pane(), 'Show in Finder'));
    expect(revealedBy(rig.fake)).toEqual(['/Users/you/Music/Wi_WWAV/Notes/Mix notes.md']);
    await click(button(pane(), 'Delete'));
    expect(rig.fake.store.note.has('n-mix')).toBe(false);
    expect(said()).toBe('Deleted ‘Mix notes’.');
    expect(titles()).toEqual(['IMG_2211', 'Te-form drills', 'Verb groups']);
    expect((await rig.call<{ undo: string }>('history.get', {})).undo).toBe('Undo delete note');
  });
});

describe('what Claude suggested', () => {
  it('shows the tasks and key terms under the note, and adds only what is clicked', async () => {
    await open();
    suggest(rig.fake, 'n-page', {
      tasks: [{ title: 'Study for the te-form quiz', due: '2026-10-09' }, { title: 'Bring the workbook' }],
      terms: ['te-form', 'group one verbs'],
    });
    await settle();
    const box = () => $(pane(), '.notes-suggest')!;
    expect($$(box(), '.notes-suggest-task').map((t) => text(t))).toEqual([
      'Study for the te-form quizDue Oct 9Add task',
      'Bring the workbookAdd task',
    ]);
    expect(text($(box(), '.notes-terms'))).toBe('Key termste-formgroup one verbs');
    expect([...rig.fake.store.task.values()].some((t) => t.title === 'Study for the te-form quiz')).toBe(false);
    await click(button($$(box(), '.notes-suggest-task')[0], 'Add task'));
    expect(called('heat.note.suggestion.accept')).toEqual([{ noteId: 'n-page', index: 0 }]);
    expect([...rig.fake.store.task.values()].some((t) => t.title === 'Study for the te-form quiz')).toBe(true);
    expect($$(box(), '.notes-suggest-task').map((t) => text(t))).toEqual([
      'Study for the te-form quizDue Oct 9Added',
      'Bring the workbookAdd task',
    ]);
    await click(button(box(), 'Dismiss'));
    expect($(pane(), '.notes-suggest')).toBeNull();
  });
});

describe('images', () => {
  it('asks the core for an image in the notes folder, and opens it larger', async () => {
    await open({}, (r) => setAttachment(r.fake, 'attachments/IMG_2211.jpg', 'data:image/png;base64,AAAA'));
    expect(called('heat.note.attachment')).toEqual([{ path: 'attachments/IMG_2211.jpg' }]);
    const img = $(pane(), 'img.notes-img') as HTMLImageElement;
    expect(img.getAttribute('src')).toBe('data:image/png;base64,AAAA');
    await click($(pane(), '.notes-img-open'));
    expect(field()).toBeNull();
    const large = $(panel(), '.notes-lightbox')!;
    expect($(large, 'img')!.getAttribute('src')).toBe('data:image/png;base64,AAAA');
    await escape(rig);
    expect($(panel(), '.notes-lightbox')).toBeNull();
  });

  it('says so quietly when an image can’t be had', async () => {
    await open({}, (r) => setAttachment(r.fake, 'attachments/IMG_2211.jpg', null));
    expect($(pane(), 'img.notes-img')).toBeNull();
    expect(text($(pane(), '.notes-img-wait'))).toBe('This image couldn’t be loaded.');
  });
});

describe('capture', () => {
  const file = (name: string, type: string, body = 'page') => new File([body], name, { type });
  async function drop(files: File[]) {
    const e = new Event('drop', { bubbles: true, cancelable: true });
    Object.assign(e, { dataTransfer: { files, types: ['Files'], getData: () => '' } });
    await act(async () => {
      $(panel(), '.notes')!.dispatchEvent(e);
      await new Promise((r) => setTimeout(r, 20));
    });
    await settle();
  }

  it('sends a dropped photo to the capture inbox, and shows the page it becomes', async () => {
    await open();
    await drop([file('IMG_3000.jpg', 'image/jpeg'), file('notes.txt', 'text/plain')]);
    expect(called('heat.capture.inbox.add')).toEqual([{ name: 'IMG_3000.jpg', base64: 'cGFnZQ==' }]);
    expect(text(side('Inbox'))).toBe('Inbox2');
    expect(titles()[0]).toBe('IMG_3000');
    // The core says what it did, quietly, with its undo.
    expect($$(rig, '.notes-notice').map((n) => text($(n, '.notes-notice-text')))).toEqual(['Added IMG_3000 to the Notes inbox']);
  });

  it('takes a pasted image when no field has the keyboard, and leaves a paste in a field alone', async () => {
    await open();
    const paste = async () => {
      const e = new Event('paste', { bubbles: true, cancelable: true });
      Object.assign(e, { clipboardData: { files: [file('image.png', 'image/png')] } });
      await act(async () => {
        document.dispatchEvent(e);
        await new Promise((r) => setTimeout(r, 20));
      });
      await settle();
    };
    await pick('Mix notes');
    await click($(blocks()[0], 'p'));
    await paste();
    expect(called('heat.capture.inbox.add')).toEqual([]);
    await escape(rig);
    await paste();
    expect(called('heat.capture.inbox.add')).toEqual([{ name: 'Pasted 2026-10-07 10.00 AM.png', base64: 'cGFnZQ==' }]);
  });

  it('says a file that is no page isn’t one', async () => {
    await open();
    await drop([file('notes.txt', 'text/plain')]);
    expect(called('heat.capture.inbox.add')).toEqual([]);
    expect(said()).toBe('Drop a photo or a PDF of a page.');
  });

  it('shows what the inbox is doing in one quiet line', async () => {
    await open();
    expect($(panel(), '.notes-capture')).toBeNull();
    setCapture(rig.fake, { doing: 'Reading IMG_2211.jpg', waiting: 2 });
    await settle();
    expect(text($(panel(), '.notes-capture'))).toBe('Reading IMG_2211.jpg · 2 pages waiting to be read');
  });

  it('explains how a page gets here, and holds the switch for what Claude may read', async () => {
    await open();
    await click(side('Send to Wi-WWAV'));
    const sheet = () => $(rig, '.notes-guide')!;
    expect(sheet().getAttribute('role')).toBe('dialog');
    expect(text(sheet())).toContain('tools/shortcut/Send to Wi-WWAV.shortcut');
    expect(text(sheet())).toContain('AirDrop it to your phone or open it from iCloud Drive, then Add Shortcut.');
    expect($$(sheet(), '.notes-guide-folders code').map((c) => c.textContent)).toEqual([
      '/Users/you/Library/Mobile Documents/com~apple~CloudDocs/Wi-WWAV Inbox',
      '/Users/you/Library/Mobile Documents/com~apple~CloudDocs/Shortcuts/Wi-WWAV Inbox',
    ]);
    expect(text(sheet())).toContain('These folders are being watched.');
    expect(text(sheet())).toContain('The reader on this Mac is ready.');
    const claude = () => $(sheet(), 'input[role="switch"]') as HTMLInputElement;
    expect(claude().checked).toBe(true);
    await click(claude());
    expect(called('heat.capture.settings.set')).toEqual([{ claude: false }]);
    expect(claude().checked).toBe(false);
    setCapture(rig.fake, { reader: 'missing', watching: false });
    await settle();
    expect(text(sheet())).toContain('The reader on this Mac is not available: install Xcode’s command-line tools.');
    expect(text(sheet())).toContain('These folders aren’t being watched right now.');
    await click(button(sheet(), 'Show the notes folder'));
    expect(revealedBy(rig.fake)).toEqual(['/Users/you/Music/Wi_WWAV/Notes']);
    await click(button(sheet(), 'Done'));
    expect($(rig, '.notes-guide')).toBeNull();
  });
});

describe('the daily note', () => {
  it('is one note, written from Today and read in Notes', async () => {
    rig = await mountHeat();
    await click($(rig, '.heat-note-line'));
    await type($(rig, '.heat-note-field'), 'Mixed the bridge.\nSee [[Mix notes]].');
    await escape(rig);
    expect(called('heat.note.daily')).toEqual([{ date: '2026-10-07', markdown: 'Mixed the bridge.\nSee [[Mix notes]].' }]);
    expect($(rig, '.heat-note-line')!.textContent).toBe('Mixed the bridge.');

    await click(button(rig, 'Notes'));
    await click(side('Today’s note'));
    expect(titleField().value).toBe('2026-10-07');
    expect(text(blocks()[0])).toBe('Mixed the bridge.See Mix notes.');
    // Nothing new was made: the note Today wrote is the one that opens.
    expect(called('heat.note.create')).toEqual([]);

    // Written in Notes, read in Today.
    await click($(blocks()[0], 'p'));
    await type(field(), 'Mastered it too.');
    await escape(rig);
    await click(button(rig, 'Today'));
    expect($(rig, '.heat-note-line')!.textContent).toBe('Mastered it too.');
  });

  it('is made the first time it is asked for from the sidebar', async () => {
    await open();
    await click(side('Today’s note'));
    expect(called('heat.note.create')).toEqual([{ title: '2026-10-07', ifMissing: true }]);
    expect(titleField().value).toBe('2026-10-07');
    await click(side('Today’s note'));
    expect(called('heat.note.create').length).toBe(1);
    // Today reads the same note.
    await click(end());
    await type(field(), 'From the Notes tab.');
    await escape(rig);
    await click(button(rig, 'Today'));
    expect($(rig, '.heat-note-line')!.textContent).toBe('From the Notes tab.');
  });
});
