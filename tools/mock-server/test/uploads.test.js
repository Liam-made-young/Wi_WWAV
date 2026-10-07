// Uploads (docs/SPEC.md 2.8, 9.7; PLAN S1.9, server side). S1.9 fails if a
// resumed upload re-sends a finished part: the mock counts every PUT of
// every part, so a client test can see a second send. Presigned URLs
// expire as R2's do, so a client that signs early fails here too.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { useServer } from './setup.js';
import { advanceClock, call, putSigned } from '../helpers.js';

const MiB = 1024 * 1024;

// Bytes that differ from one 8 MiB part to the next (251 is prime, so the
// pattern doesn't repeat on a part boundary).
function pattern(size, seed = 1) {
  const bytes = Buffer.alloc(size);
  for (let i = 0; i < size; i += 4096) bytes[i] = ((i / 4096) * 31 + seed) % 251;
  return bytes;
}

describe('single-PUT signing', () => {
  const ctx = useServer();

  test('/sign answers a URL, a key and a trackId; the PUT stores the exact bytes', async () => {
    const res = await ctx.call('GET', '/api/upload/sign?fileType=audio/wav&size=1000', { token: ctx.lmy });
    assert.equal(res.status, 200);
    assert.match(res.body.trackId, /^track_\d+_[a-z0-9]{9}$/);
    assert.equal(res.body.s3Key, `uploads/${res.body.trackId}`);
    const bytes = pattern(1000);
    const put = await putSigned(res.body.signedUrl, bytes, 'audio/wav');
    assert.equal(put.status, 200);
    assert.equal(put.etag, `"${createHash('md5').update(bytes).digest('hex')}"`);
    assert.deepEqual(ctx.state.objects.get(res.body.s3Key).bytes, bytes);
  });

  test('the URL from /sign lasts 300 s', async () => {
    const res = await ctx.call('GET', '/api/upload/sign?fileType=audio/wav&size=4', { token: ctx.lmy });
    await advanceClock(ctx.url, 301 * 1000);
    const put = await putSigned(res.body.signedUrl, Buffer.from('RIFF'), 'audio/wav');
    assert.equal(put.status, 403);
    assert.match(put.text, /Request has expired/);
  });

  test('a PUT with another content type, or a changed URL, is refused', async () => {
    const res = await ctx.call('GET', '/api/upload/sign?fileType=audio/wav&size=4', { token: ctx.lmy });
    assert.equal((await putSigned(res.body.signedUrl, Buffer.from('RIFF'), 'audio/mpeg')).status, 403);
    const tampered = res.body.signedUrl.replace('uploads/', 'uploads/x');
    assert.equal((await putSigned(tampered, Buffer.from('RIFF'), 'audio/wav')).status, 403);
  });

  test('/sign refuses over 250 MB, an empty file and an unknown type', async () => {
    const big = await ctx.call('GET', `/api/upload/sign?fileType=audio/wav&size=${250 * MiB + 1}`, { token: ctx.lmy });
    assert.deepEqual([big.status, big.body], [413, { error: 'File too large (max 250 MB)' }]);
    const empty = await ctx.call('GET', '/api/upload/sign?fileType=audio/wav&size=0', { token: ctx.lmy });
    assert.deepEqual([empty.status, empty.body], [400, { error: 'File is empty' }]);
    const odd = await ctx.call('GET', '/api/upload/sign?fileType=text/plain&size=4', { token: ctx.lmy });
    assert.deepEqual([odd.status, odd.body], [400, { error: 'Unsupported audio type: text/plain' }]);
  });

  test('/sign-video lasts 600 s and keys the file under the account', async () => {
    const me = (await ctx.call('GET', '/api/auth/me', { token: ctx.lmy })).body;
    const res = await ctx.call('GET', '/api/upload/sign-video?ext=mp4&fileType=video/mp4&size=8', { token: ctx.lmy });
    assert.equal(res.status, 200);
    assert.match(res.body.s3Key, new RegExp(`^videos/${me.id}/\\d+\\.mp4$`));
    assert.ok(Math.abs(lifetime(res.body.signedUrl, ctx) - 600) <= 1);
  });

  test('/sign-replace is only for the account that owns the track', async () => {
    const mine = await ctx.call('GET', '/api/upload/sign?fileType=audio/wav&size=4', { token: ctx.lmy });
    const t = mine.body.trackId;
    const ok = await ctx.call('GET', `/api/upload/sign-replace?trackId=${t}&fileType=audio/wav&size=4`, {
      token: ctx.lmy,
    });
    assert.equal(ok.status, 200);
    assert.match(ok.body.s3Key, new RegExp(`^uploads/${t}_v\\d+$`));
    assert.ok(Math.abs(lifetime(ok.body.signedUrl, ctx) - 600) <= 1);
    const theirs = await ctx.call('GET', `/api/upload/sign-replace?trackId=${t}&fileType=audio/wav`, {
      token: ctx.ana,
    });
    assert.deepEqual([theirs.status, theirs.body], [403, { error: 'Not authorized' }]);
    const none = await ctx.call('GET', '/api/upload/sign-replace?trackId=track_0_nothing00&fileType=audio/wav', {
      token: ctx.lmy,
    });
    assert.equal(none.status, 404);
  });

  test('an unsigned GET of an upload is refused', async () => {
    const res = await ctx.call('GET', '/api/upload/sign?fileType=audio/wav&size=3', { token: ctx.ana });
    await putSigned(res.body.signedUrl, Buffer.from('abc'), 'audio/wav');
    const raw = await fetch(new URL(`/r2/${res.body.s3Key}`, ctx.url));
    assert.equal(raw.status, 403);
  });
});

