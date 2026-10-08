import { test, expect, type Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';

async function core(cmd: string, args: Record<string, unknown> = {}): Promise<any> {
  const ws = new WebSocket('ws://127.0.0.1:8795');
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { ws.close(); reject(new Error(`Timed out: ${cmd}`)); }, 10_000);
    ws.onopen = () => ws.send(JSON.stringify({ id: 1, cmd, args }));
    ws.onerror = () => { clearTimeout(timeout); reject(new Error('Core connection failed')); };
    ws.onmessage = e => { const r = JSON.parse(String(e.data)); if (r.id !== 1) return; clearTimeout(timeout); ws.close(); if (r.ok) resolve(r.result); else reject(new Error(r.error.message)); };
  });
}
const text = (page: Page) => page.getByRole('textbox', { name: 'Document text', exact: true });
const active = (page: Page) => page.locator('.console-document:visible');
async function create(page: Page, title: string) {
  await page.goto('/console.html');
  await expect(page.getByRole('tab', { name: 'Write', exact: true })).toBeEnabled();
  await page.getByRole('tab', { name: 'Write', exact: true }).click();
  await page.locator('.console-top-actions').getByRole('button', { name: 'New document', exact: true }).click();
  await page.getByRole('dialog').getByLabel('Title', { exact: true }).fill(title);
  await page.getByRole('button', { name: 'Create document', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(text(page)).toBeEditable();
}
async function save(page: Page) {
  await page.getByRole('button', { name: 'Save version', exact: true }).click();
  await expect(page.locator('.console-version-label:visible')).not.toContainText('Unsaved');
  await expect(page.getByRole('button', { name: 'Save version', exact: true })).toBeEnabled();
}
async function add(page: Page, title: string, sub = false) {
  if (!await page.getByRole('complementary', { name: 'Section binder' }).isVisible()) await page.getByRole('button', { name: 'Toggle binder' }).click();
  await page.getByRole('button', { name: sub ? 'Add subsection' : 'Add section', exact: true }).click();
  await page.getByRole('dialog').getByLabel('Title', { exact: true }).fill(title);
  await page.getByRole('dialog').getByRole('button', { name: 'Add section', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(active(page).getByLabel('Section title', { exact: true })).toHaveValue(title);
}
async function manuscript(title: string) {
  const { documents } = await core('console.library', { tool: 'write' });
  const d = documents.find((d: any) => d.title === title);
  return core('console.write.read', { id: d.id });
}

test('multi-section writing, drag ordering, nesting, outline, research, versions and exports', async ({ page }) => {
  const errors: string[] = []; page.on('pageerror', e => errors.push(e.message));
  await create(page, 'Phase one essay');
  await active(page).getByLabel('Section title', { exact: true }).fill('Opening');
  await text(page).fill('# First light\n\nA small beginning becomes a story.');
  await add(page, 'Ending');
  await text(page).fill('The story comes home.'); await save(page);
  await add(page, 'Detail', true);
  await text(page).fill('A remembered detail.'); await save(page);
  await page.getByRole('button', { name: 'Unnest section', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Unnest section', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Move section up', exact: true }).click();
  await expect(active(page).locator('.write-section-list li').nth(1)).toContainText('Detail');
  await page.getByRole('button', { name: 'Nest section', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Unnest section', exact: true })).toBeEnabled();
  await active(page).locator('.write-section-list li').filter({ hasText: 'Ending' }).dragTo(active(page).locator('.write-section-list li').filter({ hasText: 'Opening' }));
  await expect(active(page).locator('.write-section-list li').first()).toContainText('Ending');
  await page.getByRole('button', { name: 'Toggle outline' }).click();
  await active(page).getByLabel('Section synopsis').fill('The last image is a return.');
  await page.getByRole('button', { name: 'Toggle research' }).click();
  await page.getByLabel('Document notes').fill('Keep this private.');
  const n = await core('heat.note.create', { title: 'Essay research', markdown: 'A private research observation.' });
  await page.getByLabel('Search Learn notes').fill('Essay research');
  await page.locator('.write-note-hits').getByRole('button', { name: 'Essay research' }).click();
  await expect(page.locator('.write-sources')).toContainText('Essay research');
  await page.locator('.write-sources').getByRole('button', { name: 'Essay research', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('A private research observation.');
  await page.getByRole('button', { name: 'Close Essay research' }).click();
  await page.getByRole('button', { name: 'Close research' }).click();
  await save(page);
  const before = await manuscript('Phase one essay');
  expect(before.manuscript.research[0].noteId).toBe(n.note.id);
  expect(before.manuscript.sections.map((s: any) => s.title)).toEqual(['Ending', 'Opening', 'Detail']);
  expect(before.manuscript.sections[2].parent).toBe(before.manuscript.sections[1].id);
  expect(before.stats.words).toBe(15);
  await page.getByRole('button', { name: 'Export document' }).click();
  for (const format of ['markdown', 'text', 'pdf']) {
    await page.getByLabel('Export format', { exact: true }).selectOption(format);
    const download = page.waitForEvent('download');
    await page.getByRole('dialog').getByRole('button', { name: 'Export', exact: true }).click();
    const result = await download; const path = await result.path(); const bytes = await readFile(path!);
    expect(bytes.length).toBeGreaterThan(10);
    if (format === 'pdf') expect(bytes.subarray(0, 8).toString()).toBe('%PDF-1.7');
    else { expect(bytes.toString()).toContain('A small beginning becomes a story.'); expect(bytes.toString()).not.toContain('Keep this private.'); }
  }
  await page.getByRole('button', { name: 'Close Export document' }).click();
  await page.getByRole('button', { name: 'Read saved version' }).click();
  await expect(page.getByRole('dialog')).toContainText('First light');
  await expect(page.getByRole('dialog').locator('h1')).toHaveText('First light');
  await expect(page.getByRole('dialog')).not.toContainText('Keep this private.');
  await page.getByRole('button', { name: 'Close Reader' }).click();
  await page.getByRole('button', { name: 'Focus mode' }).click();
  await page.getByRole('button', { name: 'Typewriter scroll' }).click();
  await expect(page.locator('.console-library')).not.toBeVisible();
  await page.screenshot({ path: '/private/tmp/console-write-focus.png' });
  await page.getByRole('button', { name: 'Focus mode' }).click();
  await page.screenshot({ path: '/private/tmp/console-write-desktop.png' });
  await page.reload();
  await expect(active(page).getByLabel('Document title', { exact: true })).toHaveValue('Phase one essay');
  expect(errors).toEqual([]);
});

test('screenplay live preview, Fountain export and lyrics line counts work in a compact viewport', async ({ page }) => {
  await create(page, 'Short screenplay');
  await active(page).getByLabel('Writing mode').selectOption('screenplay');
  await expect(active(page).getByLabel('Screenplay element')).toBeVisible();
  const script = 'Title: A Light\nAuthor: Liam\n\nINT. STUDIO - NIGHT\n\nA lamp clicks on.\n\nLIAM\n(quietly)\nOne line.\nAnother line.\n\nCUT TO:\n';
  await text(page).fill(script); await save(page);
  await page.getByRole('button', { name: 'Live preview', exact: true }).click();
  const preview = page.locator('.write-rendered:visible');
  await expect(preview.locator('.write-script-scene')).toContainText('INT. STUDIO - NIGHT');
  await expect(preview.locator('.write-script-character')).toContainText('LIAM');
  await expect(preview.locator('.write-script-parenthetical')).toContainText('(quietly)');
  await expect(preview.locator('.write-script-dialogue')).toHaveCount(2);
  await page.screenshot({ path: '/private/tmp/console-write-screenplay.png' });
  await page.getByRole('button', { name: 'Export document' }).click();
  await page.getByLabel('Export format', { exact: true }).selectOption('fountain');
  const download = page.waitForEvent('download'); await page.getByRole('dialog').getByRole('button', { name: 'Export', exact: true }).click();
  expect(await readFile((await (await download).path())!, 'utf8')).toBe(script);
  const savedScript = await manuscript('Short screenplay');
  const pdf = await core('console.write.export', { id: savedScript.document.id, version: savedScript.base, format: 'pdf' });
  console.log('Screenplay PDF inspection:', pdf.path);
  await page.getByRole('button', { name: 'Close Export document' }).click();
  await page.getByRole('button', { name: 'Edit source', exact: true }).click();
  await create(page, 'Small lyric');
  await active(page).getByLabel('Writing mode').selectOption('lyrics');
  await active(page).getByLabel('Lyrics section type').selectOption('hook');
  await text(page).fill('We shine\nMake a little light'); await save(page);
  await expect(active(page).locator('.write-formatting')).toContainText('2 lines');
  await page.getByText('Line counts', { exact: true }).click();
  await expect(page.getByRole('table', { name: 'Lyrics line and syllable counts' })).toContainText('Syllables (est.)');
  await page.getByText('Line counts', { exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  if (await page.getByRole('button', { name: 'Close Library' }).isVisible()) await page.getByRole('button', { name: 'Close Library' }).click();
  await page.getByRole('button', { name: 'Toggle binder' }).click();
  await expect(text(page)).toBeVisible();
  await text(page).click(); await page.keyboard.press('End'); await page.keyboard.type(' tonight');
  await expect(text(page)).toContainText('tonight');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: '/private/tmp/console-write-mobile.png' });
});

test('Claude proposals, suggestions, section reorder and explicit continuation use real version and undo paths', async ({ page }) => {
  await create(page, 'Claude writing');
  await text(page).fill('A muddled line.'); await save(page);
  await text(page).focus(); await page.keyboard.press('Meta+a'); await page.keyboard.press('Meta+t');
  const dialog = page.getByRole('dialog', { name: 'Claude', exact: true });
  await dialog.getByLabel('Write Claude action').selectOption('tighten');
  await dialog.getByRole('button', { name: 'Ask Claude', exact: true }).click();
  await expect(dialog.getByRole('region', { name: 'Proposed edit' })).toContainText('A clear line.');
  expect((await manuscript('Claude writing')).manuscript.sections[0].text).toBe('A muddled line.');
  await dialog.getByRole('button', { name: 'Apply as new version' }).click();
  await expect(dialog).toContainText('Applied as a new version.');
  await dialog.getByLabel('Write Claude action').selectOption('rhymes');
  await dialog.getByRole('button', { name: 'Ask Claude', exact: true }).click();
  await expect(dialog).toContainText('light / night / bright');
  await expect(dialog.getByRole('button', { name: 'Apply as new version' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Close Claude', exact: true }).click();
  await expect(text(page)).toHaveText('A clear line.');
  await expect(page.locator('.console-document:visible .console-provenance')).toHaveText('Claude assisted');
  await page.getByRole('button', { name: 'Undo document edit' }).click();
  await expect(text(page)).toHaveText('A muddled line.');
  await add(page, 'Last section'); await text(page).fill('An ending.'); await save(page);
  await page.getByRole('button', { name: 'Ask Claude', exact: true }).click();
  await dialog.getByLabel('Write Claude action').selectOption('reorder');
  await dialog.getByRole('button', { name: 'Ask Claude', exact: true }).click();
  await expect(dialog.getByRole('region', { name: 'Proposed edit' })).toContainText('Last section');
  await dialog.getByRole('button', { name: 'Apply as new version' }).click();
  await expect(dialog).toContainText('Applied as a new version.');
  await page.getByRole('button', { name: 'Close Claude', exact: true }).click();
  await expect(active(page).locator('.write-section-list li').first()).toContainText('Last section');
  await text(page).focus(); await page.keyboard.press('Meta+End'); await page.keyboard.press('Meta+t');
  await dialog.getByLabel('Write Claude action').selectOption('continue');
  await dialog.getByRole('button', { name: 'Ask Claude', exact: true }).click();
  await expect(dialog.getByRole('region', { name: 'Proposed edit' })).toContainText('A new sentence.');
  await dialog.getByRole('button', { name: 'Apply as new version' }).click();
  await expect(dialog).toContainText('Applied as a new version.');
  await page.getByRole('button', { name: 'Close Claude', exact: true }).click();
  await expect(text(page)).toContainText('A new sentence.');
  await page.getByRole('button', { name: 'Version history' }).click();
  await expect(page.getByRole('dialog')).toContainText('Claude');
});

test('a twenty-thousand-word manuscript remains editable and exports through Rust', async ({ page }) => {
  test.setTimeout(60_000);
  await create(page, 'Long piece');
  const body = ('A long piece can begin with one small clear line of thought and find its way back home again today.\n\n').repeat(1000);
  const started = performance.now();
  await text(page).fill(body);
  await expect(active(page).locator('.write-counts')).toContainText('20,000 words');
  const opened = performance.now() - started;
  await text(page).focus(); await page.keyboard.press('Meta+End');
  const keyStart = performance.now(); await page.keyboard.type('End.');
  await page.evaluate(() => new Promise<void>(r => requestAnimationFrame(() => requestAnimationFrame(() => r()))));
  const typed = performance.now() - keyStart;
  const saveStart = performance.now(); await save(page); const saved = performance.now() - saveStart;
  const data = await manuscript('Long piece');
  expect(data.stats.words).toBe(20_001);
  const exportStart = performance.now();
  const pdf = await core('console.write.export', { id: data.document.id, version: data.base, format: 'pdf' });
  const exported = performance.now() - exportStart;
  expect((await readFile(pdf.path)).subarray(0, 8).toString()).toBe('%PDF-1.7');
  const metrics = { words: 20_001, openAndCountMs: Math.round(opened), fourKeystrokesAndPaintMs: Math.round(typed), saveMs: Math.round(saved), pdfMs: Math.round(exported) };
  console.log('Write performance sample:', JSON.stringify(metrics));
  await test.info().attach('write-performance.json', { body: JSON.stringify(metrics), contentType: 'application/json' });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.getByLabel('Import into Console', { exact: true }).setInputFiles({ name: 'large-source.md', mimeType: 'text/markdown', buffer: Buffer.alloc(1024 * 1024 + 1, 'x') });
  await expect(active(page).getByLabel('Document title', { exact: true })).toHaveValue('large-source.md');
  await expect(text(page)).toHaveCount(0);
  await expect(active(page).locator('.console-text-preview')).toBeVisible();
  expect(await active(page).locator('.console-text-preview').evaluate(e => e.textContent?.length)).toBe(1024 * 1024 + 1);
});
