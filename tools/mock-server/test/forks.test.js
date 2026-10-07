// Family trees and ↑ Push (docs/SPEC.md 4.6, 6.8, 9.7): the endpoints 9.7
// reuses as they are, /api/tracks/:id/fork, /api/lineage/global and
// /api/tracks/:trackId/is-published, in the shapes server.md §2 and §3 record.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer } from './setup.js';
import { WORKS, addAccount } from '../fixtures.js';
import { platformSongId } from '../hash.js';

const MIX = {
  stems: {
    vocals: { level: 0.4, muted: false, solo: false },
    'track_1-drums': { level: 7, muted: true },
    kazoo: { level: 1 },
  },
  masterPitch: 30,
  masterTime: 25,
  effects: { vox: { reverb: 0.5, eqLow: 40, pan: -3, extra: 1 }, piano: { reverb: 1 } },
};

const trackOf = (ctx, work) => ctx.state.tracks.find((t) => t.songId === work.songId);

describe('↑ Push: a fork owns no audio', () => {
  const ctx = useServer();
  const fork = (token, trackId, body, headers) =>
    ctx.call('POST', `/api/tracks/${trackId}/fork`, { token, body, headers });

  test("LMY forks glass hours: a node one deeper, playing its parent's stems, with the mix sanitized", async () => {
    const res = await fork(ctx.lmy, WORKS.glassHours.trackId, { mix: MIX });
    assert.equal(res.status, 201, JSON.stringify(res.body));
    const node = res.body;
    assert.match(node.trackId, /^fork_\d+_[a-z0-9]{9}$/);
    assert.equal(node.songId, platformSongId(node.trackId));
    assert.equal(node.title, 'glass hours (fork)');
    assert.equal(node.artist, 'LMY');
    assert.deepEqual(node.uploader, { id: 1, username: 'LMY', profilePicture: null });
    assert.equal(node.parentTrackId, WORKS.glassHours.trackId);
    assert.equal(node.lineageRootTrackId, WORKS.worldEnding.trackId);
    assert.equal(node.remixDepth, 3);
    assert.equal(node.inFeed, false);
    assert.equal(node.ownsAudio, false);
    assert.equal(node.stemsTrackId, WORKS.glassHours.trackId);
    assert.deepEqual([node.bpm, node.musicalKey, node.duration], [128, 'A minor', 176]);
    const { wwav, timePitch, stemEffects, overdubs } = node.remixSnapshot;
    assert.deepEqual(wwav, {
      v: 1,
      stems: { vocals: { level: 0.4, muted: false, solo: false }, drums: { level: 1, muted: true, solo: false } },
      masterPitch: 12,
      masterTime: 25,
    });
    assert.deepEqual(timePitch, { rate: 1.25, pitchSemitones: 12 });
    assert.deepEqual(stemEffects, {
      vox: { distortion: 0, delay: 0, reverb: 0.5, tremolo: 0, eqLow: 18, eqMid: 0, eqHigh: 0, pan: -1 },
    });
    assert.deepEqual(overdubs, {});
  });

  test('the fork joins the tree at once, and Ana hears of it in "Since you last looked"', async () => {
    const tree = await ctx.call('GET', `/api/tracks/${WORKS.glassHours.trackId}/lineage`);
    const forkNode = tree.body.descendants.find((n) => n.title === 'glass hours (fork)');
    assert.ok(forkNode, 'the fork is in the tree');
    assert.equal(forkNode.ownsAudio, false);
    assert.equal(forkNode.remixSnapshot.wwav.v, 1);
    const glass = tree.body.descendants.find((n) => n.trackId === WORKS.glassHours.trackId);
    assert.deepEqual([glass.ownsAudio, glass.stemsTrackId], [true, WORKS.glassHours.trackId]);
    const since = await ctx.call('GET', '/api/v2/since', { token: ctx.ana });
    assert.ok(
      since.body.data.events.some((e) => e.kind === 'fork' && e.work === 'glass hours' && e.who === 'LMY'),
      JSON.stringify(since.body.data.events),
    );
  });

  test('a fork of a fork plays the nearest ancestor that owns audio, and takes a seat only when placed', async () => {
    const first = ctx.state.tracks.find((t) => t.title === 'glass hours (fork)');
    const res = await fork(ctx.lmy, first.trackId, { title: 'Second hand', mix: MIX });
    assert.equal(res.status, 201);
    assert.deepEqual(
      [res.body.title, res.body.remixDepth, res.body.stemsTrackId],
      ['Second hand', 4, WORKS.glassHours.trackId],
    );
    assert.ok(!ctx.state.planets.some((p) => p.trackId === res.body.trackId));
    const placed = await ctx.call('POST', '/api/v2/systems/2/planets', {
      token: ctx.lmy,
      body: { kind: 'song', trackId: res.body.trackId },
    });
    assert.equal(placed.status, 200, JSON.stringify(placed.body));
  });

  test('a retry with the same Idempotency-Key is the same fork', async () => {
    const headers = { 'idempotency-key': 'push-0001-retry' };
    const a = await fork(ctx.ana, WORKS.worldEnding.trackId, { mix: MIX }, headers);
    const b = await fork(ctx.ana, WORKS.worldEnding.trackId, { mix: MIX }, headers);
    assert.equal(a.status, 201);
    assert.deepEqual(b.body, a.body);
    assert.equal(b.headers.get('idempotent-replay'), 'true');
    assert.equal(ctx.state.tracks.filter((t) => t.trackId === a.body.trackId).length, 1);
  });

  test('a v2 project: its sources must own audio, and a second song is a second parent', async () => {
    const project = {
      sources: { primary: { trackId: WORKS.lowTide.trackId }, secondary: { trackId: WORKS.glassHours.trackId } },
      clips: [
        { id: 'a', track: 0, startTime: 0, clipDuration: 8, src: { type: 'stem', of: 'primary', kind: 'drums' } },
        { id: 'b', track: 9, startTime: -4, clipDuration: 8, src: { type: 'stem', of: 'secondary', kind: 'bass' } },
      ],
      tracks: [{ gain: 3, eq: { on: true, low: 40 } }],
      master: { pitch: 2, rate: 1.1 },
      duration: 30,
    };
    const res = await fork(ctx.lmy, WORKS.lowTide.trackId, { project });
    assert.equal(res.status, 201, JSON.stringify(res.body));
    assert.equal(res.body.secondaryParentTrackId, WORKS.glassHours.trackId);
    assert.equal(res.body.duration, 30);
    const saved = res.body.remixSnapshot.wwav;
    assert.equal(saved.v, 2);
    assert.equal(saved.project.tracks.length, 8);
    assert.equal(saved.project.tracks[0].gain, 1);
    assert.equal(saved.project.tracks[0].eq.low, 12);
    assert.deepEqual([saved.project.clips[1].track, saved.project.clips[1].startTime], [7, 0]);
    assert.deepEqual(res.body.remixSnapshot.timePitch, { rate: 1.1, pitchSemitones: 2 });

    const nowhere = await fork(ctx.lmy, WORKS.lowTide.trackId, {
      project: { ...project, sources: { primary: { trackId: 'track_0_nothing00' } } },
    });
    assert.deepEqual([nowhere.status, nowhere.body.error], [422, 'Source track_0_nothing00 has no audio']);
    const empty = await fork(ctx.lmy, WORKS.lowTide.trackId, { project: { ...project, clips: [] } });
    assert.deepEqual([empty.status, empty.body.error], [400, 'Project has no clips']);
    const imported = await fork(ctx.lmy, WORKS.lowTide.trackId, {
      project: { ...project, sources: { ...project.sources, imports: ['ra_1'] } },
    });
    assert.deepEqual([imported.status, imported.body.error], [422, 'Import ra_1 does not exist']);
  });

  test('no mix, an unknown or withdrawn track, or a tree 64 deep is refused', async () => {
    const none = await fork(ctx.lmy, WORKS.lowTide.trackId, { title: 'Nothing' });
    assert.deepEqual([none.status, none.body], [400, { error: 'Missing or invalid mix state' }]);
    const unknown = await fork(ctx.lmy, 'track_0_missing00', { mix: MIX });
    assert.deepEqual([unknown.status, unknown.body], [404, { error: 'Track not found' }]);
    const deep = await addAccount(ctx.state, { username: 'Deep' });
    const low = trackOf(ctx, WORKS.lowTide);
    const depth = low.remixDepth;
    low.remixDepth = 64;
    const tooDeep = await fork(deep.token, WORKS.lowTide.trackId, { mix: MIX });
    low.remixDepth = depth;
    assert.deepEqual(
      [tooDeep.status, tooDeep.body],
      [422, { error: 'Lineage is too deep to extend', code: 'max_depth_reached' }],
    );
    const unsigned = await fork(undefined, WORKS.lowTide.trackId, { mix: MIX });
    assert.equal(unsigned.status, 401);
  });

  test("a fork has no file of its own, so it can't be priced until it's exported", async () => {
    const mine = ctx.state.tracks.find((t) => t.title === 'Second hand');
    const res = await ctx.call('PUT', `/api/tracks/${mine.trackId}/set-price`, { token: ctx.lmy, body: { price: 4 } });
    assert.equal(res.status, 400);
    assert.equal(res.body.code, 'no_file');
  });
});

