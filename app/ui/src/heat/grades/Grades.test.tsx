import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { keys } from '../../shell/platform';
import type { Grade } from '../client';
import { draftsOf, finishReading, holdReading } from '../fake/syllabus';
import { $, $$, button, byRole, click, escape, mountHeat, press, type Rig, settle, type } from '../testkit';

// docs/SPEC.md 3.8 and docs/PLAN.md S2.4. What a fail looks like: a course
// without its percentage, letter pill and "Based on X% of the course so far";
// weights that don't add up with no sentence; a pending grade without its
// yellow "Enter score" banner, or one scored by anything but typing the
// score; "What it would take" worked out in the view instead of asked of the
// core; a Public switch that starts on, or a grade's switch without the
// sentence that says what it does; a course's name written twice; a course
// with no weights saying what it would take instead of asking for its
// syllabus; a syllabus written into the library before Accept, or one that
// takes more than one ⌘Z to take back.

let rig: Rig;
afterEach(() => rig?.unmount());

async function open() {
  rig = await mountHeat();
  await click(button(rig, 'Grades'));
}
const card = (code: string) => $(rig, `.heat-course[aria-label^="${code}"]`)!;
const calls = (cmd: string) => rig.calls.filter((c) => c.cmd === cmd);
const text = (el: Element | null | undefined) => (el?.textContent ?? '').replace(/\s+/g, ' ').trim();
const history = async () => (await rig.call<{ undo: string | null }>('history.get', {})).undo;

describe('Grades', () => {
  it('reads the term for its header and gives each course its percentage, a letter pill and what it rests on', async () => {
    await open();
    expect($(rig, '.heat-grades h1')!.textContent).toBe('Fall 2026 grades');
    const jpn = card('JPN 201');
    expect(text($(jpn, '.heat-course-pct'))).toBe('87.1%');
    expect(text($(jpn, '.heat-letter'))).toBe('B+');
    expect($(jpn, '.heat-letter')!.getAttribute('data-tone')).toBe('blue');
    expect(text($(jpn, '.heat-course-based'))).toBe('Based on 60% of the course so far');
    const mth = card('MTH 142');
    expect(text($(mth, '.heat-course-pct'))).toBe('82%');
    expect(text($(mth, '.heat-letter'))).toBe('B-');
    expect(text($(mth, '.heat-course-based'))).toBe('Based on 30% of the course so far');
    // Weights that add to 100 say nothing.
    expect($(jpn, '.heat-course-weights')).toBeNull();
  });

  it('shows the items by category, each with its weight and where it stands', async () => {
    await open();
    const cats = $$(card('JPN 201'), '.heat-cat').map((c) => text($(c, '.heat-cat-head')));
    expect(cats).toEqual([
      'Homework25% of the course90% so far',
      'Quizzes35% of the course85% so far',
      'Exams40% of the courseNot graded yet',
    ]);
    const quizzes = $$(card('JPN 201'), '.heat-cat')[1];
    expect($$(quizzes, '.heat-grade').map((g) => text(g))).toEqual(['Grammar quiz 317 / 20']);
    // An empty category says so; the waiting grade is in its banner, not in the list.
    expect(text($$(card('JPN 201'), '.heat-cat')[2])).toContain('No grades here yet.');
  });

  it('says when the weights don’t add up', async () => {
    await open();
    await rig.client.patch('course', 'c-jpn201', {
      categories: [
        { id: 'cat-hw', name: 'Homework', weight: 25, keywords: [] },
        { id: 'cat-quiz', name: 'Quizzes', weight: 35, keywords: [] },
        { id: 'cat-exam', name: 'Exams', weight: 35, keywords: [] },
      ],
    });
    await settle();
    expect(text($(card('JPN 201'), '.heat-course-weights'))).toBe('Weights add to 95%. The other 5% is unassigned.');
  });

  it('asks the core what it would take, and says when the target is out of reach', async () => {
    await open();
    const jpn = card('JPN 201');
    // It aims one step above where the course stands: B+ now, so A-.
    expect(($(jpn, '.heat-take select') as HTMLSelectElement).value).toBe('A-');
    expect(text($(jpn, '.heat-take-text'))).toBe('To finish with an A- (90%), you need 94.4% on the remaining 40%.');
    expect(
      calls('heat.whatItWouldTake')
        .filter((c) => (c.args as { courseId: string }).courseId === 'c-jpn201')
        .at(-1)!.args,
    ).toEqual({
      courseId: 'c-jpn201',
      letter: 'A-',
    });
    await type($(jpn, '.heat-take select'), 'A');
    expect(text($(jpn, '.heat-take-text'))).toBe('An A is out of reach; the highest possible is 92.2% (A-).');
    await type($(jpn, '.heat-take select'), 'C');
    expect(text($(jpn, '.heat-take-text'))).toBe('To finish with a C (73%), you need 51.9% on the remaining 40%.');
    await type($(jpn, '.heat-take select'), 'F');
    expect(text($(jpn, '.heat-take-text'))).toBe('You keep an F even with 0% on the remaining 40%.');
  });
});

