import { expect, test } from '@playwright/test';

test('the page loads', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.case-bar')).toBeVisible();
  await expect(page.locator('main')).toBeVisible();
});
