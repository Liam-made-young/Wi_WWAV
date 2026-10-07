// Publishing and ids (docs/SPEC.md 2.8, 6.8, 9.7; PLAN S1.9, server side).
// S1.9 fails if a retry posts twice. The server's half: publishing the same
// trackId again, or the same {origin, clipId} under a re-signed trackId, or
// a publish whose answer was lost and retried, leaves one work.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { useServer } from './setup.js';
import { WORKS } from '../fixtures.js';
import { call, putSigned } from '../helpers.js';
import { writeWwav } from '../wwav.js';
import { platformSongId } from '../hash.js';

const songId = () => randomBytes(16).toString('hex');

async function upload(ctx, token, bytes, fileType = 'audio/wav') {
  const signed = await ctx.call('GET', `/api/upload/sign?fileType=${fileType}&size=${bytes.length}`, { token });
  assert.equal(signed.status, 200);
  assert.equal((await putSigned(signed.body.signedUrl, bytes, fileType)).status, 200);
  return signed.body.trackId;
}

function publish(ctx, token, body) {
  return ctx.call('POST', '/api/publish', { token, body });
}

function worksBy(ctx, id) {
  return ctx.state.tracks.filter((t) => t.songId === id);
}

describe('publish: one work per song, whatever the retries', () => {
  const ctx = useServer();

  test('sign, PUT, publish works with no /process step, and answers the ids', async () => {
    const id = songId();
    const trackId = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Tide Pool', artist: 'LMY' }));
    const res = await publish(ctx, ctx.lmy, { trackId, settings: { origin: 'wi_wwav', clipId: '01JAA' } });
    assert.equal(res.status, 200);
    assert.equal(res.body.success, true);
    assert.equal(res.body.trackId, trackId);
    assert.equal(res.body.songId, id);
    assert.equal(res.body.version, 1);
    assert.ok(Number.isInteger(res.body.publishedId));
    const track = worksBy(ctx, id)[0];
    assert.equal(track.title, 'Tide Pool');
    assert.equal(track.isMaster, true);
  });

  test('the same trackId published twice is one work and one version', async () => {
    const id = songId();
    const trackId = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Twice' }));
    const body = { trackId, isMaster: true, settings: { origin: 'wi_wwav', clipId: '01JAB' } };
    assert.equal((await publish(ctx, ctx.lmy, body)).status, 200);
    const again = await publish(ctx, ctx.lmy, body);
    assert.equal(again.status, 200);
    assert.equal(again.body.updated, true);
    assert.equal(worksBy(ctx, id).length, 1);
    assert.equal(worksBy(ctx, id)[0].versions.length, 1);
  });

  test('a re-signed retry with the same clipId lands on the first trackId', async () => {
    const bytes = writeWwav({ songId: songId(), title: 'Re-signed' });
    const first = await upload(ctx, ctx.lmy, bytes);
    const settings = { origin: 'wi_wwav', clipId: '01JAC' };
    await publish(ctx, ctx.lmy, { trackId: first, settings });
    const second = await upload(ctx, ctx.lmy, bytes);
    const res = await publish(ctx, ctx.lmy, { trackId: second, settings });
    assert.equal(res.status, 200);
    assert.equal(res.body.trackId, first);
    assert.equal(res.body.message, 'Already up');
    assert.equal(ctx.state.tracks.filter((t) => t.trackId === second).length, 0);
  });

  test('a plain WAV re-signed and retried with the same clipId is still one work', async () => {
    const wav = writeWwav({ plain: true });
    const settings = { origin: 'wi_wwav', clipId: '01JAD' };
    const first = await upload(ctx, ctx.lmy, wav);
    await publish(ctx, ctx.lmy, { trackId: first, title: 'Plain', settings });
    const second = await upload(ctx, ctx.lmy, wav);
    const res = await publish(ctx, ctx.lmy, { trackId: second, title: 'Plain', settings });
    assert.equal(res.body.trackId, first);
    assert.equal(ctx.state.tracks.filter((t) => t.title === 'Plain').length, 1);
  });

  test('a publish whose answer was lost, retried, is one work', async () => {
    const id = songId();
    const trackId = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Lost answer' }));
    await call(ctx.url, 'POST', '/__mock/fail', { body: { method: 'POST', path: '/api/publish', drop: 'after' } });
    const body = { trackId, settings: { origin: 'wi_wwav', clipId: '01JAE' } };
    await assert.rejects(publish(ctx, ctx.lmy, body));
    assert.equal(worksBy(ctx, id).length, 1);
    const retry = await publish(ctx, ctx.lmy, body);
    assert.equal(retry.status, 200);
    assert.equal(worksBy(ctx, id).length, 1);
    assert.equal(worksBy(ctx, id)[0].versions.length, 1);
  });

  test('re-publishing without isMaster keeps it a master', async () => {
    const id = songId();
    const trackId = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Still master' }));
    await publish(ctx, ctx.lmy, { trackId, isMaster: true });
    await publish(ctx, ctx.lmy, { trackId, title: 'Still master, renamed' });
    assert.equal(worksBy(ctx, id)[0].isMaster, true);
  });

  test("publishing someone else's upload, or nothing uploaded yet, is refused", async () => {
    const trackId = await upload(ctx, ctx.lmy, writeWwav({ songId: songId() }));
    const theirs = await publish(ctx, ctx.ana, { trackId });
    assert.deepEqual([theirs.status, theirs.body], [403, { error: "You don't own this track" }]);
    const signed = await ctx.call('GET', '/api/upload/sign?fileType=audio/wav&size=10', { token: ctx.lmy });
    const early = await publish(ctx, ctx.lmy, { trackId: signed.body.trackId });
    assert.deepEqual([early.status, early.body], [409, { error: "The file hasn't finished uploading." }]);
  });

  test('no token is a JSON 401, not text', async () => {
    const res = await publish(ctx, undefined, { trackId: 'x' });
    assert.deepEqual([res.status, res.body], [401, { error: 'Unauthorized' }]);
  });
});

