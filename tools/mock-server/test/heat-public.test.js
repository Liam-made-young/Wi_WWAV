// The public Heat view (docs/SPEC.md 3.15, 8.7; PLAN S2.9, server side).
// S2.9 fails if a private record, grade or note appears; a public record
// shows more than the fields 3.15 lists; switching it back doesn't remove
// its copy; a Now making line outlives its clearsAt; or any response
// carries a count or a total.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer } from './setup.js';

let seq = 0;
function fields(kind, id, values) {
  return Object.entries(values).map(([field, value]) => ({ kind, id, field, value, seq: ++seq }));
}

async function push(ctx, changes) {
  const res = await ctx.call('POST', '/api/heat/changes', { token: ctx.lmy, body: { device: 'mac-a', changes } });
  assert.equal(res.status, 200);
}

async function view(ctx, userId = 1) {
  const res = await ctx.call('GET', `/api/heat/public/${userId}`);
  assert.equal(res.status, 200);
  return res.body;
}

// Every number in the answer, with where it sits, so a count can't hide.
function numbers(value, path = '') {
  if (typeof value === 'number') return [path];
  if (Array.isArray(value)) return value.flatMap((v, i) => numbers(v, `${path}[${i}]`));
  if (value && typeof value === 'object') return Object.entries(value).flatMap(([k, v]) => numbers(v, `${path}.${k}`));
  return [];
}

describe('the public Learn view', () => {
  const ctx = useServer();

  test('a private record never appears, and a public one shows only its listed fields', async () => {
    await push(ctx, [
      ...fields('task', 'secret', { title: 'Therapy', notes: 'private', difficulty: 3, public: false }),
      ...fields('task', 'shown', { title: 'Mix the second verse', due: 1791500000000, done: false, notes: 'not this', difficulty: 4, estMin: 90, public: true }),
      ...fields('note', 'n1', { title: 'Diary', markdown: 'not shown', public: false }),
    ]);
    const body = await view(ctx);
    assert.deepEqual(body.items.task, [{ id: 'shown', title: 'Mix the second verse', due: 1791500000000, done: false }]);
    assert.equal(body.items.note, undefined);
    assert.ok(!JSON.stringify(body).includes('Therapy'));
  });

  test('a public grade shows its course code, item, score and out of, and nothing worked out', async () => {
    await push(ctx, [
      ...fields('course', 'jpn', { code: 'JPN 201', name: 'Japanese 2', public: false }),
      ...fields('grade', 'g1', { courseId: 'jpn', title: 'Quiz 4', score: 9, outOf: 10, categoryId: 'quiz', public: true }),
    ]);
    const body = await view(ctx);
    assert.deepEqual(body.items.grade, [{ id: 'g1', title: 'Quiz 4', score: 9, outOf: 10, course: 'JPN 201' }]);
    assert.equal(body.items.course, undefined, 'the course itself stays private');
  });

  test('switching a record back to private, or deleting it, removes its copy', async () => {
    await push(ctx, fields('task', 'shown', { public: false }));
    await push(ctx, fields('grade', 'g1', { deleted: true }));
    const body = await view(ctx);
    assert.equal(body.items.task, undefined);
    assert.equal(body.items.grade, undefined);
  });

  test('the Now making line comes first and is gone at its clearsAt, with the Mac off', async () => {
    const now = Date.parse((await ctx.call('POST', '/__mock/clock', { body: { advanceMs: 0 } })).body.now);
    await push(ctx, fields('profileShare', 's-now', { kind: 'now', sourceId: 'shown', text: 'Now making: the second verse of More Love', targetId: 'sun', clearsAt: now + 60_000 }));
    assert.deepEqual((await view(ctx)).now, { text: 'Now making: the second verse of More Love' });
    await ctx.call('POST', '/__mock/clock', { body: { advanceMs: 61_000 } });
    assert.equal((await view(ctx)).now, null);
  });

  test("a project timeline shows its beads, each title, date and whether it's reached", async () => {
    await push(ctx, [
      ...fields('project', 'ep', { title: 'Low Tide EP', status: 'active' }),
      ...fields('milestone', 'm2', { projectId: 'ep', title: 'Mixed', date: '2026-11-01', done: false, order: 1 }),
      ...fields('milestone', 'm1', { projectId: 'ep', title: 'Written', date: '2026-10-01', done: true, order: 0 }),
      ...fields('profileShare', 's-tl', { kind: 'timeline', sourceId: 'ep', targetId: 'system-7' }),
    ]);
    const body = await view(ctx);
    assert.deepEqual(body.timelines, [
      {
        projectId: 'ep',
        targetId: 'system-7',
        title: 'Low Tide EP',
        milestones: [
          { id: 'm1', title: 'Written', date: '2026-10-01', done: true },
          { id: 'm2', title: 'Mixed', date: '2026-11-01', done: false },
        ],
      },
    ]);
  });

  test('no answer carries a count or a total', async () => {
    const body = await view(ctx);
    const allowed = /\.(due|score|outOf|startedAt|focusMin)$|^\.now$/;
    assert.deepEqual(numbers(body).filter((p) => !allowed.test(p)), []);
    for (const word of ['count', 'total', 'sum', 'average', 'rank', 'streak', 'percent', 'letter']) {
      assert.ok(!JSON.stringify(body).toLowerCase().includes(`"${word}`), word);
    }
  });

  test('anyone can read it, and an unknown account is a 404', async () => {
    const res = await ctx.call('GET', '/api/heat/public/999');
    assert.equal(res.status, 404);
  });
});
