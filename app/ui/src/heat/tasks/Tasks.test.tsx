import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { $, $$, button, click, escape, mountHeat, press, type Rig, settle, type } from '../testkit';
import { nest, matchesFilter } from './lists';

// docs/PLAN.md S2.4 and docs/SPEC.md 3.6. What a fail looks like: a sidebar
// without its seven lists or "Your average time"; rows out of the snapshot's
// order, without their columns, a subtask not under its parent or a recurring
// task without ↻; "+" without a name check; "Triage inbox" offered with an
// empty inbox; ⌘↩ or ⌫ without their Undo; the empty line missing.

let rig: Rig;
afterEach(() => rig?.unmount());

const tab = (name: string) => button(rig, name)!;
const titles = () => $$(rig, '.heat-table [role="row"]:not(.heat-thead) .heat-td-name').map((e) => e.textContent);
const rowFor = (title: string) =>
  $$(rig, '.heat-table [role="row"]').find((r) => r.querySelector('.heat-td-name')?.textContent === title)!;
const side = () => $$(rig, '.heat-tasks-sidebar-none, .heat-sidebar-slot .heat-side-row').map((r) => r.textContent);
const task = (id: string) => rig.fake.store.task.get(id)!;
const undoLabel = async () => (await rig.call<{ undo: string | null }>('history.get', {})).undo;

async function openTasks(options: Parameters<typeof mountHeat>[0] = {}) {
  rig = await mountHeat(options);
  await click(tab('Tasks'));
}

describe('Tasks’ sidebar', () => {
  it('lists Inbox, All open, Hot, Due this week, Scheduled, Someday and Done with their counts', async () => {
    await openTasks();
    const lists = $$(rig, '.heat-sidebar-slot .heat-side-row')
      .slice(0, 7)
      .map((r) => r.textContent);
    expect(lists).toEqual(['Inbox3', 'All open11', 'Hot4', 'Due this week7', 'Scheduled1', 'Someday1', 'Done1']);
  });

  it('follows with Courses, or Projects with their milestones, or Areas, then your average time', async () => {
    await openTasks();
    const headings = $$(rig, '.heat-sidebar-slot .heat-side-heading').map((h) => h.textContent);
    expect(headings).toEqual(['Library', 'Courses', 'Projects', 'Areas', 'Your average time']);
    const rows = side();
    expect(rows).toContain('JPN 2013');
    expect(rows).toContain('EP4');
    expect(rows).toContain('EP v1 mixed1');
    expect(rows).toContain('Admin1');
    expect($$(rig, '.heat-average').map((a) => a.textContent)).toEqual(['Reading 50m (1)']);
  });

  it('shows only a space’s own kind of group when a space is chosen', async () => {
    await openTasks();
    await click(button(rig, /^Classes/));
    expect($$(rig, '.heat-sidebar-slot .heat-side-heading').map((h) => h.textContent)).toEqual([
      'Library',
      'Courses',
      'Your average time',
    ]);
    // The lists count that space's tasks.
    expect(
      $$(rig, '.heat-sidebar-slot .heat-side-row')
        .slice(1, 3)
        .map((r) => r.textContent),
    ).toEqual(['All open4', 'Hot4']);
    await click(button(rig, /^WWAV/));
    expect($$(rig, '.heat-sidebar-slot .heat-side-heading').map((h) => h.textContent)).toEqual(['Library', 'Projects']);
  });
});

