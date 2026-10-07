// The real app on Linux, driven through WebKitGTK's own WebDriver (S0.1,
// docs/PLAN.md; docs/DECISIONS.md on why WebKitGTK stands in for WKWebView).
//
//   cd app/src-tauri && cargo tauri build --debug --no-bundle
//   node app/ui/e2e-webkit/run.mjs [--app target/debug/wi-wwav]
//
// It needs tauri-driver (cargo install tauri-driver --locked),
// WebKitWebDriver (Debian and Ubuntu: webkit2gtk-driver), xdotool, and a
// display; with none, it starts Xvfb.
//
// What a fail looks like, written before the checks:
// - The window doesn't open, or opens with anything but the UI built into
//   the app (tauri://localhost, not a dev server), or not at 1280 × 800, or
//   shrinks below 1024 × 680.
// - Ctrl+1 to Ctrl+4 pressed on the window (real key events through GTK,
//   which hands them to the View menu) don't reach the page as the menu
//   actions room.heat, room.space, room.console and room.unquantized.
// - Ctrl+1 to Ctrl+4 don't change which room the switcher shows, whether
//   they come through the menu or straight to the page.
// The cold start to the first paint is printed for information: the 1.5 s
// budget is measured on the reference Mac (docs/SPEC.md 9.13).

