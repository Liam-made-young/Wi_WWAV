/// <reference types="node" />
// What the end-to-end tests run against, started once per run and stopped
// after it: tools/mock-server (mi-wwav.com), wi-devbridge (the app's real
// core, over a WebSocket) with mock-engine behind it on its null device, and
// Vite serving the UI pointed at that bridge. Each run gets a fresh library
// in a temporary folder.
//
// Tests find what started here in their environment: WI_E2E_BRIDGE (the
// bridge's ws:// address), WI_E2E_MOCK (the mock server), WI_E2E_LIBRARY
// and WI_E2E_ROOT (the workspace).

import { type ChildProcess, spawn, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';

const ROOT = resolve(import.meta.dirname, '../../../..');
const UI = resolve(import.meta.dirname, '../..');
// 5173 unless WI_E2E_PORT says otherwise, so two checkouts can run at once.
const VITE_PORT = Number(process.env.WI_E2E_PORT ?? 5173);

// Starts a child and resolves with the first stdout line matching `ready`.
function start(cmd: string, args: string[], ready: RegExp, opts: { cwd?: string; env?: NodeJS.ProcessEnv } = {}) {
  const child = spawn(cmd, args, { cwd: opts.cwd, env: opts.env ?? process.env, stdio: ['pipe', 'pipe', 'inherit'] });
  return new Promise<{ child: ChildProcess; match: RegExpMatchArray }>((ok, fail) => {
    const lines = createInterface({ input: child.stdout! });
    const timer = setTimeout(() => fail(new Error(`${cmd} didn't start within 60 s`)), 60_000);
    lines.on('line', (line) => {
      const match = line.match(ready);
      if (match) {
        clearTimeout(timer);
        ok({ child, match });
      }
    });
    child.once('exit', (code) => fail(new Error(`${cmd} exited (${code}) before it was ready`)));
  });
}

async function serving(url: string) {
  for (let i = 0; i < 300; i++) {
    try {
      await fetch(url);
      return;
    } catch {
      await new Promise((r) => setTimeout(r, 200));
    }
  }
  throw new Error(`nothing answered at ${url}`);
}

export default async function globalSetup() {
  // The bridge, the engine it starts, and the .wwav packer the tests use.
  const built = spawnSync('cargo', ['build', '-q', '-p', 'wi-devbridge', '-p', 'mock-engine', '-p', 'wwav-formats', '-p', 'wi-mcp'], {
    cwd: ROOT,
    stdio: 'inherit',
  });
  if (built.status !== 0) throw new Error('cargo build failed');

  const library = mkdtempSync(join(tmpdir(), 'wi-wwav-e2e-'));
  const children: ChildProcess[] = [];
  try {
    const mock = await start('node', [join(ROOT, 'tools/mock-server/server.js'), '--port', '0'], /on (http:\/\/\S+)/);
    children.push(mock.child);
    const bridge = await start(
      join(ROOT, 'target/debug/wi-devbridge'),
      ['--library', join(library, 'Wi_WWAV'), '--server', mock.match[1], '--port', '0'],
      /listening (ws:\/\/\S+)/,
      { env: { ...process.env, TMPDIR: library } },
    );
    children.push(bridge.child);
    // A dev server already there would talk to some other bridge.
    const page = `http://localhost:${VITE_PORT}`;
    if (
      await fetch(page).then(
        () => true,
        () => false,
      )
    ) {
      throw new Error(`Something already serves ${page}; stop it, and the e2e run starts its own.`);
    }
    // Vite's own script, not npx, so stopping it stops Vite.
    const vite = spawn(
      process.execPath,
      [join(UI, 'node_modules/vite/bin/vite.js'), '--port', String(VITE_PORT), '--strictPort'],
      {
        cwd: UI,
        env: { ...process.env, VITE_BRIDGE_URL: bridge.match[1] },
        stdio: 'ignore',
      },
    );
    children.push(vite);
    await serving(page);

    process.env.WI_E2E_BRIDGE = bridge.match[1];
    process.env.WI_E2E_MOCK = mock.match[1];
    process.env.WI_E2E_LIBRARY = join(library, 'Wi_WWAV');
    process.env.WI_E2E_ROOT = ROOT;
  } catch (e) {
    for (const c of children) c.kill();
    throw e;
  }

  return async () => {
    for (const c of children.reverse()) c.kill();
    rmSync(library, { recursive: true, force: true });
  };
}
