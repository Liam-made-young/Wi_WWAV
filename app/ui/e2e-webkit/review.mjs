// A reviewer's checks of the real app on Linux, beyond run.mjs: files handed
// over by a second launch, the settings window, and the main window's fence.
//
//   cargo build -p mock-engine
//   cd app/src-tauri && cargo tauri build --debug --no-bundle
//   node app/ui/e2e-webkit/review.mjs [--app target/debug/wi-wwav]
//
// It needs what run.mjs needs, plus dbus-daemon: it runs its own session bus,
// which the single-instance hand-off travels over on Linux.
//
// What a fail looks like, written before the checks:
// - A second launch with a path (what a file manager or a terminal does
//   while the app is open) doesn't bring the song into the open app's
//   library: with an absolute path, or with a path relative to the
//   directory the second launch was started in (`wi-wwav song.wwav`).
// - Settings… (Ctrl+, on Linux) doesn't open the settings window at
//   index.html?window=settings, or that window can call a command its
//   capability leaves out, or can't call one it lists.
// - The main window can be taken off the app to a page on another origin
//   (docs/SPEC.md 9.8: "no remote scripts load"), e.g. by a link in words
//   someone else wrote.
//
// On build/tauri at 3c692be two of these fail, each a review finding: the
// relative path (open.rs drops the cwd the single-instance plugin hands
// over) and the main window's fence (no navigation guard).
//
// Run `cargo test` before `cargo tauri build`, not after: cargo test relinks
// target/debug/wi-wwav without the custom protocol, and that build shows
// about:blank (it waits for the dev server).