describe('6.8: song ids on the server', () => {
  const ctx = useServer();

  test('the same sha256 again does nothing: "Already up"', async () => {
    const id = songId();
    const bytes = writeWwav({ songId: id, title: 'Same bytes' });
    await publish(ctx, ctx.lmy, { trackId: await upload(ctx, ctx.lmy, bytes) });
    const res = await publish(ctx, ctx.lmy, { trackId: await upload(ctx, ctx.lmy, bytes) });
    assert.equal(res.status, 200);
    assert.equal(res.body.message, 'Already up');
    assert.equal(worksBy(ctx, id)[0].versions.length, 1);
  });

  test('a higher version from the same account is a new version; the link plays the newest', async () => {
    const id = songId();
    const v1 = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Grows' }));
    await publish(ctx, ctx.lmy, { trackId: v1 });
    const v2 = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Grows', version: 2 }));
    const res = await publish(ctx, ctx.lmy, { trackId: v2 });
    assert.equal(res.status, 200);
    assert.equal(res.body.trackId, v1);
    assert.equal(res.body.version, 2);
    const work = worksBy(ctx, id);
    assert.equal(work.length, 1);
    assert.deepEqual(
      work[0].versions.map((v) => v.version),
      [1, 2],
    );
  });

  test('the same version with other bytes is refused', async () => {
    const id = songId();
    await publish(ctx, ctx.lmy, { trackId: await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Fixed' })) });
    const other = writeWwav({ songId: id, title: 'Fixed', bpm: 99 });
    const res = await publish(ctx, ctx.lmy, { trackId: await upload(ctx, ctx.lmy, other) });
    assert.equal(res.status, 409);
    assert.equal(res.body.error, "Version 1 of 'Fixed' is already up. Export it again as a new version.");
  });

  test("another account's id is refused with the spec's sentence", async () => {
    const bytes = writeWwav({ songId: WORKS.lowTide.songId, title: 'Not mine' });
    const res = await publish(ctx, ctx.ana, { trackId: await upload(ctx, ctx.ana, bytes) });
    assert.equal(res.status, 409);
    assert.deepEqual(res.body, {
      error: "This file's id belongs to another work, 'Low Tide' by LMY.",
      code: 'id_taken',
    });
  });

  test('a remix reads its lineage from wlin', async () => {
    const id = songId();
    const bytes = writeWwav({
      songId: id,
      title: 'Low Tide (Ana remix)',
      type: 'remix',
      parentId: WORKS.lowTide.songId,
      rootId: WORKS.worldEnding.songId,
      generation: 2,
      creator: 'Ana',
    });
    await publish(ctx, ctx.ana, { trackId: await upload(ctx, ctx.ana, bytes) });
    const track = worksBy(ctx, id)[0];
    const parent = ctx.state.tracks.find((t) => t.songId === WORKS.lowTide.songId);
    const root = ctx.state.tracks.find((t) => t.songId === WORKS.worldEnding.songId);
    assert.equal(track.parentTrackId, parent.trackId);
    assert.equal(track.lineageRootTrackId, root.trackId);
    assert.equal(track.remixDepth, 2);
    const tree = await ctx.call('GET', `/api/tracks/${track.trackId}/lineage`);
    assert.equal(tree.status, 200);
    assert.equal(tree.body.root.trackId, root.trackId);
    assert.deepEqual(
      tree.body.ancestors.map((n) => n.title),
      ['World Ending', 'Low Tide'],
    );
  });

  test('a platform WAV takes its song id from its trackId (FNV-1a 128)', async () => {
    const trackId = await upload(ctx, ctx.lmy, writeWwav({ plain: true }));
    const res = await publish(ctx, ctx.lmy, { trackId, title: 'Platform' });
    assert.match(res.body.songId, /^[0-9a-f]{32}$/);
  });

  test('unpublish takes it out of the sky and keeps it in its family, withdrawn', async () => {
    const trackId = await upload(ctx, ctx.lmy, writeWwav({ songId: songId(), title: 'Going' }));
    await publish(ctx, ctx.lmy, { trackId });
    const theirs = await ctx.call('POST', '/api/unpublish', { token: ctx.ana, body: { trackId } });
    assert.deepEqual([theirs.status, theirs.body], [403, { error: 'Unauthorized' }]);
    const res = await ctx.call('POST', '/api/unpublish', { token: ctx.lmy, body: { trackId } });
    assert.deepEqual(res.body, { success: true });
    const tree = await ctx.call('GET', `/api/tracks/${trackId}/lineage`);
    assert.equal(tree.body.root.withdrawn, true);
    const again = await ctx.call('POST', '/api/unpublish', { token: ctx.lmy, body: { trackId } });
    assert.deepEqual([again.status, again.body], [404, { error: 'Not published' }]);
  });
});

describe('publishing again', () => {
  const ctx = useServer();

  test('a work published again after unpublish is back: not "Already up", and in the catalog', async () => {
    const id = songId();
    const trackId = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Comes back' }));
    await publish(ctx, ctx.lmy, { trackId });
    await ctx.call('POST', '/api/unpublish', { token: ctx.lmy, body: { trackId } });
    const back = await publish(ctx, ctx.lmy, { trackId });
    assert.equal(back.status, 200);
    assert.equal(back.body.message, undefined);
    assert.equal(worksBy(ctx, id)[0].withdrawn, false);
    const catalog = await ctx.call('GET', '/api/v2/catalog?search=comes');
    assert.deepEqual(
      catalog.body.data.items.map((i) => i.title),
      ['Comes back'],
    );
  });

  test('"Already up" adds no version but takes the new title; a refused publish takes nothing', async () => {
    const id = songId();
    const bytes = writeWwav({ songId: id, title: 'Draft name' });
    const trackId = await upload(ctx, ctx.lmy, bytes);
    await publish(ctx, ctx.lmy, { trackId });
    const renamed = await publish(ctx, ctx.lmy, { trackId, title: 'Final name' });
    assert.equal(renamed.body.message, 'Already up');
    assert.equal(worksBy(ctx, id)[0].title, 'Final name');
    const other = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Draft name', bpm: 70 }));
    const refused = await publish(ctx, ctx.lmy, { trackId: other, title: 'Refused name', tags: ['x'] });
    assert.equal(refused.status, 409);
    assert.deepEqual([worksBy(ctx, id)[0].title, worksBy(ctx, id)[0].tags], ['Final name', []]);
  });

  test("a version's bytes are kept under their sha256, out of reach of any upload URL", async () => {
    const id = songId();
    const bytes = writeWwav({ songId: id, title: 'Kept bytes' });
    const trackId = await upload(ctx, ctx.lmy, bytes);
    await publish(ctx, ctx.lmy, { trackId });
    const version = worksBy(ctx, id)[0].versions[0];
    assert.equal(version.s3Key, `files/${version.sha256}`);
    assert.deepEqual(ctx.state.objects.get(version.s3Key).bytes, bytes);
  });

  test("another account's upload owns its trackId's platform id, even before it is published", async () => {
    const theirs = await ctx.call('GET', '/api/upload/sign?fileType=audio/wav&size=10', { token: ctx.ana });
    const squat = await upload(
      ctx,
      ctx.lmy,
      writeWwav({ songId: platformSongId(theirs.body.trackId), title: 'Squat' }),
    );
    const res = await publish(ctx, ctx.lmy, { trackId: squat });
    assert.deepEqual(
      [res.status, res.body],
      [409, { error: "This file's id belongs to another account's upload.", code: 'id_taken' }],
    );
  });
});
