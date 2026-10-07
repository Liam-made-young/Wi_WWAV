/// <reference types="node" />
import { type Page } from '@playwright/test';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { audit, type Findings, judge, settled, stillFor5s } from './measure';
import { clearTasks, CMD, expect, packSong, plainWav, test } from './kit';

// docs/GATES.md 2.4 for the shell, measured before it is called passed.
// What a fail looks like, written down first: in light or dark, at 1024 ×
// 680 or 1280 × 800, in any view or anything that opens over one, any body
// text under 7:1 or secondary text under 4.5:1 against what is drawn
// behind it; a button, tab or orb under 44 × 44 pt; a dense row or grid
// cell under 24 × 24 pt; any text under 11 pt; or, with nothing playing,
// two screenshots taken 5 s apart that differ by a byte. A shell is first
// given time to finish arriving (settled, in measure.ts): the core's first
// answers land a few tens of milliseconds after the case paints, and a
// screen caught before them has not moved, it has not finished loading. The
// results are written to e2e/reports/shell-gate-2.4.md, fails and all.

test.describe.configure({ mode: 'serial' });
test.setTimeout(600_000);

const SIZES = [
  { width: 1024, height: 680 },
  { width: 1280, height: 800 },
];
const SCHEMES = ['light', 'dark'] as const;

type Setup = (page: Page) => Promise<void>;

const view =
  (n: number): Setup =>
  async (page) =>
    page.keyboard.press(`${CMD}+${n}`);
const pane =
  (name: RegExp): Setup =>
  async (page) => {
    await page.getByRole('button', { name: 'You' }).click();
    await page.getByRole('menuitem', { name: 'Settings…' }).click();
    await page.getByLabel('Settings panes').getByRole('tab', { name }).click();
  };

// Loads Gate Song and leaves it playing with the drawer shut: with the drawer
// open, Space loads the song under the cursor and plays it. A state sets up
// what it needs, so none of them leans on a song another test left loaded.
async function loadGateSong(page: Page) {
  await page.keyboard.press(`${CMD}+l`);
  await page
    .getByRole('option', { name: /Gate Song/ })
    .first()
    .click();
  await page.keyboard.press(' ');
  // The title comes with the load; the song is playing, and Space will pause
  // it rather than play it again, once the time has moved.
  await page.locator('.strip-title').waitFor();
  await expect(page.locator('.strip-time')).not.toHaveText(/^0:00\b/);
  await page.keyboard.press(`${CMD}+l`);
}

// Each state, from a freshly opened shell.
const STATES: [string, Setup, { idle?: boolean }?][] = [
  ['Heat', view(1), { idle: true }],
  ['Space', view(2), { idle: true }],
  ['Console', view(3), { idle: true }],
  [
    'Now strip filled, paused',
    async (page) => {
      await loadGateSong(page);
      await page.keyboard.press(' ');
      await page.locator('.strip [data-stem="drums"]').click();
    },
    { idle: true },
  ],
  [
    '⌘K with results',
    async (page) => {
      await page.keyboard.press(`${CMD}+k`);
      await page.getByRole('combobox').fill('go');
    },
  ],
  [
    '⌘⇧N after a capture',
    async (page) => {
      await page.keyboard.press(`${CMD}+Shift+n`);
      await page.getByRole('textbox', { name: 'Capture' }).fill('gate capture');
      await page.keyboard.press('Enter');
      await page.getByText(/captured ✓/).waitFor();
    },
  ],
  [
    '⌘L with a selection',
    async (page) => {
      await page.keyboard.press(`${CMD}+l`);
      await page.getByRole('option').first().click();
      await page
        .getByRole('option')
        .nth(1)
        .click({ modifiers: ['Shift'] });
    },
  ],
  [
    'Get Info',
    async (page) => {
      await page.keyboard.press(`${CMD}+l`);
      await page
        .getByRole('option', { name: /Gate Song/ })
        .first()
        .click();
      await page.keyboard.press(`${CMD}+i`);
      await page.locator('.info-verdict').waitFor();
    },
  ],
  [
    'Player, expanded',
    async (page) => {
      await loadGateSong(page);
      await page.keyboard.press(' ');
      const expand = page.getByRole('button', { name: /^Show the player/ });
      const half = (await expand.boundingBox())!;
      await expand.click({ position: { x: half.width - 20, y: 22 } });
    },
  ],
  ['Galaxy chip menu', async (page) => page.getByRole('button', { name: 'You' }).click()],
  ['Settings · Account', pane(/^Account/)],
  ['Settings · Library', pane(/^Library/)],
  ['Settings · Heat', pane(/^Heat/)],
  ['Settings · Audio & MIDI · Video', pane(/^Audio/)],
  ['Settings · Claude', pane(/^Claude/)],
  ['Settings · Privacy', pane(/^Privacy/)],
  ['Settings · Appearance · Keyboard', pane(/^Appearance/)],
  ['Export everything', async (page) => page.keyboard.press(`${CMD}+Shift+e`)],
  [
    'Undo toast',
    async (page) => {
      await page.keyboard.press(`${CMD}+Shift+n`);
      await page.getByRole('textbox', { name: 'Capture' }).fill('gate undo');
      await page.keyboard.press('Enter');
      await page.keyboard.press('Escape');
      await page.keyboard.press(`${CMD}+z`);
      await page.locator('.toast').waitFor();
    },
  ],
];

