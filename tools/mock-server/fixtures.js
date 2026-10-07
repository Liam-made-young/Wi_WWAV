// The seed: the same small world every time the mock starts or resets.
// LMY and Ana; LMY's galaxy with "World Ending" and "Covers"; three songs in
// one family (World Ending → Low Tide → glass hours); Ana's one-of-one
// jacket; Ana's claim on World Ending waiting for LMY; one sale, one payout
// and one letter, so "Since you last looked" has something to say.
import { tokenPair } from './jwt.js';
import { etagOf, sha256, unitJitter } from './hash.js';
import { nextId } from './state.js';
import { writeWwav } from './wwav.js';
import { versionKey } from './routes/publish.js';

export const ACCOUNTS = {
  lmy: { email: 'lmy@mi-wwav.com', password: 'WeWave-lmy1' },
  ana: { email: 'ana@example.com', password: 'WeWave-ana1' },
};

export const WORKS = {
  worldEnding: { trackId: 'track_1757700000000_q7m2x9k4a', songId: '9f3c41d2e7a84b06b1c5d8e2f4a6c3e1' },
  lowTide: { trackId: 'track_1758390000000_h3n8v1c6p', songId: '4b1d7e0c93a2468f8e5f1c2d3b4a5960' },
  glassHours: { trackId: 'track_1759591800000_t5w9r2d7j', songId: 'c7a2e95f1d3b4e6a9f80b1c2d3e4f5a6' },
};

function astronaut(hull) {
  return {
    v: 1,
    race: 'human',
    complexion: 4,
    physique: 3,
    hair: { style: 'crop', color: '#2b1d14' },
    cosmetics: [],
    rocket: { preset: 'classic', colors: { hull, accent: '#2946ff', flame: '#f0b90b' } },
  };
}

export function addUser(state, { username, email, password, account, tier = 'free', founding = null, createdAt }) {
  const user = {
    id: nextId(state, 'users'),
    email,
    password,
    username,
    bio: '',
    profilePicture: null,
    isPro: tier !== 'free',
    proExpiresAt: null,
    tier,
    tierExpiresAt: null,
    foundingMemberNumber: founding,
    connect: account
      ? { account, detailsSubmitted: true, chargesEnabled: true, payoutsEnabled: true }
      : { account: null, detailsSubmitted: false, chargesEnabled: false, payoutsEnabled: false },
    shipFrom: { city: 'Providence', state: 'RI', zip: '02903', country: 'US' },
    astronaut: astronaut('#f4efe6'),
    stemPlayerCustomization: null,
    createdAt,
  };
  state.users.push(user);
  return user;
}

// A third (or fourth) account for a test, signed in. `onboarded: false`
// leaves it without a payout account.
export function addAccount(state, { username, onboarded = true }) {
  const user = addUser(state, {
    username,
    email: `${username.toLowerCase()}@example.com`,
    password: 'WeWave-test1',
    account: onboarded ? `acct_${username.toLowerCase()}` : null,
    createdAt: new Date(state.now()).toISOString(),
  });
  return { user, token: tokenPair(user, state.now()).token };
}

function galaxy(state, user, at) {
  const id = nextId(state, 'galaxies');
  const skySeed = `galaxy-${user.id}`;
  const g = {
    id,
    userId: user.id,
    slug: user.username.toLowerCase(),
    displayName: user.username,
    skySeed,
    createdAt: at,
  };
  Object.assign(g, galaxyPosition(id, skySeed));
  state.galaxies.push(g);
  return g;
}

// Where a galaxy sits in the universe (server utils/universeMath.js): a
// phyllotaxis spiral by id, so newcomers land on the rim.
export function galaxyPosition(id, skySeed) {
  const angle = id * 2.399963 + unitJitter(skySeed, 'sky') * 0.6;
  const r = 1320 * Math.sqrt(id);
  return { x: 2500 + Math.cos(angle) * r, y: 2500 + Math.sin(angle) * r };
}

