import { CMD, expect, openShell, plainWav, test } from './kit';

// docs/PLAN.md S1.5, one grammar (docs/SPEC.md 2.7). What a fail looks
// like: ⌘Z that isn't labelled in the status bar and in a toast that holds
// 2.6 s ("Undone — capture"); ⌘Z in a room with nothing to undo saying
// anything but "Nothing to undo."; ⌘Z acting on another room's change; Esc
// throwing away what was typed in a sheet; Space toggling playback while
// you type; a screen with more than one ⇧Return act, or one the status bar
// doesn't name.

const isMac = process.platform === 'darwin';
const Z = isMac ? '⌘Z' : 'Ctrl+Z';
const SHIFT = isMac ? '⇧' : 'Shift+';

test('⌘Z is labelled before and after: in the status bar, then a 2.6 s toast', async ({ page }) => {
  await page.clock.install();
  await openShell(page);
  await page.keyboard.press(`${CMD}+Shift+n`);
  await page.getByRole('textbox', { name: 'Capture' }).fill('buy strings');
  await page.keyboard.press('Enter');
  await expect(page.locator('.capture-foot')).toContainText('captured ✓');
  await page.keyboard.press('Escape');
  await expect(page.locator('.case-status')).toContainText(`${Z} Undo capture`);

  await page.clock.pauseAt(Date.now() + 1000);
  await page.keyboard.press(`${CMD}+z`);
  const toast = page.locator('.toast');
  await expect(toast).toHaveText('Undone — capture');
  await page.clock.runFor(2500);
  await expect(toast).toBeVisible();
  await page.clock.runFor(200);
  await expect(toast).toBeHidden();

  await page.keyboard.press(`${CMD}+Shift+z`);
  await expect(toast).toHaveText('Redone — capture');
});

test('⌘Z acts on the room you are in, and says so when there is nothing there', async ({ page }) => {
  await openShell(page);
  await page.keyboard.press(`${CMD}+Shift+n`);
  await page.getByRole('textbox', { name: 'Capture' }).fill('a thought in Heat');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Escape');
  await page.keyboard.press(`${CMD}+4`);
  await page.keyboard.press(`${CMD}+z`);
  await expect(page.locator('.toast')).toHaveText('Nothing to undo.');
  await page.keyboard.press(`${CMD}+1`);
  await page.keyboard.press(`${CMD}+z`);
  await expect(page.locator('.toast')).toHaveText('Undone — capture');
});

test('Esc closes a sheet and never throws away what was typed in it', async ({ page }) => {
  await openShell(page);
  await page.keyboard.press(`${CMD}+Shift+n`);
  const capture = page.getByRole('textbox', { name: 'Capture' });
  await capture.fill('half a thought');
  await page.keyboard.press('Escape');
  await expect(capture).toBeHidden();
  await page.keyboard.press(`${CMD}+Shift+n`);
  await expect(capture).toHaveValue('half a thought');
  await page.keyboard.press('Escape');

  await page.keyboard.press(`${CMD}+k`);
  await page.getByRole('combobox').fill('grammar');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('combobox')).toBeHidden();
  await page.keyboard.press(`${CMD}+k`);
  await expect(page.getByRole('combobox')).toHaveValue('grammar');
});

test('Space types a space in a field instead of playing', async ({ page }) => {
  await openShell(page);
  await page.keyboard.press(`${CMD}+Shift+n`);
  const capture = page.getByRole('textbox', { name: 'Capture' });
  await capture.fill('');
  await page.keyboard.type('a b');
  await expect(capture).toHaveValue('a b');
  await capture.fill('');
  await page.keyboard.press('Escape');
});

test('each screen has at most one ⇧Return act, and the status bar names it', async ({ page, core }) => {
  await core.call('library.import', { paths: [plainWav('one.wav'), plainWav('two.wav', 2)], label: 'import' });
  await openShell(page);
  // A room with nothing selected has no secondary act yet.
  await expect(page.locator('.case-status')).not.toContainText('Return');

  await page.keyboard.press(`${CMD}+l`);
  await expect(page.locator('.case-status')).toContainText(`${SHIFT}Return Add to selection`);
  const drawer = page.getByRole('complementary', { name: 'Library' });
  const rows = drawer.getByRole('option');
  const picked = drawer.locator('[role="option"][aria-selected="true"]');
  await rows.nth(0).click();
  await page.keyboard.press('ArrowDown');
  await expect(picked).toHaveCount(1);
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Shift+Enter');
  await expect(picked).toHaveCount(2);
  // Esc deselects before it closes.
  await page.keyboard.press('Escape');
  await expect(picked).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(drawer).toBeHidden();
});
