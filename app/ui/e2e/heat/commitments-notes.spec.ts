/// <reference types="node" />
// Commitments and Notes through the real core and the real views
// (docs/COMMITMENTS.md, docs/NOTES.md): the core runs behind the dev bridge
// on a fresh library, with notes kept as files and the text reader built
// from its Swift source. What a fail looks like: a class the core holds not
// drawn in Calendar; a photographed page not reaching Notes, or reaching it
// without its image, its text, or its class; the views reading a shape the
// core doesn't send.
//
// The reader is Apple's Vision, so the capture half runs on a Mac only.

import { copyFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, openShell, test } from '../shell/kit';

const day = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
const plus = (d: Date, days: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + days);
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

test('a class is drawn in Calendar, and a page photographed in it lands in Notes, filed and readable', async ({ page, core }) => {
  test.skip(process.platform !== 'darwin', 'the text reader is Vision, on a Mac');
  test.setTimeout(120_000);
  const today = new Date();
  // The Monday of this week: a day the class meets on, and already begun.
  const monday = plus(today, -((today.getDay() + 6) % 7));
  await core.call('heat.snapshot', { date: day(today) });
  await core.call('heat.school.set', {
    name: 'URI',
    host: 'brightspace.uri.edu',
    codePattern: '([A-Z]{2,4})\\s?(\\d{3})',
    termStart: day(plus(monday, -28)),
    termEnd: day(plus(monday, 56)),
  });
  const made = await core.call<{ commitment: { id: string }; line: string }>('heat.commitment.create', {
    course: 'JPN 101',
    days: ['MO', 'WE', 'FR'],
    start: '10:00',
    end: '10:50',
    location: 'Swan Hall 201',
    bufferBefore: 25,
    bufferAfter: 10,
  });
  expect(made.line).toBe('JPN 101, MWF 10:00 AM to 10:50 AM.');

  // A photo of a notebook page, named the way the Shortcut names one: taken Monday, in class.
  const folder = mkdtempSync(join(tmpdir(), 'wi-wwav-page-'));
  const photo = join(folder, `${day(monday)} 10.22.31 page.png`);
  copyFileSync(join(process.env.WI_E2E_ROOT!, 'crates/wi-core/tests/core/fixtures/page.png'), photo);
  await core.call('heat.capture.inbox.add', { paths: [photo] });

  await openShell(page);
  await page.locator('.heat-toolbar').waitFor();

  // Calendar, by the week: the class is there on its three days, with its travel time.
  await page.getByRole('tab', { name: 'Calendar', exact: true }).click();
  await page.keyboard.press('w');
  // Every tab stays mounted, and Today draws today's class too: look in Calendar's own panel.
  const calendar = page.locator('#heat-panel-calendar');
  const blocks = calendar.locator('.heat-commit[data-kind="class"]');
  await expect(blocks).toHaveCount(3);
  await expect(blocks.first()).toContainText('JPN 101');
  await expect(calendar.locator('.heat-commit-buffer[data-buffer="before"]')).toHaveCount(3);
  await page.screenshot({ path: join(process.env.WI_E2E_SHOTS ?? folder, 'calendar-week.png') });

  // Notes: within a minute the page is a note named for the class and the day.
  await page.getByRole('tab', { name: 'Notes', exact: true }).click();
  const title = `JPN 101 · ${MONTHS[monday.getMonth()]} ${monday.getDate()}`;
  const row = page.locator('.notes-row', { hasText: title });
  await expect(row).toBeVisible({ timeout: 60_000 });
  await row.click();
  const pane = page.locator('.notes-pane');
  // The image on top, loaded from the notes folder; its text below.
  const image = pane.locator('img').first();
  await expect(image).toBeVisible({ timeout: 15_000 });
  await expect.poll(() => image.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBeGreaterThan(100);
  await expect(pane).toContainText('Group one verbs change their ending');
  await expect(pane.locator('.notes-meta')).toContainText('Filed to');
  // One quiet notice says what was done, with its undo.
  const notice = page.locator('.notes-notice', { hasText: `Filed to ${title}` });
  await expect(notice).toBeVisible();
  await expect(notice.getByRole('button', { name: 'Undo' })).toBeVisible();
  // A word from the handwritten page finds it.
  await page.getByLabel('Search the notes').fill('verbs ending');
  await expect(page.locator('.notes-row')).toHaveCount(1);
  await expect(page.locator('.notes-row')).toContainText(title);
  await page.screenshot({ path: join(process.env.WI_E2E_SHOTS ?? folder, 'notes-capture.png') });

  // Today says what is fixed and what is free, in the core's own words.
  await page.getByRole('tab', { name: 'Today', exact: true }).click();
  const free = await core.call<{ commitments: { free: { line: string } } }>('heat.snapshot', { date: day(today) });
  await expect(page.locator('.heat-today')).toContainText(free.commitments.free.line);
  await page.screenshot({ path: join(process.env.WI_E2E_SHOTS ?? folder, 'today.png') });
});

test('typing [[ offers notes and courses, backlinks show, and a checkbox becomes a task', async ({ page, core }) => {
  const today = day(new Date());
  await core.call('heat.snapshot', { date: today });
  // A course of its own, so this stands alone whatever ran before it.
  const term = await core.call<{ record: { id: string } }>('heat.put', { kind: 'term', record: { name: 'Links term' } });
  await core.call('heat.put', { kind: 'course', record: { termId: term.record.id, code: 'ELE 209', name: 'Circuits Lab' } });
  await core.call('heat.note.create', { title: 'Verb groups', markdown: 'Three groups of verbs.\n' });
  await core.call('heat.note.create', { title: 'Te-form drills', markdown: 'Twenty verbs.\n\n- [ ] Do worksheet 4\n' });

  await openShell(page);
  await page.locator('.heat-toolbar').waitFor();
  await page.getByRole('tab', { name: 'Notes', exact: true }).click();
  await page.locator('.notes-row', { hasText: 'Te-form drills' }).click();
  const pane = page.locator('.notes-pane');
  await expect(pane.getByLabel('Title')).toHaveValue('Te-form drills');

  // [[ lists what can be linked: a note by its title, a course by its code.
  await pane.getByRole('button', { name: 'Write at the end of the note' }).click();
  const field = pane.getByLabel('Note text');
  await field.pressSequentially('See [[');
  const menu = pane.getByRole('listbox', { name: 'Link to' });
  await expect(menu.getByRole('option', { name: /Verb groups/ })).toBeVisible();
  await expect(menu.getByRole('option', { name: /ELE 209/ })).toBeVisible();
  await field.pressSequentially('verb');
  await expect(menu.getByRole('option')).toHaveCount(2); // the note, and "New note"
  await page.keyboard.press('Enter');
  await expect(field).toHaveValue('See [[Verb groups]]');
  await page.keyboard.press('Escape');
  // Saved through the core: the note's file on disk has the link.
  await expect
    .poll(async () => (await core.call<{ hits: { title: string }[] }>('heat.note.search', { q: 'See' })).hits.map((h) => h.title))
    .toEqual(['Te-form drills']);

  // The checkbox becomes a real task, and the line says so.
  await pane.locator('.notes-block', { hasText: 'Do worksheet 4' }).hover();
  await pane.getByRole('button', { name: 'Make task' }).click();
  await expect
    .poll(async () => {
      const s = await core.call<{ records: { task: { title: string; noteId?: string }[] } }>('heat.snapshot', { date: today });
      return s.records.task.find((t) => t.title === 'Do worksheet 4')?.noteId !== undefined;
    })
    .toBe(true);
  await expect(pane.getByRole('button', { name: 'Make task' })).toHaveCount(0);

  // The note that was linked to shows what links to it.
  await page.locator('.notes-row', { hasText: 'Verb groups' }).click();
  const back = page.locator('.notes-pane .notes-backlinks');
  await expect(back).toContainText('Te-form drills');
  await expect(back).toContainText('See Verb groups');
  await page.screenshot({ path: join(process.env.WI_E2E_SHOTS ?? tmpdir(), 'notes-backlinks.png') });
});