describe('a grade waiting for its score', () => {
  const banner = () => $(rig, '.heat-sticky')!;

  it('keeps its yellow banner with "Enter score", and the header says one is waiting', async () => {
    await open();
    expect(text(banner())).toContain('New grade posted · Quizzes');
    expect(text(banner())).toContain('Kanji quiz 3');
    expect(button(rig, 'Enter score: Kanji quiz 3')).toBeTruthy();
    expect($(rig, '.heat-grades-head .heat-subtitle')!.textContent).toBe('1 new grade to enter');
    expect(button(rig, 'Open in Brightspace')).toBeTruthy();
  });

  it('is scored by typing the score, and the percentages move', async () => {
    await open();
    await click(button(rig, 'Enter score: Kanji quiz 3'));
    const score = $(rig, 'input[aria-label="Score for Kanji quiz 3"]') as HTMLInputElement;
    expect(document.activeElement).toBe(score);
    await type(score, '88');
    await click(button(rig, 'Save'));
    expect(calls('heat.score').map((c) => c.args)).toEqual([{ gradeId: 'g-3', score: 88 }]);
    // Typed at the 100 it started with, so the "out of" wasn't touched.
    expect(calls('heat.patch').filter((c) => (c.args as { kind: string }).kind === 'grade')).toEqual([]);
    expect($(rig, '.heat-sticky')).toBeNull();
    const quizzes = $$(card('JPN 201'), '.heat-cat')[1];
    expect($$(quizzes, '.heat-grade').map((g) => text(g))).toEqual(['Grammar quiz 317 / 20', 'Kanji quiz 388 / 100']);
    expect(rig.status().count).toBe('Kanji quiz 3: 88 out of 100.');
    expect(await history()).toBe('Undo enter score');
  });

  it('takes the real "out of" with the score, and refuses what isn’t a number', async () => {
    await open();
    await click(button(rig, 'Enter score: Kanji quiz 3'));
    await click(button(rig, 'Save'));
    expect(text($(rig, '.heat-sticky-why'))).toBe('Type the score as a number.');
    await type($(rig, 'input[aria-label="Score for Kanji quiz 3"]'), '41');
    await type($(rig, 'input[aria-label="Kanji quiz 3 is out of"]'), '0');
    await click(button(rig, 'Save'));
    expect(text($(rig, '.heat-sticky-why'))).toBe('Type what it is out of as a number above 0.');
    expect(calls('heat.score')).toEqual([]);
    await type($(rig, 'input[aria-label="Kanji quiz 3 is out of"]'), '50');
    await click(button(rig, 'Save'));
    expect(calls('heat.patch').at(-1)!.args).toMatchObject({ kind: 'grade', id: 'g-3', set: { outOf: 50 } });
    expect(calls('heat.score').map((c) => c.args)).toEqual([{ gradeId: 'g-3', score: 41 }]);
    expect(rig.fake.store.grade.get('g-3')).toMatchObject({ score: 41, outOf: 50, pending: false });
  });

  it('closes its form on Esc, keeping nothing scored', async () => {
    await open();
    await click(button(rig, 'Enter score: Kanji quiz 3'));
    expect($(rig, '.heat-sticky-form')).toBeTruthy();
    await escape(rig);
    expect($(rig, '.heat-sticky-form')).toBeNull();
    expect($(rig, '.heat-sticky')).toBeTruthy();
    expect(calls('heat.score')).toEqual([]);
  });

  // The test the brief asks for: Claude records the notice and never the number,
  // and nothing in the app slips one in either. Typing into the banner is the way.
  it('can’t be scored by anything but typing', async () => {
    await open();
    const waiting = () => rig.fake.store.grade.get('g-3')!;
    const stillWaiting = () => expect(waiting()).toMatchObject({ pending: true, score: null });
    const sentence = 'A grade waiting for its score takes it through Enter score.';

    // Not a patch, not a put, not a score that isn't a typed number.
    await expect(rig.client.patch('grade', 'g-3', { score: 90 })).rejects.toThrow(sentence);
    await expect(rig.client.patch('grade', 'g-3', { pending: false })).rejects.toThrow(sentence);
    await expect(rig.client.put('grade', { ...(waiting() as Grade), score: 90, pending: false })).rejects.toThrow(
      sentence,
    );
    await expect(rig.client.put('grade', { ...(waiting() as Grade), pending: false })).rejects.toThrow(sentence);
    for (const bad of [Number.NaN, -1, '90' as unknown as number, null as unknown as number]) {
      await expect(rig.client.score('g-3', bad)).rejects.toThrow('Type the score as a number.');
    }
    await expect(
      rig.client.put('grade', {
        courseId: 'c-jpn201',
        categoryId: 'cat-quiz',
        title: 'Sneaky',
        score: 99,
        outOf: 100,
        dropped: false,
        pending: true,
        source: 'claude',
      }),
    ).rejects.toThrow(sentence);
    stillWaiting();

    // The app's own way is the banner: pressing Save with nothing typed scores nothing,
    // and the grade's own sheet has no score field to type into.
    await settle();
    const tried = calls('heat.score').length;
    await click(button(rig, 'Enter score: Kanji quiz 3'));
    await click(button(rig, 'Save'));
    expect(calls('heat.score')).toHaveLength(tried);
    stillWaiting();
    await click(button(rig, 'Cancel'));
    await click(button(rig, 'Edit grade: Kanji quiz 3'));
    const sheet = byRole(rig, 'dialog', 'Edit grade Kanji quiz 3')!;
    expect($$(sheet, '.field span').map((s) => s.textContent)).not.toContain('Score');
    expect(text(sheet)).toContain('This grade is waiting for its score. Type it in the yellow banner on the course.');
    await click($$(sheet, 'button[type="submit"]')[0]);
    stillWaiting();
    expect(calls('heat.score')).toHaveLength(tried);

    // Typing it works, and is its own undo step.
    await click(button(rig, 'Enter score: Kanji quiz 3'));
    await type($(rig, 'input[aria-label="Score for Kanji quiz 3"]'), '77');
    await click(button(rig, 'Save'));
    expect(waiting()).toMatchObject({ pending: false, score: 77 });
    // Once scored it is an ordinary grade, which the person may edit; the banner's path is closed to it.
    await expect(rig.client.patch('grade', 'g-3', { score: 78 })).resolves.toBeTruthy();
    await expect(rig.client.score('g-3', 79)).rejects.toThrow('Only a grade waiting for its score takes one here.');
  });
});

