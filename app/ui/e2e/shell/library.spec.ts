/// <reference types="node" />
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';
import { CMD, dropFiles, expect, openShell, packSong, plainWav, test } from './kit';

// docs/PLAN.md S1.6 and S1.7 on the UI's side, ⌘K, ⌘⇧N and ⌘L (docs/SPEC.md
// 2.5, 2.7; docs/ASK.md). What a fail looks like: the prompt box not finding
// a task, a clip, a setting or an action as it is typed, reading a filter as
// a search word, opening a result on Return when none was picked (Return
// then asks Claude), or letting Return and ⌘Return do the same thing to a
// picked song; capture closing
// on Enter or not reading "N in inbox · captured ✓"; the drawer anything
// but 280 pt, or not over every view; an import that copies before it says
// what it brings, or a plain WAV that comes in without "master only" being
// said; a tag shown with a capital, a 13th tag or a fifth pin taken
// silently; a delete that can't be undone; Get Info's verdict differing
// from wwav_pack.py info's; a rename that renames the file.

function pranaVerdict(file: string): string {
  const py = join(process.env.WI_E2E_ROOT!, 'formats/prana/tools/wwav_pack.py');
  const out = spawnSync('python3', [py, 'info', file], { encoding: 'utf8' }).stdout;
  return /^ {2}on PRANA: (.*)$/m.exec(out)![1];
}

test('⌘K finds tasks, clips, settings and actions; filters narrow; the newest answer wins', async ({ page, core }) => {
  const song = packSong({ title: 'Palette Song', key: 'D minor', bpm: 92, seconds: 2 });
  await core.call('library.import', { paths: [song], label: 'import' });
  await core.call('library.tag', {
    ids: (await core.call<{ clips: { id: string }[] }>('library.search', { q: 'Palette Song' })).clips.map((c) => c.id),
    add: ['live-drums'],
    label: 'tag',
  });
  await core.call('records.mutate', {
    label: 'add task',
    room: 'heat',
    ops: [
      {
        op: 'put',
        kind: 'task',
        id: 'palette-task',
        value: { title: 'Palette quiz', due: Date.now() + 3_600_000, difficulty: 3 },
      },
    ],
  });
  await openShell(page);
  await page.keyboard.press(`${CMD}+k`);
  const box = page.getByRole('combobox');
  const results = page.getByRole('listbox', { name: 'Results' });
  // The first row sends what was typed to Claude; what was found is under it.
  const found = results.locator('.palette-row:not(.ask-send)');

  await box.fill('palette');
  await expect(results.getByRole('option', { name: /Palette quiz/ })).toBeVisible();
  await expect(results.getByRole('option', { name: /Palette Song/ })).toBeVisible(); // after 200 ms, from the library
  await box.fill('text size');
  await expect(results.getByRole('option', { name: 'Text size' })).toBeVisible();
  await box.fill('capture');
  await expect(results.getByRole('option', { name: /Quick capture/ })).toBeVisible();

  await box.fill('tag:live-drums key:D-minor bpm:90-95');
  await expect(found).toHaveText([/^Palette Song/]);
  await box.fill('tag:no-such-tag');
  await expect(results).toContainText('Nothing on this Mac matches.');
  await box.fill('is:hot');
  await expect(found).toContainText(['Palette quiz']);
  await box.fill('is:remix');
  await expect(results).toContainText('is:remix finds nothing yet');

  // With nothing picked, Return is for Claude: the first row says so and is the one selected.
  await box.fill('Palette Song');
  await expect(results.getByRole('option', { name: /Palette Song/ })).toBeVisible();
  await expect(results.locator('.ask-send')).toHaveAttribute('aria-selected', 'true');
  await expect(results.locator('.ask-send')).toContainText('Palette Song');
  // ↓ picks the song. Return on it plays it; ⌘Return opens it in its other room.
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press(`${CMD}+Enter`);
  await expect(page.locator('.toast')).toContainText('Console');
  await page.keyboard.press(`${CMD}+k`);
  await expect(results.getByRole('option', { name: /Palette Song/ })).toBeVisible();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(page.locator('.strip-title')).toHaveText('Palette Song');
  await page.keyboard.press(' ');
  await core.call('records.mutate', {
    label: 'clear',
    room: 'heat',
    ops: [{ op: 'delete', kind: 'task', id: 'palette-task' }],
  });
});

