import { expect, test } from '@playwright/test';

test('the page loads', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('main')).toHaveText('Wi_WWAV');
});