import { spawn, execFile, execFileSync } from 'node:child_process';
import { promisify } from 'node:util';
import { existsSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import { WebDriver } from './webdriver.mjs';

const ROOT = resolve(import.meta.dirname, '../../..');
const ROOMS = ['Heat', 'Space', 'Console', 'Unquantized'];
const argApp = process.argv.indexOf('--app');
const app = resolve(argApp > 0 ? process.argv[argApp + 1] : join(ROOT, 'target/debug/wi-wwav'));

const children = [];
const library = mkdtempSync(join(tmpdir(), 'wi-wwav-e2e-'));
let failures = 0;

function check(ok, what, detail = '') {
  console.log(`${ok ? 'pass' : 'FAIL'}  ${what}${detail ? `: ${detail}` : ''}`);
  if (!ok) failures++;
}

function start(cmd, args, env = {}) {
  const child = spawn(cmd, args, { env: { ...process.env, ...env }, stdio: ['ignore', 'ignore', 'pipe'] });
  let stderr = '';
  child.stderr.on('data', (d) => (stderr += d));
  child.on('error', (e) => console.error(`${cmd}: ${e.message}`));
  children.push(child);
  return { child, stderr: () => stderr };
}

async function until(what, test, ms = 15000) {
  const end = Date.now() + ms;
  for (;;) {
    const value = await test().catch(() => undefined);
    if (value) return value;
    if (Date.now() > end) throw new Error(`timed out waiting for ${what}`);
    await sleep(50);
  }
}

async function display() {
  if (process.env.DISPLAY) return process.env.DISPLAY;
  for (let n = 90; n < 100; n++) {
    if (existsSync(`/tmp/.X11-unix/X${n}`)) continue;
    start('Xvfb', [`:${n}`, '-screen', '0', '1920x1200x24', '-nolisten', 'tcp']);
    await until('Xvfb', async () => existsSync(`/tmp/.X11-unix/X${n}`));
    return `:${n}`;
  }
  throw new Error('no free display number for Xvfb');
}

// Real key events, through the X server and GTK, as a person's would be.
function pressOnWindow(DISPLAY, keys) {
  const env = { ...process.env, DISPLAY };
  const id = execFileSync('xdotool', ['search', '--name', '^Wi_WWAV$'], { env }).toString().trim().split('\n')[0];
  execFileSync('xdotool', ['windowfocus', '--sync', id], { env });
  execFileSync('xdotool', ['key', '--clearmodifiers', keys], { env });
}

// When a visible window called Wi_WWAV first exists, as Date.now().
async function windowShown(DISPLAY) {
  const env = { ...process.env, DISPLAY };
  const run = promisify(execFile);
  await until('the window', () => run('xdotool', ['search', '--onlyvisible', '--name', '^Wi_WWAV$'], { env }).then(() => true), 30000);
  return Date.now();
}

// The room the switcher shows as chosen, by the accessibility state a
// segmented control carries.
const CURRENT_ROOM = `
  const names = ${JSON.stringify(ROOMS)};
  const on = '[aria-selected="true"],[aria-checked="true"],[aria-pressed="true"],[aria-current="page"],[aria-current="true"]';
  const chosen = [...document.querySelectorAll(on)].map((e) => e.textContent.trim()).filter((t) => names.includes(t));
  const shown = [...document.querySelectorAll('button, [role]')].some((e) => names.includes(e.textContent.trim()));
  return { chosen, shown };`;

// Collects the shell's events in the page, as @tauri-apps/api's listen() does.
const LISTEN = `
  const done = arguments[arguments.length - 1];
  window.__heard = [];
  const t = window.__TAURI_INTERNALS__;
  const handler = t.transformCallback((e) => window.__heard.push(e.payload));
  t.invoke('plugin:event|listen', { event: 'core', target: { kind: 'Any' }, handler }).then(() => done(true), (e) => done(String(e)));`;

async function main() {
  if (!existsSync(app)) throw new Error(`no app at ${app}; build it: cd app/src-tauri && cargo tauri build --debug --no-bundle`);
  const DISPLAY = await display();
  const port = 4444 + Math.floor(Math.random() * 1000);
  const driver = start('tauri-driver', ['--port', String(port), '--native-port', String(port + 1)], {
    DISPLAY,
    WI_WWAV_LIBRARY: library,
  });
  const wd = new WebDriver(`http://127.0.0.1:${port}`);
  await until('tauri-driver', () => wd.status()).catch((e) => {
    throw new Error(`${e.message}\n${driver.stderr()}`);
  });

  const launched = Date.now();
  const shown = windowShown(DISPLAY);
  await wd.newSession({ 'tauri:options': { application: app } });
  try {
    await until('the page to load', () => wd.run('return document.readyState === "complete"'));
    // The paint entry can land a moment after load.
    await until('a paint entry', () => wd.run('return performance.getEntriesByType("paint").length > 0'), 1000).catch(() => {});
    const page = await wd.run(`return {
      url: location.href, title: document.title, rendered: !!document.querySelector('#root')?.childElementCount,
      origin: performance.timeOrigin, paints: performance.getEntriesByType('paint').map((p) => [p.name, p.startTime]),
      loaded: performance.getEntriesByType('navigation')[0]?.loadEventEnd }`);
    check(page.url.startsWith('tauri://localhost') && page.rendered && page.title === 'Wi_WWAV', 'the window opens with the UI from dist', page.url);

    // WebKitGTK keeps no paint timing, so the first paint is bounded by the
    // window appearing and the page finishing its load (React renders the
    // shell before load ends; the paint follows within a frame).
    const paint = Object.fromEntries(page.paints)['first-contentful-paint'];
    const ms = (t) => `${Math.round(t - launched)} ms`;
    const times = [`window shown ${ms(await shown)}`, `UI loaded ${ms(page.origin + page.loaded)}`];
    if (paint !== undefined) times.push(`first contentful paint ${ms(page.origin + paint)}`);
    console.log(`info  cold start: ${times.join(', ')} (Linux, ${app.includes('/debug/') ? 'a debug build' : 'a release build'}; information only: the 1.5 s budget is for the reference Mac)`);

    const rect = await wd.rect();
    check(rect.width === 1280 && rect.height === 800, 'it opens at 1280 × 800', `${rect.width} × ${rect.height}`);
    await wd.setRect({ width: 800, height: 500 });
    const small = await until('the resize', async () => {
      const r = await wd.rect();
      return r.width !== rect.width || r.height !== rect.height ? r : undefined;
    }, 3000).catch(() => wd.rect());
    check(small.width >= 1024 && small.height >= 680, 'it shrinks no further than 1024 × 680', `${small.width} × ${small.height}`);
    await wd.setRect({ width: 1280, height: 800 });

    const listening = await wd.runAsync(LISTEN);
    check(listening === true, 'the page can listen to the shell', String(listening));
    const actions = [];
    for (let n = 1; n <= 4; n++) {
      pressOnWindow(DISPLAY, `ctrl+${n}`);
      const heard = await until(`the menu action for Ctrl+${n}`, async () => {
        const all = await wd.run('return window.__heard');
        return all.length >= n ? all : undefined;
      }, 3000).catch(() => []);
      actions.push(heard[n - 1]?.event === 'menu' ? heard[n - 1].payload.action : '—');
    }
    const want = ['room.heat', 'room.space', 'room.console', 'room.unquantized'];
    check(want.every((a, i) => actions[i] === a), 'Ctrl+1–4 on the window reach the page as the View menu’s room actions', actions.join(', '));

    // The switcher itself is the web UI's (app/ui/src); until it is built
    // there is nothing to switch, which is a fail, not a pass.
    const before = await wd.run(CURRENT_ROOM);
    if (!before.shown) {
      check(false, 'Ctrl+1–4 switch rooms', 'this build’s UI has no room switcher yet');
    } else {
      const viaMenu = [];
      const viaPage = [];
      for (let n = 1; n <= 4; n++) {
        pressOnWindow(DISPLAY, `ctrl+${n}`);
        await sleep(300);
        viaMenu.push((await wd.run(CURRENT_ROOM)).chosen.join('+') || '—');
      }
      for (let n = 4; n >= 1; n--) {
        await wd.keys(['', String(n)]);
        await sleep(300);
        viaPage.unshift((await wd.run(CURRENT_ROOM)).chosen.join('+') || '—');
      }
      check(ROOMS.every((r, i) => viaMenu[i] === r), 'Ctrl+1–4 through the menu switch rooms', viaMenu.join(', '));
      check(ROOMS.every((r, i) => viaPage[i] === r), 'Ctrl+1–4 in the page switch rooms', viaPage.join(', '));
    }
  } finally {
    await wd.deleteSession().catch(() => {});
  }
}

try {
  await main();
} catch (e) {
  console.error(`FAIL  ${e.message}`);
  failures++;
} finally {
  for (const c of children.reverse()) c.kill();
  rmSync(library, { recursive: true, force: true });
}
console.log(failures ? `${failures} failed` : 'all passed');
process.exitCode = failures ? 1 : 0;