describe('adding and editing', () => {
  it('"+" adds a grade, filed by the course’s keywords when no category is named', async () => {
    await open();
    await click(button(rig, 'New grade'));
    const sheet = byRole(rig, 'dialog', 'Add grade')!;
    await type($(sheet, 'input'), 'Kanji worksheet 8');
    const numbers = $$(sheet, 'input[type="number"]') as HTMLInputElement[];
    await type(numbers[0], '8');
    await type(numbers[1], '10');
    await click(button(rig, 'Add grade'));
    expect(byRole(rig, 'dialog', 'Add grade')).toBeUndefined();
    const put = calls('heat.put').at(-1)!.args as { kind: string; record: Record<string, unknown> };
    expect(put.kind).toBe('grade');
    expect(put.record).toMatchObject({
      courseId: 'c-jpn201',
      title: 'Kanji worksheet 8',
      score: 8,
      outOf: 10,
      pending: false,
    });
    expect(put.record.categoryId).toBeUndefined();
    // The core filed it under Homework by the keyword "worksheet".
    const homework = $$(card('JPN 201'), '.heat-cat')[0];
    expect($$(homework, '.heat-grade').map((g) => text(g))).toEqual([
      'Kanji worksheet 69 / 10',
      'Kanji worksheet 88 / 10',
    ]);
    expect(rig.status().count).toBe('Added Kanji worksheet 8.');
  });

  it('a grade with no score is waiting for one, with its banner', async () => {
    await open();
    await click(button(rig, 'New grade'));
    await type($(byRole(rig, 'dialog', 'Add grade'), 'input'), 'Lab 5a');
    await click(button(rig, 'Add grade'));
    expect(calls('heat.put').at(-1)!.args).toMatchObject({
      record: { title: 'Lab 5a', score: null, pending: true, source: 'you' },
    });
    expect($$(rig, '.heat-sticky').map((s) => text(s))).toHaveLength(2);
    expect(rig.status().count).toBe('Added Lab 5a, waiting for its score.');
  });

  it('asks for a name first, in one line', async () => {
    await open();
    await click(button(rig, 'New grade'));
    await click(button(rig, 'Add grade'));
    expect(text($(rig, '.heat-sheet-why'))).toBe('Give the grade a name first.');
    expect(calls('heat.put')).toEqual([]);
  });

  it('edits a grade from its row, and a dropped grade says so', async () => {
    await open();
    await click(button(rig, /^Grammar quiz 3/));
    await click(button(rig, 'Edit grade…'));
    const sheet = byRole(rig, 'dialog', 'Edit grade Grammar quiz 3')!;
    const numbers = $$(sheet, 'input[type="number"]') as HTMLInputElement[];
    await type(numbers[0], '19');
    await click($(sheet, 'input[type="checkbox"]'));
    await click(button(rig, 'Save'));
    expect(rig.fake.store.grade.get('g-1')).toMatchObject({ score: 19, dropped: true });
    expect(text($$(card('JPN 201'), '.heat-grade')[1])).toContain('Dropped');
    expect(await history()).toBe('Undo edit grade');
  });

  it('deletes a grade, and Undo brings it back', async () => {
    await open();
    await click(button(rig, /^Grammar quiz 3/));
    await click(button(rig, 'Edit grade…'));
    await click(button(rig, 'Delete grade'));
    expect(rig.fake.store.grade.has('g-1')).toBe(false);
    await rig.call('history.undo', {});
    expect(rig.fake.store.grade.has('g-1')).toBe(true);
  });
});