const FIRST_STEPS = ['Sign in', 'Claim your galaxy', 'Import your folder', 'Add your calendars', 'Connect Claude'];

interface Row {
  state: string;
  size: string;
  scheme: string;
  found: Findings;
  still: boolean | null;
}

const rows: Row[] = [];

test('gate 2.4: contrast, targets, type sizes and stillness, in every shell state', async ({ browser, core }) => {
  await clearTasks(core);
  await core.call('records.mutate', {
    label: 'add task',
    room: 'heat',
    ops: [
      {
        op: 'put',
        kind: 'task',
        id: 'gate-task',
        value: { title: 'Grammar quiz 4', due: Date.now() + 7_200_000, difficulty: 3 },
      },
    ],
  });
  // A song, and a few plain takes, so the drawer has rows to select from.
  await core.call('library.import', {
    paths: [
      packSong({ title: 'Gate Song', key: 'F minor', bpm: 128, seconds: 20 }),
      ...[1, 2, 3].map((n) => plainWav(`gate take ${n}.wav`, 1 + n / 10)),
    ],
    label: 'import',
  });

  for (const size of SIZES) {
    for (const scheme of SCHEMES) {
      const at = `${size.width} × ${size.height}`;
      for (const [state, setup, opts] of STATES) {
        const context = await browser.newContext({ viewport: size, colorScheme: scheme });
        await context.addInitScript(() => localStorage.setItem('wi.firstLaunch', 'done'));
        const page = await context.newPage();
        await page.goto('/');
        await page.locator('.case-bar').waitFor();
        await settled(page);
        await setup(page);
        const found = judge(await audit(page));
        const still = opts?.idle ? await stillFor5s(page) : null;
        rows.push({ state, size: at, scheme, found, still });
        await context.close();
      }
      // First launch, each of its five steps.
      const context = await browser.newContext({ viewport: size, colorScheme: scheme });
      const page = await context.newPage();
      await page.goto('/');
      for (const [i, step] of FIRST_STEPS.entries()) {
        await page.getByRole('heading', { name: step }).waitFor();
        rows.push({
          state: `First launch ${i + 1}: ${step}`,
          size: at,
          scheme,
          found: judge(await audit(page)),
          still: null,
        });
        await page.getByRole('button', { name: 'Skip for now' }).click();
      }
      await context.close();
    }
  }

  writeReport(rows);
  const fails = rows.flatMap((r) => [
    ...r.found.fails.map((f) => `${r.state}, ${r.size}, ${r.scheme}: ${f}`),
    ...(r.still === false ? [`${r.state}, ${r.size}, ${r.scheme}: moved while idle`] : []),
  ]);
  expect(fails).toEqual([]);
  await core.call('records.mutate', {
    label: 'clear',
    room: 'heat',
    ops: [{ op: 'delete', kind: 'task', id: 'gate-task' }],
  });
});

function writeReport(rows: Row[]) {
  const path = join(dirname(new URL(import.meta.url).pathname), '../reports/shell-gate-2.4.md');
  mkdirSync(dirname(path), { recursive: true });
  const cell = (n: number | null) => (n === null ? '—' : String(n));
  const lines = [
    '# Gate 2.4: the shell, measured',
    '',
    'Written by `e2e/shell/gate.spec.ts` (Playwright, Chromium, against the real core through the dev bridge).',
    'Bars (docs/GATES.md 2.4, docs/SPEC.md 3.18 and 8.9): body text 7:1, secondary text 4.5:1, buttons, tabs and',
    'orbs 44 × 44 pt, dense rows and grid cells 24 × 24 pt, no text under 11 pt, and nothing moving while idle',
    '(two screenshots 5 s apart, byte for byte). Contrast is the worst over every colour the grounds behind a',
    'text can show. Fonts are Linux stand-ins for Lucida Grande, Menlo, Inter and Jost; sizes and colours are the',
    'tokens’ own, so the ratios hold on a Mac, while widths there will differ slightly.',
    '',
    `Run ${new Date().toISOString().slice(0, 10)}.`,
    '',
    '| State | Window | Appearance | Texts | Lowest body | Lowest secondary | Smallest text | Targets | Tightest target | Still 5 s | Fails |',
    '|---|---|---|---|---|---|---|---|---|---|---|',
    ...rows.map(
      (r) =>
        `| ${r.state} | ${r.size} | ${r.scheme} | ${r.found.texts} | ${cell(r.found.minBody)} | ${cell(r.found.minSecondary)} | ${cell(r.found.smallestText)} | ${r.found.targets} | ${r.found.smallestTarget ?? '—'} | ${r.still === null ? '—' : r.still ? 'yes' : '**no**'} | ${r.found.fails.length} |`,
    ),
    '',
    '## Fails',
    '',
    ...(rows.some((r) => r.found.fails.length || r.still === false)
      ? rows.flatMap((r) => [
          ...r.found.fails.map((f) => `- ${r.state}, ${r.size}, ${r.scheme}: ${f}`),
          ...(r.still === false ? [`- ${r.state}, ${r.size}, ${r.scheme}: moved while idle`] : []),
        ])
      : ['None.']),
    '',
  ];
  writeFileSync(path, lines.join('\n'));
}
