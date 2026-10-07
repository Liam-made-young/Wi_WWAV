/// <reference types="node" />
import { expect, test } from '@playwright/test';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { connect } from './support/core';
import { writeWav } from './support/wav';

// docs/PLAN.md S1.10, "export, turn Wi-Fi off, and open index.html". What a
// fail looks like: with every network request refused, a master or a film
// doesn't play from disk; or, after the one press ("Open this folder"), a
// song's four stems don't decode, or muting one doesn't change what comes
// out; or the list doesn't end with "That's everything.".

// Four tones, one per stem, the vocals loudest so muting them is plain.
const STEMS: Record<string, [number, number]> = {
  vocals: [440, 0.4],
  drums: [110, 0.1],
  other: [330, 0.1],
  bass: [55, 0.1],
};

test.use({ launchOptions: { args: ['--autoplay-policy=no-user-gesture-required'] } });

test('the export plays offline: masters and films, then every song apart', async ({ page, context }) => {
  const root = process.env.WI_E2E_ROOT!;
  const work = mkdtempSync(join(tmpdir(), 'wi-wwav-export-'));

  // A song packed by the app's own packer, a film, and a plain WAV.
  const folder = join(work, 'Offline Song');
  mkdirSync(folder);
  for (const [stem, tone] of Object.entries(STEMS)) writeWav(join(folder, `${stem}.wav`), [tone], 2);
  writeWav(join(folder, 'master.wav'), Object.values(STEMS), 2);
  const song = join(work, 'Offline Song.wwav');
  const packed = spawnSync(join(root, 'target/debug/wwav'), ['pack', folder, '-o', song], { encoding: 'utf8' });
  expect(packed.status, packed.stderr).toBe(0);

  // A film Chromium can decode without proprietary codecs (VP9), as a .swav.
  const mp4 = join(work, 'Flash.mp4');
  const film = join(work, 'Flash.swav');
  const encoded = spawnSync('ffmpeg', [
    ...['-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i', 'testsrc=size=64x64:rate=25:duration=2'],
    ...['-c:v', 'libvpx-vp9', '-pix_fmt', 'yuv420p', mp4],
  ]);
  expect(encoded.status, String(encoded.stderr)).toBe(0);
  const wrapped = spawnSync(join(root, 'target/debug/wwav'), ['swav', 'pack', mp4, '-o', film], { encoding: 'utf8' });
  expect(wrapped.status, wrapped.stderr).toBe(0);

  const core = await connect();
  await core.call('library.import', { paths: [song, film, join(root, 'tests/corpus/mono.wav')], label: 'import' });
  const out = join(work, 'Export');
  const exported = await core.call<{ mismatched: string[] }>('export.everything', { to: out, zip: false });
  expect(exported.mismatched).toEqual([]);
  core.close();

  // Wi-Fi off: nothing but the folder itself.
  const reached: string[] = [];
  await context.route('**/*', (route) => {
    const url = route.request().url();
    if (url.startsWith('file:')) return route.continue();
    reached.push(url);
    return route.abort();
  });
  page.on('request', (r) => {
    if (!r.url().startsWith('file:')) reached.push(r.url());
  });
  await page.goto(pathToFileURL(join(out, 'index.html')).href);

  // Every master and film plays from disk, at once, with no press.
  const masters = page.locator('audio[data-master]');
  await expect(masters).toHaveCount(2);
  const films = page.locator('video[data-film]');
  await expect(films).toHaveCount(1);
  for (const media of [...(await masters.all()), ...(await films.all())]) {
    await media.evaluate((m: HTMLMediaElement) => m.play());
    await expect.poll(() => media.evaluate((m: HTMLMediaElement) => m.currentTime)).toBeGreaterThan(0);
    await media.evaluate((m: HTMLMediaElement) => m.pause());
  }
  await expect(page.locator('main > :last-child')).toHaveText("That's everything.");

  // One press: open this folder. The song comes apart; the plain WAV stays whole.
  await page.setInputFiles('#folder', out);
  await expect(page.locator('#opened')).toHaveText('1 song is apart. 1 plays as the master only.');
  const apart = page.locator('[data-apart]');
  await expect(apart).toHaveCount(1);
  await expect(apart.locator('.stem')).toHaveCount(4);

  const level = () => apart.locator('meter').evaluate((m) => Number(m.getAttribute('data-level')));
  await apart.locator('[data-play]').click();
  await expect.poll(level).toBeGreaterThan(0.15);
  const together = await level();
  await apart.locator('[data-mute=vocals]').click();
  await expect.poll(level).toBeLessThan(together * 0.6);
  const muted = await level();
  expect(muted).toBeGreaterThan(0.02);
  // Solo the bass: only its tone is left.
  await apart.locator('[data-mute=vocals]').click();
  await apart.locator('[data-solo=bass]').click();
  await expect.poll(level).toBeLessThan(muted);

  expect(reached).toEqual([]);
  rmSync(work, { recursive: true, force: true });
});
