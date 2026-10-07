// Space over /api/v2 (docs/SPEC.md 4.2-4.13; PLAN S4.4, server side). S4.4
// fails if a 22nd world is accepted, or the refusal isn't "A solar system
// holds 21 worlds. Start another one."
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { useServer } from './setup.js';
import { WORKS } from '../fixtures.js';
import { putSigned } from '../helpers.js';
import { writeWwav } from '../wwav.js';

async function newSong(ctx, token, title) {
  const bytes = writeWwav({ songId: randomBytes(16).toString('hex'), title });
  const signed = await ctx.call('GET', `/api/upload/sign?fileType=audio/wav&size=${bytes.length}`, { token });
  await putSigned(signed.body.signedUrl, bytes, 'audio/wav');
  const res = await ctx.call('POST', '/api/publish', { token, body: { trackId: signed.body.trackId } });
  assert.equal(res.status, 200);
  return signed.body.trackId;
}

describe('galaxies, systems and the 21-world cap', () => {
  const ctx = useServer();

  test("LMY's galaxy holds World Ending and Covers", async () => {
    const res = await ctx.call('GET', '/api/v2/galaxies/lmy');
    assert.equal(res.status, 200);
    assert.equal(res.body.ok, true);
    assert.deepEqual(
      res.body.data.systems.map((s) => s.title),
      ['World Ending', 'Covers'],
    );
    const mine = await ctx.call('GET', '/api/v2/galaxies/mine', { token: ctx.lmy });
    assert.equal(mine.body.data.galaxy.slug, 'lmy');
  });

  test('a system lists its worlds with their facts and no attention', async () => {
    const res = await ctx.call('GET', '/api/v2/galaxies/lmy/systems/world-ending');
    const titles = res.body.data.planets.map((p) => p.title);
    assert.deepEqual(titles, ['World Ending', 'Low Tide']);
    const lowTide = res.body.data.planets[1];
    assert.equal(lowTide.key, 'A minor');
    assert.equal(lowTide.bpm, 86);
    assert.equal(lowTide.viewCount, undefined);
  });

  test("a 22nd world is refused with the spec's sentence", async () => {
    const made = await ctx.call('POST', '/api/v2/galaxies/1/systems', {
      token: ctx.lmy,
      body: { title: 'Full House' },
    });
    assert.equal(made.status, 200);
    const systemId = made.body.data.system.id;
    let last;
    for (let i = 1; i <= 21; i++) {
      const trackId = await newSong(ctx, ctx.lmy, `Seat ${i}`);
      last = await ctx.call('POST', `/api/v2/systems/${systemId}/planets`, {
        token: ctx.lmy,
        body: { kind: 'song', trackId },
      });
      assert.equal(last.status, 200, `world ${i}`);
      assert.equal(last.body.data.planet.orbitIndex, i - 1);
    }
    assert.equal(last.body.data.remaining, 0);
    const trackId = await newSong(ctx, ctx.lmy, 'Seat 22');
    const res = await ctx.call('POST', `/api/v2/systems/${systemId}/planets`, {
      token: ctx.lmy,
      body: { kind: 'song', trackId },
    });
    assert.equal(res.status, 409);
    assert.deepEqual(res.body, {
      ok: false,
      error: { code: 'system_full', message: 'A solar system holds 21 worlds. Start another one.' },
    });
    assert.equal(ctx.state.planets.filter((p) => p.systemId === systemId).length, 21);
  });

  test('a world goes only into your own system, and a song is a world once', async () => {
    const theirs = await ctx.call('POST', '/api/v2/systems/3/planets', {
      token: ctx.lmy,
      body: { kind: 'song', trackId: await newSong(ctx, ctx.lmy, 'Wanders') },
    });
    assert.equal(theirs.status, 403);
    const twice = await ctx.call('POST', '/api/v2/systems/2/planets', {
      token: ctx.lmy,
      body: { kind: 'song', trackId: WORKS.worldEnding.trackId },
    });
    assert.deepEqual(twice.body.error, { code: 'already_placed', message: 'That song is already a planet somewhere' });
    const nobody = await ctx.call('POST', '/api/v2/systems/2/planets', {
      token: ctx.lmy,
      body: { kind: 'song', trackId: 'track_0_000000000' },
    });
    assert.equal(nobody.body.error.code, 'no_track');
  });

  test("a draft system is the owner's alone", async () => {
    await ctx.call('POST', '/api/v2/galaxies/1/systems', { token: ctx.lmy, body: { title: 'Sketches' } });
    const seen = await ctx.call('GET', '/api/v2/galaxies/lmy');
    assert.ok(!seen.body.data.systems.some((s) => s.title === 'Sketches'));
    const own = await ctx.call('GET', '/api/v2/galaxies/lmy', { token: ctx.lmy });
    assert.ok(own.body.data.systems.some((s) => s.title === 'Sketches' && s.status === 'draft'));
  });

  test("moving a world and placing a system follow the server's bounds", async () => {
    const planet = ctx.state.planets.find((p) => p.trackId === WORKS.lowTide.trackId);
    const far = await ctx.call('PATCH', `/api/v2/planets/${planet.id}`, {
      token: ctx.lmy,
      body: { orbitRadius: 2000 },
    });
    assert.deepEqual(far.body.error, { code: 'bad_radius', message: 'An orbit fits between 400 and 1800' });
    const ok = await ctx.call('PATCH', `/api/v2/planets/${planet.id}`, { token: ctx.lmy, body: { orbitRadius: 900 } });
    assert.equal(ok.body.data.planet.orbitRadius, 900);
    const half = await ctx.call('PATCH', '/api/v2/systems/2', { token: ctx.lmy, body: { posX: 10 } });
    assert.equal(half.body.error.code, 'bad_position');
  });
});

