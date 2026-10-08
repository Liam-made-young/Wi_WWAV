// The Focus layout, in a browser on the fake core (docs/FOCUS.md): what a
// launch shows, the three ways to a tool, Esc, and the ⌘ map.

import { test as base, expect, type Page } from '@playwright/test';
import { CMD, MORNING } from './kit';

const test = base.extend({
  page: async ({ page }, use) => {
    await page.addInitScript(() => localStorage.setItem('wi.firstLaunch', 'done'));
    await use(page);
  },
});

async function open(page: Page) {
  await page.goto(`/?fakeNow=${encodeURIComponent(MORNING)}&fakeZone=America%2FNew_York`);
  await page.locator('.focus-title').waitFor();
}

test('a launch shows the readout, the Now task with its timer, and at most one interrupt line', async ({ page }) => {
  await open(page);
  await expect(page.locator('.focus-title')).toHaveText('Mix the second verse');
  await expect(page.getByRole('img', { name: /^Readout\. Now: Mix the second verse/ })).toBeVisible();
  await expect(page.getByRole('timer')).toHaveText('25:00');
  expect(await page.locator('.focus-interrupt').count()).toBeLessThanOrEqual(1);
  for (const gone of ['.case-bar', '.heat-toolbar', '.heat-sidebar', '.heat-right', '.case-status', '.top-switch']) {
    await expect(page.locator(gone).first()).not.toBeInViewport();
  }
  // The readout is dots on a canvas, drawn: its field is not blank.
  const lit = await page.locator('.readout-dots').evaluate((c: HTMLCanvasElement) => {
    const d = c.getContext('2d')!.getImageData(0, 0, c.width, c.height).data;
    let n = 0;
    for (let i = 0; i < d.length; i += 4) if (d[i] > 200) n += 1;
    return n;
  });
  expect(lit).toBeGreaterThan(500);
});

test('every tool opens by its number, by ⌘K and from the left edge, and Esc returns to Focus', async ({ page }) => {
  await open(page);
  await page.keyboard.press('3');
  await expect(page.locator('.tool-name')).toHaveText('Calendar');
  await page.keyboard.press('Escape');
  await expect(page.locator('.focus-title')).toBeVisible();

  await page.keyboard.press(`${CMD}+k`);
  await page.locator('.palette-input').fill('grades');
  await page.keyboard.press('Enter');
  await expect(page.locator('.tool-name')).toHaveText('Grades');
  await page.keyboard.press('Escape');
  await expect(page.locator('.focus-title')).toBeVisible();

  await page.mouse.move(600, 400);
  await page.mouse.move(3, 400);
  const tools = page.getByRole('navigation', { name: 'Tools' });
  await expect(tools).toBeInViewport();
  await tools.getByRole('button', { name: 'Mail' }).click();
  await expect(page.locator('.tool-name')).toHaveText('Mail');
  await expect(tools).not.toBeInViewport();
  await page.keyboard.press('Escape');
  await expect(page.locator('.focus-title')).toBeVisible();
});

test('holding ⌘ shows the map, and letting go puts it away', async ({ page }) => {
  await open(page);
  const map = page.getByRole('note', { name: 'Map of tools and shortcuts' });
  await page.keyboard.down(CMD);
  await expect(map).toBeVisible();
  await expect(map).toContainText('Tasks');
  await page.keyboard.up(CMD);
  await expect(map).toBeHidden();
  // ⌘K is a shortcut, not a hold: no map.
  await page.keyboard.press(`${CMD}+k`);
  await expect(map).toBeHidden();
});

test('the view switch comes down from the top edge, and ⌥2 goes without it', async ({ page }) => {
  await open(page);
  await page.mouse.move(600, 400);
  await page.mouse.move(600, 1);
  const views = page.getByRole('navigation', { name: 'Views' });
  await expect(views).toBeInViewport();
  await views.getByRole('button', { name: 'Space' }).click();
  await expect(page.locator('.room[data-room="space"]')).toHaveAttribute('data-current', 'true');
  await page.keyboard.press('Alt+1');
  await expect(page.locator('.room[data-room="heat"]')).toHaveAttribute('data-current', 'true');
});

test('Classic layout still works behind the setting', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('wi.layout', 'classic'));
  await page.goto(`/?fakeNow=${encodeURIComponent(MORNING)}&fakeZone=America%2FNew_York`);
  await expect(page.locator('.heat-toolbar')).toBeVisible();
  await expect(page.locator('.case-bar .strip')).toBeVisible();
  await expect(page.locator('.heat-right')).toBeVisible();
  await expect(page.locator('.focus')).toHaveCount(0);
});

test('no gradient is left in the Focus layout’s shell', async ({ page }) => {
  await open(page);
  await page.keyboard.press('2');
  await expect(page.locator('.tool-name')).toHaveText('Tasks');
  const found = await page.evaluate(() =>
    [...document.querySelectorAll('.case *')]
      .filter((el) => (el as HTMLElement).checkVisibility({ visibilityProperty: true }))
      .flatMap((el) =>
        [getComputedStyle(el), getComputedStyle(el, '::before'), getComputedStyle(el, '::after')].flatMap((s) => {
          const image = s.backgroundImage;
          if (!/gradient/.test(image)) return [];
          // A gradient whose stops are all one colour is a flat fill.
          const stops = [...image.matchAll(/(?:rgba?|oklab|color)\([^)]*\)|transparent/g)].map((m) => m[0]);
          return new Set(stops).size > 1 ? [`${el.className}: ${image}`] : [];
        }),
      ),
  );
  expect(found).toEqual([]);
});