describe('Tasks’ list', () => {
  it('draws the open tasks in the order the snapshot gave, with a column each', async () => {
    await openTasks();
    const snap = await rig.client.snapshot('2026-10-07');
    const order = snap.derived.lists.allOpen.filter((id) => !snap.records.task.find((t) => t.id === id)!.parentTaskId);
    const top = order.map((id) => snap.records.task.find((t) => t.id === id)!.title);
    // Subtasks sit under their parent, so the top-level titles keep the snapshot's order.
    expect(titles().filter((t) => top.includes(t!))).toEqual(top);
    expect($$(rig, '.heat-th').map((h) => h.textContent)).toEqual([
      '',
      'Heat',
      'Task',
      'Group',
      'Type',
      'Due',
      'Diff',
      'Time',
      'When',
    ]);
    const cells = [...rowFor('Grammar quiz 4').querySelectorAll('[role="gridcell"]')].map((c) => c.textContent);
    expect(cells).toEqual(['', 'Hot', 'Grammar quiz 4', 'JPN 201', 'Quiz', 'Tomorrow 11:59 PM', '3', '1h', '']);
  });

  it('names the When column from the scheduled date, and Done tasks by when they were done', async () => {
    await openTasks();
    expect([...rowFor('Problem set 5').querySelectorAll('.heat-td-when')][0].textContent).toBe('Tomorrow');
    await click(button(rig, /^Done/));
    expect(titles()).toEqual(['Read the syllabus']);
    expect($(rig, '.heat-td-due')!.textContent).toBe('Done Oct 6');
  });

  it('indents a subtask under its parent behind a disclosure triangle, and marks a recurring task ↻', async () => {
    await openTasks();
    const order = titles();
    expect(order.slice(order.indexOf('Release EP v1'), order.indexOf('Release EP v1') + 3)).toEqual([
      'Release EP v1',
      'Master the tracks',
      'Finish the cover art',
    ]);
    expect(rowFor('Master the tracks').getAttribute('data-depth')).toBe('1');
    expect(rowFor('Listening practice').querySelector('.heat-repeat')!.textContent).toBe('↻');
    expect(rowFor('Order PCBs').querySelector('.heat-source')!.textContent).toBe('Claude');
    await click(rowFor('Release EP v1').querySelector('.heat-disclose'));
    expect(titles()).not.toContain('Master the tracks');
    expect(titles()).toContain('Release EP v1');
    // The parent's estimate is the sum of its open children: 90m and 60m.
    expect(rowFor('Release EP v1').querySelector('.heat-td-time')!.textContent).toBe('2h 30m');
  });

  it('filters the list by words, on ⌘F', async () => {
    await openTasks();
    expect(await press(rig, 'f', { metaKey: true, ctrlKey: true })).toBe(true);
    expect(document.activeElement).toBe($(rig, '.heat-filter'));
    await type($(rig, '.heat-filter'), 'jpn quiz');
    expect(titles()).toEqual(['Grammar quiz 4']);
    await escape(rig);
    expect(document.activeElement).not.toBe($(rig, '.heat-filter'));
  });

  it('shows the group a sidebar row names, and a bead on the timeline does the same for a milestone', async () => {
    await openTasks();
    await click(button(rig, /^JPN 201/));
    expect(titles()).toEqual(['Kanji worksheet 7', 'Listening practice', 'Grammar quiz 4']);
    await click(button(rig, /^WWAV/));
    await click(button(rig, /^All open/));
    const bead = $(rig, '.heat-bead')!;
    expect(bead.getAttribute('aria-label')).toBe('EP v1 mixed, Oct 21');
    await click(bead);
    expect($(rig, '.heat-tasks-head h1')!.textContent).toBe('EP v1 mixed');
    expect(titles()).toEqual(['Mix the second verse']);
  });

  it('says what to do when there is nothing to rank', async () => {
    rig = await mountHeat({ empty: true });
    await rig.client.put('space', {
      name: 'WWAV',
      hue: 6,
      groupKind: 'milestone',
      groupLabel: 'Milestone',
      types: ['Music'],
      persona: '',
    });
    await settle();
    await click(tab('Tasks'));
    await click(button(rig, /^WWAV/));
    expect($(rig, '.heat-tasks .heat-empty')!.textContent).toBe('Add your first WWAV task and Learn will rank it.');
    await click(button(rig, /^All/));
    expect($(rig, '.heat-tasks .heat-empty')!.textContent).toBe('Add your first task and Learn will rank it.');
  });

  it('says Nothing here right now for an empty list when other tasks exist', async () => {
    await openTasks();
    await click(button(rig, /^Done/));
    await rig.client.delete('task', 't-syllabus');
    await settle();
    expect($(rig, '.heat-tasks .heat-empty')!.textContent).toBe('Nothing here right now.');
  });
});

