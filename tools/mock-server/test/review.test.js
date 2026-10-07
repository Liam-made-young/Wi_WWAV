// An adversarial review of the mock (PLAN S1.9, S1.11, S2.7, S4.4, S5.4 and
// gate 1.3). Each test states what the spec asks for. A test marked `todo`
// fails today because of the defect its todo names (a FINDING with its
// severity); it runs and reports without failing the suite, and should pass
// once the mock is fixed, when the todo comes off. The tests without a todo
// pass and are kept as evidence for the criteria they press on.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash, randomBytes } from 'node:crypto';
import { useServer } from './setup.js';
import { WORKS, addAccount } from '../fixtures.js';
import { call, completeCheckout, pkcePair, putSigned, signInWithoutBrowser, DESKTOP_CLIENT } from '../helpers.js';
import { platformSongId } from '../hash.js';
import { writeWwav } from '../wwav.js';
import { attentionKeys } from './gate.test.js';

const songId = () => randomBytes(16).toString('hex');
const sha = (b) => createHash('sha256').update(b).digest('hex');

async function upload(ctx, token, bytes) {
  const signed = await ctx.call('GET', `/api/upload/sign?fileType=audio/wav&size=${bytes.length}`, { token });
  assert.equal(signed.status, 200);
  assert.equal((await putSigned(signed.body.signedUrl, bytes, 'audio/wav')).status, 200);
  return signed.body;
}

const trackOf = (ctx, work) => ctx.state.tracks.find((t) => t.songId === work.songId);

describe('review: publish', () => {
  const ctx = useServer();

  test(
    'a work unpublished and then published again is back in the sky',
    { todo: 'FINDING (high): publish after unpublish answers success but the work stays withdrawn' },
    async () => {
      const id = songId();
      const { trackId } = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Comeback' }));
      const body = { trackId, settings: { origin: 'wi_wwav', clipId: '01JREVIEW1' } };
      assert.equal((await ctx.call('POST', '/api/publish', { token: ctx.lmy, body })).status, 200);
      assert.equal((await ctx.call('POST', '/api/unpublish', { token: ctx.lmy, body: { trackId } })).status, 200);
      const again = await ctx.call('POST', '/api/publish', { token: ctx.lmy, body });
      assert.equal(again.status, 200);
      // The answer says success; the work must then be placeable and listed.
      const work = ctx.state.tracks.find((t) => t.songId === id);
      assert.equal(work.withdrawn, false, 'the re-published work is still withdrawn');
      const placed = await ctx.call('POST', '/api/v2/systems/2/planets', {
        token: ctx.lmy,
        body: { kind: 'song', trackId },
      });
      assert.equal(placed.status, 200, JSON.stringify(placed.body));
    },
  );

  test(
    "a buyer's download is the exact bytes on the receipt, even if the maker PUTs again",
    { todo: "FINDING (medium): a re-PUT to a still-valid signed URL replaces a published version's bytes" },
    async () => {
      const id = songId();
      const first = writeWwav({ songId: id, title: 'Receipt' });
      const signed = await upload(ctx, ctx.lmy, first);
      await ctx.call('POST', '/api/publish', { token: ctx.lmy, body: { trackId: signed.trackId } });
      await ctx.call('PUT', `/api/tracks/${signed.trackId}/set-price`, { token: ctx.lmy, body: { price: 3 } });
      const work = ctx.state.tracks.find((t) => t.songId === id);
      const bought = await ctx.call('POST', '/api/purchase/create-checkout', {
        token: ctx.ana,
        body: { type: 'track', id: work.id },
      });
      await completeCheckout(ctx.url, bought.body.sessionId);
      // The signed URL from /sign is still good for the rest of its 300 s.
      const other = writeWwav({ songId: id, title: 'Receipt, swapped', bpm: 99 });
      const reput = await putSigned(signed.signedUrl, other, 'audio/wav');
      assert.equal(reput.status, 200);
      const file = (await ctx.call('GET', `/api/entitlements/track/${work.id}/file`, { token: ctx.ana })).body;
      const got = Buffer.from(await (await fetch(file.url)).arrayBuffer());
      assert.equal(sha(got), file.sha256, 'the download does not match the sha256 on the receipt');
    },
  );

  test(
    "a trackId's platform id can't be claimed by another account before its owner publishes",
    { todo: "FINDING (low): counted trackIds let another account claim a trackId's FNV-1a platform id first" },
    async () => {
      // Ana signs an upload of a plain WAV; its song id will be FNV-1a of the trackId (6.1).
      const signed = await upload(ctx, ctx.ana, writeWwav({ plain: true }));
      // LMY knows (or predicts: trackIds are counted) the trackId and claims its platform id first.
      const squat = await upload(ctx, ctx.lmy, writeWwav({ songId: platformSongId(signed.trackId), title: 'Squat' }));
      assert.equal(
        (await ctx.call('POST', '/api/publish', { token: ctx.lmy, body: { trackId: squat.trackId } })).status,
        200,
      );
      const mine = await ctx.call('POST', '/api/publish', {
        token: ctx.ana,
        body: { trackId: signed.trackId, title: 'Hers' },
      });
      assert.equal(mine.status, 200, JSON.stringify(mine.body));
    },
  );

  test(
    "a remix's generation is its parent's + 1, not whatever wlin claims",
    { todo: 'FINDING (low): remixDepth is taken from wlin.generation unchecked against the parent' },
    async () => {
      const id = songId();
      const bytes = writeWwav({
        songId: id,
        title: 'Deep claim',
        type: 'remix',
        parentId: WORKS.worldEnding.songId,
        rootId: WORKS.worldEnding.songId,
        generation: 7,
      });
      const { trackId } = await upload(ctx, ctx.ana, bytes);
      await ctx.call('POST', '/api/publish', { token: ctx.ana, body: { trackId } });
      const work = ctx.state.tracks.find((t) => t.songId === id);
      assert.equal(work.parentTrackId, WORKS.worldEnding.trackId);
      assert.equal(work.remixDepth, 1);
    },
  );
});