describe('the course editor', () => {
  it('"Add course" is the tab’s secondary act, on ⇧Return and as a button', async () => {
    await open();
    expect(rig.status().act).toBe('Add course');
    rig.view.current!.secondary();
    await settle();
    expect(byRole(rig, 'dialog', 'Add course')).toBeTruthy();
  });

  it('says so as the weights are typed, and saves categories, keywords and a scale as one undo step', async () => {
    await open();
    await click(button(rig, 'Add course'));
    const sheet = () => byRole(rig, 'dialog', 'Add course')!;
    await type($(sheet(), 'input[placeholder="JPN 201"]'), 'ENG 110');
    await type($(sheet(), 'input[placeholder="Intermediate Japanese"]'), 'Writing');
    await click(button(rig, 'Add category'));
    await type($(sheet(), 'input[aria-label="Category 1 name"]'), 'Essays');
    await type($(sheet(), 'input[aria-label="Category 1 weight"]'), '60');
    await type($(sheet(), 'input[aria-label="Category 1 keywords"]'), 'essay, draft');
    expect(text($(sheet(), '.heat-weights'))).toBe('Weights add to 60%. The other 40% is unassigned.');
    await click(button(rig, 'Add category'));
    await type($(sheet(), 'input[aria-label="Category 2 name"]'), 'Final');
    await type($(sheet(), 'input[aria-label="Category 2 weight"]'), '50');
    expect(text($(sheet(), '.heat-weights'))).toBe('Weights add to 110%. That is 10% more than 100.');
    await type($(sheet(), 'input[aria-label="Category 2 weight"]'), '40');
    expect(text($(sheet(), '.heat-weights'))).toBe('');
    // The scale is the third set, after the categories and the assignment types.
    await click($(sheet(), '.heat-set:nth-of-type(3) input[type="checkbox"]'));
    await type($(sheet(), 'input[aria-label="Step 1 from percent"]'), '95');
    await click($(sheet(), 'button[type="submit"]'));

    expect(byRole(rig, 'dialog', 'Add course')).toBeUndefined();
    const course = [...rig.fake.store.course.values()].find((c) => c.code === 'ENG 110')!;
    expect(course.name).toBe('Writing');
    expect(course.termId).toBe('term-fall-26');
    expect(course.categories.map((c) => [c.name, c.weight, c.keywords])).toEqual([
      ['Essays', 60, ['essay', 'draft']],
      ['Final', 40, []],
    ]);
    expect(course.scale![0]).toEqual({ letter: 'A', min: 95 });
    expect(course.public).toBe(false);
    expect(await history()).toBe('Undo add course');
    expect(text($(card('ENG 110'), '.heat-course-pct'))).toBe('—');
    expect(text($(card('ENG 110'), '.heat-course-based'))).toBe('Nothing is graded yet.');
  });

  it('keeps what was typed when Esc closes it, and throws it away on Cancel', async () => {
    await open();
    const field = () => $(byRole(rig, 'dialog', 'Add course'), 'input[placeholder="JPN 201"]') as HTMLInputElement;
    await click(button(rig, 'Add course'));
    await type(field(), 'BIO 101');
    await escape(rig);
    expect(byRole(rig, 'dialog', 'Add course')).toBeUndefined();
    await click(button(rig, 'Add course'));
    expect(field().value).toBe('BIO 101');
    await click(button(rig, 'Cancel'));
    await click(button(rig, 'Add course'));
    expect(field().value).toBe('');
  });

  it('asks for a code first, in one line', async () => {
    await open();
    await click(button(rig, 'Add course'));
    await click($$(rig, '.heat-sheet button[type="submit"]')[0]);
    expect(text($(rig, '.heat-sheet-why'))).toBe('Give the course a code first.');
    expect(calls('heat.put')).toEqual([]);
  });

  it('edits a course in one step, and Delete course takes its grades with it, with Undo', async () => {
    await open();
    await click(button(rig, 'Edit course…'));
    const sheet = byRole(rig, 'dialog', 'Edit course JPN 201')!;
    await type($(sheet, 'input[aria-label="Category 3 weight"]'), '35');
    expect(text($(sheet, '.heat-weights'))).toBe('Weights add to 95%. The other 5% is unassigned.');
    await click(button(rig, 'Save'));
    expect(rig.fake.store.course.get('c-jpn201')!.categories[2].weight).toBe(35);
    expect(await history()).toBe('Undo edit course');
    expect(text($(card('JPN 201'), '.heat-course-weights'))).toBe('Weights add to 95%. The other 5% is unassigned.');

    await click(button(rig, 'Edit course…'));
    await click(button(rig, 'Delete course'));
    expect(rig.fake.store.course.has('c-jpn201')).toBe(false);
    expect([...rig.fake.store.grade.values()].some((g) => g.courseId === 'c-jpn201')).toBe(false);
    await rig.call('history.undo', {});
    expect(rig.fake.store.course.has('c-jpn201')).toBe(true);
    expect(rig.fake.store.grade.has('g-1')).toBe(true);
  });
});

describe('the Public switches', () => {
  it('start off on every course and every grade, with the sentence that says what each does', async () => {
    await open();
    const course = $$(rig, '.heat-course-foot input[role="switch"]') as HTMLInputElement[];
    expect(course.map((s) => [s.getAttribute('aria-label'), s.checked])).toEqual([
      ['Public: JPN 201', false],
      ['Public: MTH 142', false],
    ]);
    expect(text($(card('JPN 201'), '.heat-course-foot .heat-switch-hint'))).toContain(
      'Its grades, percentage and letter stay with you.',
    );
    await click(button(rig, /^Grammar quiz 3/));
    const grade = $(rig, '.heat-grade-more input[role="switch"]') as HTMLInputElement;
    expect(grade.checked).toBe(false);
    expect(text($(rig, '.heat-grade-more .heat-switch-hint'))).toBe(
      'Grades are private by default. Your school keeps them as education records. Turning this on shows this grade to anyone who opens your sun.',
    );
  });

  it('turn a grade public one item at a time, as their own undo step', async () => {
    await open();
    await click(button(rig, /^Grammar quiz 3/));
    await click($(rig, '.heat-grade-more input[role="switch"]'));
    expect(calls('heat.public.set').map((c) => c.args)).toEqual([{ kind: 'grade', id: 'g-1', public: true }]);
    expect(rig.fake.store.grade.get('g-1')!.public).toBe(true);
    expect(rig.fake.store.grade.get('g-2')!.public).toBe(false);
    expect(rig.fake.store.course.get('c-jpn201')!.public).toBe(false);
    expect(await history()).toBe('Undo make public');
    expect(text($(rig, '.heat-grade-more .heat-switch-hint'))).toContain(
      'can see this grade’s course, item, score and what it was out of',
    );
  });
});

