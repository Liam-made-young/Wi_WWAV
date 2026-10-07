import type { Page } from '@playwright/test';
import { audit, judge } from '../e2e/shell/measure';
import { CMD, expect, openHeat, tab, test } from './kit';

// Gate 2.4 for Heat (docs/GATES.md, docs/SPEC.md 2.12, 3.18, 8.9), measured on
// what Chromium draws, as the shell's own gate does: body text 7:1 and
// secondary 4.5:1, in light and dark; buttons, tabs and orbs at least
// 44 × 44, dense rows and grid cells (data-dense, options, menu items,
// fields) at least 24 × 24, with every action on a key; nothing under 11 pt.
// What a fail looks like: any of those, in any tab, at either window size.

const SIZES = [
  { width: 1024, height: 680 },
  { width: 1280, height: 800 },
];

async function judged(page: Page) {
  const found = judge(await audit(page));
  expect(found.fails).toEqual([]);
  return found;
}

for (const colorScheme of ['light', 'dark'] as const) {
  for (const size of SIZES) {
    test.describe(`${colorScheme}, ${size.width} × ${size.height}`, () => {
      test.use({ colorScheme, viewport: size });

      test('Today passes the bars, with a block, a draft and Get Info', async ({ page }) => {
        await openHeat(page);
        await judged(page);
        await page.getByRole('button', { name: 'Plan my day' }).click();
        await expect(page.locator('[data-draft]').first()).toBeVisible();
        await judged(page);
        await page.keyboard.press('Enter');
        await page.locator('.heat-row', { hasText: 'Problem set 5' }).click();
        await expect(page.locator('.heat-info')).toBeVisible();
        await judged(page);
      });

      test('Tasks passes the bars, with a task selected', async ({ page }) => {
        await openHeat(page);
        await tab(page, 'Tasks').click();
        await judged(page);
        await page.locator('.heat-trow', { hasText: 'Order PCBs' }).click();
        await expect(page.locator('.heat-info')).toBeVisible();
        await judged(page);
      });

      test('Calendar passes the bars in Month, Week and Day', async ({ page }) => {
        await openHeat(page);
        await tab(page, 'Calendar').click();
        await judged(page);
        await page.keyboard.press('w');
        await expect(page.locator('.heat-week')).toBeVisible();
        await judged(page);
        await page.keyboard.press('d');
        await judged(page);
      });

      test('the sheets and the folded column’s popover pass the bars', async ({ page }) => {
        await openHeat(page);
        await page.keyboard.press('n');
        await expect(page.getByRole('dialog', { name: 'New task' })).toBeVisible();
        await judged(page);
        await page.keyboard.press('Escape');
        await page.keyboard.press('1');
        if (size.width < 1240) {
          await page.getByRole('button', { name: 'Hot tasks' }).click();
          await expect(page.getByRole('dialog', { name: 'Hot tasks' })).toBeVisible();
          await judged(page);
        }
        await page.keyboard.press(`${CMD}+2`);
        await page.keyboard.press(`${CMD}+1`);
        await judged(page);
      });
    });
  }
}
