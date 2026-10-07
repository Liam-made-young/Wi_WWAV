// Buying (docs/SPEC.md 6.12, 7.10-7.14, 7.20, 10.4; PLAN S5.4, server side).
// S5.4 fails if buying needs walking, a price adds a fee at checkout, or a
// count of sales appears on the floor. The server's half: one call buys one
// thing from anywhere, the charge is the price exactly with the fee inside
// it, and no store payload carries a count.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { useServer } from './setup.js';
import { WORKS, addAccount } from '../fixtures.js';
import { advanceClock, call, completeCheckout, putSigned } from '../helpers.js';
import { writeWwav } from '../wwav.js';

const MINUTE = 60 * 1000;

function trackOf(ctx, work) {
  return ctx.state.tracks.find((t) => t.songId === work.songId);
}

async function publishedTrack(ctx, token) {
  const bytes = writeWwav({ songId: createHash('md5').update(token).digest('hex'), title: 'First' });
  const signed = await ctx.call('GET', `/api/upload/sign?fileType=audio/wav&size=${bytes.length}`, { token });
  await putSigned(signed.body.signedUrl, bytes, 'audio/wav');
  await ctx.call('POST', '/api/publish', { token, body: { trackId: signed.body.trackId } });
  return signed.body.trackId;
}

function checkout(ctx, token, body, headers) {
  return ctx.call('POST', '/api/purchase/create-checkout', { token, body, headers });
}

describe('one item, one checkout, the price exactly', () => {
  const ctx = useServer();

  test('LMY buys glass hours: the charge is $4.00 with the 10% fee inside it', async () => {
    const glass = trackOf(ctx, WORKS.glassHours);
    const res = await checkout(ctx, ctx.lmy, { type: 'track', id: glass.id });
    assert.equal(res.status, 200);
    assert.match(res.body.url, /\/__stripe\/checkout\/cs_test_/);
    const session = ctx.state.sessions.get(res.body.sessionId);
    assert.equal(session.amountTotal, 400);
    assert.deepEqual(
      session.lineItems.map((l) => l.amountCents),
      [400],
    );
    assert.match(session.lineItems[0].description, /\(10% platform fee included\)$/);
    assert.equal(session.feeCents, 40);
    assert.equal(session.expiresAt - Date.parse(session.createdAt), 30 * MINUTE);
    const page = await ctx.call('GET', new URL(res.body.url).pathname);
    assert.match(page.body, /\$4\.00/);
    assert.match(page.body, /10% platform fee included/);
  });

  test('a pending purchase is not owned; only a completed one is', async () => {
    const low = trackOf(ctx, WORKS.lowTide);
    const res = await checkout(ctx, ctx.ana, { type: 'track', id: low.id });
    const check = () => ctx.call('GET', `/api/purchase/check?type=track&id=${low.id}`, { token: ctx.ana });
    assert.deepEqual((await check()).body, { purchased: false });
    const file = await ctx.call('GET', `/api/entitlements/track/${low.id}/file`, { token: ctx.ana });
    assert.equal(file.status, 403);
    const pay = await ctx.call('POST', new URL(res.body.url).pathname);
    assert.equal(pay.status, 302);
    assert.deepEqual((await check()).body, { purchased: true });
  });

  test('your own items and digital items you already own are refused', async () => {
    const world = trackOf(ctx, WORKS.worldEnding);
    const own = await checkout(ctx, ctx.lmy, { type: 'track', id: world.id });
    assert.deepEqual([own.status, own.body], [400, { error: 'Cannot purchase your own content' }]);
    const owned = await checkout(ctx, ctx.ana, { type: 'track', id: world.id });
    assert.deepEqual([owned.status, owned.body], [400, { error: 'Already purchased' }]);
    const jacket = ctx.state.listings[0];
    const hers = await checkout(ctx, ctx.ana, { type: 'fashion', id: jacket.id });
    assert.deepEqual([hers.status, hers.body], [400, { error: 'Cannot purchase your own listing' }]);
  });

  test('entitlements list completed purchases; the file URL lasts 60 s and gives the exact bytes', async () => {
    const world = trackOf(ctx, WORKS.worldEnding);
    const list = await ctx.call('GET', '/api/entitlements', { token: ctx.ana });
    const item = list.body.items.find((i) => i.id === world.id);
    assert.equal(item.title, 'World Ending');
    const file = await ctx.call('GET', `/api/entitlements/track/${world.id}/file`, { token: ctx.ana });
    assert.equal(file.status, 200);
    assert.equal(file.body.expiresIn, 60);
    assert.equal(file.body.fileName, 'World Ending.wwav');
    const bytes = Buffer.from(await (await fetch(file.body.url)).arrayBuffer());
    assert.equal(bytes.length, file.body.bytes);
    assert.equal(createHash('sha256').update(bytes).digest('hex'), file.body.sha256);
    assert.equal(file.body.sha256, item.versions.at(-1).sha256);
    await advanceClock(ctx.url, 61 * 1000);
    assert.equal((await fetch(file.body.url)).status, 403);
    const nobody = await ctx.call('GET', `/api/entitlements/track/${world.id}/file`, { token: ctx.lmy });
    assert.deepEqual(nobody.body, { error: "You haven't bought this yet.", code: 'not_entitled' });
  });

  test('a purchase covers every version: the new version arrives', async () => {
    const world = trackOf(ctx, WORKS.worldEnding);
    world.versions.push({ ...world.versions[0], version: 2, sha256: world.versions[0].sha256 });
    const list = await ctx.call('GET', '/api/entitlements', { token: ctx.ana });
    assert.deepEqual(
      list.body.items.find((i) => i.id === world.id).versions.map((v) => v.version),
      [1, 2],
    );
    const v1 = await ctx.call('GET', `/api/entitlements/track/${world.id}/file?version=1`, { token: ctx.ana });
    assert.equal(v1.body.version, 1);
    world.versions.pop();
  });
});

