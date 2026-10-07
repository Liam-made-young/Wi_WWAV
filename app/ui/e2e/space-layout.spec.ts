import { expect, test } from '@playwright/test';
import { TRIG_FINGERPRINT } from '../src/shared/dmath/fingerprint';
import { LAYOUT_HASH, LAYOUT_HASH_LATER } from '../src/space/model/fixtures/layoutHash';

// docs/PLAN.md S4.1, "the same sky everywhere": the fixed catalogue laid out
// inside the browser's own engine must hash to the values pinned in Node.
// The module loads through Vite, exactly as the app loads it.
test('the sky lays out to the same bits in the browser as in Node', async ({ page }) => {
  await page.goto('/');
  const hashes = await page.evaluate(async () => {
    const path = '/src/space/model/fixtures/layoutHash.ts';
    const layout = await import(/* @vite-ignore */ path);
    return layout.hashesHere();
  });
  expect(hashes).toEqual({ layout: LAYOUT_HASH, later: LAYOUT_HASH_LATER, trig: TRIG_FINGERPRINT });
});