describe('review: a refused publish changes nothing', () => {
  const ctx = useServer();

  test(
    'a 409 version_taken leaves the work as it was',
    {
      todo: 'FINDING (medium): applyBody runs before the version check, so a refused publish renames the work and can clear isMaster',
    },
    async () => {
      const id = songId();
      const first = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Kept', version: 2 }));
      await ctx.call('POST', '/api/publish', { token: ctx.lmy, body: { trackId: first.trackId, title: 'Kept' } });
      const other = await upload(ctx, ctx.lmy, writeWwav({ songId: id, title: 'Kept', version: 2, bpm: 101 }));
      const refused = await ctx.call('POST', '/api/publish', {
        token: ctx.lmy,
        body: { trackId: other.trackId, title: 'Renamed by a refused request', isMaster: false },
      });
      assert.equal(refused.status, 409);
      const work = ctx.state.tracks.find((t) => t.songId === id);
      assert.deepEqual([work.title, work.isMaster], ['Kept', true]);
    },
  );
});

describe('review: Newest filters by medium', () => {
  const ctx = useServer();

  test(
    "the medium filter takes the words the rest of the API uses ('music', or the family 'Mi')",
    { todo: "FINDING (low): Newest's medium filter knows only 'song'" },
    async () => {
      const all = await ctx.call('GET', '/api/v2/feed?limit=40');
      assert.ok(all.body.data.items.length > 0);
      for (const medium of ['music', 'Mi']) {
        const res = await ctx.call('GET', `/api/v2/feed?limit=40&medium=${medium}`);
        assert.equal(res.body.data.items.length, all.body.data.items.length, `medium=${medium}`);
      }
    },
  );
});

describe('review: a large file in 8 MiB parts (S1.9)', () => {
  const ctx = useServer();

  test(
    'a 2.5 GB .wwav can sign every part as it goes, with no 429 halfway',
    { todo: "FINDING (medium): signing each part counts against upload_sign's 240 an hour" },
    async () => {
      // A 47-minute .wwav is about 2.5 GB (6.1): 300 parts of 8 MiB, each signed just before it goes up (9.7).
      const size = 300 * 8 * 1024 * 1024;
      const created = await ctx.call('POST', '/api/upload/parts', {
        token: ctx.lmy,
        body: { size, fileType: 'audio/wav' },
      });
      assert.equal(created.status, 200);
      for (let n = 1; n <= created.body.parts; n++) {
        const res = await ctx.call('GET', `/api/upload/parts/${created.body.uploadId}/${n}`, { token: ctx.lmy });
        assert.equal(res.status, 200, `part ${n} of ${created.body.parts}: ${JSON.stringify(res.body)}`);
      }
    },
  );
});