describe('checkouts: 20 an hour', () => {
  const ctx = useServer();
  test('the 21st checkout in an hour is 429 with Retry-After', async () => {
    const glass = trackOf(ctx, WORKS.glassHours);
    for (let i = 0; i < 20; i++)
      assert.equal((await checkout(ctx, ctx.lmy, { type: 'track', id: glass.id })).status, 200);
    const res = await checkout(ctx, ctx.lmy, { type: 'track', id: glass.id });
    assert.equal(res.status, 429);
    assert.equal(res.body.code, 'rate_limited');
    assert.equal(res.body.scope, 'checkout');
    assert.ok(Number(res.headers.get('retry-after')) > 0);
  });
});

describe('the one-of-one hold', () => {
  const ctx = useServer();
  let carol;

  test('two buyers at once: one holds it, the other gets 409', async () => {
    carol = await addAccount(ctx.state, { username: 'Carol' });
    const jacket = ctx.state.listings[0];
    const [a, b] = await Promise.all([
      checkout(ctx, ctx.lmy, { type: 'fashion', id: jacket.id }),
      checkout(ctx, carol.token, { type: 'fashion', id: jacket.id }),
    ]);
    const statuses = [a.status, b.status].sort();
    assert.deepEqual(statuses, [200, 409]);
    const refused = a.status === 409 ? a : b;
    assert.deepEqual(refused.body, { error: 'Just sold or being purchased' });
    const halls = await ctx.call('GET', '/api/store/halls', { token: ctx.lmy });
    const item = halls.body.halls.find((h) => h.medium === 'fashion').shops[0].crates[0].items[0];
    assert.equal(item.status, 'reserved');
    assert.equal(item.hold.yours, a.status === 200);
  });

  test("hold and checkout end together at 30 minutes; the stale session can't be paid", async () => {
    const jacket = ctx.state.listings[0];
    const holder = [...ctx.state.sessions.values()].find((s) => s.status === 'open');
    await advanceClock(ctx.url, 30 * MINUTE);
    const late = await completeCheckout(ctx.url, holder.id);
    assert.deepEqual([late.status, late.body], [409, { error: 'This checkout has expired.' }]);
    assert.equal(jacket.status, 'active');
    const next = await checkout(ctx, carol.token, { type: 'fashion', id: jacket.id });
    assert.equal(next.status, 200);
    assert.equal((await completeCheckout(ctx.url, next.body.sessionId)).status, 200);
    assert.equal(jacket.status, 'sold');
    const halls = await ctx.call('GET', '/api/store/halls', { token: ctx.lmy });
    assert.ok(!halls.body.halls.find((h) => h.medium === 'fashion').shops.length);
  });

  test("a physical sale is a destination charge on the seller's behalf, shipped in the US", async () => {
    const session = [...ctx.state.sessions.values()].find((s) => s.status === 'complete');
    assert.equal(session.onBehalfOf, 'acct_ana');
    assert.equal(session.applicationFeeCents, 1200);
    assert.deepEqual(session.shippingCountries, ['US']);
  });
});

