import { clearTasks, CMD, expect, openShell, packSong, test } from './kit';

// docs/PLAN.md S1.3 and S1.4, the Now strip (docs/SPEC.md 2.2). What a fail
// looks like: an empty half that doesn't read "All clear / Nothing open
// right now." or "Nothing playing / Select anything and press Space."; a
// task half that shows anything but the current task, or else the hottest
// open one, as "Hot: Grammar quiz 4" over its due, course and load, or
// whose click doesn't open Heat with that task selected; a track half
// without its key-coloured planet, title, four lights and "m:ss / m:ss",
// or whose cursor stands still while playing or moves while paused; stem
// state shown by colour alone, or a light without its spoken label; a
// click that doesn't mute at once; a second click within 250 ms that
// doesn't revert the mute and solo instead; one after 250 ms that does;
// any light's hit area under 44 × 44 pt.

test.describe.configure({ mode: 'serial' });

const HOUR = 3_600_000;

test('the empty halves read 2.2’s sentences', async ({ page, core }) => {
  await clearTasks(core);
  await openShell(page);
  const strip = page.locator('.strip');
  await expect(strip.locator('.strip-task')).toHaveText('All clearNothing open right now.');
  await expect(strip.locator('.strip-track')).toHaveText('Nothing playingSelect anything and press Space.');
});

test('the task half shows the hottest open task, or the current one, and opens Heat on it', async ({ page, core }) => {
  await clearTasks(core);
  const now = Date.now();
  await core.call('records.mutate', {
    label: 'add tasks',
    room: 'heat',
    ops: [
      {
        op: 'put',
        kind: 'course',
        id: 'jpn201',
        value: { termId: 't', code: 'JPN 201', name: 'Japanese', categories: [], notes: '' },
      },
      {
        op: 'put',
        kind: 'task',
        id: 'quiz',
        value: {
          spaceId: 's',
          title: 'Grammar quiz 4',
          type: 'Quiz',
          courseId: 'jpn201',
          due: now + 2 * HOUR,
          difficulty: 3,
        },
      },
      {
        op: 'put',
        kind: 'task',
        id: 'essay',
        value: { spaceId: 's', title: 'Essay draft', type: 'Essay', due: now + 6 * 24 * HOUR, difficulty: 2 },
      },
    ],
  });
  await openShell(page);
  const task = page.locator('.strip-task');
  await expect(task.locator('.strip-line1')).toHaveText('Hot: Grammar quiz 4');
  await expect(task.locator('.strip-line2')).toContainText('JPN 201');
  await expect(task.locator('.strip-heat')).toHaveAttribute('data-level', 'hot');

  // The current task (C in Heat) wins over the hottest.
  await core.call('records.mutate', {
    label: 'make current',
    room: 'heat',
    ops: [{ op: 'put', kind: 'heatState', id: 'heat', value: { currentTaskId: 'essay' } }],
  });
  await expect(task.locator('.strip-line1')).toHaveText(/: Essay draft$/);

  await page.keyboard.press(`${CMD}+3`);
  await task.click();
  await expect(page.locator('[data-room="heat"]')).toHaveAttribute('data-current', 'true');
  await expect(page.locator('[data-room="heat"]')).toHaveAttribute('data-selected-task', 'essay');
  await core.call('records.mutate', {
    label: 'clear',
    room: 'heat',
    ops: [{ op: 'delete', kind: 'heatState', id: 'heat' }],
  });
  await clearTasks(core);
});

