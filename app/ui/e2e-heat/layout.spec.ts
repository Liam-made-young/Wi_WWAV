import { expect, openHeat, tab, test } from './kit';

// docs/PLAN.md S1.1 and docs/SPEC.md 3.3: Heat fills the view area, 1280 × 726
// at the default window and 1024 × 606 at the smallest, with nothing
// overflowing. What a fail looks like: the page or Heat's room scrolling, a
// region of the frame wider than itself, or anything in the showing tab
// sticking out past its panel (what sits in a scroller is allowed to scroll).

const SIZES = [
  { width: 1024, height: 680 },
  { width: 1280, height: 800 },
];
const TABS = ['Today', 'Tasks', 'Calendar'];

for (const size of SIZES) {
  for (const name of TABS) {
    test(`${name} renders with no overflow at ${size.width} × ${size.height}`, async ({ page }) => {
      await page.setViewportSize(size);
      await openHeat(page);
      await tab(page, name).click();
      if (name === 'Calendar') await expect(page.locator('.heat-month')).toBeVisible();
      await page.waitForTimeout(200);

      const over = await page.evaluate(() => {
        const out: string[] = [];
        const doc = document.documentElement;
        if (doc.scrollWidth > innerWidth || doc.scrollHeight > innerHeight)
          out.push(`page ${doc.scrollWidth} × ${doc.scrollHeight}`);
        const room = document.querySelector('[data-room="heat"]')!;
        if (room.scrollWidth > room.clientWidth || room.scrollHeight > room.clientHeight) {
          out.push(`room ${room.scrollWidth} × ${room.scrollHeight} in ${room.clientWidth} × ${room.clientHeight}`);
        }
        for (const sel of [
          '.heat-toolbar',
          '.heat-sidebar',
          '.heat-main',
          '.heat-right',
          '.heat-tabpanel[data-current="true"]',
        ]) {
          const el = document.querySelector(sel)!;
          if (el.scrollWidth > el.clientWidth + 1)
            out.push(`${sel} scrolls sideways: ${el.scrollWidth} in ${el.clientWidth}`);
        }
        const bar = document.querySelector('.heat-toolbar')!.getBoundingClientRect();
        for (const el of document.querySelectorAll('.heat-toolbar > *')) {
          const r = el.getBoundingClientRect();
          if (r.right > bar.right + 0.5 || r.left < bar.left - 0.5 || r.bottom > bar.bottom + 0.5)
            out.push(`toolbar ${el.className} ${r.left}–${r.right}`);
        }
        // Anything in the showing tab that sticks out past its panel, unless a scroller holds it.
        const panel = document.querySelector('.heat-tabpanel[data-current="true"]')!;
        const box = panel.getBoundingClientRect();
        const scrolls = (el: Element) => {
          for (let e: Element | null = el.parentElement; e && e !== panel; e = e.parentElement) {
            const o = getComputedStyle(e);
            if (/(auto|scroll|hidden)/.test(o.overflowX) || /(auto|scroll)/.test(o.overflowY)) return true;
          }
          return false;
        };
        for (const el of panel.querySelectorAll('*')) {
          if (!el.checkVisibility() || scrolls(el)) continue;
          const r = el.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) continue;
          if (r.right > box.right + 1 || r.left < box.left - 1)
            out.push(
              `${el.tagName.toLowerCase()}.${String(el.className).slice(0, 40)} ${Math.round(r.left)}–${Math.round(r.right)} outside ${Math.round(box.left)}–${Math.round(box.right)}`,
            );
        }
        return out.slice(0, 10);
      });
      expect(over).toEqual([]);

      // The case holds: a 52 pt title bar and a 22 pt status bar, with Heat between.
      const heat = await page.locator('[data-room="heat"]').boundingBox();
      expect(heat).toMatchObject({ x: 0, y: 52, width: size.width, height: size.height - 52 - 22 });
      await expect(page.locator('.case-status')).toHaveCSS('height', '22px');
    });
  }
}

test('the right column is five panels at 1280 and a 44 px strip of icons at 1024', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await openHeat(page);
  expect((await page.locator('.heat-right').boundingBox())!.width).toBe(290);
  await expect(page.locator('.heat-widget-title')).toHaveText(['Now', 'Habits', 'Hot tasks', 'Mail', 'Grades']);
  await page.setViewportSize({ width: 1239, height: 800 });
  await expect(page.locator('.heat-strip')).toBeVisible();
  expect((await page.locator('.heat-right').boundingBox())!.width).toBe(44);
  await page.getByRole('button', { name: 'Hot tasks' }).click();
  await expect(page.getByRole('dialog', { name: 'Hot tasks' })).toContainText('Grammar quiz 4');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Hot tasks' })).toHaveCount(0);
  await page.setViewportSize({ width: 1240, height: 800 });
  await expect(page.locator('.heat-strip')).toHaveCount(0);
});

test('the sidebar is 190 px and the spaces filter reads All, each space with its count, and New space…', async ({
  page,
}) => {
  await openHeat(page);
  expect((await page.locator('.heat-sidebar').boundingBox())!.width).toBe(190);
  await expect(page.locator('.heat-spaces .heat-side-row')).toHaveText([
    'All11',
    'Classes4',
    'WWAV5',
    'Personal2',
    'New space…',
  ]);
});
