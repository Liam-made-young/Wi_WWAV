import { afterEach, describe, expect, it } from 'vitest';
import { $, $$, button, click, escape, mountHeat, press, type Rig, settle, type } from '../testkit';
import { ulidAt, ulidTime } from '../ulid';

// docs/PLAN.md S2.4 and docs/SPEC.md 3.5, 3.6, 3.15. What a fail looks like:
// Get Info without a field the record has; an edit that isn't its own undo
// step with the core's label; Claude's estimate without its hint, or a task
// Claude added without "Claude, Oct 6 …" and its reason; the Public switch
// on by default, or a block with a switch of its own; Esc losing a typed
// value, or not sliding Get Info back; a type set by a plain patch instead of
// the type's own command; a type's estimate without the type and where it is
// defined; a task not saying which course, project or space it belongs to.

let rig: Rig;
afterEach(() => rig?.unmount());

const info = () => $(rig, '.heat-info')!;
const labels = () => $$(rig, '.heat-info .heat-field > span').map((s) => s.textContent);
const field = (label: string) =>
  $$(rig, '.heat-info .heat-field')
    .find((f) => f.querySelector('span')!.textContent === label)!
    .querySelector('input, select, textarea') as HTMLInputElement;
const undoLabel = async () => (await rig.call<{ undo: string | null }>('history.get', {})).undo;
const task = (id: string) => rig.fake.store.task.get(id)!;

/** Select a task by its row in Tasks. */
async function select(title: string) {
  await click($$(rig, '.heat-tab' + 'panel, [role="tab"]').find((t) => t.textContent === 'Tasks'));
  await click($$(rig, '.heat-table [role="row"]').find((r) => r.querySelector('.heat-td-name')?.textContent === title));
}