// Seconds a presigned URL has left (within a second of when it was signed).
function lifetime(url, ctx) {
  return Number(new URL(url).searchParams.get('expires')) - Math.floor(ctx.state.now() / 1000);
}

describe('multipart: /api/upload/parts in 8 MiB parts', () => {
  const ctx = useServer();
  const size = 20 * MiB + 123;
  const file = pattern(size, 7);

  async function create(token = ctx.lmy) {
    const res = await ctx.call('POST', '/api/upload/parts', { token, body: { size, fileType: 'audio/wav' } });
    assert.equal(res.status, 200);
    return res.body;
  }

  async function sendPart(upload, n) {
    const signed = await ctx.call('GET', `/api/upload/parts/${upload.uploadId}/${n}`, { token: ctx.lmy });
    assert.equal(signed.status, 200);
    const start = (n - 1) * upload.partSize;
    const put = await putSigned(signed.body.signedUrl, file.subarray(start, start + signed.body.size));
    assert.equal(put.status, 200);
    return put.etag;
  }

  test('create answers the part size, the part total and the key; ownership starts now', async () => {
    const upload = await create();
    assert.equal(upload.partSize, 8 * MiB);
    assert.equal(upload.parts, 3);
    assert.equal(upload.s3Key, `uploads/${upload.trackId}`);
    assert.ok(ctx.state.uploads.some((u) => u.trackId === upload.trackId && u.s3Key === upload.s3Key));
  });

  test('parts signed one at a time, sent once each, complete into the exact file', async () => {
    const upload = await create();
    const parts = [];
    for (let n = 1; n <= upload.parts; n++) parts.push({ n, etag: await sendPart(upload, n) });
    const done = await ctx.call('POST', `/api/upload/parts/${upload.uploadId}/complete`, {
      token: ctx.lmy,
      body: { parts },
    });
    assert.equal(done.status, 200);
    assert.equal(done.body.sha256, createHash('sha256').update(file).digest('hex'));
    assert.equal(done.body.size, size);
    assert.deepEqual(ctx.state.objects.get(upload.s3Key).bytes, file);
    const again = await ctx.call('POST', `/api/upload/parts/${upload.uploadId}/complete`, {
      token: ctx.lmy,
      body: { parts },
    });
    assert.deepEqual(again.body, done.body);
  });

  test('a resume after part 2 sends only part 3, and the mock shows each part arrived once', async () => {
    const upload = await create();
    const parts = [
      { n: 1, etag: await sendPart(upload, 1) },
      { n: 2, etag: await sendPart(upload, 2) },
    ];
    // The app quits here. On relaunch upload_part has rows for 1 and 2.
    parts.push({ n: 3, etag: await sendPart(upload, 3) });
    await ctx.call('POST', `/api/upload/parts/${upload.uploadId}/complete`, { token: ctx.lmy, body: { parts } });
    const sends = [...ctx.state.multipart.get(upload.uploadId).parts.values()].map((p) => p.sends);
    assert.deepEqual(sends, [1, 1, 1]);
  });

  test('a part sent twice shows as two sends, so a client test can catch it', async () => {
    const upload = await create();
    await sendPart(upload, 1);
    await sendPart(upload, 1);
    assert.equal(ctx.state.multipart.get(upload.uploadId).parts.get(1).sends, 2);
    const state = await call(ctx.url, 'GET', '/__mock/state');
    assert.equal(state.body.multipart[upload.uploadId].parts['1'].sends, 2);
  });

  test('a part of the wrong length is refused', async () => {
    const upload = await create();
    const signed = await ctx.call('GET', `/api/upload/parts/${upload.uploadId}/1`, { token: ctx.lmy });
    const put = await putSigned(signed.body.signedUrl, file.subarray(0, 100));
    assert.equal(put.status, 400);
    assert.match(put.text, /Part 1 should be 8388608 bytes/);
  });

  test('complete refuses a missing part or a wrong etag', async () => {
    const upload = await create();
    const e1 = await sendPart(upload, 1);
    const e2 = await sendPart(upload, 2);
    const missing = await ctx.call('POST', `/api/upload/parts/${upload.uploadId}/complete`, {
      token: ctx.lmy,
      body: {
        parts: [
          { n: 1, etag: e1 },
          { n: 2, etag: e2 },
        ],
      },
    });
    assert.deepEqual([missing.status, missing.body], [400, { error: "Part 3 hasn't arrived" }]);
    const e3 = await sendPart(upload, 3);
    const wrong = await ctx.call('POST', `/api/upload/parts/${upload.uploadId}/complete`, {
      token: ctx.lmy,
      body: {
        parts: [
          { n: 1, etag: e2 },
          { n: 2, etag: e2 },
          { n: 3, etag: e3 },
        ],
      },
    });
    assert.deepEqual([wrong.status, wrong.body], [400, { error: "Part 1's etag doesn't match what arrived" }]);
  });

  test('another account can neither sign nor complete it; part numbers outside the file are refused', async () => {
    const upload = await create();
    const theirs = await ctx.call('GET', `/api/upload/parts/${upload.uploadId}/1`, { token: ctx.ana });
    assert.equal(theirs.status, 403);
    const zero = await ctx.call('GET', `/api/upload/parts/${upload.uploadId}/4`, { token: ctx.lmy });
    assert.deepEqual([zero.status, zero.body], [400, { error: 'There is no part 4' }]);
    const done = await ctx.call('POST', `/api/upload/parts/${upload.uploadId}/complete`, {
      token: ctx.ana,
      body: { parts: [] },
    });
    assert.equal(done.status, 403);
  });

  test('a part URL signed and left for 301 s is refused, so parts sign just before they go', async () => {
    const upload = await create();
    const signed = await ctx.call('GET', `/api/upload/parts/${upload.uploadId}/3`, { token: ctx.lmy });
    await advanceClock(ctx.url, 301 * 1000);
    const put = await putSigned(signed.body.signedUrl, file.subarray(16 * MiB));
    assert.equal(put.status, 403);
  });
});