describe('the tab’s acts', () => {
  it('"+" is off, and says why, until there is a course', async () => {
    rig = await mountHeat({ empty: true });
    await click(button(rig, 'Grades'));
    const plus = $(rig, '.heat-plus')!;
    expect(plus.getAttribute('aria-label')).toBe('New grade');
    expect(plus.hasAttribute('disabled')).toBe(true);
    expect(text($(rig, '.heat-why'))).toBe('Add a course first.');
    expect(text($(rig, '.heat-grades-body'))).toBe('No courses yet. Add one to start keeping grades.');
    expect(await press(rig, 'n')).toBe(true);
    expect(rig.status().count).toBe('Add a course first.');
  });

  it('a new library’s first course makes its first term', async () => {
    rig = await mountHeat({ empty: true });
    await click(button(rig, 'Grades'));
    await click(button(rig, 'Add course'));
    const sheet = byRole(rig, 'dialog', 'Add course')!;
    expect(($(sheet, '.heat-sheet-grid label:nth-of-type(3) input') as HTMLInputElement).value).toBe('Fall 2026');
    await type($(sheet, 'input[placeholder="JPN 201"]'), 'JPN 101');
    await click($$(sheet, 'button[type="submit"]')[0]);
    expect([...rig.fake.store.term.values()].map((t) => t.name)).toEqual(['Fall 2026']);
    expect($(rig, '.heat-grades h1')!.textContent).toBe('Fall 2026 grades');
  });
});

describe('a course’s name', () => {
  it('is written once, as the code, a dot and the name', async () => {
    await open();
    const jpn = card('JPN 201');
    expect(jpn.getAttribute('aria-label')).toBe('JPN 201 · Intermediate Japanese');
    expect(text($(jpn, '.heat-course-name'))).toBe('JPN 201 · Intermediate Japanese');
    expect($$(jpn, '.heat-course-head h2')).toHaveLength(1);
  });

  it('is the code alone when the name only says the code again', async () => {
    await open();
    await rig.client.put('course', { termId: 'term-fall-26', code: 'JPN 101', name: 'jpn101', categories: [], notes: '' });
    await settle();
    expect(card('JPN 101').getAttribute('aria-label')).toBe('JPN 101');
    expect(text($(card('JPN 101'), '.heat-course-name'))).toBe('JPN 101');
  });
});

describe('a course waiting for its syllabus', () => {
  const stub = () =>
    rig.client.put('course', {
      termId: 'term-fall-26',
      code: 'ELE 209',
      name: 'Intro to Computer Systems Lab',
      categories: [],
      notes: '',
      status: 'stub',
    });
  const take = (code: string) => $(card(code), '[aria-label="What it would take"]');

  it('asks for the syllabus instead of saying what it would take, and wears a quiet tag', async () => {
    await open();
    const { record } = await stub();
    await settle();
    const ele = card('ELE 209');
    expect(text($(ele, '.heat-course-name'))).toBe('ELE 209 · Intro to Computer Systems LabNeeds syllabus');
    expect(take('ELE 209')).toBeNull();
    expect(button(ele, 'Import syllabus')).toBeTruthy();
    // Weights that add to nothing aren't called out as a mistake: the ask covers it.
    expect($(ele, '.heat-course-weights')).toBeNull();
    expect(calls('heat.whatItWouldTake').filter((c) => c.args.courseId === record.id)).toEqual([]);
  });

  it('says what it would take, and offers no import, once the weights add to something', async () => {
    await open();
    expect(take('JPN 201')).toBeTruthy();
    expect(button(card('JPN 201'), 'Import syllabus')).toBeUndefined();
    expect($(card('JPN 201'), '.heat-tag-quiet')).toBeNull();
    // Give the stub its weights by hand and it reads like any other course.
    const { record } = await stub();
    await rig.client.updateCourse(record.id, {
      categories: [{ id: 'cat-labs', name: 'Labs', weight: 100, keywords: [] }],
      status: 'confirmed',
    });
    await settle();
    expect(take('ELE 209')).toBeTruthy();
    expect(button(card('ELE 209'), 'Import syllabus')).toBeUndefined();
    expect($(card('ELE 209'), '.heat-tag-quiet')).toBeNull();
  });

  it('says to drop the PDF where there is no open panel to show', async () => {
    await open();
    await stub();
    await settle();
    await click(button(card('ELE 209'), 'Import syllabus'));
    const sheet = byRole(rig, 'dialog', 'Import syllabus')!;
    expect(text(sheet)).toContain('Drop the syllabus PDF on the course’s card in Grades.');
    expect(calls('heat.course.importSyllabus')).toEqual([]);
    await click(button(sheet, 'Done'));
    expect(byRole(rig, 'dialog', 'Import syllabus')).toBeUndefined();
  });
});