test('the track half shows the planet, title, four lights and time, and the cursor moves only while playing', async ({
  page,
  core,
}) => {
  const song = packSong({ title: 'World Ending', key: 'A minor', bpm: 128, seconds: 30 });
  const { clips } = await core.call<{ clips: { id: string }[] }>('library.import', { paths: [song], label: 'import' });
  await openShell(page);
  await page.keyboard.press(`${CMD}+l`);
  await page
    .getByRole('option', { name: /World Ending/ })
    .first()
    .click();
  await page.keyboard.press(' ');

  const track = page.locator('.strip-track');
  await expect(track.locator('.strip-title')).toHaveText('World Ending');
  // A minor's key colour: hue ((0 · 7) mod 12) · 30 = 0, minor hsl(0, 58%, 42%).
  await expect(track.locator('.planet')).toHaveAttribute('style', /--key: #a92d2d/);
  const lights = track.getByRole('button', { name: /^(Vocals|Drums|Other|Bass), / });
  await expect(lights).toHaveCount(4);
  await expect(lights.first()).toHaveAccessibleName('Vocals, 100 percent, audible');

  const time = track.locator('.strip-time');
  await expect(time).toHaveText(/^0:0[0-9] \/ 0:30$/);
  // 2.2's "1:42 / 3:58" is as wide as this, and fits its box whole: with the
  // old 8 pt inset and 4 pt of padding it was clipped by a pixel and a half.
  expect(await time.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  await expect(time).not.toHaveText(/^0:00 /, { timeout: 5000 });
  // Paused: the cursor stands still.
  await page.keyboard.press(' ');
  await expect(page.locator('.strip-playhead')).toBeVisible();
  await page.waitForTimeout(300); // the engine's last clock, where it stopped
  const still = await time.textContent();
  await page.waitForTimeout(1500);
  await expect(time).toHaveText(still!);
  expect(clips).toHaveLength(1);
});

test('a light mutes at once; a second click within 250 ms reverts that and solos instead', async ({ page }) => {
  await page.clock.install();
  await openShell(page);
  await page.keyboard.press(`${CMD}+l`);
  await page
    .getByRole('option', { name: /World Ending/ })
    .first()
    .click();
  await page.keyboard.press(`${CMD}+l`);
  await page.keyboard.press(' ');
  const vocals = page.locator('.strip [data-stem="vocals"]');
  const drums = page.locator('.strip [data-stem="drums"]');
  await expect(vocals).toHaveAccessibleName(/^Vocals, 100 percent, audible$/);

  // Every light's hit area is 44 × 44 pt.
  for (const stem of ['vocals', 'drums', 'other', 'bass']) {
    const box = await page.locator(`.strip [data-stem="${stem}"]`).boundingBox();
    expect([box!.width, box!.height]).toEqual([44, 44]);
  }

  await page.clock.pauseAt(Date.now() + 1000);
  await vocals.click();
  await expect(vocals).toHaveAccessibleName('Vocals, muted');
  // State is a shape: a muted light has an open centre and a stem-colour ring.
  await expect(vocals.locator('circle[fill-opacity="0"]')).toHaveCount(1);

  await page.clock.runFor(120);
  await vocals.click();
  await expect(vocals).toHaveAccessibleName('Vocals, soloed');
  await expect(drums).toHaveAccessibleName('Drums, silent under a solo');
  await expect(vocals.locator('circle[r="7"]')).toHaveCount(1); // the outer ring

  // Past 250 ms a click is a fresh click: it mutes, and the solo stays.
  await page.clock.runFor(400);
  await drums.click();
  await page.clock.runFor(400);
  await drums.click();
  await expect(drums).toHaveAccessibleName('Drums, silent under a solo');
  await vocals.click();
  await expect(vocals).toHaveAccessibleName('Vocals, soloed and muted');

  // M and S on a focused light, through the one router.
  await page.clock.runFor(400);
  await vocals.focus();
  await page.keyboard.press('s');
  await expect(vocals).toHaveAccessibleName('Vocals, muted');
  await page.keyboard.press('m');
  await expect(vocals).toHaveAccessibleName(/^Vocals, \d+ percent, audible$/);
});

test('a click on the track half expands the player, and Esc collapses it', async ({ page }) => {
  await openShell(page);
  await page.keyboard.press(`${CMD}+l`);
  await page
    .getByRole('option', { name: /World Ending/ })
    .first()
    .click();
  await page.keyboard.press(`${CMD}+l`);
  await page.keyboard.press(' ');
  // The lights own their 44 pt squares across the half's left 176 pt
  // (docs/QUESTIONS.md #118), so the player opens from the rest of it.
  const expand = page.getByRole('button', { name: /^Show the player: World Ending/ });
  await expect(page.locator('.strip [data-stem="vocals"]')).toBeVisible();
  const half = (await expand.boundingBox())!;
  await expand.click({ position: { x: half.width - 20, y: 22 } });
  const sheet = page.getByRole('dialog', { name: 'Player' });
  await expect(sheet).toBeVisible();
  await expect(sheet.getByRole('button', { name: /^Vocals, / })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(sheet).toBeHidden();
  await page.keyboard.press(' ');
});
