import { CMD, expect, openShell, test } from './kit';

// docs/PLAN.md S0.1, S1.1 and S1.2. What a fail looks like: ⌘1–⌘3 or a
// click on a segment doesn't change the view, or there is a fourth segment
// or a ⌘4; the title bar isn't 52 pt with the switcher (76 pt segments), the
// Now strip, the search pill and the galaxy chip; below 1180 pt the pill
// isn't a 28 pt magnifier and the strip isn't 440 pt; anything overflows at
// 1024 × 680; switching away and back loses a view's scroll, selection,
// open sheet or half-typed text, or rebuilds the view; a view change isn't a
// 140 ms cross-fade, or isn't a cut under Reduce Motion.

const ROOMS = ['heat', 'space', 'console'] as const;
const NAMES = ['Learn', 'Space', 'Console'];

test('⌘1–⌘3 and the segments switch between the three views, and there is no fourth', async ({ page }) => {
  await openShell(page);
  for (const [i, room] of ROOMS.entries()) {
    await page.keyboard.press(`${CMD}+${i + 1}`);
    await expect(page.locator(`[data-room="${room}"]`)).toHaveAttribute('data-current', 'true');
    await expect(page.getByRole('tab', { name: NAMES[i] })).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('[data-current="true"]')).toHaveCount(1);
  }
  for (const [i, room] of [...ROOMS.entries()].reverse()) {
    await page.getByRole('tab', { name: NAMES[i] }).click();
    await expect(page.locator(`[data-room="${room}"]`)).toHaveAttribute('data-current', 'true');
  }
  // ⌘1 reaches the shell from inside a text field too.
  await page.keyboard.press(`${CMD}+k`);
  await page.getByRole('combobox').fill('half');
  await page.keyboard.press(`${CMD}+3`);
  await expect(page.locator('[data-room="console"]')).toHaveAttribute('data-current', 'true');
  // The shop is gone: three tabs, three views, and ⌘4 does nothing.
  await expect(page.getByRole('tab')).toHaveCount(3);
  await expect(page.locator('[data-room]')).toHaveCount(3);
  await page.keyboard.press(`${CMD}+1`);
  await page.keyboard.press(`${CMD}+4`);
  await expect(page.locator('[data-room="heat"]')).toHaveAttribute('data-current', 'true');
});

test('the title bar holds its four controls at 52 pt, and narrows below 1180 pt', async ({ page }) => {
  await openShell(page);
  const bar = await page.locator('.case-bar').boundingBox();
  expect(bar!.height).toBe(52);
  for (const name of NAMES) expect((await page.getByRole('tab', { name }).boundingBox())!.width).toBe(76);
  expect((await page.locator('.strip').boundingBox())!.width).toBe(520);
  expect((await page.locator('.strip').boundingBox())!.height).toBe(44);
  await expect(page.locator('.search-pill')).toBeVisible();
  await expect(page.getByRole('button', { name: 'You' })).toBeVisible();

  await page.setViewportSize({ width: 1179, height: 800 });
  await expect(page.locator('.strip')).toHaveAttribute('data-narrow', 'true');
  expect((await page.locator('.strip').boundingBox())!.width).toBe(440);
  await expect(page.locator('.search-pill')).toHaveCount(0);
  const glass = await page.locator('.magnifier').evaluate((el) => {
    const ring = getComputedStyle(el, '::before');
    const box = el.getBoundingClientRect();
    return { w: box.width - parseFloat(ring.left) - parseFloat(ring.right), target: box.width };
  });
  expect(glass).toEqual({ w: 28, target: 44 });
  // The galaxy chip is a 28 pt miniature of your own galaxy.
  const galaxy = await page.locator('.galaxy').boundingBox();
  expect([galaxy!.width, galaxy!.height]).toEqual([28, 28]);
});