describe('the bag: one charge for the files, one per seller for goods', () => {
  const ctx = useServer();

  test("glass hours and Ana's jacket in LMY's bag are two payments", async () => {
    const glass = trackOf(ctx, WORKS.glassHours);
    const jacket = ctx.state.listings[0];
    await ctx.call('PUT', '/api/store/bag', { token: ctx.lmy, body: { type: 'track', id: glass.id } });
    const bag = await ctx.call('PUT', '/api/store/bag', { token: ctx.lmy, body: { type: 'fashion', id: jacket.id } });
    assert.equal(bag.status, 200);
    assert.equal(bag.body.totalCents, 12400);
    assert.deepEqual(
      bag.body.payments.map((p) => [p.kind, p.amountCents, p.seller?.username ?? null]),
      [
        ['digital', 400, null],
        ['physical', 12000, 'Ana'],
      ],
    );
    const own = await ctx.call('PUT', '/api/store/bag', {
      token: ctx.lmy,
      body: { type: 'track', id: trackOf(ctx, WORKS.worldEnding).id },
    });
    assert.deepEqual([own.status, own.body], [400, { error: 'Cannot purchase your own content' }]);

    const paid = await ctx.call('POST', '/api/store/bag/checkout', { token: ctx.lmy });
    assert.equal(paid.status, 200);
    assert.equal(paid.body.payments.length, 2);
    const [files, goods] = paid.body.payments.map((p) => ctx.state.sessions.get(p.sessionId));
    assert.ok(files.transferGroup);
    assert.equal(files.onBehalfOf, undefined);
    assert.equal(goods.onBehalfOf, 'acct_ana');
    for (const p of paid.body.payments) await completeCheckout(ctx.url, p.sessionId);
    const transfer = ctx.state.transfers.find((t) => t.transferGroup === files.transferGroup);
    assert.deepEqual([transfer.destination, transfer.amountCents], ['acct_ana', 360]);
    assert.deepEqual((await ctx.call('GET', '/api/store/bag', { token: ctx.lmy })).body.items, []);
  });

  test('a bag of files from two sellers is one charge, with a transfer to each', async () => {
    const dana = await addAccount(ctx.state, { username: 'Dana' });
    const low = trackOf(ctx, WORKS.lowTide);
    const glass = trackOf(ctx, WORKS.glassHours);
    for (const t of [low, glass]) {
      await ctx.call('PUT', '/api/store/bag', { token: dana.token, body: { type: 'track', id: t.id } });
    }
    const paid = await ctx.call('POST', '/api/store/bag/checkout', { token: dana.token });
    assert.equal(paid.body.payments.length, 1);
    const session = ctx.state.sessions.get(paid.body.payments[0].sessionId);
    assert.equal(session.amountTotal, 800);
    await completeCheckout(ctx.url, session.id);
    const transfers = ctx.state.transfers.filter((t) => t.transferGroup === session.transferGroup);
    assert.deepEqual(transfers.map((t) => [t.destination, t.amountCents]).sort(), [
      ['acct_ana', 360],
      ['acct_lmy', 360],
    ]);
  });
});

