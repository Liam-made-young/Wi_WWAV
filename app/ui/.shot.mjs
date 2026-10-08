// Drives the real UI (Vite + wi-devbridge) in the installed Chrome.
// node shot.mjs <script.mjs>  — the script exports default async (page, shot) => {}
import { chromium } from '@playwright/test';
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1, permissions: ['clipboard-read', 'clipboard-write'] });
const page = await context.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));
page.on('console', (m) => m.type() === 'error' && errors.push(`console: ${m.text()}`));
await page.addInitScript(() => localStorage.setItem('wi.firstLaunch', 'done'));
await page.goto('http://localhost:5313/');
await page.locator('.focus, .heat-toolbar').first().waitFor({ timeout: 20000 });
const shot = async (name) => { await page.screenshot({ path: `/tmp/wi-spike/shots/${name}.png` }); console.log('shot', name); };
try {
  const run = (await import(process.argv[2])).default;
  await run(page, shot);
} catch (e) { console.log('SCRIPT ERROR', e.message.split('\n').slice(0, 6).join('\n')); await shot('error'); }
if (errors.length) console.log('ERRORS\n' + [...new Set(errors)].slice(0, 12).join('\n'));
await browser.close();