import { spawn, execFileSync } from 'node:child_process';
import { createServer } from 'node:http';
import { existsSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import { WebDriver } from './webdriver.mjs';

const ROOT = resolve(import.meta.dirname, '../../..');
const CORPUS = join(ROOT, 'tests/corpus');
const ENGINE = join(ROOT, 'target/debug/mock-engine');
const argApp = process.argv.indexOf('--app');
const app = resolve(argApp > 0 ? process.argv[argApp + 1] : join(ROOT, 'target/debug/wi-wwav'));

const children = [];
const library = mkdtempSync(join(tmpdir(), 'wi-wwav-review-'));
let failures = 0;

function check(ok, what, detail = '') {
  console.log(`${ok ? 'pass' : 'FAIL'}  ${what}${detail ? `: ${detail}` : ''}`);
  if (!ok) failures++;
}

function start(cmd, args, opts = {}) {
  const child = spawn(cmd, args, { stdio: ['ignore', 'pipe', 'pipe'], ...opts });
  let out = '';
  child.stdout.on('data', (d) => (out += d));
  child.stderr.on('data', (d) => (out += d));
  child.on('error', (e) => console.error(`${cmd}: ${e.message}`));
  children.push(child);
  return { child, out: () => out };
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
  for (let n = 80; n < 90; n++) {
    if (existsSync(`/tmp/.X11-unix/X${n}`)) continue;
    start('Xvfb', [`:${n}`, '-screen', '0', '1920x1200x24', '-nolisten', 'tcp']);
    await until('Xvfb', async () => existsSync(`/tmp/.X11-unix/X${n}`));
    return `:${n}`;
  }
  throw new Error('no free display number for Xvfb');
}

// A bare session bus: no services to activate, so nothing on it (portals,
// the accessibility bus) can hold up GTK while it starts.
async function sessionBus() {
  const config = join(library, 'bus.conf');
  writeFileSync(
    config,
    `<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig><type>session</type><listen>unix:path=${join(library, 'bus')}</listen>
<policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>`,
  );
  const bus = start('dbus-daemon', [`--config-file=${config}`, '--nofork', '--print-address']);
  return until('the session bus', async () => bus.out().split('\n').find((l) => l.startsWith('unix:')));
}

const CALL = `
  const [cmd, args, done] = arguments;
  window.__TAURI_INTERNALS__.invoke('core', { cmd, args }).then((ok) => done({ ok }), (refused) => done({ refused }));`;

const LISTEN = `
  const done = arguments[arguments.length - 1];
  window.__heard = [];
  const t = window.__TAURI_INTERNALS__;
  const handler = t.transformCallback((e) => window.__heard.push(e.payload));
  t.invoke('plugin:event|listen', { event: 'core', target: { kind: 'Any' }, handler }).then(() => done(true), (e) => done(String(e)));`;

// (The answer is wrapped: the WebDriver client takes any value with an
// `error` key for a WebDriver error.)
// A page on another origin that tries the app's command when it loads.
function remoteSite() {
  const server = createServer((req, res) => {
    res.writeHead(200, { 'content-type': 'text/html' });
    res.end(`<!doctype html><title>remote</title><script>
      window.__ipc = 'no __TAURI_INTERNALS__';
      if (window.__TAURI_INTERNALS__) {
        window.__ipc = 'pending';
        window.__TAURI_INTERNALS__.invoke('core', { cmd: 'app.hello', args: {} })
          .then((r) => (window.__ipc = 'answered: ' + JSON.stringify(r)), (e) => (window.__ipc = 'refused: ' + JSON.stringify(e)));
      }
    </script><p>a page on another origin</p>`);
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

async function clips(wd) {
  const got = await wd.runAsync(CALL, ['library.list', {}]);
  return got.ok?.clips ?? [];
}

async function main() {
  if (!existsSync(app)) throw new Error(`no app at ${app}`);
  if (!existsSync(ENGINE)) throw new Error(`no engine at ${ENGINE}`);
  const DISPLAY = await display();
  const DBUS_SESSION_BUS_ADDRESS = await sessionBus();
  const env = { ...process.env, DISPLAY, DBUS_SESSION_BUS_ADDRESS, WI_WWAV_LIBRARY: library, WI_WWAV_ENGINE: ENGINE };
  const port = 5444 + Math.floor(Math.random() * 1000);
  const driver = start('tauri-driver', ['--port', String(port), '--native-port', String(port + 1)], { env });
  const wd = new WebDriver(`http://127.0.0.1:${port}`);
  await until('tauri-driver', () => wd.status()).catch((e) => {
    throw new Error(`${e.message}\n${driver.out()}`);
  });

  await wd.newSession({ 'tauri:options': { application: app, args: [] } });
  try {
    await until('the page to load', () => wd.run('return location.href.startsWith("tauri://") && document.readyState === "complete"')).catch(async (e) => {
      throw new Error(`${e.message}: at ${await wd.run('return location.href + " " + document.readyState').catch(String)}\n${driver.out()}`);
    });
    const listening = await wd.runAsync(LISTEN);
    check(listening === true, 'the page listens to the shell', String(listening));
    check((await clips(wd)).length === 0, 'the library starts empty');

    // A second launch hands its paths to the open app and leaves.
    const second = (args, cwd) =>
      new Promise((ok) => {
        const c = spawn(app, args, { cwd, env, stdio: 'ignore' });
        const timer = setTimeout(() => c.kill(), 15000);
        c.on('exit', (code, signal) => {
          clearTimeout(timer);
          ok(code ?? signal);
        });
      });
    const opened = async (n) => {
      await until(`open event ${n}`, async () => (await wd.run('return window.__heard.filter((e) => e.event === "open")')).length >= n, 10000).catch(() => {});
      const all = await wd.run('return window.__heard.filter((e) => e.event === "open")');
      return all[n - 1]?.payload;
    };

    const absolute = await second([join(CORPUS, 'original.wwav')], tmpdir());
    const first = await opened(1);
    const afterAbsolute = await clips(wd);
    check(
      absolute === 0 && afterAbsolute.length === 1,
      'a second launch with an absolute path brings the song into the open app',
      `exit ${absolute}; ${afterAbsolute.length} clip(s); open ${JSON.stringify(first)}`,
    );

    const relative = await second(['master-only.wwav'], CORPUS);
    const answer = await opened(2);
    const afterRelative = await clips(wd);
    check(
      relative === 0 && afterRelative.length === 2,
      'a second launch with a path relative to where it started (wi-wwav song.wwav) brings the song in',
      `exit ${relative}; ${afterRelative.length} clip(s); open ${JSON.stringify(answer)}`,
    );

    // Settings… opens the settings window, fenced to its capability.
    const main = await wd.call('GET', `${wd.session}/window`);
    const id = execFileSync('xdotool', ['search', '--name', '^Wi_WWAV$'], { env }).toString().trim().split('\n')[0];
    execFileSync('xdotool', ['windowfocus', '--sync', id], { env });
    execFileSync('xdotool', ['key', '--clearmodifiers', 'ctrl+comma'], { env });
    const handles = await until('a second window', async () => {
      const h = await wd.call('GET', `${wd.session}/window/handles`);
      return h.length > 1 ? h : undefined;
    }, 5000).catch(() => [main]);
    const other = handles.find((h) => h !== main);
    check(!!other, 'Settings… (Ctrl+,) opens a second window', `${handles.length} window(s)`);
    if (other) {
      await wd.call('POST', `${wd.session}/window`, { handle: other });
      await until('the settings page', () => wd.run('return document.readyState === "complete"'), 5000).catch(() => {});
      const url = await wd.call('GET', `${wd.session}/url`);
      check(url === 'tauri://localhost/index.html?window=settings', 'the settings window loads index.html?window=settings', url);
      const allowed = await wd.runAsync(CALL, ['app.settings.get', {}]);
      const refused = await wd.runAsync(CALL, ['library.delete', { ids: [], label: 'x' }]);
      check(!!allowed.ok && refused.refused?.code === 'not_allowed', 'the settings window gets its commands and no others', `${JSON.stringify(allowed).slice(0, 60)} / ${JSON.stringify(refused)}`);
      await wd.call('DELETE', `${wd.session}/window`).catch(() => {});
      await wd.call('POST', `${wd.session}/window`, { handle: main });
    }

    // The main window stays on the app.
    const site = await remoteSite();
    const remote = `http://127.0.0.1:${site.address().port}/page.html`;
    try {
      await wd.run(`location.href = ${JSON.stringify(remote)}; return true;`).catch(() => {});
      await sleep(1500);
      const url = await wd.call('GET', `${wd.session}/url`);
      const ipc = await wd.run('return window.__ipc ?? null').catch((e) => String(e));
      check(url.startsWith('tauri://localhost'), 'the main window stays on the app when a page tries to take it elsewhere', `now at ${url}; the remote page's call: ${ipc}`);
    } finally {
      site.close();
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