describe("suns keep what they don't know", () => {
  const ctx = useServer();

  test("a quote's source, a photo's width, a new block type and top-level keys all survive", async () => {
    const sun = ctx.state.suns.find((s) => s.kind === 'system');
    const doc = {
      v: 1,
      kind: 'work',
      medium: 'writing',
      layout: { b1: { x: 0, y: 0, w: 1, h: 1, z: 0, rot: 0 } },
      blocks: [
        { id: 'b1', type: 'text', style: 'quote', text: 'the drums leave', source: { workId: 'ab', from: 4, to: 9 } },
        { id: 'b2', type: 'photo', imageKey: 'someone/else.jpg', width: 60, alt: 'room' },
        { id: 'b3', type: 'timeline', beads: [{ title: 'EP v1 mixed', reached: true }] },
        { id: 'bad id!', type: 'text', text: 'dropped' },
      ],
    };
    const res = await ctx.call('PUT', `/api/v2/suns/${sun.id}/blocks`, { token: ctx.lmy, body: { blocks: doc } });
    assert.equal(res.status, 200);
    const saved = res.body.data.sun.blocks;
    assert.equal(saved.kind, 'work');
    assert.deepEqual(saved.layout, doc.layout);
    assert.deepEqual(saved.blocks[0].source, { workId: 'ab', from: 4, to: 9 });
    assert.equal(saved.blocks[1].width, 60);
    assert.equal(saved.blocks[1].imageKey, `suns/${sun.id}/b2.jpg`);
    assert.deepEqual(saved.blocks[2].beads, [{ title: 'EP v1 mixed', reached: true }]);
    assert.equal(saved.blocks.length, 3);
  });

  test("the document itself as the body is refused, and someone else's sun too", async () => {
    const sun = ctx.state.suns.find((s) => s.kind === 'system');
    const flat = await ctx.call('PUT', `/api/v2/suns/${sun.id}/blocks`, { token: ctx.lmy, body: { v: 1, blocks: [] } });
    assert.equal(flat.body.error.code, 'bad_blocks');
    const theirs = await ctx.call('PUT', `/api/v2/suns/${sun.id}/blocks`, {
      token: ctx.ana,
      body: { blocks: { v: 1, blocks: [] } },
    });
    assert.deepEqual(theirs.body.error, { code: 'not_yours', message: 'That sun belongs to someone else' });
  });
});

