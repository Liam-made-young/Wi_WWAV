import { defineConfig } from '@playwright/test';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const library = mkdtempSync(join(tmpdir(), 'wi-console-e2e-'));
const quote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
const uiPort = 5185;
const bridgePort = 8795;
export default defineConfig({
  testDir: 'e2e-console', workers: 1, fullyParallel: false,
  outputDir: join(tmpdir(), 'wi-console-browser-results'),
  reporter: 'list',
  use: { baseURL: `http://127.0.0.1:${uiPort}`, browserName: 'chromium', channel: 'chrome', viewport: { width: 1280, height: 800 }, trace: 'retain-on-failure' },
  webServer: [
    { command: `cargo run --offline --manifest-path ../../Cargo.toml -p wi-devbridge -- --library ${quote(library)} --engine ${quote(join(library, 'no-engine'))} --port ${bridgePort}`, port: bridgePort, timeout: 120_000, reuseExistingServer: false },
    { command: `npm run dev -- --host 127.0.0.1 --port ${uiPort}`, env: { VITE_BRIDGE_URL: `ws://127.0.0.1:${bridgePort}` }, port: uiPort, timeout: 30_000, reuseExistingServer: false },
  ],
});