describe('Tasks’ keys', () => {
  it('moves the selection on ↑ ↓ and opens Get Info on it', async () => {
    await openTasks();
    await press(rig, 'ArrowDown');
    expect(rowFor('Kanji worksheet 7').getAttribute('aria-selected')).toBe('true');
    expect($(rig, '.heat-info')).not.toBeNull();
    await press(rig, 'ArrowDown');
    expect(rowFor('Listening practice').getAttribute('aria-selected')).toBe('true');
    await press(rig, 'ArrowUp');
    await press(rig, 'ArrowUp');
    expect(rowFor('Kanji worksheet 7').getAttribute('aria-selected')).toBe('true');
    await escape(rig);
    expect($(rig, '.heat-info')).toBeNull();
    expect($$(rig, '[role="row"][aria-selected="true"]')).toHaveLength(0);
  });

  it('checks a task off with the tick, and ⌘↩ marks the selection done with its Undo', async () => {
    await openTasks();
    await click(rowFor('Renew passport'));
    await press(rig, 'Enter', { metaKey: true, ctrlKey: true });
    // No time was logged, so it asks first.
    expect($(rig, '[aria-label="Time it took"]')).not.toBeNull();
    await click($$(rig, '[aria-label="Time it took"] button').find((b) => b.textContent === 'Skip'));
    expect(rig.fake.store.task.get('t-passport')!.done).toBe(true);
    expect(rig.status().count).toBe('Done.');
    expect(await undoLabel()).toBe('Undo mark done');
  });

  it('ticks a recurring task’s next occurrence and leaves the series open', async () => {
    await openTasks();
    await click(rowFor('Listening practice').querySelector('.heat-tick'));
    expect(rig.fake.store.task.get('t-listen')!.done).toBe(false);
    expect([...rig.fake.store.taskOccurrence.values()].map((o) => [o.taskId, o.date])).toEqual([
      ['t-listen', '2026-10-07'],
    ]);
    expect(await undoLabel()).toBe('Undo mark done');
    // The tick shows today's occurrence done; ticking again unticks it.
    expect(rowFor('Listening practice').querySelector('.heat-tick')!.getAttribute('aria-checked')).toBe('true');
    await click(rowFor('Listening practice').querySelector('.heat-tick'));
    expect(rig.fake.store.taskOccurrence.size).toBe(0);
  });

  it('deletes on ⌫ with Undo', async () => {
    await openTasks();
    await click(rowFor('Call the dentist'));
    await press(rig, 'Backspace');
    expect(titles()).not.toContain('Call the dentist');
    expect(await undoLabel()).toBe('Undo delete task');
    await rig.call('history.undo', { room: 'heat' });
    await settle();
    expect(titles()).toContain('Call the dentist');
  });

  it('plans the selected task with P, from the list as from the column', async () => {
    await openTasks();
    await click(rowFor('Renew passport'));
    await press(rig, 'p');
    expect([...rig.fake.store.timeBlock.values()].find((b) => b.taskId === 't-passport')).toMatchObject({
      start: 11 * 60 + 15,
      minutes: 45,
    });
  });

  it('opens on the task the strip or ⌘K named, switching list if it has to', async () => {
    rig = await mountHeat({ open: { id: 't-syllabus', n: 1 } });
    expect($(rig, '[role="tab"][aria-selected="true"]')!.textContent).toBe('Tasks');
    expect($(rig, '.heat-tasks-head h1')!.textContent).toBe('Done');
    expect(rowFor('Read the syllabus').getAttribute('aria-selected')).toBe('true');
  });

  it('carries a row’s task id in a drag', async () => {
    await openTasks();
    const carried: Record<string, string> = {};
    const e = Object.assign(new Event('dragstart', { bubbles: true }), {
      dataTransfer: { setData: (t: string, v: string) => (carried[t] = v), effectAllowed: '' },
    });
    await act(async () => void rowFor('Grammar quiz 4').dispatchEvent(e));
    expect(carried['application/x-heat-task']).toBe('t-quiz4');
  });
});

