/// <reference types="node" />
import { expect, test } from '@playwright/test';

// The page reaches the real core through src/bridge and the dev bridge the
// global setup started: an answer, a CoreError, and an event, in Chromium.
test('the UI reaches the core through the dev bridge', async ({ page }) => {
  await page.goto('/');
  const seen = await page.evaluate(async () => {
    const path = '/src/bridge/index.ts';
    const bridge = await import(/* @vite-ignore */ path);
    const hello = await bridge.call('app.hello');
    const refusal = await bridge.call('history.undo', { room: 'console' }).catch((e: Error & { code: string }) => ({
      code: e.code,
      message: e.message,
      isCoreError: e instanceof bridge.CoreError,
    }));
    const event = new Promise((ok) => {
      const off = bridge.on('records', (payload: unknown) => {
        off();
        ok(payload);
      });
    });
    await bridge.call('records.mutate', {
      label: 'add task',
      room: 'heat',
      ops: [{ op: 'put', kind: 'task', id: 'e2e', value: { title: 'Bridge check' } }],
    });
    return { hello, refusal, event: await event };
  });
  expect(seen.hello).toMatchObject({ library: process.env.WI_E2E_LIBRARY, signedIn: false });
  expect(seen.refusal).toEqual({ code: 'nothing_to_undo', message: 'Nothing to undo.', isCoreError: true });
  expect(seen.event).toEqual({ kinds: ['task'] });
});
