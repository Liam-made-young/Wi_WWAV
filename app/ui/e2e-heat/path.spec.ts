import { advance, CMD, expect, openHeat, tab, test } from './kit';

// docs/PLAN.md S2.2, S2.3 and S2.4 on the fake core in a real browser. The
// one full path of the brief: add a task, plan the day, accept, start focus,
// stop, mark done. What a fail looks like: a step that needs the mouse where
// 3.17 gives a key, a draft without its reason, a round that starts without a
// press, minutes that aren't logged to the task, or a check-off that asks
// "Time it took" after time was logged.

test('add a task, plan the day, accept, start focus, stop, mark done', async ({ page }) => {
  await openHeat(page);

  // Add: "+" on Tasks, a name, and Return.
  await tab(page, 'Tasks').click();
  await page.getByRole('button', { name: 'New task' }).click();
  const sheet = page.getByRole('dialog', { name: 'New task' });
  await sheet.getByRole('textbox').first().fill('Write the liner notes');
  await sheet.getByRole('button', { name: 'Add task' }).click();
  await expect(sheet).toHaveCount(0);
  await expect(page.locator('.heat-trow', { hasText: 'Write the liner notes' })).toBeVisible();

  // Plan: Today, Plan my day, its dashed drafts with their reasons; Return accepts them all.
  await tab(page, 'Today').click();
  await page.getByRole('button', { name: 'Plan my day' }).click();
  const draft = page.locator('[data-draft]', { hasText: 'Write the liner notes' });
  await expect(draft).toBeVisible();
  await expect(draft.locator('.heat-row-why')).toHaveText('No due date.');
  await expect(page.locator('[data-draft-block]').first()).toHaveCSS('border-top-style', 'dashed');
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-draft]')).toHaveCount(0);
  const planned = page.locator('.heat-plan [role="group"][aria-label="Planned"] [role="option"]', {
    hasText: 'Write the liner notes',
  });
  await expect(planned).toBeVisible();
  await expect(page.locator('.case-status')).toContainText('Undo plan my day');

  // Focus: nothing runs until F; then the round counts down on the LCD and in the Now strip.
  await expect(page.locator('.heat-pomo')).toHaveAttribute('data-running', 'false');
  await planned.click();
  await page.keyboard.press('f');
  await expect(page.locator('.heat-pomo')).toHaveAttribute('data-running', 'true');
  await expect(page.locator('.heat-lcd-line')).toHaveText('Focus 1 of 4 · Write the liner notes');
  await expect(page.locator('.strip-task .strip-line2')).toContainText(/focus \d+:\d\d left/);

  // Stop: ⇧F logs the minutes to the task.
  await advance(page, 12);
  await page.keyboard.press('Shift+f');
  await expect(page.locator('.heat-lcd-note')).toHaveText('Focus stopped. 12m logged to Write the liner notes.');
  await expect(page.locator('.heat-pomo')).toHaveAttribute('data-phase', 'idle');

  // Done: ⌘↩ with time logged completes it at once, and says what it took.
  await page.keyboard.press(`${CMD}+Enter`);
  await expect(page.locator('.status-left')).toContainText('Done. Took 12m across 1 focus session.');
  await expect(page.getByRole('dialog', { name: 'Time it took' })).toHaveCount(0);
  await tab(page, 'Tasks').click();
  await page.getByRole('button', { name: /^Done/ }).click();
  await expect(page.locator('.heat-trow', { hasText: 'Write the liner notes' })).toBeVisible();
  await expect(page.locator('.case-status')).toContainText('Undo mark done');
});

test('a task with no logged time asks "Time it took" before it is done', async ({ page }) => {
  await openHeat(page);
  await tab(page, 'Tasks').click();
  await page.locator('.heat-trow', { hasText: 'Renew passport' }).click();
  await page.keyboard.press(`${CMD}+Enter`);
  const sheet = page.getByRole('dialog', { name: 'Time it took' });
  await expect(sheet).toContainText('This trains your time averages for this type of task.');
  await sheet.getByRole('spinbutton').fill('40');
  await page.keyboard.press('Enter');
  await expect(page.locator('.status-left')).toContainText('Done. Took 40m.');
  await expect(page.locator('.heat-trow', { hasText: 'Renew passport' })).toHaveCount(0);
});