/** Files dropped the way a browser carries them: as file:// URLs on the element under the pointer. */
async function drop(el: Element, uris: string[]) {
  const e = new Event('drop', { bubbles: true, cancelable: true });
  Object.assign(e, { dataTransfer: { getData: (kind: string) => (kind === 'text/uri-list' ? uris.join('\n') : '') } });
  await act(async () => {
    el.dispatchEvent(e);
    await new Promise((r) => setTimeout(r, 0));
  });
  await settle();
}

describe('a syllabus dropped on Grades', () => {
  const lines = (scope: Element | null) => $$(scope, '.heat-syllabus-state').map((l) => text(l));

  it('is read for the course whose card it lands on, and says so while it is read', async () => {
    await open();
    holdReading(rig.fake);
    await drop($(card('MTH 142'), '.heat-cats')!, ['file:///Users/you/Desktop/notes.txt', 'file:///Users/you/Desktop/MTH%20142.pdf']);
    expect(calls('heat.course.importSyllabus').map((c) => c.args)).toEqual([
      { path: '/Users/you/Desktop/MTH 142.pdf', courseId: 'c-mth142' },
    ]);
    expect(lines(card('MTH 142'))).toEqual(['Reading MTH 142.pdf…']);
    expect(lines(card('JPN 201'))).toEqual([]);
    expect(byRole(rig, 'dialog', 'Import syllabus')).toBeUndefined();
    // Nothing is written while it is read.
    expect(await history()).toBeNull();

    await act(async () => finishReading(rig.fake, draftsOf(rig.fake)[0].id));
    await settle();
    expect(text($(byRole(rig, 'dialog', 'Import syllabus'), '.heat-syllabus-line'))).toBe(
      'MTH 142 · Calculus II. Labs 40%, Quizzes 20%, Final 40%. 2 types. 2 new tasks, 0 dates changed.',
    );
  });

  it('names its own course when it lands on the page, and anything but a PDF is turned away', async () => {
    await open();
    holdReading(rig.fake);
    await drop($(rig, '.heat-grades-head')!, ['file:///Users/you/Desktop/syllabus.docx']);
    expect(calls('heat.course.importSyllabus')).toEqual([]);
    expect(rig.status().count).toBe('Drop the syllabus as a PDF.');
    await drop($(rig, '.heat-grades-head')!, ['file:///Users/you/Desktop/ELE209.pdf']);
    expect(calls('heat.course.importSyllabus').map((c) => c.args)).toEqual([{ path: '/Users/you/Desktop/ELE209.pdf' }]);
    expect(lines($(rig, '.heat-grades-head'))).toEqual(['Reading ELE209.pdf…']);
  });

  it('says why in one sentence when it can’t be read, and Dismiss drops the draft', async () => {
    await open();
    holdReading(rig.fake);
    await drop(card('JPN 201'), ['file:///Users/you/Desktop/scan.pdf']);
    await act(async () => finishReading(rig.fake, draftsOf(rig.fake)[0].id, 'scan.pdf has no text to read. Export it with its text and try again.'));
    await settle();
    expect(lines(card('JPN 201'))).toEqual(['scan.pdf has no text to read. Export it with its text and try again.Dismiss']);
    expect(byRole(rig, 'dialog', 'Import syllabus')).toBeUndefined();
    await click(button(card('JPN 201'), 'Dismiss'));
    expect(calls('heat.syllabus.discard').map((c) => c.args)).toEqual([{ draftId: 'fake-000001' }]);
    expect(lines(card('JPN 201'))).toEqual([]);
  });
});