export function addSun(state, { kind, galaxyId = null, systemId = null, title, blocks = [], createdAt }) {
  const sun = {
    id: nextId(state, 'suns'),
    kind,
    galaxyId,
    systemId,
    title,
    blocks: { v: 1, blocks },
    appearance: null,
    createdAt,
  };
  state.suns.push(sun);
  return sun;
}

function system(state, g, { title, slug, at }) {
  const s = {
    id: nextId(state, 'systems'),
    galaxyId: g.id,
    slug,
    title,
    status: 'published',
    orbitIndex: state.systems.filter((x) => x.galaxyId === g.id).length,
    colorSeed: null,
    posX: null,
    posY: null,
    createdAt: at,
  };
  state.systems.push(s);
  return s;
}

function song(state, user, work, { title, bpm, key, duration, priceCents, at, parent = null, root, generation }) {
  const bytes = writeWwav({
    songId: work.songId,
    title,
    artist: user.username,
    bpm,
    key,
    type: parent ? 'remix' : 'original',
    created: at.slice(0, 10),
    parentId: parent?.songId ?? null,
    rootId: (root ?? work).songId,
    generation,
    creator: parent ? user.username : '',
  });
  const s3Key = `uploads/${work.trackId}`;
  const object = { bytes, contentType: 'audio/wav', etag: etagOf(bytes), sha256: sha256(bytes) };
  state.objects.set(s3Key, object);
  state.objects.set(versionKey(object.sha256), object);
  state.uploads.push({ trackId: work.trackId, userId: user.id, s3Key, createdAt: at });
  const track = {
    id: nextId(state, 'tracks'),
    trackId: work.trackId,
    songId: work.songId,
    uploaderId: user.id,
    title,
    artist: user.username,
    album: 'Single',
    coverArtUrl: null,
    duration,
    bpm,
    musicalKey: key,
    isMaster: true,
    settings: { origin: 'wi_wwav' },
    tags: [],
    parentTrackId: parent?.trackId ?? null,
    secondaryParentTrackId: null,
    lineageRootTrackId: (root ?? work).trackId,
    remixDepth: generation,
    inFeed: true,
    remixSnapshot: null,
    priceCents,
    isForSale: priceCents !== null,
    withdrawn: false,
    createdAt: at,
    versions: [
      {
        version: 1,
        sha256: object.sha256,
        bytes: bytes.length,
        s3Key: versionKey(object.sha256),
        fileName: `${title}.wwav`,
        createdAt: at,
      },
    ],
  };
  state.tracks.push(track);
  return track;
}

function planet(state, sys, track) {
  const p = {
    id: nextId(state, 'planets'),
    systemId: sys.id,
    kind: 'song',
    trackId: track.trackId,
    publishedTrackId: track.id,
    orbitIndex: state.planets.filter((x) => x.systemId === sys.id).length,
    orbitRadius: null,
    phaseOffset: null,
    appearance: null,
    createdAt: track.createdAt,
  };
  state.planets.push(p);
  return p;
}