test('the timer keeps running in another room and the strip reads "focus 18:42 left"', async ({ page }) => {
  await openHeat(page);
  await page.keyboard.press('f');
  await expect(page.locator('.heat-pomo')).toHaveAttribute('data-running', 'true');
  await page.keyboard.press(`${CMD}+3`);
  await expect(page.locator('[data-room="console"]')).toHaveAttribute('data-current', 'true');
  await advance(page, 6);
  // 6 minutes into 25: 19 minutes left, read in the strip with the Console in front.
  await expect(page.locator('.strip-task .strip-line2')).toContainText(/focus 1[89]:\d\d left/);
  await page.keyboard.press(`${CMD}+1`);
  await expect(page.locator('.heat-lcd-digits')).toHaveText(/^1[89]:\d\d$/);
});

test('⌘K opens a task in Heat, and ⌘⇧N captures into the inbox', async ({ page }) => {
  await openHeat(page);
  await page.keyboard.press(`${CMD}+3`);
  await page.keyboard.press(`${CMD}+k`);
  await page.getByRole('combobox').fill('passport');
  await expect(page.getByRole('option', { name: /Renew passport/ })).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-room="heat"]')).toHaveAttribute('data-current', 'true');
  await expect(page.locator('.heat-tabs [role="tab"][aria-selected="true"]')).toHaveText('Tasks');
  await expect(page.locator('.heat-info')).toHaveAttribute('aria-label', 'Get Info: Renew passport');

  await page.keyboard.press(`${CMD}+Shift+n`);
  await page.getByRole('textbox', { name: 'Capture' }).fill('fix the snare at 1:32');
  await page.keyboard.press('Enter');
  await expect(page.locator('.capture-foot')).toHaveText('4 in inbox · captured ✓');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('button', { name: /^Inbox/ })).toContainText('4');
});

test('a task dragged from the hot list onto the time column becomes a block as long as its estimate', async ({
  page,
}) => {
  await openHeat(page);
  const row = page.getByRole('listbox', { name: 'Hot tasks' }).getByRole('option', { name: /Grammar quiz 4/ });
  const column = page.locator('[data-column]');
  const box = (await column.boundingBox())!;
  // 4 PM is nine hours down a 44 px-an-hour column.
  await row.dragTo(column, { targetPosition: { x: 150, y: 9 * 44 + 2 } });
  await expect(
    page.locator('.heat-plan [role="group"][aria-label="Planned"] [role="option"]', { hasText: 'Grammar quiz 4' }),
  ).toContainText('4:00 PM');
  expect(box.height).toBe(17 * 44);
  await expect(page.locator('.case-status')).toContainText('Undo add block');
});

test('P plans the selected task into the next free gap, and every key of 3.17 reaches Heat', async ({ page }) => {
  await openHeat(page);
  await page.keyboard.press('2');
  await expect(page.locator('.heat-tabs [role="tab"][aria-selected="true"]')).toHaveText('Tasks');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('.heat-info')).toHaveAttribute('aria-label', 'Get Info: Listening practice');
  await page.keyboard.press('ArrowDown');
  await expect(page.locator('.heat-info')).toHaveAttribute('aria-label', 'Get Info: Grammar quiz 4');
  await page.keyboard.press('p');
  await expect(page.locator('.status-left')).toContainText('Planned ‘Grammar quiz 4’ at 11:15 AM, 1h.');
  await page.keyboard.press('c');
  await page.keyboard.press('1');
  await expect(page.locator('.heat-now-title')).toHaveText('Grammar quiz 4');
  await page.keyboard.press('3');
  await page.keyboard.press('w');
  await expect(page.locator('.heat-week')).toBeVisible();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('t');
  await expect(page.locator('.heat-cal-head h1')).toHaveText('Oct 4 – Oct 10');
});