test('nothing overflows at 1024 × 680', async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 680 });
  await openShell(page);
  const over = await page.evaluate(() => {
    const out: string[] = [];
    const width = window.innerWidth;
    if (document.documentElement.scrollWidth > width) out.push(`page ${document.documentElement.scrollWidth}`);
    const bar = document.querySelector('.case-bar')!.getBoundingClientRect();
    for (const el of document.querySelectorAll('.case-bar > *')) {
      const r = el.getBoundingClientRect();
      if (r.right > bar.right || r.left < bar.left || r.bottom > bar.bottom)
        out.push(`${el.className} ${r.left}–${r.right}`);
    }
    return out;
  });
  expect(over).toEqual([]);
  expect((await page.locator('.case-status').boundingBox())!).toMatchObject({ y: 680 - 22, height: 22 });
});

test('a view keeps its scroll, selection, open sheet and half-typed text, and is never rebuilt', async ({ page }) => {
  await openShell(page);
  // Put state into every view: a tall page scrolled down, a field with
  // half a sentence in it, a selected row and an open sheet. If a view were
  // unmounted, React would build it again without any of this.
  for (const [i, room] of ROOMS.entries()) {
    await page.keyboard.press(`${CMD}+${i + 1}`);
    await page.evaluate((room) => {
      const section = document.querySelector(`[data-room="${room}"]`)!;
      const extra = document.createElement('div');
      extra.innerHTML = `<div style="height:3000px"></div><input data-probe aria-label="probe"><div role="option" data-probe-row aria-selected="false">row</div><div data-probe-sheet>sheet</div>`;
      section.append(extra);
    }, room);
    await page.locator(`[data-room="${room}"] [data-probe]`).fill(`half-typed in ${room}`);
    await page
      .locator(`[data-room="${room}"] [data-probe-row]`)
      .evaluate((r) => r.setAttribute('aria-selected', 'true'));
    // Scrolled last, since typing scrolls the field into view.
    await page.locator(`[data-room="${room}"]`).evaluate((s, room) => (s.scrollTop = 1200 + 10 * room.length), room);
  }
  for (const [i, room] of [...ROOMS.entries()].reverse()) {
    await page.keyboard.press(`${CMD}+${i + 1}`);
    const kept = await page.evaluate((room) => {
      const section = document.querySelector(`[data-room="${room}"]`)!;
      return {
        scroll: section.scrollTop,
        text: (section.querySelector('[data-probe]') as HTMLInputElement | null)?.value,
        selected: section.querySelector('[data-probe-row]')?.getAttribute('aria-selected'),
        sheet: !!section.querySelector('[data-probe-sheet]'),
        focused: document.activeElement === section.querySelector('[data-probe]'),
      };
    }, room);
    expect(kept).toEqual({
      scroll: 1200 + 10 * room.length,
      text: `half-typed in ${room}`,
      selected: 'true',
      sheet: true,
      focused: true,
    });
  }
});

test('a view change is a 140 ms cross-fade, and a cut under Reduce Motion', async ({ page }) => {
  await openShell(page);
  const fade = () =>
    page.locator('[data-room="space"]').evaluate((el) => {
      const s = getComputedStyle(el);
      return { property: s.transitionProperty, duration: s.transitionDuration };
    });
  expect(await fade()).toEqual({ property: 'opacity, visibility', duration: '0.14s, 0s' });
  // Midway through a switch both views are partly there: a cross-fade, not a cut.
  await page.keyboard.press(`${CMD}+2`);
  const midway = await page.evaluate(
    () =>
      new Promise<number>((done) =>
        setTimeout(() => done(Number(getComputedStyle(document.querySelector('[data-room="space"]')!).opacity)), 50),
      ),
  );
  expect(midway).toBeGreaterThan(0);
  expect(midway).toBeLessThan(1);

  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(page.locator('html')).toHaveAttribute('data-reduce-motion', 'true');
  expect((await fade()).duration).toBe('0s');
  await page.keyboard.press(`${CMD}+1`);
  expect(await page.locator('[data-room="heat"]').evaluate((el) => getComputedStyle(el).opacity)).toBe('1');
});