describe('linking by drop', () => {
  const dropOn = async (el: Element | undefined, id: string) => {
    const e = Object.assign(new Event('drop', { bubbles: true, cancelable: true }), {
      dataTransfer: {
        types: ['application/x-heat-task'],
        getData: (t: string) => (t === 'application/x-heat-task' ? id : ''),
      },
    });
    await act(async () => {
      el!.dispatchEvent(e);
      await new Promise((r) => setTimeout(r, 0));
    });
    await settle();
  };

  it('links a task dropped on a milestone, a project or a course in the sidebar', async () => {
    await openTasks();
    await dropOn(button(rig, /^EP v1 mixed/), 't-passport');
    expect(task('t-passport').milestoneId).toBe('ms-mixed');
    expect(rig.status().count).toBe('Linked ‘Renew passport’ to EP v1 mixed.');
    expect(await undoLabel()).toBe('Undo edit task');
    await dropOn(button(rig, /^MTH 142/), 't-dentist');
    expect(task('t-dentist').courseId).toBe('c-mth142');
    await dropOn(button(rig, /^EP4/), 't-dentist');
    expect(task('t-dentist').projectId).toBe('proj-ep');
  });

  it('links a task dropped on a bead of the timeline', async () => {
    await openTasks();
    await click(button(rig, /^WWAV/));
    await dropOn($(rig, '.heat-bead') ?? undefined, 't-passport');
    expect(task('t-passport').milestoneId).toBe('ms-mixed');
  });
});