describe('the global forest and is-published', () => {
  const ctx = useServer();

  test('/api/lineage/global lists every family: roots, then nodes by depth, without snapshots', async () => {
    const res = await ctx.call('GET', '/api/lineage/global');
    assert.equal(res.status, 200);
    assert.equal(res.headers.get('cache-control'), 'public, max-age=15');
    assert.deepEqual(res.body.roots, [WORKS.worldEnding.trackId]);
    assert.deepEqual(
      res.body.nodes.map((n) => [n.title, n.remixDepth, n.ownsAudio, n.stemsTrackId]),
      [
        ['World Ending', 0, true, WORKS.worldEnding.trackId],
        ['Low Tide', 1, true, WORKS.lowTide.trackId],
        ['glass hours', 2, true, WORKS.glassHours.trackId],
      ],
    );
    assert.ok(res.body.nodes.every((n) => !('remixSnapshot' in n)));
    const fresh = await ctx.call('GET', '/api/lineage/global?fresh=1');
    assert.equal(fresh.headers.get('cache-control'), null);
  });

  test('a withdrawn work stays in its family, marked', async () => {
    const glass = trackOf(ctx, WORKS.glassHours);
    glass.withdrawn = true;
    const res = await ctx.call('GET', '/api/lineage/global');
    glass.withdrawn = false;
    assert.equal(res.body.nodes.find((n) => n.trackId === WORKS.glassHours.trackId).withdrawn, true);
  });

  test('is-published answers the numeric id, and whether you may unpublish', async () => {
    const path = `/api/tracks/${WORKS.lowTide.trackId}/is-published`;
    const owner = await ctx.call('GET', path, { token: ctx.lmy });
    assert.equal(owner.status, 200);
    assert.equal(owner.body.isPublished, true);
    assert.equal(owner.body.canUnpublish, true);
    assert.equal(owner.body.publishedTrack.id, trackOf(ctx, WORKS.lowTide).id);
    assert.equal(owner.body.publishedTrack.trackId, WORKS.lowTide.trackId);
    assert.equal(owner.body.publishedTrack.price, '4.00');
    const other = await ctx.call('GET', path, { token: ctx.ana });
    assert.equal(other.body.canUnpublish, false);
    const anyone = await ctx.call('GET', path);
    assert.equal(anyone.body.canUnpublish, false);
    const never = await ctx.call('GET', '/api/tracks/track_0_never0000/is-published');
    assert.deepEqual(never.body, { isPublished: false, publishedTrack: null, canUnpublish: false });
    const bad = await ctx.call('GET', path, { headers: { authorization: 'Bearer not.a.token' } });
    assert.equal(bad.status, 401);
  });
});
