import { afterEach, describe, expect, it } from 'vitest';
import type { Grade } from '../client';
import { $, $$, button, byRole, click, escape, mountHeat, press, type Rig, settle, type } from '../testkit';

// docs/SPEC.md 3.8 and docs/PLAN.md S2.4. What a fail looks like: a course
// without its percentage, letter pill and "Based on X% of the course so far";
// weights that don't add up with no sentence; a pending grade without its
// yellow "Enter score" banner, or one scored by anything but typing the
// score; "What it would take" worked out in the view instead of asked of the
// core; a Public switch that starts on, or a grade's switch without the
// sentence that says what it does.

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
    await click($(sheet(), '.heat-set:nth-of-type(2) input[type="checkbox"]'));
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