describe('"+" and Triage inbox', () => {
  it('adds a task from a sheet that asks for a name first, and keeps what you typed when Esc closes it', async () => {
    await openTasks();
    await click($(rig, '.heat-plus'));
    expect($(rig, '[role="dialog"][aria-label="New task"]')).not.toBeNull();
    await click($$(rig, '[role="dialog"][aria-label="New task"] button').find((b) => b.textContent === 'Add task'));
    expect($(rig, '#heat-new-why')!.textContent).toBe('Give the task a name first.');
    await type($(rig, '[role="dialog"][aria-label="New task"] input'), 'Draft the liner notes');
    await escape(rig);
    expect($(rig, '[role="dialog"][aria-label="New task"]')).toBeNull();
    // Reopened, the draft is still there.
    await press(rig, 'n');
    expect(($(rig, '[role="dialog"][aria-label="New task"] input') as HTMLInputElement).value).toBe(
      'Draft the liner notes',
    );
    await click($$(rig, '[role="dialog"][aria-label="New task"] button').find((b) => b.textContent === 'Add task'));
    const made = [...rig.fake.store.task.values()].find((t) => t.title === 'Draft the liner notes')!;
    expect(made).toMatchObject({ spaceId: 'sp-classes', difficulty: 3, public: false });
    expect(await undoLabel()).toBe('Undo add task');
    expect(titles()).toContain('Draft the liner notes');
  });

  it('adds into the space that is chosen', async () => {
    await openTasks();
    await click(button(rig, /^Personal/));
    await click($(rig, '.heat-plus'));
    await type($(rig, '[role="dialog"][aria-label="New task"] input'), 'Buy stamps');
    await click($$(rig, '[role="dialog"][aria-label="New task"] button').find((b) => b.textContent === 'Add task'));
    expect([...rig.fake.store.task.values()].find((t) => t.title === 'Buy stamps')!.spaceId).toBe('sp-personal');
  });

  it('offers Triage inbox only while the inbox has items, and says so in the status bar', async () => {
    await openTasks();
    expect(rig.status().act).toBe('Triage inbox');
    expect(button(rig, 'Triage inbox')).toBeTruthy();
    await click(button(rig, 'Triage inbox'));
    expect($$(rig, '.heat-inbox [role="option"]').map((r) => r.querySelector('.heat-row-title')!.textContent)).toEqual([
      'fix the snare at 1:32',
      'email Prof. Tanaka about the extension',
      'https://example.com/why-tape-saturation-works',
    ]);
    for (const id of ['cap-1', 'cap-2', 'cap-3'])
      rig.fake.store.capture.set(id, { ...rig.fake.store.capture.get(id)!, triagedAt: 1 });
    rig.fake.emit(['capture']);
    await settle();
    expect(rig.status().act).toBeNull();
    expect($(rig, '.heat-inbox')).toBeNull();
    expect($(rig, '.heat-empty')!.textContent).toBe('Inbox zero ✓');
  });

  it('turns a capture into a task on Return, and opens it for editing', async () => {
    await openTasks();
    await act(async () => rig.view.current!.secondary());
    await settle();
    await press(rig, 'ArrowDown');
    expect($(rig, '.heat-inbox [aria-selected="true"]')!.textContent).toContain('email Prof. Tanaka');
    await press(rig, 'Enter');
    const made = [...rig.fake.store.task.values()].find((t) => t.title === 'email Prof. Tanaka about the extension')!;
    expect(made).toMatchObject({ source: 'capture' });
    expect(rig.fake.store.capture.get('cap-2')).toMatchObject({ resultType: 'task', resultId: made.id });
    expect(rowFor('email Prof. Tanaka about the extension').getAttribute('aria-selected')).toBe('true');
    expect(await undoLabel()).toBe('Undo triage capture');
  });

  it('turns a capture into a note, a project or an upload with its own button', async () => {
    await openTasks();
    await click(button(rig, 'Triage inbox'));
    await click(button(rig, /^Make a note: fix the snare/));
    expect([...rig.fake.store.note.values()].map((n) => n.markdown)).toEqual(['fix the snare at 1:32']);
    await click(button(rig, /^Make a project: email Prof/));
    expect([...rig.fake.store.project.values()].map((p) => p.title)).toContain(
      'email Prof. Tanaka about the extension',
    );
    await click(button(rig, /^Make an upload: https/));
    expect(rig.fake.store.capture.get('cap-3')).toMatchObject({ resultType: 'upload' });
    expect($(rig, '.heat-empty')!.textContent).toBe('Inbox zero ✓');
  });
});

describe('the list helpers', () => {
  it('nests children under a parent that is in the list, and leaves a child whose parent is not', () => {
    const parents: Record<string, string> = { b: 'a', c: 'a', e: 'zzz' };
    const rows = nest(['a', 'b', 'c', 'd', 'e'], (id) => parents[id], new Set());
    expect(rows.map((r) => [r.id, r.depth, r.children])).toEqual([
      ['a', 0, 2],
      ['b', 1, 0],
      ['c', 1, 0],
      ['d', 0, 0],
      ['e', 0, 0],
    ]);
    expect(nest(['a', 'b', 'c'], (id) => parents[id], new Set(['a'])).map((r) => r.id)).toEqual(['a']);
  });

  it('walks a loop of parents once and loses nothing', () => {
    const parents: Record<string, string> = { a: 'b', b: 'a', c: 'c' };
    expect(
      nest(['a', 'b', 'c'], (id) => parents[id], new Set())
        .map((r) => r.id)
        .sort(),
    ).toEqual(['a', 'b', 'c']);
  });

  it('matches a filter on every word, in any field', () => {
    expect(matchesFilter('jpn quiz', 'Grammar quiz 4', 'Quiz', 'JPN 201')).toBe(true);
    expect(matchesFilter('jpn essay', 'Grammar quiz 4', 'Quiz', 'JPN 201')).toBe(false);
    expect(matchesFilter('', 'x')).toBe(true);
  });
});