describe('review: the 21-world cap under concurrency (S4.4)', () => {
  const ctx = useServer();

  test('30 adds at once into an empty system: exactly 21 succeed, the rest get the sentence', async () => {
    const made = await ctx.call('POST', '/api/v2/galaxies/1/systems', { token: ctx.lmy, body: { title: 'Race' } });
    const systemId = made.body.data.system.id;
    const trackIds = [];
    for (let i = 0; i < 30; i++) {
      const { trackId } = await upload(ctx, ctx.lmy, writeWwav({ songId: songId(), title: `R${i}` }));
      await ctx.call('POST', '/api/publish', { token: ctx.lmy, body: { trackId } });
      trackIds.push(trackId);
    }
    const answers = await Promise.all(
      trackIds.map((trackId) =>
        ctx.call('POST', `/api/v2/systems/${systemId}/planets`, { token: ctx.lmy, body: { kind: 'song', trackId } }),
      ),
    );
    assert.equal(answers.filter((a) => a.status === 200).length, 21);
    for (const a of answers.filter((x) => x.status !== 200)) {
      assert.deepEqual(a.body, {
        ok: false,
        error: { code: 'system_full', message: 'A solar system holds 21 worlds. Start another one.' },
      });
    }
    assert.equal(ctx.state.planets.filter((p) => p.systemId === systemId).length, 21);
    const seats = ctx.state.planets.filter((p) => p.systemId === systemId).map((p) => p.orbitIndex);
    assert.equal(new Set(seats).size, 21);
  });
});

describe('review: desktop sign-in (S1.11)', () => {
  const ctx = useServer();
  const redirectUri = 'http://127.0.0.1:53682/callback';

  test('two redemptions of one code at once: exactly one gets tokens', async () => {
    const { verifier, challenge } = pkcePair();
    const state = 'st-' + randomBytes(6).toString('hex');
    const { code } = await signInWithoutBrowser(ctx.url, {
      email: 'lmy@mi-wwav.com',
      password: 'WeWave-lmy1',
      challenge,
      state,
      redirectUri,
    });
    const form = {
      grant_type: 'authorization_code',
      code,
      code_verifier: verifier,
      redirect_uri: redirectUri,
      client_id: DESKTOP_CLIENT,
      state,
    };
    const answers = await Promise.all([1, 2, 3].map(() => ctx.call('POST', '/oauth/desktop/token', { form })));
    assert.deepEqual(answers.map((a) => a.status).sort(), [200, 400, 400]);
  });

  test('the verifier of another sign-in cannot redeem this code', async () => {
    const a = pkcePair();
    const b = pkcePair();
    const { code } = await signInWithoutBrowser(ctx.url, {
      email: 'lmy@mi-wwav.com',
      password: 'WeWave-lmy1',
      challenge: a.challenge,
      state: 's1',
      redirectUri,
    });
    const res = await ctx.call('POST', '/oauth/desktop/token', {
      form: {
        grant_type: 'authorization_code',
        code,
        code_verifier: b.verifier,
        redirect_uri: redirectUri,
        client_id: DESKTOP_CLIENT,
        state: 's1',
      },
    });
    assert.equal(res.status, 400);
    assert.equal(res.body.error, 'invalid_grant');
  });

  test(
    'the sign-in form refuses a password after repeated wrong guesses',
    { todo: 'FINDING (low): no limit on password guesses at /oauth/desktop/login (or /api/auth/login)' },
    async () => {
      const { challenge } = pkcePair();
      const q = new URLSearchParams({
        response_type: 'code',
        client_id: DESKTOP_CLIENT,
        redirect_uri: redirectUri,
        code_challenge: challenge,
        code_challenge_method: 'S256',
        state: 'guess',
      });
      const page = await ctx.call('GET', `/oauth/desktop/authorize?${q}`);
      const request = page.body.match(/name="request" value="([^"]*)"/)[1];
      let last;
      for (let i = 0; i < 50; i++) {
        last = await ctx.call('POST', '/oauth/desktop/login', {
          form: { request, email: 'lmy@mi-wwav.com', password: `guess-${i}` },
        });
      }
      // The server limits /mcp/login to 10 per 15 min and login to 20 (server.md §0, §9).
      assert.equal(last.status, 429, 'fifty guesses on one sign-in request were all answered');
    },
  );
});

