import { defineConfig, devices } from '@playwright/test';

// Chromium runs here against the dev bridge; the WebKit pass runs against the
// built app through WebKitGTK's WebDriver (docs/DECISIONS.md).
export default defineConfig({
  testDir: 'e2e',
  fullyParallel: false,
  reporter: [['list']],
  use: { baseURL: 'http://localhost:5173', viewport: { width: 1280, height: 800 } },
  // Starts the mock server, the dev bridge (the real core, with mock-engine)
  // and Vite pointed at that bridge, on a fresh library, so `npm run e2e`
  // stands alone (e2e/support/global-setup.ts).
  globalSetup: './e2e/support/global-setup.ts',
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } } }],
});
