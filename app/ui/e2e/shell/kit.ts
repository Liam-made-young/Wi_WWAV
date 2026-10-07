/// <reference types="node" />
// What the shell's end-to-end tests share: a page past first launch, songs
// packed by the app's own packer, and the core on a line of the test's own
// (e2e/support/core.ts).

import { test as base, type Page } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { connect, type Core } from '../support/core';
import { writeWav } from '../support/wav';

export { expect } from '@playwright/test';

/** A page whose first launch is already through (first-launch.spec.ts walks it). */
export const test = base.extend<{ core: Core }>({
  page: async ({ page }, use) => {
    await page.addInitScript(() => localStorage.setItem('wi.firstLaunch', 'done'));
    await use(page);
  },
  core: async ({}, use) => {
    const core = await connect();
    await use(core);
    core.close();
  },
});

/** ⌘ as this machine's shell reads it: Command on a Mac, Ctrl elsewhere. */
export const CMD = 'ControlOrMeta';

/** Opens the shell and waits for its case. */
export async function openShell(page: Page) {
  await page.goto('/');
  await page.locator('.case-bar').waitFor();
}

// Files made for a run live in one folder, gone when the worker exits: the
// library keeps its own copies.
const WORK = mkdtempSync(join(tmpdir(), 'wi-wwav-shell-'));
process.on('exit', () => rmSync(WORK, { recursive: true, force: true }));

const TONES: Record<string, [number, number]> = {
  vocals: [440, 0.3],
  drums: [110, 0.1],
  other: [330, 0.1],
  bass: [55, 0.1],
};

/** A .wwav with four stems, packed by the app's own packer, with its title, key and tempo in song.txt. */
export function packSong(meta: { title: string; key: string; bpm: number; seconds: number }): string {
  const work = mkdtempSync(join(WORK, 'song-'));
  const folder = join(work, meta.title);
  mkdirSync(folder);
  for (const [stem, tone] of Object.entries(TONES)) writeWav(join(folder, `${stem}.wav`), [tone], meta.seconds);
  writeWav(join(folder, 'master.wav'), Object.values(TONES), meta.seconds);
  writeFileSync(join(folder, 'song.txt'), `title=${meta.title}\nkey=${meta.key}\nbpm=${meta.bpm}\n`);
  const song = join(work, `${meta.title}.wwav`);
  const packed = spawnSync(join(process.env.WI_E2E_ROOT!, 'target/debug/wwav'), ['pack', folder, '-o', song], {
    encoding: 'utf8',
  });
  if (packed.status !== 0) throw new Error(packed.stderr);
  return song;
}

/**
 * A plain WAV: it comes in as master only. Its tone comes from its name, so
 * two names are never the same bytes (the library brings a file in once).
 */
export function plainWav(name: string, seconds = 1): string {
  const path = join(mkdtempSync(join(WORK, 'wav-')), name);
  const hz = 110 + ([...name].reduce((sum, c) => sum * 31 + c.charCodeAt(0), 7) % 880);
  writeWav(path, [[hz, 0.2]], seconds);
  return path;
}

/** Takes every task out of Heat, so the strip's task half starts empty. */
export async function clearTasks(core: Core) {
  const { records } = await core.call<{ records: { id: string }[] }>('records.list', { kind: 'task' });
  if (!records.length) return;
  await core.call('records.mutate', {
    label: 'clear tasks',
    room: 'heat',
    ops: records.map((r) => ({ op: 'delete', kind: 'task', id: r.id })),
  });
}

/** Drops files onto an element marked data-drop, as file:// URLs, the way a browser carries them. */
export async function dropFiles(page: Page, selector: string, paths: string[]) {
  const uris = paths.map((p) => `file://${p.split('/').map(encodeURIComponent).join('/')}`).join('\n');
  const transfer = await page.evaluateHandle((list) => {
    const dt = new DataTransfer();
    dt.setData('text/uri-list', list);
    return dt;
  }, uris);
  await page.dispatchEvent(selector, 'dragover', { dataTransfer: transfer });
  await page.dispatchEvent(selector, 'drop', { dataTransfer: transfer });
}