test('⌘⇧N saves on Enter, stays open and counts the inbox', async ({ page, core }) => {
  const { records } = await core.call<{ records: { triagedAt?: number }[] }>('records.list', { kind: 'capture' });
  const inbox = records.filter((r) => !r.triagedAt).length;
  await openShell(page);
  await page.keyboard.press(`${CMD}+Shift+n`);
  const panel = page.getByRole('dialog', { name: 'Quick capture' });
  const size = (await panel.boundingBox())!;
  expect([Math.round(size.width), Math.round(size.height)]).toEqual([420, 160]);
  await page.keyboard.type('fix the snare at 1:32');
  await page.keyboard.press('Enter');
  await expect(panel.locator('.capture-foot')).toHaveText(`${inbox + 1} in inbox · captured ✓`);
  await expect(panel).toBeVisible();
  await expect(panel.getByRole('textbox')).toHaveValue('');
  await page.keyboard.type('https://example.com/a-link');
  await page.keyboard.press('Enter');
  await expect(panel.locator('.capture-foot')).toHaveText(`${inbox + 2} in inbox · captured ✓`);

  // A dropped file comes into the library and is captured by its name.
  await dropFiles(page, '.capture', [plainWav('dropped idea.wav', 1.5)]);
  await expect(panel.locator('.capture-foot')).toHaveText(`${inbox + 3} in inbox · captured ✓`);
  const after = await core.call<{ records: { text: string }[] }>('records.list', { kind: 'capture' });
  expect(after.records.map((r) => r.text)).toEqual(
    expect.arrayContaining(['fix the snare at 1:32', 'https://example.com/a-link', 'dropped idea.wav']),
  );
  await page.keyboard.press('Escape');
});

test('⌘L slides a 280 pt drawer over every view', async ({ page }) => {
  await openShell(page);
  for (let i = 1; i <= 3; i++) {
    await page.keyboard.press(`${CMD}+${i}`);
    await page.keyboard.press(`${CMD}+l`);
    const drawer = page.getByRole('complementary', { name: 'Library' });
    await expect(drawer).toBeVisible();
    expect(Math.round((await drawer.boundingBox())!.width)).toBe(280);
    await page.keyboard.press(`${CMD}+l`);
    await expect(drawer).toBeHidden();
  }
});

test('a drop says what it holds before anything comes in; plain audio says master only', async ({ page, core }) => {
  const wav = plainWav('field recording.wav', 1.25);
  const song = packSong({ title: 'Dropped Song', key: 'E minor', bpm: 120, seconds: 1 });
  await openShell(page);
  await page.keyboard.press(`${CMD}+l`);
  await dropFiles(page, '.drawer', [wav.replace(/\/[^/]+$/, ''), song]);
  const note = page.locator('.drawer-note');
  await expect(note).toContainText('1 file: 1 plain audio. Plain audio comes in as master only.');
  await expect(note).toContainText('1 file: 1 .wwav.');
  expect((await core.call<{ clips: unknown[] }>('library.search', { q: 'field recording' })).clips).toHaveLength(0);
  await note.getByRole('button', { name: 'Bring them in' }).click();
  await expect(page.locator('.drawer-said')).toHaveText('Brought in 2 files.');
  // The drop said a plain WAV is master only (2.5) before it came in; once
  // it is in, its row says what the reference tool says of it, word for word
  // ("the master only: no wmet and wlin, so a plain WAV"), as a song's does.
  await expect(page.getByRole('option', { name: /field recording/ })).toContainText(pranaVerdict(wav));
  await expect(page.getByRole('option', { name: /Dropped Song/ })).toContainText(pranaVerdict(song));
});

