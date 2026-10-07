import { defineConfig, devices } from '@playwright/test';

// Chromium runs here against the dev bridge; the WebKit pass runs against the
// built app through WebKitGTK's WebDriver (docs/DECISIONS.md).
export default defineConfig({
  testDir: 'e2e',
  fullyParallel: false,
  // One core serves every test, and its events reach every page that is open:
  // a test that adds a task or undoes an edit moves another's screen and
  // drops its redo. So the tests take turns.
  workers: 1,
  reporter: [['list']],
  // A click or a fill that can't find its element fails in 15 s, not at the end of the test.
  use: {
    baseURL: `http://localhost:${process.env.WI_E2E_PORT ?? 5173}`,
    viewport: { width: 1280, height: 800 },
    actionTimeout: 15_000,
  },
  // Starts the mock server, the dev bridge (the real core, with mock-engine)
  // and Vite pointed at that bridge, on a fresh library, so `npm run e2e`
  // stands alone (e2e/support/global-setup.ts).
  globalSetup: './e2e/support/global-setup.ts',
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } } }],
});