describe('the syllabus preview', () => {
  const QUIZ4 = Date.UTC(2026, 9, 9, 3, 59); // Oct 8, 11:59 PM in New York
  const MOVED = Date.UTC(2026, 9, 10, 3, 59);
  const json = {
    fileName: 'JPN201-syllabus.pdf',
    pages: 4,
    weights: [
      { category: 'Homework', percent: 25 },
      { category: 'Quizzes', percent: 30, dropLowest: 1 },
      { category: 'Exams', percent: 40 },
    ],
    types: [
      { name: 'Quiz', patterns: ['quiz'], estMin: 25, difficulty: 2, category: 'Quizzes' },
      { name: 'Worksheet', patterns: ['worksheet'], estMin: 40, difficulty: 2, category: 'Homework' },
    ],
    tasks: [
      { title: 'Grammar quiz 4', due: MOVED },
      { title: 'Grammar quiz 5', due: Date.UTC(2026, 9, 16, 3, 59) },
    ],
  };
  const sheet = () => byRole(rig, 'dialog', 'Import syllabus');
  const rows = (legend: string) =>
    $$(
      $$(sheet(), '.heat-set').find((set) => text($(set, 'legend')) === legend),
      'tbody tr',
    ).map((r) => $$(r, 'th, td').map((c) => text(c)));
  const jpn = () => rig.fake.store.course.get('c-jpn201')!;

  async function preview() {
    await open();
    await act(async () => void (await rig.client.importSyllabus({ json, courseId: 'c-jpn201' })));
    await settle();
  }

  it('shows what the syllabus would change, with the weights’ warning, and writes none of it', async () => {
    await preview();
    expect(text($(sheet(), '.heat-syllabus-line'))).toBe(
      'JPN 201 · Intermediate Japanese. Homework 25%, Quizzes 30%, Exams 40%. 2 types. 1 new task, 1 date changed.',
    );
    expect(text($(sheet(), '.heat-syllabus-course'))).toBe('JPN 201 · Intermediate JapaneseFall 2026 · JPN201-syllabus.pdf');
    expect(rows('Weights')).toEqual([
      ['Homework', '25%', ''],
      ['Quizzes', '30%', 'drops lowest 1'],
      ['Exams', '40%', ''],
    ]);
    const flag = $(sheet(), '.heat-syllabus-flag')!;
    expect(text(flag)).toBe('Weights add to 95%. The other 5% is unassigned.');
    expect(flag.getAttribute('role')).toBe('alert');
    expect(rows('Types')).toEqual([
      ['Quiz', '25m', 'difficulty 2'],
      ['Worksheet', '40m', 'difficulty 2'],
    ]);
    expect(rows('New tasks')).toEqual([['Grammar quiz 5', 'Oct 15 11:59 PM', 'Quiz · 25m']]);
    expect(rows('Date changes')).toEqual([['Grammar quiz 4', 'Oct 8 11:59 PM → Oct 9 11:59 PM']]);
    expect(button(sheet(), 'Accept')).toBeTruthy();
    expect(button(sheet(), 'Discard')).toBeTruthy();
    // Nothing is written before Accept.
    expect(jpn().categories.map((c) => c.weight)).toEqual([25, 35, 40]);
    expect(rig.fake.store.task.get('t-quiz4')!.due).toBe(QUIZ4);
    expect(await history()).toBeNull();
  });

  it('marks a course Learn doesn’t have yet as new', async () => {
    await open();
    await act(
      async () =>
        void (await rig.client.importSyllabus({
          json: { ...json, course: { code: 'ELE 209', name: 'Intro to Computer Systems Lab' }, tasks: [] },
        })),
    );
    await settle();
    expect(text($(sheet(), '.heat-syllabus-course'))).toBe(
      'ELE 209 · Intro to Computer Systems LabNew courseFall 2026 · JPN201-syllabus.pdf',
    );
  });

  it('applies it all on Accept, and one undo takes it all back', async () => {
    await preview();
    await click(button(sheet(), 'Accept'));
    expect(calls('heat.syllabus.accept').map((c) => c.args)).toEqual([{ draftId: 'fake-000001' }]);
    expect(sheet()).toBeUndefined();
    expect(rig.status().count).toBe(`Syllabus imported. ${keys('⌘Z')} undoes it.`);

    expect(jpn().categories.map((c) => [c.id, c.name, c.weight, c.dropLowest ?? null])).toEqual([
      ['cat-hw', 'Homework', 25, null],
      ['cat-quiz', 'Quizzes', 30, 1],
      ['cat-exam', 'Exams', 40, null],
    ]);
    expect(jpn().types!.map((t) => t.name)).toEqual(['Quiz', 'Worksheet']);
    expect(jpn()).toMatchObject({ status: 'confirmed', syllabusSource: { name: 'JPN201-syllabus.pdf', pages: 4 } });
    // The date moved, the new task is in the course, and the quiz now takes what the course says a quiz takes.
    expect(rig.fake.store.task.get('t-quiz4')).toMatchObject({ due: MOVED, estMin: 25, estBy: 'type', difficulty: 2 });
    const made = [...rig.fake.store.task.values()].find((t) => t.title === 'Grammar quiz 5')!;
    expect(made).toMatchObject({ courseId: 'c-jpn201', spaceId: 'sp-classes', type: 'Quiz', estMin: 25, estBy: 'type' });
    expect(text($(card('JPN 201'), '.heat-course-weights'))).toBe('Weights add to 95%. The other 5% is unassigned.');
    expect($$(card('JPN 201'), '.heat-syllabus-state')).toEqual([]);

    expect(await history()).toBe('Undo import syllabus');
    await act(async () => void (await rig.call('history.undo', {})));
    await settle();
    expect(jpn().categories.map((c) => c.weight)).toEqual([25, 35, 40]);
    expect(jpn().types).toBeUndefined();
    expect(jpn().syllabusSource).toBeUndefined();
    expect(rig.fake.store.task.get('t-quiz4')).toMatchObject({ due: QUIZ4, estMin: null, difficulty: 3 });
    expect([...rig.fake.store.task.values()].some((t) => t.title === 'Grammar quiz 5')).toBe(false);
    expect(await history()).toBeNull();
  });

  it('drops the draft on Discard, and changes nothing', async () => {
    await preview();
    await click(button(sheet(), 'Discard'));
    expect(calls('heat.syllabus.discard').map((c) => c.args)).toEqual([{ draftId: 'fake-000001' }]);
    expect(sheet()).toBeUndefined();
    expect(draftsOf(rig.fake)).toEqual([]);
    expect(jpn().categories.map((c) => c.weight)).toEqual([25, 35, 40]);
    expect(await history()).toBeNull();
  });

  it('waits behind Review after Esc, and is there again after a relaunch', async () => {
    await preview();
    await escape(rig);
    expect(sheet()).toBeUndefined();
    expect($$(card('JPN 201'), '.heat-syllabus-state').map((l) => text(l))).toEqual([
      'JPN201-syllabus.pdf is read and waits for you.Review',
    ]);
    await click(button(card('JPN 201'), 'Review'));
    expect(text($(sheet(), '.heat-syllabus-line'))).toContain('JPN 201 · Intermediate Japanese.');
    await escape(rig);

    // A draft that is simply there in the snapshot (left from before a relaunch, or read by Claude in the
    // background) drops down when Grades is next opened, and not over another tab.
    const { fake } = rig;
    rig.unmount();
    rig = await mountHeat();
    draftsOf(rig.fake).push(...draftsOf(fake));
    await act(async () => rig.fake.emit([]));
    await settle();
    expect(sheet()).toBeUndefined();
    await click(button(rig, 'Grades'));
    expect(text($(sheet(), '.heat-syllabus-line'))).toContain('2 types. 1 new task, 1 date changed.');
  });
});