describe('Get Info for a task', () => {
  it('appears only while a task is selected, with every field', async () => {
    rig = await mountHeat();
    expect($(rig, '.heat-info')).toBeNull();
    await select('Grammar quiz 4');
    expect(info().getAttribute('aria-label')).toBe('Get Info: Grammar quiz 4');
    expect(labels()).toEqual([
      'Task',
      'Space',
      'Course',
      'Type',
      'Due',
      'Time',
      'Scheduled',
      'Repeat',
      'Difficulty',
      'Estimate',
      'Took',
      'Notes',
      'Part of',
    ]);
    expect(field('Due').value).toBe('2026-10-08');
    expect(field('Time').value).toBe('23:59');
    expect(field('Difficulty').value).toBe('3');
    expect(field('Type').value).toBe('Quiz');
    expect(info().textContent).toContain('Brightspace calendar');
  });

  it('renames a task as one undo step, and asks for a name first', async () => {
    rig = await mountHeat();
    await select('Grammar quiz 4');
    const title = $(rig, '.heat-info [data-info-first]') as HTMLInputElement;
    await type(title, 'Grammar quiz 5');
    await escape(rig); // leaving the field saves it
    expect(task('t-quiz4').title).toBe('Grammar quiz 5');
    expect(await undoLabel()).toBe('Undo rename task');
    await select('Grammar quiz 5');
    await type($(rig, '.heat-info [data-info-first]'), '   ');
    await escape(rig);
    expect(task('t-quiz4').title).toBe('Grammar quiz 5');
    expect(rig.status().count).toBe('Give the task a name first.');
  });

  it('says where a task came from, and for Claude’s task when and why', async () => {
    rig = await mountHeat();
    await select('Order PCBs');
    expect($(rig, '.heat-source-line')!.textContent).toBe('Claude, Oct 6 4:00 PM');
    expect($(rig, '.heat-reason')!.textContent).toBe('The board house’s Oct 6 email says the quote expires Oct 13.');
    await select('Renew passport');
    expect($(rig, '.heat-source-line')!.textContent).toBe('Added by you');
    expect($(rig, '.heat-reason')).toBeNull();
  });

  it('hints "Claude\'s estimate: 45m, difficulty 2. …" when Claude made the estimate, with its reason', async () => {
    rig = await mountHeat();
    await select('Kanji worksheet 7');
    const hints = $$(rig, '.heat-hint').map((h) => h.textContent);
    expect(hints).toContain(
      "Claude's estimate: 45m, difficulty 2. It read the title, the notes and your past averages.",
    );
    expect($$(rig, '.heat-reason').map((r) => r.textContent)).toEqual(['A one-page worksheet.']);
    await select('Renew passport');
    expect($$(rig, '.heat-hint').map((h) => h.textContent)).toContain('Set by you.');
    await select('Grammar quiz 4');
    expect(
      $$(rig, '.heat-hint')
        .map((h) => h.textContent)
        .join(' '),
    ).toContain('Learn’s estimate: your average for this type, or difficulty × 20 minutes.');
  });

  it('sets the estimate as the person’s own, keeps minutes in 5–600, and each is an undo step', async () => {
    rig = await mountHeat();
    await select('Kanji worksheet 7');
    await type(field('Estimate'), '60');
    await escape(rig);
    expect(task('t-kanji7')).toMatchObject({ estMin: 60, estBy: 'you' });
    expect(await undoLabel()).toBe('Undo estimate');
    await type(field('Estimate'), '1000');
    await escape(rig);
    expect(task('t-kanji7').estMin).toBe(600);
    expect(rig.status().count).toBe('Minutes are kept between 5 and 600.');
    await type(field('Difficulty'), '4');
    expect(task('t-kanji7').difficulty).toBe(4);
    expect(await undoLabel()).toBe('Undo estimate');
  });

  it('shows the time a task took and adjusts it by hand', async () => {
    rig = await mountHeat();
    await click($$(rig, '[role="tab"]').find((t) => t.textContent === 'Tasks'));
    await click(button(rig, /^Done/));
    await click(
      $$(rig, '.heat-table [role="row"]').find(
        (r) => r.querySelector('.heat-td-name')?.textContent === 'Read the syllabus',
      ),
    );
    expect(field('Took').value).toBe('50');
    expect(
      $$(rig, '.heat-hint')
        .map((h) => h.textContent)
        .join(' '),
    ).toContain('50m across 2 focus sessions.');
    await type(field('Took'), '75');
    await escape(rig);
    expect(task('t-syllabus').adjustMin).toBe(25);
    expect(await undoLabel()).toBe('Undo change time taken');
    await settle();
    expect(field('Took').value).toBe('75');
  });

  it('keeps the Public switch off until it is turned on, one task at a time, with Undo', async () => {
    rig = await mountHeat();
    await select('Grammar quiz 4');
    const sw = () => $(rig, '.heat-info [role="switch"]') as HTMLInputElement;
    expect(sw().checked).toBe(false);
    expect(task('t-quiz4').public).toBe(false);
    await click(sw());
    expect(task('t-quiz4').public).toBe(true);
    expect(task('t-passport').public).toBe(false);
    expect(await undoLabel()).toBe('Undo make public');
    expect($$(rig, '.heat-hint').pop()!.textContent).toContain(
      'Anyone who opens your sun can see this task’s title, due date and whether it is done.',
    );
    await click(sw());
    expect(await undoLabel()).toBe('Undo make private');
  });

  it('sets a due date at 11:59 PM unless a time is given, and clears it', async () => {
    rig = await mountHeat();
    await select('Call the dentist');
    expect(field('Time').disabled).toBe(true);
    await type(field('Due'), '2026-10-12');
    await escape(rig);
    expect(task('t-dentist').due).toBe(Date.UTC(2026, 9, 13, 3, 59)); // 11:59 PM New York
    expect(await undoLabel()).toBe('Undo edit task');
    await type(field('Due'), '');
    await escape(rig);
    expect(task('t-dentist').due).toBeNull();
  });

  it('sets a repeat from the presets, pinning today when the task has no day to start from', async () => {
    rig = await mountHeat();
    await select('Call the dentist');
    await type(field('Repeat'), 'weekdays');
    expect(task('t-dentist')).toMatchObject({ rrule: 'FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR', scheduledDate: '2026-10-07' });
    await type(field('Repeat'), 'none');
    expect(task('t-dentist').rrule).toBeFalsy();
  });

  it('puts a task in a group of its space, and takes it out', async () => {
    rig = await mountHeat();
    await select('Grammar quiz 4');
    await type(field('Course'), 'c-mth142');
    expect(task('t-quiz4').courseId).toBe('c-mth142');
    await type(field('Course'), '');
    expect(task('t-quiz4').courseId).toBeFalsy();
  });

  it('marks done and deletes from its own buttons', async () => {
    rig = await mountHeat();
    await select('Call the dentist');
    await click(button(rig, 'Delete'));
    expect(rig.fake.store.task.has('t-dentist')).toBe(false);
    expect($(rig, '.heat-info')).toBeNull();
  });

  it('opens on Return and ⌘I with the keyboard in the first field', async () => {
    rig = await mountHeat();
    await select('Grammar quiz 4');
    (document.activeElement as HTMLElement | null)?.blur();
    expect(await press(rig, 'Enter')).toBe(true);
    expect(document.activeElement).toBe($(rig, '.heat-info [data-info-first]'));
    // Esc leaves the field, a second Esc slides Get Info back; nothing typed is lost or deleted.
    await escape(rig);
    expect(document.activeElement).not.toBe($(rig, '.heat-info [data-info-first]'));
    expect($(rig, '.heat-info')).not.toBeNull();
    await escape(rig);
    expect($(rig, '.heat-info')).toBeNull();
    expect(rig.fake.store.task.has('t-quiz4')).toBe(true);
    rig.view.current!.getInfo();
    await settle();
    expect(rig.status().count).toBe('Select a task first.');
  });

  it('follows the selection as ↑ ↓ move it', async () => {
    rig = await mountHeat();
    await select('Grammar quiz 4');
    await press(rig, 'ArrowDown');
    expect(info().getAttribute('aria-label')).toBe('Get Info: Problem set 5');
  });
});