describe('payouts, prices and the floor', () => {
  // Noon UTC, so an hour later is still the same day.
  const ctx = useServer({ startAt: Date.parse('2026-10-07T12:00:00Z') });

  test('a shelf opens only once payouts are ready', async () => {
    const erin = await addAccount(ctx.state, { username: 'Erin', onboarded: false });
    const none = await ctx.call('GET', '/api/connect/status', { token: erin.token });
    assert.deepEqual(none.body, { connected: false, onboarded: false, message: 'No Stripe account linked' });
    const trackId = await publishedTrack(ctx, erin.token);
    const early = await ctx.call('PUT', `/api/tracks/${trackId}/set-price`, { token: erin.token, body: { price: 4 } });
    assert.deepEqual([early.status, early.body], [400, { error: 'Selling needs a payout account. Set it up once.' }]);
    const onboard = await ctx.call('POST', '/api/connect/onboard', { token: erin.token });
    const page = new URL(onboard.body.url).pathname;
    assert.equal((await ctx.call('POST', page)).status, 200);
    const ready = await ctx.call('GET', '/api/connect/status', { token: erin.token });
    assert.equal(ready.body.onboarded, true);
    const priced = await ctx.call('PUT', `/api/tracks/${trackId}/set-price`, { token: erin.token, body: { price: 4 } });
    assert.deepEqual(priced.body, { success: true, isForSale: true, price: 4 });
    const high = await ctx.call('PUT', `/api/tracks/${trackId}/set-price`, {
      token: erin.token,
      body: { price: 2001 },
    });
    assert.deepEqual([high.status, high.body], [400, { error: 'Price cannot exceed $2000' }]);
    const off = await ctx.call('PUT', `/api/tracks/${trackId}/set-price`, { token: erin.token, body: { price: 0 } });
    assert.deepEqual(off.body, { success: true, isForSale: false });
  });

  test('the floor lists every item with its price, family and nothing counted', async () => {
    const res = await ctx.call('GET', '/api/store/halls?order=lineage');
    assert.equal(res.status, 200);
    const music = res.body.halls.find((h) => h.medium === 'music');
    const titles = music.shops.flatMap((s) => s.crates.flatMap((c) => c.items.map((i) => i.title)));
    assert.deepEqual(titles.sort(), ['Low Tide', 'World Ending', 'glass hours']);
    const glass = music.shops.flatMap((s) => s.crates.flatMap((c) => c.items)).find((i) => i.title === 'glass hours');
    assert.deepEqual(glass.provenance, { generation: 2, parent: { title: 'Low Tide', artist: 'LMY' } });
    assert.equal(glass.priceCents, 400);
    const again = await ctx.call('GET', '/api/store/halls?order=lineage');
    assert.deepEqual(again.body, res.body);
    const newest = await ctx.call('GET', '/api/store/new');
    assert.deepEqual(
      newest.body.items.map((i) => i.title),
      ['Waxed chore jacket', 'glass hours', 'Low Tide', 'World Ending'],
    );
  });

  test('a shop keeps its seat within a day and moves on the next', async () => {
    const seat = async () => (await ctx.call('GET', '/api/store/halls')).body.halls[0].shops[0].seat;
    const first = await seat();
    await advanceClock(ctx.url, 60 * MINUTE);
    assert.deepEqual(await seat(), first);
    await advanceClock(ctx.url, 24 * 60 * MINUTE);
    const next = await seat();
    assert.equal(next.track, first.track);
    assert.notEqual(next.phase, first.phase);
    const res = await call(ctx.url, 'GET', '/api/store/halls');
    assert.equal(res.body.halls.length, 4);
  });
});