export function seed(state) {
  const lmy = addUser(state, {
    username: 'LMY',
    ...ACCOUNTS.lmy,
    account: 'acct_lmy',
    tier: 'founding',
    founding: 1,
    createdAt: '2026-08-17T12:00:00.000Z',
  });
  lmy.bio = 'Making a handheld and an album.';
  const ana = addUser(state, {
    username: 'Ana',
    ...ACCOUNTS.ana,
    account: 'acct_ana',
    createdAt: '2026-08-20T12:00:00.000Z',
  });
  ana.bio = 'Bass on borrowed instruments.';
  ana.astronaut = astronaut('#d23c2a');

  const home = galaxy(state, lmy, '2026-08-17T12:00:00.000Z');
  const hers = galaxy(state, ana, '2026-08-20T12:00:00.000Z');
  addSun(state, {
    kind: 'galaxy',
    galaxyId: home.id,
    title: 'LMY',
    blocks: [{ id: 'b1', type: 'text', style: 'body', text: lmy.bio }],
    createdAt: home.createdAt,
  });
  addSun(state, {
    kind: 'galaxy',
    galaxyId: hers.id,
    title: 'Ana',
    blocks: [{ id: 'b1', type: 'text', style: 'body', text: ana.bio }],
    createdAt: hers.createdAt,
  });

  const worldEndingSystem = system(state, home, {
    title: 'World Ending',
    slug: 'world-ending',
    at: '2026-09-01T12:00:00.000Z',
  });
  const covers = system(state, home, { title: 'Covers', slug: 'covers', at: '2026-09-02T12:00:00.000Z' });
  const borrowed = system(state, hers, {
    title: 'Borrowed bass',
    slug: 'borrowed-bass',
    at: '2026-09-25T12:00:00.000Z',
  });
  for (const s of [worldEndingSystem, covers, borrowed]) {
    addSun(state, { kind: 'system', systemId: s.id, title: s.title, createdAt: s.createdAt });
  }

  const world = song(state, lmy, WORKS.worldEnding, {
    title: 'World Ending',
    bpm: 92,
    key: 'E minor',
    duration: 238,
    priceCents: 900,
    at: '2026-09-12T18:00:00.000Z',
    generation: 0,
  });
  const low = song(state, lmy, WORKS.lowTide, {
    title: 'Low Tide',
    bpm: 86,
    key: 'A minor',
    duration: 201,
    priceCents: 400,
    at: '2026-09-20T18:00:00.000Z',
    parent: WORKS.worldEnding,
    root: WORKS.worldEnding,
    generation: 1,
  });
  const glass = song(state, ana, WORKS.glassHours, {
    title: 'glass hours',
    bpm: 128,
    key: 'A minor',
    duration: 176,
    priceCents: 400,
    at: '2026-10-04T15:30:00.000Z',
    parent: WORKS.lowTide,
    root: WORKS.worldEnding,
    generation: 2,
  });
  const worldPlanet = planet(state, worldEndingSystem, world);
  planet(state, worldEndingSystem, low);
  const glassPlanet = planet(state, borrowed, glass);

  state.links.push({
    id: nextId(state, 'links'),
    from: { type: 'planet', id: glassPlanet.id },
    to: { type: 'planet', id: worldPlanet.id },
    kind: 'sample',
    note: 'The pad under the second verse.',
    status: 'pending',
    declaredById: ana.id,
    createdAt: '2026-10-04T16:00:00.000Z',
  });
  state.saved.push(
    { userId: ana.id, kind: 'galaxy', targetId: home.id, createdAt: '2026-09-30T12:00:00.000Z' },
    { userId: lmy.id, kind: 'song', targetId: glass.id, createdAt: '2026-10-05T09:00:00.000Z' },
  );
  state.letters.push({
    id: nextId(state, 'letters'),
    userId: lmy.id,
    greeting: 'Dear Wi-WWAV,',
    blocks: [{ type: 'text', style: 'body', text: 'Hardware is hard…' }],
    signoff: 'Sincerely LMY',
    sentAt: '2026-10-03',
    createdAt: '2026-10-03T14:00:00.000Z',
  });

  state.listings.push({
    id: nextId(state, 'listings'),
    sellerId: ana.id,
    title: 'Waxed chore jacket',
    priceCents: 12000,
    size: 'M',
    condition: 'like_new',
    category: 'outerwear',
    brand: '',
    color: 'olive',
    material: 'waxed cotton',
    measurements: { chest: '21 in', length: '28 in' },
    photos: ['fashion/1/front.jpg', 'fashion/1/back.jpg', 'fashion/1/label.jpg'],
    status: 'active',
    hold: null,
    createdAt: '2026-10-06T10:00:00.000Z',
  });

  state.purchases.push({
    id: nextId(state, 'purchases'),
    userId: ana.id,
    sellerId: lmy.id,
    type: 'track',
    itemId: world.id,
    amountCents: 900,
    feeCents: 90,
    status: 'completed',
    sessionId: null,
    createdAt: '2026-10-05T12:00:00.000Z',
    completedAt: '2026-10-05T12:00:00.000Z',
  });
  state.payouts.push({
    id: nextId(state, 'payouts'),
    userId: lmy.id,
    amountCents: 810,
    at: '2026-10-06T09:00:00.000Z',
  });
}