describe('lineage links wait for consent', () => {
  const ctx = useServer();

  test("Ana's claim on World Ending is pending for LMY, and hidden from everyone until he agrees", async () => {
    const pending = await ctx.call('GET', '/api/v2/lineage-links/pending', { token: ctx.lmy });
    const link = pending.body.data.links.find((l) => l.kind === 'sample');
    assert.equal(link.status, 'pending');
    assert.equal(link.declaredBy.username, 'Ana');
    const target = `type=planet&id=${link.to.id}`;
    const before = await ctx.call('GET', `/api/v2/lineage-links?${target}`);
    assert.ok(!before.body.data.links.some((l) => l.id === link.id));
    const self = await ctx.call('POST', `/api/v2/lineage-links/${link.id}/accept`, { token: ctx.ana });
    assert.equal(self.body.error.code, 'your_own');
    const agree = await ctx.call('POST', `/api/v2/lineage-links/${link.id}/accept`, { token: ctx.lmy });
    assert.equal(agree.body.data.link.status, 'accepted');
    const after = await ctx.call('GET', `/api/v2/lineage-links?${target}`);
    assert.ok(after.body.data.links.some((l) => l.id === link.id));
  });

  test('a link inside your own galaxy accepts itself; one touching nothing of yours is refused', async () => {
    const [a, b] = ctx.state.planets.filter((p) => p.systemId === 1);
    const own = await ctx.call('POST', '/api/v2/lineage-links', {
      token: ctx.lmy,
      body: { from: { type: 'planet', id: b.id }, to: { type: 'planet', id: a.id }, kind: 'cover' },
    });
    assert.equal(own.body.data.link.status, 'accepted');
    const stranger = await ctx.call('POST', '/api/v2/lineage-links', {
      token: ctx.ana,
      body: { from: { type: 'planet', id: b.id }, to: { type: 'planet', id: a.id }, kind: 'influence' },
    });
    assert.deepEqual(stranger.body.error, { code: 'not_yours', message: 'A link has to touch something of yours' });
  });

  test('Refuse removes the claim', async () => {
    const glass = ctx.state.planets.find((p) => p.trackId === WORKS.glassHours.trackId);
    const world = ctx.state.planets.find((p) => p.trackId === WORKS.worldEnding.trackId);
    const made = await ctx.call('POST', '/api/v2/lineage-links', {
      token: ctx.ana,
      body: { from: { type: 'planet', id: glass.id }, to: { type: 'planet', id: world.id }, kind: 'influence' },
    });
    assert.equal(made.body.data.link.status, 'pending');
    const refuse = await ctx.call('DELETE', `/api/v2/lineage-links/${made.body.data.link.id}`, { token: ctx.lmy });
    assert.deepEqual(refuse.body.data, { deleted: true });
  });
});

describe('saved, Newest, the universe and travel', () => {
  const ctx = useServer();

  test('saving is private: the maker is never told, and nothing counts it', async () => {
    const world = ctx.state.tracks.find((t) => t.songId === WORKS.worldEnding.songId);
    const body = { kind: 'song', publishedTrackId: world.id };
    const put = await ctx.call('PUT', '/api/v2/saved', { token: ctx.ana, body });
    assert.deepEqual(put.body.data, { saved: true });
    const hers = await ctx.call('GET', '/api/v2/saved', { token: ctx.ana });
    assert.ok(hers.body.data.songs.some((s) => s.publishedId === world.id));
    const his = await ctx.call('GET', '/api/v2/saved', { token: ctx.lmy });
    assert.ok(!his.body.data.songs.some((s) => s.publishedId === world.id));
    for (const path of ['/api/v2/since', '/api/v2/galaxies/lmy/systems/world-ending', '/api/v2/feed']) {
      const res = await ctx.call('GET', path, { token: ctx.lmy });
      assert.ok(!/sav/i.test(JSON.stringify(res.body)), path);
    }
    const off = await ctx.call('DELETE', '/api/v2/saved', { token: ctx.ana, body });
    assert.deepEqual(off.body.data, { saved: false });
  });

  test("Add galaxy is a private list that feeds Newest's filter", async () => {
    const added = await ctx.call('PUT', '/api/v2/saved', { token: ctx.lmy, body: { kind: 'galaxy', galaxyId: 2 } });
    assert.deepEqual(added.body.data, { saved: true });
    const feed = await ctx.call('GET', '/api/v2/feed?added=1', { token: ctx.lmy });
    assert.ok(feed.body.data.items.length > 0);
    assert.ok(feed.body.data.items.every((i) => i.galaxyId === 2));
  });

  test('Newest is newest first, then ends', async () => {
    const all = [];
    let cursor = null;
    do {
      const res = await ctx.call('GET', `/api/v2/feed?limit=2${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ''}`);
      assert.equal(res.status, 200);
      all.push(...res.body.data.items);
      cursor = res.body.data.nextCursor;
    } while (cursor);
    const times = all.map((i) => i.createdAt);
    assert.deepEqual(times, [...times].sort().reverse());
    assert.deepEqual(
      all.map((i) => i.title),
      ['glass hours', 'Low Tide', 'World Ending'],
    );
    const minor = await ctx.call('GET', '/api/v2/feed?key=A%20minor&bpmMin=100&bpmMax=140');
    assert.deepEqual(
      minor.body.data.items.map((i) => i.title),
      ['glass hours'],
    );
  });

  test('the catalog lists originals with their facts', async () => {
    const res = await ctx.call('GET', '/api/v2/catalog?search=world');
    assert.deepEqual(
      res.body.data.items.map((i) => i.title),
      ['World Ending'],
    );
    assert.equal(res.body.data.nextCursor, null);
  });

  test('the universe has no fog, no fuel and no counts', async () => {
    const res = await ctx.call('GET', '/api/v2/universe', { token: ctx.lmy });
    const lmy = res.body.data.galaxies.find((g) => g.slug === 'lmy');
    assert.ok(Number.isFinite(lmy.x) && Number.isFinite(lmy.y));
    assert.deepEqual(Object.keys(lmy).sort(), ['displayName', 'id', 'skySeed', 'slug', 'x', 'y']);
    assert.equal(res.body.data.viewer, undefined);
  });

  test('travel is free, needs an Idempotency-Key, and a replay answers the same', async () => {
    const bare = await ctx.call('POST', '/api/v2/travel', { token: ctx.lmy, body: { toGalaxyId: 2 } });
    assert.equal(bare.body.code, 'idempotency_key_required');
    const headers = { 'idempotency-key': 'trip-0001' };
    const go = await ctx.call('POST', '/api/v2/travel', { token: ctx.lmy, body: { toGalaxyId: 2 }, headers });
    assert.equal(go.body.data.outcome, 'arrived');
    assert.equal(go.body.data.landedGalaxy.slug, 'ana');
    assert.equal(go.body.data.path.length, 64);
    assert.equal(go.body.data.fuelSpent, undefined);
    const replay = await ctx.call('POST', '/api/v2/travel', { token: ctx.lmy, body: { toGalaxyId: 2 }, headers });
    assert.equal(replay.headers.get('idempotent-replay'), 'true');
    assert.deepEqual(replay.body, go.body);
    const there = await ctx.call('POST', '/api/v2/travel', {
      token: ctx.lmy,
      body: { toGalaxyId: 2 },
      headers: { 'idempotency-key': 'trip-0002' },
    });
    assert.equal(there.body.error.code, 'already_there');
  });
});

