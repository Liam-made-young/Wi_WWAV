// What Heat's end-to-end specs share: the app on the fake core at a known
// morning, past first launch, with a handle on the fake's clock.

import { test as base, expect, type Page } from '@playwright/test';

export { expect };

/** ⌘ as this machine's shell reads it: Command on a Mac, Ctrl elsewhere. */
export const CMD = 'ControlOrMeta';

/** 7 Oct 2026, 10:00 AM in New York, a Wednesday. */
export const MORNING = '2026-10-07T10:00:00-04:00';

export const test = base.extend({
  page: async ({ page }, use) => {
    await page.addInitScript(() => {
      localStorage.setItem('wi.firstLaunch', 'done');
      // These specs walk the Classic layout; the Focus layout has its own (e2e-heat/focus.spec.ts).
      localStorage.setItem('wi.layout', 'classic');
    });
    await use(page);
  },
});

/** Opens the app on the fake core, with its clock at `at`, and waits for Heat. */
export async function openHeat(page: Page, at = MORNING) {
  await page.goto(`/?fakeNow=${encodeURIComponent(at)}&fakeZone=America%2FNew_York`);
  await page.locator('.heat-toolbar').waitFor();
  await expect(page.locator('.heat-spaces .heat-side-row').first()).toContainText('All');
}

/** Moves the fake core's clock on by `minutes`, as if that long had passed. */
export async function advance(page: Page, minutes: number) {
  await page.evaluate(
    (ms) => (window as unknown as { __wiFake: { advance(ms: number): void } }).__wiFake.advance(ms),
    minutes * 60_000,
  );
}

export const tab = (page: Page, name: string) => page.getByRole('tab', { name, exact: true });