describe('the course editor’s assignment types', () => {
  it('saves a type through the course’s own command, which retimes its tasks, as one undo step', async () => {
    await open();
    await click(button(card('JPN 201'), 'Edit course…'));
    const sheet = byRole(rig, 'dialog', 'Edit course JPN 201')!;
    await click(button(sheet, 'Add type'));
    await click(button(sheet, 'Save'));
    expect(text($(sheet, '.heat-sheet-why'))).toBe('Give every type a name.');
    await type($(sheet, 'input[aria-label="Type 1 name"]'), 'Quiz');
    await type($(sheet, 'input[aria-label="Type 1 words"]'), 'Quiz, pop quiz');
    await type($(sheet, 'input[aria-label="Type 1 minutes"]'), '25');
    await type($(sheet, 'select[aria-label="Type 1 difficulty"]'), '2');
    expect($$(sheet, 'select[aria-label="Type 1 category"] option').map((o) => o.textContent)).toEqual([
      'No category',
      'Homework',
      'Quizzes',
      'Exams',
    ]);
    await type($(sheet, 'select[aria-label="Type 1 category"]'), 'Quizzes');
    await click(button(sheet, 'Save'));

    expect(calls('heat.patch').filter((c) => c.args.kind === 'course')).toEqual([]);
    const sent = calls('heat.course.update').at(-1)!.args as { id: string; set: { types: unknown } };
    expect(sent.id).toBe('c-jpn201');
    expect(sent.set.types).toEqual([
      { name: 'Quiz', patterns: ['quiz', 'pop quiz'], estMin: 25, difficulty: 2, category: 'Quizzes' },
    ]);
    // The quiz nobody had timed takes the course's 25 minutes; the worksheet Claude timed and the practice you timed stay.
    expect(rig.fake.store.task.get('t-quiz4')).toMatchObject({ estMin: 25, estBy: 'type', difficulty: 2 });
    expect(rig.fake.store.task.get('t-kanji7')).toMatchObject({ estMin: 45, estBy: 'claude', difficulty: 2 });
    expect(rig.fake.store.task.get('t-listen')).toMatchObject({ estMin: 20, estBy: 'you', difficulty: 1 });
    expect(rig.status().count).toBe('Saved JPN 201. 1 task retimed.');
    expect(await history()).toBe('Undo edit course');
    await rig.call('history.undo', {});
    expect(rig.fake.store.course.get('c-jpn201')!.types).toBeUndefined();
    expect(rig.fake.store.task.get('t-quiz4')).toMatchObject({ estMin: null, difficulty: 3 });
  });

  it('shows the types a course has, and Remove takes one away', async () => {
    await open();
    await rig.client.updateCourse('c-mth142', {
      types: [
        { name: 'Problem set', patterns: ['problem set', 'pset'], estMin: 120, difficulty: 4, category: 'Problem sets' },
        { name: 'Test', patterns: ['test'], estMin: null, difficulty: null },
      ],
    });
    await settle();
    await click(button(card('MTH 142'), 'Edit course…'));
    const sheet = byRole(rig, 'dialog', 'Edit course MTH 142')!;
    const value = (label: string) => ($(sheet, `[aria-label="${label}"]`) as HTMLInputElement).value;
    expect([value('Type 1 name'), value('Type 1 words'), value('Type 1 minutes'), value('Type 1 difficulty')]).toEqual([
      'Problem set',
      'problem set, pset',
      '120',
      '4',
    ]);
    expect(value('Type 1 category')).toBe('Problem sets');
    expect([value('Type 2 minutes'), value('Type 2 difficulty'), value('Type 2 category')]).toEqual(['', '', '']);
    await click(button(sheet, 'Remove type 2'));
    await click(button(sheet, 'Save'));
    expect(rig.fake.store.course.get('c-mth142')!.types).toEqual([
      { name: 'Problem set', patterns: ['problem set', 'pset'], estMin: 120, difficulty: 4, category: 'Problem sets' },
    ]);
  });
});