describe('review: Heat (S2.7)', () => {
  const ctx = useServer();
  const push = (token, device, changes) => ctx.call('POST', '/api/heat/changes', { token, body: { device, changes } });

  test('interleaved pushes from two devices, every order: the highest seq wins each field', async () => {
    const writes = [];
    for (let seq = 1; seq <= 12; seq++) writes.push({ device: seq % 2 ? 'mac-a' : 'mac-b', seq });
    // A deterministic shuffle.
    const order = writes.map((w, i) => ({ w, k: sha(String(i)) })).sort((x, y) => (x.k < y.k ? -1 : 1));
    for (const { w } of order) {
      await push(ctx.lmy, w.device, [{ kind: 'task', id: 'shuffle', field: 'title', value: `v${w.seq}`, seq: w.seq }]);
    }
    const pulled = await ctx.call('GET', '/api/heat/changes?cursor=0', { token: ctx.lmy });
    const title = pulled.body.changes.filter((c) => c.id === 'shuffle');
    assert.deepEqual(
      title.map((c) => [c.value, c.seq]),
      [['v12', 12]],
    );
  });

  test(
    'a cursor past the end of the log is refused, not kept, so no later change is skipped',
    { todo: "FINDING (low): a Heat cursor past the log's end is echoed back and hides the next changes" },
    async () => {
      const heat = await addAccount(ctx.state, { username: 'Cursor' });
      await push(heat.token, 'mac-a', [{ kind: 'task', id: 'a', field: 'title', value: 'one', seq: 1 }]);
      // A cursor from before a restore (or a reset) is larger than the log.
      const ahead = await ctx.call('GET', '/api/heat/changes?cursor=5', { token: heat.token });
      await push(heat.token, 'mac-b', [
        { kind: 'task', id: 'b', field: 'title', value: 'two', seq: 2 },
        { kind: 'task', id: 'c', field: 'title', value: 'three', seq: 3 },
        { kind: 'task', id: 'd', field: 'title', value: 'four', seq: 4 },
        { kind: 'task', id: 'e', field: 'title', value: 'five', seq: 5 },
      ]);
      const next = await ctx.call('GET', `/api/heat/changes?cursor=${ahead.body.cursor}`, { token: heat.token });
      // Either the server refuses the cursor, or the device still gets every change.
      if (ahead.status === 200) {
        assert.deepEqual(
          next.body.changes.map((c) => c.value),
          ['two', 'three', 'four', 'five'],
          `a cursor of ${ahead.body.cursor} swallowed the next changes`,
        );
      }
    },
  );
});

describe('review: a withdrawn work in a bag', () => {
  const ctx = useServer();

  test(
    'a bag that holds a work since withdrawn can still be read',
    { todo: 'FINDING (medium): a withdrawn work in a bag makes GET /api/store/bag answer 404' },
    async () => {
      const buyer = await addAccount(ctx.state, { username: 'Bagger' });
      const glass = trackOf(ctx, WORKS.glassHours);
      const add = await ctx.call('PUT', '/api/store/bag', {
        token: buyer.token,
        body: { type: 'track', id: glass.id },
      });
      assert.equal(add.status, 200);
      await ctx.call('POST', '/api/unpublish', { token: ctx.ana, body: { trackId: glass.trackId } });
      const bag = await ctx.call('GET', '/api/store/bag', { token: buyer.token });
      assert.equal(bag.status, 200, JSON.stringify(bag.body));
    },
  );
});

describe("review: Heat push's cursor", () => {
  const ctx = useServer();
  const push = (token, device, changes) => ctx.call('POST', '/api/heat/changes', { token, body: { device, changes } });

  test(
    "pulling from the cursor a push answered doesn't skip another device's changes",
    {
      todo: "FINDING (low): the push answer's cursor is the log's end, which skips other devices' changes if pulled from",
    },
    async () => {
      const { token } = await addAccount(ctx.state, { username: 'TwoMacs' });
      const a0 = await ctx.call('GET', '/api/heat/changes?cursor=0', { token });
      await push(token, 'mac-b', [{ kind: 'task', id: 'b', field: 'title', value: 'from B', seq: 1 }]);
      const pushed = await push(token, 'mac-a', [{ kind: 'task', id: 'a', field: 'title', value: 'from A', seq: 2 }]);
      // Mac A keeps the cursor it was answered and pulls from there next time.
      const next = await ctx.call('GET', `/api/heat/changes?cursor=${pushed.body.cursor}`, { token });
      const seen = [...a0.body.changes, ...next.body.changes].map((c) => c.value);
      assert.ok(seen.includes('from B'), `Mac A never sees B's change: pulled ${JSON.stringify(seen)}`);
    },
  );
});