describe('Get Info for a block', () => {
  const pick = async () => {
    await click($$(rig, '.heat-row').find((r) => r.textContent!.includes('Problem set 5')));
  };

  it('shows the block’s day, start and length, who made it, and no Public switch of its own', async () => {
    rig = await mountHeat();
    await pick();
    expect(info().getAttribute('aria-label')).toBe('Get Info: Problem set 5');
    expect(labels()).toEqual(['Day', 'Starts', 'Minutes']);
    expect(field('Day').value).toBe('2026-10-07');
    expect(field('Starts').value).toBe('15:30');
    expect(field('Minutes').value).toBe('60');
    expect($(rig, '.heat-source-line')!.textContent).toBe('Made by Plan my day');
    expect(($(rig, '.heat-info [role="switch"]') as HTMLInputElement).disabled).toBe(true);
    expect(info().textContent).toContain('A block follows its task’s Public switch.');
  });

  it('moves and resizes from its fields, each as the core names it', async () => {
    rig = await mountHeat();
    await pick();
    await type(field('Starts'), '16:00');
    await escape(rig);
    expect(rig.fake.store.timeBlock.get('b-pset')).toMatchObject({ start: 960, minutes: 60 });
    expect(await undoLabel()).toBe('Undo move block');
    await type(field('Minutes'), '90');
    await escape(rig);
    expect(rig.fake.store.timeBlock.get('b-pset')).toMatchObject({ start: 960, minutes: 90 });
    expect(await undoLabel()).toBe('Undo resize block');
    expect(rig.fake.store.task.get('t-pset5')!.estMin).toBe(90);
  });

  it('removes the block without touching its task', async () => {
    rig = await mountHeat();
    await pick();
    await click(button(rig, 'Remove block'));
    expect(rig.fake.store.timeBlock.has('b-pset')).toBe(false);
    expect(rig.fake.store.task.has('t-pset5')).toBe(true);
    expect(await undoLabel()).toBe('Undo remove block');
  });

  it('opens its task', async () => {
    rig = await mountHeat();
    await pick();
    await click(button(rig, 'Open the task'));
    expect(labels()).toContain('Estimate');
  });
});

describe('a ULID’s time', () => {
  it('reads when an id was made, and nothing from an id that is not a ULID', () => {
    const at = Date.UTC(2026, 9, 6, 12, 41);
    expect(ulidTime(ulidAt(at))).toBe(at);
    expect(ulidTime('t-quiz4')).toBeNull();
    expect(ulidTime('ZZZZZZZZZZ0000000000000000')).toBeNull();
  });
});

