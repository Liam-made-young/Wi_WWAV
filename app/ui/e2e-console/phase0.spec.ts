import { test, expect, type Page } from '@playwright/test';

async function core(cmd: string, args: Record<string, unknown> = {}): Promise<any> {
  const ws = new WebSocket('ws://127.0.0.1:8795');
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { ws.close(); reject(new Error(`Timed out: ${cmd}`)); }, 10_000);
    ws.onopen = () => ws.send(JSON.stringify({ id: 1, cmd, args }));
    ws.onerror = () => { clearTimeout(timeout); reject(new Error('Core connection failed')); };
    ws.onmessage = e => { const r = JSON.parse(String(e.data)); if (r.id !== 1) return; clearTimeout(timeout); ws.close(); if (r.ok) resolve(r.result); else reject(new Error(r.error.message)); };
  });
}
async function create(page: Page, title: string) {
  await page.locator('.console-top-actions').getByRole('button', { name: 'New document', exact: true }).click();
  await page.getByRole('dialog').getByLabel('Title', { exact: true }).fill(title);
  await page.getByRole('button', { name: 'Create document', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: 'Document title', exact: true })).toHaveValue(title);
}

test('four workspaces preserve drafts; hand and Claude versions, forks, reader and packages agree with disk', async ({ page }) => {
  const errors: string[] = []; page.on('pageerror', e => errors.push(e.message));
  await page.goto('/console.html');
  await expect(page.getByRole('tab', { name: 'Write', exact: true })).toBeEnabled();
  await create(page, 'Small beginnings');
  const text = page.getByRole('textbox', { name: 'Document text', exact: true });
  await text.fill('A song begins with one line.\n');
  await page.getByRole('tab', { name: 'Image', exact: true }).click();
  await create(page, 'Poster study');
  await page.getByRole('tab', { name: 'Audiovisual', exact: true }).click();
  await create(page, 'Evening session');
  await page.getByRole('tab', { name: '3D', exact: true }).click();
  await create(page, 'Player body');
  await page.getByRole('tab', { name: 'Write', exact: true }).click();
  await expect(text).toHaveText('A song begins with one line.');
  await page.getByRole('button', { name: 'Save version', exact: true }).click();
  await expect(page.locator('.console-version-label:visible')).toHaveText('v2');
  const listing = await core('console.library', { tool: 'write' });
  const first = listing.documents.find((d: any) => d.title === 'Small beginnings');
  expect(first.marker).toBe('Made by hand');
  await core('console.tool.call', { name: 'console_save', args: { id: first.id, base: first.head, text: 'One line becomes a song.\n' } });
  await expect(text).toHaveText('One line becomes a song.');
  await expect(page.locator('.console-document:visible .console-provenance')).toHaveText('Claude assisted');
  await page.getByRole('button', { name: 'Undo document edit', exact: true }).click();
  await expect(text).toHaveText('A song begins with one line.');
  await page.getByRole('button', { name: 'Redo document edit', exact: true }).click();
  await expect(text).toHaveText('One line becomes a song.');
  await page.getByRole('button', { name: 'Save as variation', exact: true }).click();
  await page.getByRole('dialog').getByLabel('Title', { exact: true }).fill('Second beginning');
  await page.getByRole('button', { name: 'Create variation', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Document title', exact: true })).toHaveValue('Second beginning');
  await page.getByRole('button', { name: 'Version history', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('Small beginnings');
  await page.getByRole('button', { name: 'Read version 1', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('One line becomes a song.');
  await expect(page.getByRole('dialog')).toContainText('Read only');
  await page.getByRole('button', { name: 'Close Reader', exact: true }).click();
  await page.getByRole('button', { name: 'Prepare Space post', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Posting to Space is not connected yet');
  await page.getByLabel('Search Console Library').fill('Second beginning');
  await expect(page.locator('.console-library-item')).toHaveCount(1);
  await page.getByLabel('Search Console Library').fill('');
  await page.getByLabel('Filter Library by tool').selectOption('image');
  await expect(page.locator('.console-library-item')).toHaveCount(1);
  await expect(page.locator('.console-library-item')).toContainText('Poster study');
  await page.getByLabel('Filter Library by tool').selectOption('');
  await expect(page.locator('.console-library-item')).toHaveCount(5);
  await text.focus();
  await page.keyboard.press('Meta+t');
  await expect(page.getByRole('dialog', { name: 'Claude', exact: true })).toContainText('Second beginning');
  await page.getByRole('button', { name: 'Close Claude', exact: true }).click();
  await page.screenshot({ path: '/private/tmp/console-desktop.png' });
  await page.reload();
  await expect(page.getByRole('textbox', { name: 'Document title', exact: true })).toHaveValue('Second beginning');
  await expect(page.getByRole('textbox', { name: 'Document text', exact: true })).toHaveText('One line becomes a song.');
  expect(errors).toEqual([]);
});

test('imports a real bitmap and previews it; compact layout stays within its viewport', async ({ page }) => {
  await page.goto('/console.html');
  await expect(page.getByRole('tab', { name: 'Write', exact: true })).toBeEnabled();
  await page.getByRole('tab', { name: 'Write', exact: true }).click();
  await create(page, 'Compact document');
  const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aE1kAAAAASUVORK5CYII=', 'base64');
  await page.locator('input[type=file]').setInputFiles({ name: 'test-image.png', mimeType: 'image/png', buffer: png });
  await expect(page.getByRole('textbox', { name: 'Document title', exact: true })).toHaveValue('test-image.png');
  await expect(page.getByRole('img', { name: 'test-image.png', exact: true })).toBeVisible();
  expect(await page.getByRole('img', { name: 'test-image.png', exact: true }).evaluate((img: HTMLImageElement) => img.naturalWidth)).toBe(1);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: 'Close Library', exact: true }).click();
  await page.getByRole('tab', { name: 'Write', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Document text', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const controls = await page.locator('.console-tool-switch').boundingBox(); expect(controls?.width).toBeGreaterThan(0);
  await page.screenshot({ path: '/private/tmp/console-mobile.png' });
});

test('Console coexists with the combined Focus shell and keeps work across room changes', async ({ page }) => {
  await page.addInitScript(() => { localStorage.setItem('wi.firstLaunch', 'done'); localStorage.setItem('wi.layout', 'focus'); });
  await page.goto('/');
  await expect(page.locator('.case')).toBeVisible();
  await page.keyboard.press('Meta+3');
  await expect(page.locator('[data-room="console"]')).toHaveAttribute('data-current', 'true');
  await page.getByRole('tab', { name: 'Write', exact: true }).click();
  await create(page, 'Room persistence');
  const text = page.getByRole('textbox', { name: 'Document text', exact: true });
  await text.fill('Keep these unsaved words.');
  await page.keyboard.press('Meta+1');
  await expect(page.locator('[data-room="heat"]')).toHaveAttribute('data-current', 'true');
  await page.keyboard.press('Meta+3');
  await expect(text).toHaveText('Keep these unsaved words.');
  await page.locator('.console-name').click();
  await page.keyboard.press('2');
  await expect(page.getByRole('tab', { name: 'Image', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('[data-room="console"]')).toHaveAttribute('data-current', 'true');
  await page.keyboard.press('1');
  await expect(text).toHaveText('Keep these unsaved words.');
  await page.keyboard.press('Meta+s');
  await expect(page.locator('.console-version-label:visible')).toHaveText('v2');
  await page.setViewportSize({ width: 1024, height: 768 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: '/private/tmp/console-full-shell.png' });
});
