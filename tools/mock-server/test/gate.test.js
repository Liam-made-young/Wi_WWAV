// Gate 1.3 (docs/GATES.md): no endpoint returns a count of other people's
// attention. This walks every key of every JSON answer the mock gives
// during a tour that reaches every route, and fails on any key that names
// likes, plays, views, followers, sales, a count or "seen". It also fails if
// a route was left out of the tour, so a new endpoint can't slip past.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { startMockServer } from '../server.js';
import { ACCOUNTS, WORKS } from '../fixtures.js';
import { call, completeCheckout, desktopTokens, putSigned } from '../helpers.js';
import { writeWwav } from '../wwav.js';

const FORBIDDEN = new Set(
  (
    'like likes liked play plays played listen listens listened view views viewed viewer viewers ' +
    'follower followers following follows count counts seen sold sales saves popular popularity trending ' +
    'rank ranked ranking attendee attendees repost reposts comments fuel fans rating ratings reviews ' +
    'visits visitors downloads buyers listener listeners streams streamed subscriber subscribers subscribed ' +
    'favorite favorites favorited favourite favourites hearts hearted upvote upvotes downvotes impressions reach ' +
    'engagement'
  ).split(' '),
);

// Facts about one's own work, not anyone's attention: how many worlds a
// system holds and how many blocks a sun has, as the scene payloads give.
const ALLOWED = new Set(['planetCount', 'blockCount']);

function words(key) {
  return key
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean);
}

export function attentionKeys(value, path = '$') {
  if (Array.isArray(value)) return value.flatMap((v, i) => attentionKeys(v, `${path}[${i}]`));
  if (!value || typeof value !== 'object') return [];
  return Object.entries(value).flatMap(([key, v]) => {
    const here = !ALLOWED.has(key) && words(key).some((w) => FORBIDDEN.has(w)) ? [`${path}.${key}`] : [];
    return [...here, ...attentionKeys(v, `${path}.${key}`)];
  });
}

test('the walker catches the keys the gate names', () => {
  const found = attentionKeys({ a: { viewCount: 1, items: [{ likes: 2, play_count: 3 }] }, followers: [], seen: true });
  assert.deepEqual(found, ['$.a.viewCount', '$.a.items[0].likes', '$.a.items[0].play_count', '$.followers', '$.seen']);
  assert.deepEqual(attentionKeys({ displayName: 'LMY', planetCount: 2, playhead: 1 }), []);
});

test('no answer from any route carries a count of attention', async () => {
  const seen = [];
  const server = await startMockServer({ port: 0, onResponse: (r) => seen.push(r) });
  const base = server.url;
  const c = (method, path, opts) => call(base, method, path, opts);
  try {
    await tour(base, c, server.state);
    const offending = seen.flatMap((r) => attentionKeys(r.body).map((k) => `${r.route}: ${k}`));
    assert.deepEqual(offending, []);
    const reached = new Set(seen.filter((r) => r.status < 400).map((r) => r.route));
    const missed = server.routes.filter((r) => !r.includes(' /__mock/') && !reached.has(r));
    assert.deepEqual(missed, [], 'routes the tour never reached with a success');
  } finally {
    await server.close();
  }
});