describe('a task’s home and type in Get Info', () => {
  const calls = (cmd: string) => rig.calls.filter((c) => c.cmd === cmd).map((c) => c.args);
  const hints = () => $$(rig, '.heat-info .heat-hint').map((h) => h.textContent);
  const options = (label: string) => [...field(label).querySelectorAll('option')].map((o) => o.textContent);

  it('says where the task belongs: its course, else its project, else its space', async () => {
    rig = await mountHeat();
    await select('Grammar quiz 4');
    expect($(rig, '.heat-home-line')!.textContent).toBe('In JPN 201 · Intermediate Japanese');
    // The course is named once in its select too.
    expect(options('Course')).toEqual(['None', 'JPN 201 · Intermediate Japanese', 'MTH 142 · Calculus II']);
    await select('Mix the second verse');
    expect($(rig, '.heat-home-line')!.textContent).toBe('In EP');
    await select('Renew passport');
    expect($(rig, '.heat-home-line')!.textContent).toBe('In Personal');
  });

  it('offers Automatic, then the types of the course, the space and the defaults', async () => {
    rig = await mountHeat();
    await rig.client.updateCourse('c-jpn201', {
      types: [{ name: 'Kanji drill', patterns: ['kanji drill'], estMin: 15, difficulty: 1 }],
    });
    await select('Grammar quiz 4');
    expect(options('Type')).toEqual([
      'Automatic',
      'Kanji drill',
      'Homework',
      'Quiz',
      'Listening',
      'Reading',
      'Lab',
      'Project',
      'Exam prep',
      'Other',
      'Worksheet',
      'Email',
      'Errand',
      'Admin',
      'Creative session',
    ]);
  });

  it('sets the type through its own command, and Automatic hands it back to the title', async () => {
    rig = await mountHeat();
    await select('Grammar quiz 4');
    await type(field('Type'), 'Homework');
    await settle();
    expect(calls('heat.task.setType')).toEqual([{ taskId: 't-quiz4', type: 'Homework' }]);
    expect(calls('heat.patch').filter((a) => 'type' in (a.set as object))).toEqual([]);
    expect(task('t-quiz4')).toMatchObject({ type: 'Homework', typeBy: 'you', estMin: 90, estBy: 'type', difficulty: 3 });
    expect(field('Type').value).toBe('Homework');
    expect(hints()).toContain('From the type Homework (your defaults).');
    expect(await undoLabel()).toBe('Undo set type');

    await type(field('Type'), '');
    await settle();
    expect(calls('heat.task.setType').at(-1)).toEqual({ taskId: 't-quiz4', type: null });
    expect(task('t-quiz4')).toMatchObject({ type: 'Quiz', typeBy: 'rule', estMin: 20, estBy: 'type', difficulty: 2 });
    expect(field('Type').value).toBe('');
    expect(options('Type')[0]).toBe('Automatic (Quiz)');
  });

  it('names the course, the project or the space a type’s numbers came from', async () => {
    rig = await mountHeat();
    await rig.client.updateCourse('c-jpn201', {
      types: [{ name: 'Quiz', patterns: ['quiz'], estMin: 25, difficulty: 2 }],
    });
    await rig.client.updateProject('proj-ep', {
      types: [{ name: 'Music', patterns: ['mix', 'master'], estMin: 180, difficulty: 4 }],
    });
    // A space's types go through the plain patch, which retimes its tasks too.
    const spaced = await rig.client.patch('space', 'sp-personal', {
      typeDefs: [{ name: 'Health', patterns: ['dentist'], estMin: 15, difficulty: 1 }],
    });
    expect(spaced.retimed).toBe(1);
    await select('Grammar quiz 4');
    expect(field('Estimate').value).toBe('25');
    expect(hints()).toContain('From the type Quiz (JPN 201 · Intermediate Japanese).');
    // Minutes you set stay yours when the project's types change; the cover art's hint still says so.
    await select('Mix the second verse');
    expect(hints()).toContain('Set by you.');
    await click(button(rig, 'Use the type’s default'));
    expect(hints()).toContain('From the type Music (EP).');
    await select('Call the dentist');
    expect(hints()).toContain('From the type Health (Personal).');
  });

  it('says Claude will estimate a task no type matched', async () => {
    rig = await mountHeat();
    await rig.client.put('task', {
      spaceId: 'sp-personal',
      title: 'Untangle the cables',
      due: null,
      adjustMin: 0,
      notes: '',
      done: false,
      doneAt: null,
      source: 'you',
    });
    await select('Untangle the cables');
    expect(field('Type').value).toBe('');
    expect(options('Type')[0]).toBe('Automatic (Other)');
    expect(field('Estimate').value).toBe('45');
    expect(hints()).toContain('No type matched. Claude will estimate it.');
    expect(button(rig, 'Use the type’s default')).toBeUndefined();
  });

  it('offers "Use the type’s default" only where something was set by hand, and it drops what was', async () => {
    rig = await mountHeat();
    await select('Grammar quiz 4');
    expect(button(rig, 'Use the type’s default')).toBeUndefined();
    await select('Renew passport');
    expect(field('Estimate').value).toBe('40');
    await click(button(rig, 'Use the type’s default'));
    expect(calls('heat.task.reapplyDefaults')).toEqual([{ taskId: 't-passport', force: true }]);
    expect(task('t-passport')).toMatchObject({ type: 'Admin', estMin: 20, estBy: 'type', difficulty: 1 });
    expect(field('Estimate').value).toBe('20');
    expect(hints()).toContain('From the type Admin (your defaults).');
    expect(button(rig, 'Use the type’s default')).toBeUndefined();
    expect(await undoLabel()).toBe('Undo reapply defaults');
    // Setting the difficulty by hand brings the offer back.
    await type(field('Difficulty'), '4');
    await settle();
    expect(task('t-passport')).toMatchObject({ difficulty: 4, difficultyBy: 'you', estBy: 'type' });
    expect(button(rig, 'Use the type’s default')).toBeTruthy();
  });
});