describe('the bag after things change', () => {
  const ctx = useServer();

  test('a withdrawn work stays in the bag, marked, and checkout charges only what can still be bought', async () => {
    const buyer = await addAccount(ctx.state, { username: 'Fern' });
    const low = trackOf(ctx, WORKS.lowTide);
    const glass = trackOf(ctx, WORKS.glassHours);
    for (const t of [low, glass]) {
      await ctx.call('PUT', '/api/store/bag', { token: buyer.token, body: { type: 'track', id: t.id } });
    }
    await ctx.call('POST', '/api/unpublish', { token: ctx.ana, body: { trackId: glass.trackId } });
    const bag = await ctx.call('GET', '/api/store/bag', { token: buyer.token });
    assert.equal(bag.status, 200);
    assert.deepEqual(
      bag.body.items.map((i) => [i.title, i.available]),
      [
        ['Low Tide', true],
        ['glass hours', false],
      ],
    );
    assert.equal(bag.body.totalCents, 400);
    assert.deepEqual(bag.body.payments[0].items, [{ type: 'track', id: low.id }]);
    const paid = await ctx.call('POST', '/api/store/bag/checkout', { token: buyer.token });
    assert.equal(paid.status, 200);
    assert.equal(ctx.state.sessions.get(paid.body.payments[0].sessionId).amountTotal, 400);
    const left = await ctx.call('GET', '/api/store/bag', { token: buyer.token });
    assert.deepEqual(
      left.body.items.map((i) => [i.id, i.available]),
      [[glass.id, false]],
    );
    const again = await ctx.call('POST', '/api/store/bag/checkout', { token: buyer.token });
    assert.deepEqual([again.status, again.body.code], [409, 'unavailable']);
    const gone = await ctx.call('DELETE', `/api/store/bag/track/${glass.id}`, { token: buyer.token });
    assert.deepEqual(gone.body.items, []);
  });

  test('the holder asking again for their held garment gets back the checkout that holds it', async () => {
    const buyer = await addAccount(ctx.state, { username: 'Gus' });
    const jacket = ctx.state.listings[0];
    const first = await checkout(ctx, buyer.token, { type: 'fashion', id: jacket.id });
    const again = await checkout(ctx, buyer.token, { type: 'fashion', id: jacket.id });
    assert.equal(again.status, 200);
    assert.equal(again.body.sessionId, first.body.sessionId);
    const other = await checkout(ctx, ctx.lmy, { type: 'fashion', id: jacket.id });
    assert.deepEqual([other.status, other.body], [409, { error: 'Just sold or being purchased' }]);
    await advanceClock(ctx.url, 30 * MINUTE);
  });

  test('a bag checkout retried with its Idempotency-Key after a lost answer gets the same sessions', async () => {
    const buyer = await addAccount(ctx.state, { username: 'Hal' });
    const jacket = ctx.state.listings[0];
    await ctx.call('PUT', '/api/store/bag', { token: buyer.token, body: { type: 'fashion', id: jacket.id } });
    await call(ctx.url, 'POST', '/__mock/fail', {
      body: { method: 'POST', path: '/api/store/bag/checkout', drop: 'after' },
    });
    const headers = { 'idempotency-key': 'bag-checkout-0001' };
    await assert.rejects(ctx.call('POST', '/api/store/bag/checkout', { token: buyer.token, headers }));
    const retry = await ctx.call('POST', '/api/store/bag/checkout', { token: buyer.token, headers });
    assert.equal(retry.status, 200);
    assert.equal(retry.headers.get('idempotent-replay'), 'true');
    assert.equal(ctx.state.sessions.get(retry.body.payments[0].sessionId).status, 'open');
    assert.equal([...ctx.state.sessions.values()].filter((s) => s.buyerId === buyer.user.id).length, 1);
  });

  test('a file paid for twice, once alone and once in a bag, is refunded the second time', async () => {
    const buyer = await addAccount(ctx.state, { username: 'Ivy' });
    const low = trackOf(ctx, WORKS.lowTide);
    const world = trackOf(ctx, WORKS.worldEnding);
    for (const t of [low, world]) {
      await ctx.call('PUT', '/api/store/bag', { token: buyer.token, body: { type: 'track', id: t.id } });
    }
    const bag = await ctx.call('POST', '/api/store/bag/checkout', { token: buyer.token });
    const alone = await checkout(ctx, buyer.token, { type: 'track', id: low.id });
    assert.equal(alone.status, 200);
    assert.notEqual(alone.body.sessionId, bag.body.payments[0].sessionId);
    await completeCheckout(ctx.url, alone.body.sessionId);
    assert.equal((await completeCheckout(ctx.url, bag.body.payments[0].sessionId)).status, 200);
    const mine = ctx.state.purchases.filter((p) => p.userId === buyer.user.id);
    assert.deepEqual(
      mine.map((p) => [p.itemId, p.status]).sort(),
      [
        [world.id, 'completed'],
        [low.id, 'completed'],
        [low.id, 'refunded'],
      ].sort(),
    );
    const session = ctx.state.sessions.get(bag.body.payments[0].sessionId);
    assert.equal(session.refundedCents, 400);
    const transfers = ctx.state.transfers.filter((t) => t.transferGroup === session.transferGroup);
    assert.deepEqual(
      transfers.map((t) => t.amountCents),
      [world.priceCents - Math.round(world.priceCents * 0.1)],
    );
  });

  test('asking twice for one file gives the same open checkout', async () => {
    const buyer = await addAccount(ctx.state, { username: 'Jo' });
    const low = trackOf(ctx, WORKS.lowTide);
    const a = await checkout(ctx, buyer.token, { type: 'track', id: low.id });
    const b = await checkout(ctx, buyer.token, { type: 'track', id: low.id });
    assert.equal(b.body.sessionId, a.body.sessionId);
  });
});

describe('prices and checks', () => {
  const ctx = useServer();

  test('a paid file costs at least $2 (Open #44)', async () => {
    const trackId = await publishedTrack(ctx, ctx.lmy);
    const low = await ctx.call('PUT', `/api/tracks/${trackId}/set-price`, { token: ctx.lmy, body: { price: 1.99 } });
    assert.deepEqual([low.status, low.body], [400, { error: 'A paid file costs at least $2.', code: 'price_too_low' }]);
    const two = await ctx.call('PUT', `/api/tracks/${trackId}/set-price`, { token: ctx.lmy, body: { price: 2 } });
    assert.deepEqual(two.body, { success: true, isForSale: true, price: 2 });
  });

  test('purchase/check answers about the type asked: a film or an album is never a track', async () => {
    const world = trackOf(ctx, WORKS.worldEnding);
    const ask = (type) => ctx.call('GET', `/api/purchase/check?type=${type}&id=${world.id}`, { token: ctx.ana });
    assert.deepEqual((await ask('track')).body, { purchased: true });
    for (const type of ['film', 'album', 'event', 'sticker'])
      assert.deepEqual((await ask(type)).body, { purchased: false });
  });
});