describe('Since you last looked', () => {
  const ctx = useServer();

  test("LMY sees Ana's fork, her waiting link, the sale and the payout, newest first", async () => {
    const res = await ctx.call('GET', '/api/v2/since?after=2026-10-01T00:00:00.000Z', { token: ctx.lmy });
    assert.equal(res.status, 200);
    const kinds = res.body.data.events.map((e) => e.kind);
    assert.deepEqual(kinds.sort(), ['fork', 'link', 'payout', 'sale']);
    const fork = res.body.data.events.find((e) => e.kind === 'fork');
    assert.deepEqual(
      { who: fork.who, work: fork.work, fork: fork.fork },
      { who: 'Ana', work: 'Low Tide', fork: 'glass hours' },
    );
    const sale = res.body.data.events.find((e) => e.kind === 'sale');
    assert.deepEqual({ work: sale.work, priceCents: sale.priceCents }, { work: 'World Ending', priceCents: 900 });
    const times = res.body.data.events.map((e) => e.at);
    assert.deepEqual(times, [...times].sort().reverse());
  });

  test("Ana, who added LMY's galaxy, sees his letter; a waiting link shows however old", async () => {
    const res = await ctx.call('GET', '/api/v2/since?after=2026-10-01T00:00:00.000Z', { token: ctx.ana });
    const letter = res.body.data.events.find((e) => e.kind === 'letter');
    assert.deepEqual(
      { who: letter.who, greeting: letter.greeting, opening: letter.opening },
      { who: 'LMY', greeting: 'Dear Wi-WWAV,', opening: 'Hardware is hard…' },
    );
    const late = await ctx.call('GET', '/api/v2/since?after=2027-01-01T00:00:00.000Z', { token: ctx.lmy });
    assert.deepEqual(
      late.body.data.events.map((e) => e.kind),
      ['link'],
    );
  });

  test('once LMY agrees, the link no longer waits on him', async () => {
    const link = ctx.state.links.find((l) => l.status === 'pending');
    await ctx.call('POST', `/api/v2/lineage-links/${link.id}/accept`, { token: ctx.lmy });
    const res = await ctx.call('GET', '/api/v2/since?after=2027-01-01T00:00:00.000Z', { token: ctx.lmy });
    assert.deepEqual(res.body.data.events, []);
  });
});
