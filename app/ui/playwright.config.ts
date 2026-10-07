import { defineConfig, devices } from '@playwright/test';

// Chromium runs here against the dev bridge; the WebKit pass runs against the
// built app through WebKitGTK's WebDriver (docs/DECISIONS.md).
export default defineConfig({
  testDir: 'e2e',
  fullyParallel: false,
  reporter: [['list']],
  use: { baseURL: 'http://localhost:5173', viewport: { width: 1280, height: 800 } },
  // Starts the dev server when none is running, so `npm run e2e` stands alone.
  webServer: { command: 'npm run dev', url: 'http://localhost:5173', reuseExistingServer: true, timeout: 60_000 },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } } }],
});