// Every route once, as the app would use it.
async function tour(base, c, state) {
  await c('GET', '/');
  const lmyPair = (await c('POST', '/api/auth/login', { body: ACCOUNTS.lmy })).body;
  const lmy = lmyPair.token;
  const ana = (await c('POST', '/api/auth/login', { body: ACCOUNTS.ana })).body.token;
  await c('POST', '/api/auth/refresh', { body: { refreshToken: lmyPair.refreshToken } });
  await c('GET', '/api/auth/me', { token: lmy });
  await desktopTokens(base, ACCOUNTS.ana);

  const device = (await c('POST', '/oauth/device/code', { form: { client_id: 'prana' } })).body;
  await c('GET', `/device?user_code=${device.user_code}`);
  await c('POST', '/device', { form: { user_code: device.user_code, ...ACCOUNTS.lmy } });
  await c('POST', '/oauth/desktop/token', {
    form: {
      grant_type: 'urn:ietf:params:oauth:grant-type:device_code',
      device_code: device.device_code,
      client_id: 'prana',
    },
  });

  // A new song, uploaded whole, and a new version in parts.
  const songId = randomBytes(16).toString('hex');
  const v1 = writeWwav({ songId, title: 'Tour' });
  const signed = (await c('GET', `/api/upload/sign?fileType=audio/wav&size=${v1.length}`, { token: lmy })).body;
  await putSigned(signed.signedUrl, v1, 'audio/wav');
  await c('POST', '/api/publish', { token: lmy, body: { trackId: signed.trackId, settings: { origin: 'wi_wwav' } } });
  await c('GET', `/api/upload/sign-video?ext=mp4&size=10`, { token: lmy });
  await c('GET', `/api/upload/sign-replace?trackId=${signed.trackId}&fileType=audio/wav`, { token: lmy });
  const v2 = writeWwav({ songId, title: 'Tour', version: 2 });
  const parts = (
    await c('POST', '/api/upload/parts', { token: lmy, body: { size: v2.length, trackId: signed.trackId } })
  ).body;
  const part = (await c('GET', `/api/upload/parts/${parts.uploadId}/1`, { token: lmy })).body;
  const { etag } = await putSigned(part.signedUrl, v2);
  await c('POST', `/api/upload/parts/${parts.uploadId}/complete`, { token: lmy, body: { parts: [{ n: 1, etag }] } });
  await c('POST', '/api/publish', { token: lmy, body: { trackId: signed.trackId, s3Key: parts.s3Key } });
  await c('GET', `/api/tracks/${signed.trackId}/lineage`);
  await c('GET', `/api/tracks/${signed.trackId}/is-published`, { token: lmy });
  await c('PUT', `/api/tracks/${signed.trackId}/set-price`, { token: lmy, body: { price: 3 } });
  await c('POST', `/api/tracks/${WORKS.glassHours.trackId}/fork`, {
    token: lmy,
    body: { mix: { stems: { vocals: { level: 0.5 } } } },
  });
  await c('GET', '/api/lineage/global');

  // Space.
  const claim = await c('POST', '/api/v2/galaxies', { token: lmy, body: {} });
  const galaxyId = claim.body.data.galaxy.id;
  await c('GET', '/api/v2/galaxies/mine', { token: lmy });
  await c('GET', '/api/v2/galaxies/lmy', { token: ana });
  await c('GET', '/api/v2/galaxies/lmy/sun');
  await c('GET', '/api/v2/galaxies/lmy/letters');
  const system = (await c('POST', `/api/v2/galaxies/${galaxyId}/systems`, { token: lmy, body: { title: 'Tour' } })).body
    .data.system;
  await c('PATCH', `/api/v2/systems/${system.id}`, { token: lmy, body: { status: 'published' } });
  const placed = await c('POST', `/api/v2/systems/${system.id}/planets`, {
    token: lmy,
    body: { kind: 'song', trackId: signed.trackId },
  });
  const planetId = placed.body.data.planet.id;
  await c('PATCH', `/api/v2/planets/${planetId}`, { token: lmy, body: { orbitIndex: 3 } });
  await c('GET', `/api/v2/galaxies/lmy/systems/${system.slug}`, { token: ana });
  const sun = (await c('GET', `/api/v2/galaxies/lmy/systems/${system.slug}/sun`)).body.data.sun;
  await c('GET', `/api/v2/suns/${sun.id}`);
  await c('PUT', `/api/v2/suns/${sun.id}/blocks`, {
    token: lmy,
    body: { blocks: { v: 1, blocks: [{ id: 'a', type: 'text', text: 'Hello' }] } },
  });
  const glassPlanet = state.planets.find((p) => p.trackId === WORKS.glassHours.trackId);
  const link = await c('POST', '/api/v2/lineage-links', {
    token: lmy,
    body: { from: { type: 'planet', id: planetId }, to: { type: 'planet', id: glassPlanet.id }, kind: 'influence' },
  });
  await c('GET', '/api/v2/lineage-links/pending', { token: ana });
  await c('POST', `/api/v2/lineage-links/${link.body.data.link.id}/accept`, { token: ana });
  await c('GET', `/api/v2/lineage-links?type=planet&id=${planetId}`);
  await c('DELETE', `/api/v2/lineage-links/${link.body.data.link.id}`, { token: lmy });
  const world = state.tracks.find((t) => t.songId === WORKS.worldEnding.songId);
  await c('PUT', '/api/v2/saved', { token: ana, body: { kind: 'song', publishedTrackId: world.id } });
  await c('GET', '/api/v2/saved', { token: ana });
  await c('DELETE', '/api/v2/saved', { token: ana, body: { kind: 'song', publishedTrackId: world.id } });
  await c('GET', '/api/v2/catalog');
  await c('GET', '/api/v2/feed', { token: lmy });
  await c('GET', '/api/v2/universe', { token: lmy });
  await c('POST', '/api/v2/travel', {
    token: lmy,
    body: { toGalaxyId: 2 },
    headers: { 'idempotency-key': 'tour-0001' },
  });
  await c('GET', '/api/v2/since', { token: lmy });
  await c('GET', '/api/v2/since', { token: ana });

  // Buying.
  const glass = state.tracks.find((t) => t.songId === WORKS.glassHours.songId);
  const bought = (
    await c('POST', '/api/purchase/create-checkout', { token: lmy, body: { type: 'track', id: glass.id } })
  ).body;
  const page = new URL(bought.url).pathname;
  await c('GET', page);
  await c('POST', page);
  await c('GET', `/api/purchase/check?type=track&id=${glass.id}`, { token: lmy });
  await c('GET', '/api/entitlements', { token: lmy });
  const file = (await c('GET', `/api/entitlements/track/${glass.id}/file`, { token: lmy })).body;
  await c('GET', new URL(file.url).pathname + new URL(file.url).search);
  const onboard = (await c('POST', '/api/connect/onboard', { token: ana })).body;
  await c('GET', new URL(onboard.url).pathname);
  await c('POST', new URL(onboard.url).pathname);
  await c('GET', '/api/connect/status', { token: ana });
  await c('GET', '/api/store/halls', { token: lmy });
  await c('GET', '/api/store/new');
  const jacket = state.listings[0];
  await c('PUT', '/api/store/bag', { token: lmy, body: { type: 'fashion', id: jacket.id } });
  await c('GET', '/api/store/bag', { token: lmy });
  await c('DELETE', `/api/store/bag/fashion/${jacket.id}`, { token: lmy });
  await c('PUT', '/api/store/bag', { token: lmy, body: { type: 'fashion', id: jacket.id } });
  const bag = (await c('POST', '/api/store/bag/checkout', { token: lmy })).body;
  await completeCheckout(base, bag.payments[0].sessionId);

  // Heat, Claude, updates.
  await c('POST', '/api/heat/changes', {
    token: lmy,
    body: { device: 'mac-a', changes: [{ kind: 'task', id: 't', field: 'title', value: 'Quiz', seq: 1 }] },
  });
  await c('GET', '/api/heat/changes?cursor=0', { token: lmy });
  for (const task of ['score', 'clerk']) {
    await c('POST', `/api/assist/${task}`, {
      token: lmy,
      body: { title: 'x', record: { title: 'glass hours', artist: 'Ana' }, question: 'Who plays guitar?' },
    });
  }
  await c('GET', '/desktop/latest.json');

  // Taking it all back: the world leaves its system, the work is
  // unpublished, and the system goes.
  await c('DELETE', `/api/v2/planets/${planetId}`, { token: lmy });
  await c('POST', '/api/unpublish', { token: lmy, body: { trackId: signed.trackId } });
  await c('DELETE', `/api/v2/systems/${system.id}`, { token: lmy });
}