describe('review: buying (S5.4)', () => {
  const ctx = useServer();

  test(
    'a bag checkout whose answer was lost can be recovered by the buyer',
    { todo: 'FINDING (medium): a lost bag-checkout answer locks the holder out of their own hold for 30 minutes' },
    async () => {
      const buyer = await addAccount(ctx.state, { username: 'Lost' });
      const jacket = ctx.state.listings[0];
      await ctx.call('PUT', '/api/store/bag', { token: buyer.token, body: { type: 'fashion', id: jacket.id } });
      await call(ctx.url, 'POST', '/__mock/fail', {
        body: { method: 'POST', path: '/api/store/bag/checkout', drop: 'after' },
      });
      await assert.rejects(ctx.call('POST', '/api/store/bag/checkout', { token: buyer.token }));
      // The jacket now reads "Held for you"; the buyer tries again.
      const retry = await ctx.call('POST', '/api/store/bag/checkout', { token: buyer.token });
      const direct = await ctx.call('POST', '/api/purchase/create-checkout', {
        token: buyer.token,
        body: { type: 'fashion', id: jacket.id },
      });
      assert.ok(
        retry.status === 200 || direct.status === 200,
        `the holder is locked out of their own hold: bag ${retry.status} ${JSON.stringify(retry.body)}, ` +
          `checkout ${direct.status} ${JSON.stringify(direct.body)}`,
      );
    },
  );

  test(
    'a file already bought is not charged for again when a second open session is paid',
    { todo: 'FINDING (medium): two open sessions for one file both complete, so the buyer pays twice' },
    async () => {
      const buyer = await addAccount(ctx.state, { username: 'Twice' });
      const glass = trackOf(ctx, WORKS.glassHours);
      const body = { type: 'track', id: glass.id };
      const one = await ctx.call('POST', '/api/purchase/create-checkout', { token: buyer.token, body });
      const two = await ctx.call('POST', '/api/purchase/create-checkout', { token: buyer.token, body });
      assert.equal(one.status, 200);
      assert.equal(two.status, 200);
      await completeCheckout(ctx.url, one.body.sessionId);
      const second = await completeCheckout(ctx.url, two.body.sessionId);
      const completed = ctx.state.purchases.filter(
        (p) => p.userId === buyer.user.id && p.itemId === glass.id && p.status === 'completed',
      );
      assert.equal(completed.length, 1, `charged twice for one file (second webhook answered ${second.status})`);
    },
  );

  test(
    "a bag's files pay each seller 90% of the total, to the cent",
    { todo: "FINDING (low): per-item fee rounding differs from the session's, losing a cent" },
    async () => {
      const seller = await addAccount(ctx.state, { username: 'Cents' });
      const ids = [];
      for (const title of ['Penny A', 'Penny B']) {
        const { trackId } = await upload(ctx, seller.token, writeWwav({ songId: songId(), title }));
        await ctx.call('POST', '/api/publish', { token: seller.token, body: { trackId } });
        await ctx.call('PUT', `/api/tracks/${trackId}/set-price`, { token: seller.token, body: { price: 1.05 } });
        ids.push(ctx.state.tracks.find((t) => t.trackId === trackId).id);
      }
      for (const id of ids) await ctx.call('PUT', '/api/store/bag', { token: ctx.lmy, body: { type: 'track', id } });
      const out = await ctx.call('POST', '/api/store/bag/checkout', { token: ctx.lmy });
      const sessionId = out.body.payments[0].sessionId;
      await completeCheckout(ctx.url, sessionId);
      const session = ctx.state.sessions.get(sessionId);
      const paid = ctx.state.transfers.filter((t) => t.transferGroup === `tg_${sessionId}`);
      assert.equal(paid[0].amountCents + session.feeCents, session.amountTotal, 'the fee and the transfer miss a cent');
    },
  );

  test(
    'a price under a cent is not a $0.00 record on the floor that checkout refuses',
    { todo: 'FINDING (low): set-price 0.004 lists a $0.00 record that checkout refuses' },
    async () => {
      const seller = await addAccount(ctx.state, { username: 'Fraction' });
      const { trackId } = await upload(ctx, seller.token, writeWwav({ songId: songId(), title: 'Fraction' }));
      await ctx.call('POST', '/api/publish', { token: seller.token, body: { trackId } });
      const set = await ctx.call('PUT', `/api/tracks/${trackId}/set-price`, {
        token: seller.token,
        body: { price: 0.004 },
      });
      const id = ctx.state.tracks.find((t) => t.trackId === trackId).id;
      const halls = await ctx.call('GET', '/api/store/halls');
      const items = halls.body.halls.flatMap((h) => h.shops.flatMap((s) => s.crates.flatMap((c) => c.items)));
      const onFloor = items.find((i) => i.type === 'track' && i.id === id);
      const buy = await ctx.call('POST', '/api/purchase/create-checkout', {
        token: ctx.lmy,
        body: { type: 'track', id },
      });
      assert.ok(
        set.body.isForSale === false || (onFloor && onFloor.priceCents > 0 && buy.status === 200),
        `set-price said ${JSON.stringify(set.body)}, the floor shows ${onFloor?.priceCents} cents, checkout ${buy.status} ${JSON.stringify(buy.body)}`,
      );
    },
  );
});

