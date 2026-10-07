// Heat sync (docs/SPEC.md 2.8, 3.15, 9.7; PLAN S2.7, server side). S2.7
// fails if a slower older write overwrites a newer one on any field. The
// server keeps the higher sequence number field by field.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer } from './setup.js';

function push(ctx, token, device, changes) {
  return ctx.call('POST', '/api/heat/changes', { token, body: { device, changes } });
}

async function pullAll(ctx, token, cursor = 0, limit) {
  const out = [];
  for (;;) {
    const res = await ctx.call('GET', `/api/heat/changes?cursor=${cursor}${limit ? `&limit=${limit}` : ''}`, { token });
    assert.equal(res.status, 200);
    out.push(...res.body.changes);
    cursor = res.body.cursor;
    if (!res.body.more) return { changes: out, cursor };
  }
}

function field(changes, id, name) {
  return changes.find((c) => c.id === id && c.field === name);
}

describe('heat changes', () => {
  const ctx = useServer();

  test("a slow older write arriving late doesn't overwrite the newer one", async () => {
    const newer = await push(ctx, ctx.lmy, 'mac-a', [
      { kind: 'task', id: 't1', field: 'title', value: 'Quiz 4', seq: 7 },
    ]);
    assert.equal(newer.status, 200);
    const older = await push(ctx, ctx.lmy, 'mac-a', [
      { kind: 'task', id: 't1', field: 'title', value: 'Quiz', seq: 6 },
    ]);
    assert.deepEqual(older.body.kept, [
      { kind: 'task', id: 't1', field: 'title', value: 'Quiz 4', seq: 7, device: 'mac-a' },
    ]);
    const { changes } = await pullAll(ctx, ctx.lmy);
    assert.equal(field(changes, 't1', 'title').value, 'Quiz 4');
  });

  test('fields merge one by one: each keeps its own higher write', async () => {
    await push(ctx, ctx.lmy, 'mac-a', [
      { kind: 'task', id: 't2', field: 'title', value: 'Lab 5a', seq: 10 },
      { kind: 'task', id: 't2', field: 'due', value: '2026-10-09T23:59', seq: 3 },
    ]);
    await push(ctx, ctx.lmy, 'mac-b', [
      { kind: 'task', id: 't2', field: 'title', value: 'Lab 5', seq: 4 },
      { kind: 'task', id: 't2', field: 'due', value: '2026-10-10T23:59', seq: 11 },
    ]);
    const { changes } = await pullAll(ctx, ctx.lmy);
    assert.equal(field(changes, 't2', 'title').value, 'Lab 5a');
    assert.equal(field(changes, 't2', 'due').value, '2026-10-10T23:59');
  });

  test('equal sequence numbers settle by device id, the same way on every replica', async () => {
    await push(ctx, ctx.lmy, 'mac-b', [{ kind: 'habit', id: 'h1', field: 'title', value: 'B', seq: 5 }]);
    await push(ctx, ctx.lmy, 'mac-a', [{ kind: 'habit', id: 'h1', field: 'title', value: 'A', seq: 5 }]);
    const { changes } = await pullAll(ctx, ctx.lmy);
    assert.equal(field(changes, 'h1', 'title').value, 'B');
  });

  test('pulling from a cursor answers only what changed since, a page at a time', async () => {
    const { cursor } = await pullAll(ctx, ctx.lmy);
    await push(ctx, ctx.lmy, 'mac-a', [
      { kind: 'note', id: 'n1', field: 'markdown', value: 'one', seq: 1 },
      { kind: 'note', id: 'n2', field: 'markdown', value: 'two', seq: 1 },
      { kind: 'note', id: 'n3', field: 'markdown', value: 'three', seq: 1 },
    ]);
    const first = await ctx.call('GET', `/api/heat/changes?cursor=${cursor}&limit=2`, { token: ctx.lmy });
    assert.equal(first.body.changes.length, 2);
    assert.equal(first.body.more, true);
    const rest = await pullAll(ctx, ctx.lmy, first.body.cursor);
    assert.deepEqual(
      rest.changes.map((c) => c.id),
      ['n3'],
    );
    const nothing = await pullAll(ctx, ctx.lmy, rest.cursor);
    assert.deepEqual(nothing.changes, []);
  });

  test("an overwritten value isn't pulled twice: only the field's current value comes back", async () => {
    const { cursor } = await pullAll(ctx, ctx.lmy);
    await push(ctx, ctx.lmy, 'mac-a', [{ kind: 'task', id: 't3', field: 'done', value: false, seq: 1 }]);
    await push(ctx, ctx.lmy, 'mac-a', [{ kind: 'task', id: 't3', field: 'done', value: true, seq: 2 }]);
    const { changes } = await pullAll(ctx, ctx.lmy, cursor);
    assert.deepEqual(
      changes.map((c) => [c.id, c.value]),
      [['t3', true]],
    );
  });

  test('Heat is private: another account pulls nothing of it', async () => {
    const { changes } = await pullAll(ctx, ctx.ana);
    assert.deepEqual(changes, []);
  });

  test('a batch over 500, a bad sequence number or a missing device is refused', async () => {
    const many = Array.from({ length: 501 }, (_, i) => ({
      kind: 'task',
      id: `x${i}`,
      field: 'title',
      value: '',
      seq: 1,
    }));
    assert.equal((await push(ctx, ctx.lmy, 'mac-a', many)).status, 413);
    const bad = await push(ctx, ctx.lmy, 'mac-a', [{ kind: 'task', id: 'x', field: 'title', value: '', seq: 0 }]);
    assert.equal(bad.status, 400);
    const anon = await push(ctx, ctx.lmy, '', [{ kind: 'task', id: 'x', field: 'title', value: '', seq: 1 }]);
    assert.equal(anon.status, 400);
  });
});
