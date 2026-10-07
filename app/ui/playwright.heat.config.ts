import { defineConfig, devices } from '@playwright/test';

// Heat's own end-to-end specs (e2e-heat/): Chromium against `npm run dev`
// with VITE_FAKE_CORE=1, so the whole UI runs on Heat's fake core and its
// seeded sample with no Rust behind it. The fake's clock starts where a spec's
// ?fakeNow says and runs at the machine's pace; the time zone is New York's.
//
//   npm run e2e:heat            (HEAT_E2E_PORT=5200 npm run e2e:heat, if 5197 is taken)
const PORT = Number(process.env.HEAT_E2E_PORT ?? 5197);

export default defineConfig({
  testDir: 'e2e-heat',
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  use: { baseURL: `http://localhost:${PORT}`, timezoneId: 'America/New_York', viewport: { width: 1280, height: 800 } },
  webServer: {
    command: `npm run dev -- --port ${PORT} --strictPort`,
    env: { VITE_FAKE_CORE: '1' },
    url: `http://localhost:${PORT}`,
    reuseExistingServer: false,
    timeout: 60_000,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1280, height: 800 } } }],
});