describe('review: the one-of-one under a rush', () => {
  const ctx = useServer();

  test('ten buyers at once for the one-of-one: exactly one holds it', async () => {
    const buyers = [];
    for (let i = 0; i < 10; i++) buyers.push(await addAccount(ctx.state, { username: `Rush${i}` }));
    const jacket = ctx.state.listings[0];
    assert.equal(jacket.status, 'active');
    const answers = await Promise.all(
      buyers.map((b) =>
        ctx.call('POST', '/api/purchase/create-checkout', { token: b.token, body: { type: 'fashion', id: jacket.id } }),
      ),
    );
    assert.equal(answers.filter((a) => a.status === 200).length, 1);
    for (const a of answers.filter((x) => x.status !== 200)) {
      assert.deepEqual([a.status, a.body], [409, { error: 'Just sold or being purchased' }]);
    }
  });
});

describe('review: purchase/check', () => {
  const ctx = useServer();

  test(
    'purchase/check about an album or film is not answered from a track with the same id',
    { todo: 'FINDING (low): purchase/check maps every non-fashion type to a track' },
    async () => {
      // Ana owns track 1 (World Ending).
      const res = await ctx.call('GET', '/api/purchase/check?type=film&id=1', { token: ctx.ana });
      assert.deepEqual(res.body, { purchased: false });
    },
  );
});

describe('review: tokens and malformed requests', () => {
  const ctx = useServer();

  test(
    'a bad token answers 401 on public v2 reads too, as the README promises',
    { todo: 'FINDING (low): catalog, suns and lineage ignore a bad token, against the README' },
    async () => {
      const headers = { authorization: 'Bearer not.a.token' };
      for (const path of [
        '/api/v2/catalog',
        '/api/v2/suns/1',
        '/api/v2/galaxies/lmy/sun',
        `/api/tracks/${WORKS.worldEnding.trackId}/lineage`,
      ]) {
        const res = await ctx.call('GET', path, { headers });
        assert.equal(res.status, 401, path);
      }
    },
  );

  test(
    'a JSON body of null is a 400, not a 500',
    { todo: 'FINDING (low): a JSON null body crashes handlers into a 500' },
    async () => {
      for (const [path, token] of [
        ['/api/auth/login', undefined],
        ['/api/heat/changes', ctx.lmy],
        ['/api/publish', ctx.lmy],
      ]) {
        const res = await fetch(new URL(path, ctx.url), {
          method: 'POST',
          headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
          body: 'null',
        });
        assert.equal(res.status, 400, path);
      }
    },
  );

  test(
    'a path with a broken percent-escape gets an answer, not a reset connection',
    { todo: 'FINDING (low): a malformed percent-escape throws outside the try and resets the connection' },
    async () => {
      const res = await fetch(new URL('/api/v2/galaxies/%E0%A4%A', ctx.url)).catch((e) => e);
      assert.ok(!(res instanceof Error), `the connection was dropped: ${res?.cause?.code ?? res}`);
      assert.ok(res.status >= 400 && res.status < 500);
    },
  );
});

describe('review: the gate walker (1.3)', () => {
  test(
    'it catches other names for attention',
    { todo: "FINDING (low): the gate walker's word list misses common synonyms" },
    () => {
      const planted = { listeners: 3, streams: 9, subscribers: 2, favorites: 1, hearts: 4, upvotes: 5, impressions: 6 };
      assert.deepEqual(
        attentionKeys(planted).length,
        Object.keys(planted).length,
        JSON.stringify(attentionKeys(planted)),
      );
    },
  );
});
