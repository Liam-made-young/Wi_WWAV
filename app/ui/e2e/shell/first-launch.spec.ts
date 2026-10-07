import { test as base } from '@playwright/test';
import { clearTasks, expect, plainWav } from './kit';
import { connect } from '../support/core';

// docs/SPEC.md 2.14. What a fail looks like: a step with no "Skip for now",
// or with any other secondary action; a disabled control that doesn't say
// why; the import step copying before it says what the folder holds, or
// saying it in other words than 2.14's; the flow ending anywhere but Heat
// → Today with "All clear" and "Nothing playing"; signing in required to
// get through; the flow coming back after it was finished.

base(
  'five steps, each with only "Skip for now", ending on Heat with All clear and Nothing playing',
  async ({ page }) => {
    const core = await connect();
    await clearTasks(core);
    core.close();
    const folder = plainWav('a take.wav');

    await page.goto('/');
    const flow = page.getByRole('dialog', { name: 'Welcome to Wi_WWAV' });
    await expect(flow).toBeVisible();
    const titles = [
      'Sign in',
      'Make your astronaut',
      'Claim your galaxy',
      'Import your folder',
      'Connect your school calendar',
    ];
    for (const [i, title] of titles.entries()) {
      await expect(flow.getByRole('heading')).toHaveText(title);
      await expect(flow.getByText(`Step ${i + 1} of 5`)).toBeVisible();
      // Exactly one secondary action, and it is "Skip for now".
      await expect(flow.locator('.gel.plain')).toHaveText(['Skip for now']);
      for (const off of await flow.locator('button:disabled').all()) {
        await expect(off.locator('xpath=following-sibling::p[@class="why"]')).toHaveText(/\.$/);
      }
      if (title === 'Import your folder') {
        await flow.getByRole('textbox').fill(folder.replace(/\/[^/]+$/, ''));
        await flow.getByRole('button', { name: 'Look inside' }).click();
        await expect(flow.locator('.first-summary')).toHaveText(
          '1 file: 1 plain audio. Plain audio comes in as master only.',
        );
      }
      await flow.getByRole('button', { name: 'Skip for now' }).click();
    }
    await expect(flow).toBeHidden();
    await expect(page.locator('[data-room="heat"]')).toHaveAttribute('data-current', 'true');
    await expect(page.locator('[data-room="heat"] h1')).toHaveText('Today');
    await expect(page.locator('.strip-task .strip-line1')).toHaveText('All clear');
    await expect(page.locator('.strip-track .strip-line1')).toHaveText('Nothing playing');

    // Finished is finished: a reload goes straight to the rooms.
    await page.reload();
    await page.locator('.case-bar').waitFor();
    await expect(flow).toBeHidden();
  },
);

base('the import step brings a folder in once you press, and the flow goes on', async ({ page }) => {
  const take = plainWav('first launch take.wav');
  await page.goto('/');
  const flow = page.getByRole('dialog', { name: 'Welcome to Wi_WWAV' });
  for (let i = 0; i < 3; i++) await flow.getByRole('button', { name: 'Skip for now' }).click();
  await flow.getByRole('textbox').fill(take.replace(/\/[^/]+$/, ''));
  await flow.getByRole('button', { name: 'Look inside' }).click();
  await flow.getByRole('button', { name: 'Bring them in' }).click();
  await expect(flow.getByRole('heading')).toHaveText('Connect your school calendar');
  await expect(page.locator('.toast')).toHaveText('Brought in 1 file.');
  const core = await connect();
  const found = await core.call<{ clips: { title: string; verdict: string }[] }>('library.search', {
    q: 'first launch take',
  });
  core.close();
  expect(found.clips.map((c) => c.title)).toContain('first launch take');
});
