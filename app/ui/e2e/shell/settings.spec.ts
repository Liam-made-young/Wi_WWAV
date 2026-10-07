import { expect, openShell, test } from './kit';

// docs/SPEC.md 2.13: Account holds the galaxy address, the stem player skin,
// sign out and delete account. What a fail looks like: any of the four
// missing, a control this build can't offer that is enabled or doesn't say
// why in a sentence, or anything from the cut (the astronaut and rocket) still
// there; the toolbar showing other than the seven panes.

test('Settings → Account has the galaxy address, stem player skin, sign out and delete account', async ({ page }) => {
  await openShell(page);
  await page.getByRole('button', { name: 'You' }).click();
  await page.getByRole('menuitem', { name: 'Settings…' }).click();
  const tabs = page.getByLabel('Settings panes').getByRole('tab');
  await expect(tabs).toHaveText([
    'Account',
    'Library',
    'Learn',
    'Audio & MIDI · Video',
    'Claude',
    'Privacy',
    'Appearance · Keyboard',
  ]);
  const pane = page.getByRole('tabpanel', { name: 'Account' });
  await expect(pane.getByText('Galaxy address')).toBeVisible();
  // Signed out on the dev bridge, so the one button says how to sign in.
  await expect(pane.getByRole('button', { name: /^Sign (in|out)/ })).toBeVisible();
  for (const name of ['Stem player skin…', 'Delete account…']) {
    const off = pane.getByRole('button', { name });
    await expect(off).toBeDisabled();
    await expect(off.locator('xpath=following-sibling::p[@class="why"]')).toHaveText(/\.$/);
  }
  await expect(pane).not.toContainText(/astronaut|rocket/i);
});
