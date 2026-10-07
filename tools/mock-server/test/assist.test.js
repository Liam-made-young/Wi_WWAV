// Claude through the server (docs/SPEC.md 2.11, 3.12, 7.16, 9.8, 10.4) and
// the update manifest (9.9).
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer } from './setup.js';
import { advanceClock, call } from '../helpers.js';

function ask(ctx, token, task, body) {
  return ctx.call('POST', `/api/assist/${task}`, { token, body });
}

describe('assist', () => {
  // Noon UTC, so the day doesn't turn over mid-test.
  const ctx = useServer({ startAt: Date.parse('2026-10-07T12:00:00Z') });

  test('each task answers the same way to the same input', async () => {
    const inputs = {
      score: { title: 'Grammar quiz 4', type: 'Quiz', notes: '', averages: { Quiz: 35 } },
      'score-batch': { items: [{ index: 0, title: 'Lab 5a', type: 'Lab' }] },
      'read-mail': {
        messages: [{ id: 'm1', subject: 'Lab 5a - Due Oct 9', body: 'Due Thursday.' }],
        today: '2026-10-07',
        zone: 'America/New_York',
        titles: [],
      },
      syllabus: { text: 'Homework 40%, Exams 60%' },
      'review-note': { facts: ['Classes: 6 tasks done, 4h 10m of focus'] },
      'release-plan': { title: 'World Ending', releaseDate: '2026-11-01' },
      feedback: { question: 'Does the bass come back?', analysis: { sections: [] } },
      clerk: { record: { title: 'glass hours', artist: 'Ana', notes: '' }, question: 'Who plays guitar?' },
    };
    for (const [task, body] of Object.entries(inputs)) {
      await advanceClock(ctx.url, 61 * 1000); // stay under 10 calls a minute
      const a = await ask(ctx, ctx.ana, task, body);
      const b = await ask(ctx, ctx.ana, task, body);
      assert.equal(a.status, 200, task);
      assert.deepEqual(a.body, b.body, task);
    }
    await advanceClock(ctx.url, 61 * 1000);
    const score = await ask(ctx, ctx.ana, 'score', inputs.score);
    assert.deepEqual(Object.keys(score.body.result).sort(), ['difficulty', 'minutes', 'reason']);
    assert.equal(score.body.result.minutes, 35);
    const clerk = await ask(ctx, ctx.ana, 'clerk', inputs.clerk);
    assert.equal(clerk.body.result.answer, "I don't know. Neither the file nor Ana's notes say.");
  });

  test('an unknown task, a missing title, or too many messages', async () => {
    await advanceClock(ctx.url, 61 * 1000);
    assert.deepEqual((await ask(ctx, ctx.ana, 'decide', {})).body, { error: 'No such task' });
    assert.equal((await ask(ctx, ctx.ana, 'score', {})).status, 400);
    const nine = Array.from({ length: 9 }, (_, i) => ({ id: `m${i}`, subject: 's', body: 'b' }));
    assert.equal((await ask(ctx, ctx.ana, 'read-mail', { messages: nine, today: '2026-10-07' })).status, 400);
  });

  test("more than 10 calls in a minute is 429 with the app's sentence", async () => {
    let last;
    for (let i = 0; i < 11; i++) last = await ask(ctx, ctx.lmy, 'score', { title: `t${i}` });
    assert.equal(last.status, 429);
    assert.equal(last.body.code, 'rate_limited');
    assert.equal(last.body.error, 'Too many requests. Wait a minute, then try again.');
    assert.ok(Number(last.headers.get('retry-after')) > 0);
  });

  test('50 calls a day per account, back at midnight', async () => {
    await advanceClock(ctx.url, 61 * 1000);
    const used = ctx.state.assist.get(ctx.state.users[0].id)?.used ?? 0;
    for (let i = used; i < 50; i++) {
      if (i % 10 === 0) await advanceClock(ctx.url, 61 * 1000);
      assert.equal((await ask(ctx, ctx.lmy, 'score', { title: `t${i}` })).status, 200, `call ${i + 1}`);
    }
    await advanceClock(ctx.url, 61 * 1000);
    const over = await ask(ctx, ctx.lmy, 'score', { title: 'one more' });
    assert.equal(over.status, 429);
    assert.deepEqual(
      { error: over.body.error, code: over.body.code },
      { error: "Claude's 50 calls for today are used. They come back at midnight.", code: 'daily_limit' },
    );
    assert.equal((await ask(ctx, ctx.ana, 'score', { title: 'hers' })).status, 200);
    await advanceClock(ctx.url, 12 * 60 * 60 * 1000);
    assert.equal((await ask(ctx, ctx.lmy, 'score', { title: 'tomorrow' })).status, 200);
  });

  test('assist needs a signed-in account', async () => {
    assert.equal((await ask(ctx, undefined, 'score', { title: 'x' })).status, 401);
  });
});

describe('the update manifest', () => {
  const ctx = useServer();
  test('/desktop/latest.json is a Tauri updater manifest', async () => {
    const res = await call(ctx.url, 'GET', '/desktop/latest.json');
    assert.equal(res.status, 200);
    assert.match(res.body.version, /^\d+\.\d+\.\d+$/);
    assert.ok(res.body.platforms['darwin-aarch64'].url);
    assert.equal(typeof res.body.platforms['darwin-aarch64'].signature, 'string');
    const next = { ...res.body, version: '0.2.0', notes: 'Faster export.' };
    assert.equal((await call(ctx.url, 'PUT', '/__mock/latest', { body: next })).status, 200);
    assert.deepEqual((await call(ctx.url, 'GET', '/desktop/latest.json')).body, next);
  });
});