test('Get Info: the reader’s verdict, lowercase tags up to 12, four pins, rename keeps the file, delete undoes', async ({
  page,
  core,
}) => {
  const song = packSong({ title: 'Info Song', key: 'C major', bpm: 100, seconds: 1 });
  const { clips } = await core.call<{ clips: { id: string }[] }>('library.import', { paths: [song], label: 'import' });
  const id = clips[0].id;
  await openShell(page);
  await page.keyboard.press(`${CMD}+l`);
  await page.getByRole('option', { name: /Info Song/ }).click();
  await page.keyboard.press(`${CMD}+i`);
  const info = page.getByRole('dialog', { name: /^Get Info/ });
  await expect(info.locator('.info-verdict')).toHaveText(pranaVerdict(song));
  await expect(info).toContainText(`${id}, in media/`);

  // Tags are lowercase as they are typed; the 13th is refused in a sentence.
  const tag = info.getByRole('textbox', { name: 'Add a tag' });
  await tag.fill('Live Drums');
  await expect(tag).toHaveValue('live drums');
  await tag.press('Enter');
  await expect(info.locator('.tag')).toHaveText(['live-drums×']);
  for (let n = 2; n <= 12; n++) {
    await tag.fill(`t${n}`);
    await tag.press('Enter');
    await expect(info.locator('.tag')).toHaveCount(n);
  }
  await tag.fill('thirteenth');
  await tag.press('Enter');
  await expect(info.getByRole('alert')).toHaveText('A clip holds 12 tags. Remove one first.');
  await expect(info.locator('.tag')).toHaveCount(12);

  // Renaming changes the library's title, never the file.
  const before = await core.call<{ clip: { sha256: string } }>('library.get', { id });
  const title = info.getByRole('textbox', { name: 'Title' });
  await title.fill('Info Song (renamed)');
  await title.press('Enter');
  await expect(page.getByRole('option', { name: /Info Song \(renamed\)/ })).toBeVisible();
  const after = await core.call<{ clip: { sha256: string; id: string } }>('library.get', { id });
  expect(after.clip.sha256).toBe(before.clip.sha256);
  expect(after.clip.id).toBe(id);

  // Delete removes the row; ⌘Z, in the library, brings it back.
  await page.keyboard.press('Escape');
  await page.getByRole('option', { name: /Info Song \(renamed\)/ }).click();
  await page.keyboard.press('Backspace');
  await expect(page.getByRole('option', { name: /Info Song \(renamed\)/ })).toHaveCount(0);
  await expect(page.locator('.case-status')).toContainText('Undo delete clip');
  await page.keyboard.press(`${CMD}+z`);
  await expect(page.locator('.toast')).toHaveText('Undone — delete clip');
  await expect(page.getByRole('option', { name: /Info Song \(renamed\)/ })).toBeVisible();
});

test('a fifth pin is refused in a sentence', async ({ page, core }) => {
  const paths = [1, 2, 3, 4, 5].map((n) => plainWav(`pin ${n}.wav`, 1 + n / 10));
  const { clips } = await core.call<{ clips: { id: string }[] }>('library.import', { paths, label: 'import' });
  for (const c of clips.slice(0, 4)) await core.call('library.pin', { id: c.id, slot: 0, label: 'pin clip' });
  await openShell(page);
  await page.keyboard.press(`${CMD}+l`);
  await expect(page.locator('.pin:not(:disabled)')).toHaveCount(4);
  await page.getByRole('option', { name: /pin 5/ }).click();
  await page.keyboard.press(`${CMD}+i`);
  await page
    .getByRole('dialog', { name: /^Get Info/ })
    .getByRole('button', { name: 'Pin' })
    .click();
  await expect(page.getByRole('alert')).toHaveText('Pins hold 4. Unpin one first.');
  for (const c of clips.slice(0, 4)) await core.call('library.pin', { id: c.id, label: 'unpin clip' });
});
