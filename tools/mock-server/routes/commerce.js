// Buying (docs/SPEC.md 6.12, 7.10-7.14, 7.20, 10.4), with a fake Stripe.
//
// The rules are purchase.js's: one item per checkout, the 10% fee inside the
// price, your own items and digital items you already own refused, 20
// checkouts an hour. What the spec adds or fixes (server.md §7, §11):
// - only a *completed* purchase counts as owned, in /purchase/check and for
//   downloads (today a pending one counts, and nothing checks downloads);
// - /api/entitlements signs a 60 s URL for the exact bytes, any version;
// - the one-of-one hold and its Checkout session both end at 30 minutes, so
//   a stale session can't sell a garment twice (today the session lives 24 h);
// - a listing can't be priced until payouts are set up (mismatch 17);
// - the bag: one charge for every file in it, transferred to each seller
//   under one transfer_group, and one destination charge per seller of goods.
// The mock refuses with the server's status codes (400 for your own item
// and for one you own, mismatch 18).
import { escapeHtml, error, html, json, page, redirect } from '../http.js';
import { onboarded, optionalUser, requireUser } from '../auth.js';
import { idempotent, iso, limit, nextId } from '../state.js';
import { presign } from './uploads.js';
import { ownsPurchase } from './space.js';
import { unitJitter } from '../hash.js';

const FEE = 0.1;
const HOLD_MS = 30 * 60 * 1000;
const MAX_DIGITAL_CENTS = 200000;
const GETS = '.wwav, master and four stems';

const byId = (rows, id) => rows.find((r) => r.id === Number(id));

// --- items ---------------------------------------------------------------------

// What a {type, id} names, in one shape for checkout and the bag.
function itemOf(state, type, id) {
  if (type === 'fashion') {
    const listing = byId(state.listings, id);
    if (!listing) throw error(404, 'Listing not found');
    return {
      type,
      id: listing.id,
      title: listing.title,
      sellerId: listing.sellerId,
      priceCents: listing.priceCents,
      physical: true,
      listing,
    };
  }
  const track = byId(state.tracks, id);
  if (!track || track.withdrawn) throw error(404, 'Track not found');
  return {
    type: 'track',
    id: track.id,
    title: track.title,
    sellerId: track.uploaderId,
    priceCents: track.priceCents,
    physical: false,
    track,
  };
}

// The refusals every way of buying shares.
function checkBuyable(state, me, item) {
  if (item.sellerId === me.id) {
    throw error(400, item.physical ? 'Cannot purchase your own listing' : 'Cannot purchase your own content');
  }
  if (!item.physical) {
    if (!item.track.isForSale || !item.priceCents) throw error(400, 'Track not for sale');
    if (ownsPurchase(state, me, 'track', item.id)) throw error(400, 'Already purchased');
  }
  if (!onboarded(byId(state.users, item.sellerId))) throw error(409, 'Seller payout setup incomplete');
}

function limitCheckouts(state, me) {
  limit(state, {
    scope: 'checkout',
    key: `u:${me.id}`,
    max: 20,
    windowMs: 60 * 60 * 1000,
    message: 'Too many checkout attempts. Please wait a moment.',
  });
}

// --- the fake Stripe ---------------------------------------------------------

// Holds and sessions that have passed their 30 minutes end, as Stripe's
// checkout.session.expired webhook would end them. Runs before every request.
export function sweep(state) {
  const now = state.now();
  for (const session of state.sessions.values()) {
    if (session.status === 'open' && session.expiresAt <= now) {
      session.status = 'expired';
      for (const p of state.purchases) if (p.sessionId === session.id) p.status = 'expired';
    }
  }
  for (const listing of state.listings) {
    if (listing.status === 'reserved' && listing.hold.until <= now) {
      listing.status = 'active';
      listing.hold = null;
    }
  }
}

function openSession(state, buyer, items, { physical }) {
  const id = `cs_test_${String(nextId(state, 'sessions')).padStart(6, '0')}`;
  const amountTotal = items.reduce((n, i) => n + i.priceCents, 0);
  const feeCents = Math.round(amountTotal * FEE);
  const describe = physical ? 'Streetwear purchase on WWAV' : 'Purchase on WWAV';
  const session = {
    id,
    url: `${state.base}/__stripe/checkout/${id}`,
    buyerId: buyer.id,
    lineItems: items.map((i) => ({
      name: i.title,
      description: `${describe} (10% platform fee included)`,
      amountCents: i.priceCents,
    })),
    amountTotal,
    currency: 'usd',
    feeCents,
    expiresAt: state.now() + HOLD_MS,
    status: 'open',
    successUrl: `${state.base}/?purchase=success`,
    cancelUrl: `${state.base}/?purchase=cancelled`,
    createdAt: iso(state.now()),
  };
  if (physical) {
    // A destination charge on the seller's behalf: "we don't want the
    // platform absorbing physical-goods disputes" (7.11).
    const seller = byId(state.users, items[0].sellerId);
    session.onBehalfOf = seller.connect.account;
    session.applicationFeeCents = feeCents;
    session.shippingCountries = ['US'];
  } else {
    session.transferGroup = `tg_${id}`;
  }
  state.sessions.set(id, session);
  for (const item of items) {
    state.purchases.push({
      id: nextId(state, 'purchases'),
      userId: buyer.id,
      sellerId: item.sellerId,
      type: item.type,
      itemId: item.id,
      amountCents: item.priceCents,
      feeCents: Math.round(item.priceCents * FEE),
      status: 'pending',
      sessionId: id,
      createdAt: iso(state.now()),
      completedAt: null,
    });
  }
  return session;
}

// checkout.session.completed: the purchases complete, a garment is sold,
// and each digital seller is sent 90% of their items under the session's
// transfer group. False when the session can no longer be paid.
export function completeSession(state, id) {
  sweep(state);
  const session = state.sessions.get(id);
  if (!session || session.status !== 'open') return false;
  session.status = 'complete';
  const at = iso(state.now());
  const owed = new Map();
  for (const p of state.purchases.filter((x) => x.sessionId === id)) {
    p.status = 'completed';
    p.completedAt = at;
    if (p.type === 'fashion') {
      const listing = byId(state.listings, p.itemId);
      listing.status = 'sold';
      listing.hold = null;
    } else {
      owed.set(p.sellerId, (owed.get(p.sellerId) ?? 0) + p.amountCents - p.feeCents);
    }
  }
  for (const [sellerId, amountCents] of owed) {
    const destination = byId(state.users, sellerId).connect.account;
    state.transfers.push({
      id: nextId(state, 'transfers'),
      destination,
      amountCents,
      transferGroup: session.transferGroup,
      at,
    });
  }
  return true;
}

function money(cents) {
  return `$${(cents / 100).toFixed(2)}`;
}

function checkoutPage(ctx) {
  sweep(ctx.state);
  const session = ctx.state.sessions.get(ctx.params.id);
  if (!session) return html(404, page('Checkout', '<p>No checkout here.</p>'));
  if (session.status !== 'open') {
    const said = session.status === 'complete' ? 'This checkout is paid.' : 'This checkout has expired.';
    return html(410, page('Checkout', `<p>${said}</p>`));
  }
  const lines = session.lineItems
    .map(
      (l) => `<li>${escapeHtml(l.name)}: ${money(l.amountCents)}<br><small>${escapeHtml(l.description)}</small></li>`,
    )
    .join('');
  return html(
    200,
    page(
      'Checkout',
      `<ul>${lines}</ul><p>Total ${money(session.amountTotal)}</p>
<form method="post"><button>Pay ${money(session.amountTotal)}</button></form>`,
    ),
  );
}

function pay(ctx) {
  if (!completeSession(ctx.state, ctx.params.id)) {
    return html(410, page('Checkout', '<p>This checkout has expired.</p>'));
  }
  return redirect(ctx.state.sessions.get(ctx.params.id).successUrl);
}

// --- checkout, check, entitlements -------------------------------------------

function holdListing(state, me, item) {
  // One turn of JavaScript checks and holds, the mock's form of the
  // server's single UPDATE … WHERE status='active'.
  if (item.listing.status !== 'active') throw error(409, 'Just sold or being purchased');
  item.listing.status = 'reserved';
  item.listing.hold = { userId: me.id, until: state.now() + HOLD_MS };
}

function createCheckout(ctx) {
  const me = requireUser(ctx);
  limitCheckouts(ctx.state, me);
  return idempotent(ctx, me, 'checkout.purchase', false, () => {
    const { state } = ctx;
    const item = itemOf(state, ctx.body.type, ctx.body.id);
    checkBuyable(state, me, item);
    if (item.physical) holdListing(state, me, item);
    const session = openSession(state, me, [item], { physical: item.physical });
    return json(200, { url: session.url, sessionId: session.id });
  });
}

function check(ctx) {
  const me = requireUser(ctx);
  const type = ctx.query.type === 'fashion' ? 'fashion' : 'track';
  return json(200, { purchased: ownsPurchase(ctx.state, me, type, ctx.query.id) });
}

function versionsOf(track) {
  return track.versions.map((v) => ({ version: v.version, fileName: v.fileName, bytes: v.bytes, sha256: v.sha256 }));
}

// The files bought, each with every version: a purchase covers them all (6.12).
function entitlements(ctx) {
  const me = requireUser(ctx);
  const { state } = ctx;
  const items = state.purchases
    .filter((p) => p.userId === me.id && p.status === 'completed' && p.type === 'track')
    .map((p) => {
      const t = byId(state.tracks, p.itemId);
      return {
        kind: 'track',
        id: t.id,
        trackId: t.trackId,
        songId: t.songId,
        title: t.title,
        artist: t.artist,
        purchasedAt: p.completedAt,
        versions: versionsOf(t),
      };
    });
  return json(200, { items });
}

function entitlementFile(ctx) {
  const me = requireUser(ctx);
  const { state } = ctx;
  if (ctx.params.kind !== 'track') return error(404, 'No film there');
  const track = byId(state.tracks, ctx.params.id);
  if (!track) return error(404, 'Track not found');
  if (!ownsPurchase(state, me, 'track', track.id)) {
    return error(403, "You haven't bought this yet.", { code: 'not_entitled' });
  }
  const wanted = ctx.query.version === undefined ? null : Number(ctx.query.version);
  const v =
    wanted === null ? track.versions[track.versions.length - 1] : track.versions.find((x) => x.version === wanted);
  if (!v) return error(404, 'No such version');
  return json(200, {
    url: presign(state, 'GET', v.s3Key, { seconds: 60 }),
    expiresIn: 60,
    fileName: v.fileName,
    bytes: v.bytes,
    sha256: v.sha256,
    version: v.version,
  });
}

// --- payouts and prices -------------------------------------------------------

function onboard(ctx) {
  const me = requireUser(ctx);
  if (!me.connect.account) me.connect.account = `acct_${me.username.toLowerCase()}`;
  return json(200, { url: `${ctx.state.base}/__stripe/onboard/${me.connect.account}` });
}

function connectStatus(ctx) {
  const me = requireUser(ctx);
  const c = me.connect;
  if (!c.account) return json(200, { connected: false, onboarded: false, message: 'No Stripe account linked' });
  return json(200, {
    connected: true,
    onboarded: onboarded(me),
    chargesEnabled: c.chargesEnabled,
    payoutsEnabled: c.payoutsEnabled,
    detailsSubmitted: c.detailsSubmitted,
  });
}

function onboardingPage(ctx) {
  return html(
    200,
    page(
      'Set up payouts',
      '<p>Stripe Express, as far as the mock goes.</p><form method="post"><button>Finish</button></form>',
    ),
  );
}

function finishOnboarding(ctx) {
  const user = ctx.state.users.find((u) => u.connect.account === ctx.params.account);
  if (!user) return html(404, page('Set up payouts', '<p>No account here.</p>'));
  Object.assign(user.connect, { detailsSubmitted: true, chargesEnabled: true, payoutsEnabled: true });
  return html(200, page('Set up payouts', '<p>Payouts are set up. You can close this page.</p>'));
}

// Your booth opens when payouts are ready (7.12).
function setPrice(ctx) {
  const me = requireUser(ctx);
  const track = ctx.state.tracks.find((t) => t.trackId === ctx.params.trackId && !t.withdrawn);
  if (!track) return error(404, 'Track not found');
  if (track.uploaderId !== me.id) return error(403, 'Not authorized');
  const price = Number(ctx.body.price);
  if (!ctx.body.price || !Number.isFinite(price) || price <= 0) {
    track.isForSale = false;
    track.priceCents = null;
    return json(200, { success: true, isForSale: false });
  }
  if (Math.round(price * 100) > MAX_DIGITAL_CENTS) return error(400, 'Price cannot exceed $2000');
  if (!onboarded(me)) return error(400, 'Selling needs a payout account. Set it up once.');
  track.priceCents = Math.round(price * 100);
  track.isForSale = true;
  return json(200, { success: true, isForSale: true, price });
}

// --- the floor -------------------------------------------------------------------

const HALLS = [
  { medium: 'music', family: 'Mi' },
  { medium: 'writing', family: 'Ri' },
  { medium: 'film', family: 'Si' },
  { medium: 'fashion', family: 'Gi' },
];

const CONDITIONS = { new: 'new', like_new: 'like new', good: 'good', fair: 'fair', worn: 'worn' };

function seller(state, id) {
  const user = byId(state.users, id);
  return { id: user.id, username: user.username };
}

// An item as the floor, List view and the bag show it: what it is and what
// it costs, never how it sold.
function itemPayload(state, viewer, item) {
  if (item.physical) {
    const l = item.listing;
    const from = byId(state.users, l.sellerId).shipFrom;
    return {
      type: 'fashion',
      id: l.id,
      medium: 'fashion',
      title: l.title,
      seller: seller(state, l.sellerId),
      priceCents: l.priceCents,
      size: l.size,
      condition: CONDITIONS[l.condition],
      category: l.category,
      measurements: l.measurements,
      material: l.material,
      brand: l.brand,
      color: l.color,
      photos: l.photos,
      oneOfOne: true,
      shipsFrom: `${from.city}, ${from.state}`,
      // Shipping is US-only for now, and every tag says so (7.20).
      shipsTo: 'US',
      status: l.status,
      hold: l.hold ? { until: iso(l.hold.until), yours: Boolean(viewer && l.hold.userId === viewer.id) } : null,
      createdAt: l.createdAt,
    };
  }
  const t = item.track;
  const parent = state.tracks.find((p) => p.trackId === t.parentTrackId);
  return {
    type: 'track',
    id: t.id,
    medium: 'music',
    title: t.title,
    artist: t.artist,
    seller: seller(state, t.uploaderId),
    priceCents: t.priceCents,
    gets: GETS,
    key: t.musicalKey,
    bpm: t.bpm,
    durationSeconds: t.duration,
    provenance: { generation: t.remixDepth, parent: parent ? { title: parent.title, artist: parent.artist } : null },
    yours: Boolean(viewer && ownsPurchase(state, viewer, 'track', t.id)),
    createdAt: t.createdAt,
  };
}

function forSale(state) {
  const tracks = state.tracks
    .filter((t) => t.isForSale && !t.withdrawn && onboarded(byId(state.users, t.uploaderId)))
    .map((t) => itemOf(state, 'track', t.id));
  const goods = state.listings.filter((l) => l.status !== 'sold').map((l) => itemOf(state, 'fashion', l.id));
  return [...tracks, ...goods];
}

// Shelf order (7.13): newest, artist (List view), lineage family (by root,
// oldest ancestor first, generations in order) or alphabet. Never by sales.
function ordered(state, items, order) {
  const at = (i) => (i.physical ? i.listing.createdAt : i.track.createdAt);
  const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
  const newest = (a, b) => cmp(at(b), at(a)) || b.id - a.id;
  if (order === 'alphabet') return [...items].sort((a, b) => a.title.localeCompare(b.title, 'en') || newest(a, b));
  if (order === 'artist') {
    const who = (i) => byId(state.users, i.sellerId).username;
    return [...items].sort((a, b) => who(a).localeCompare(who(b), 'en') || newest(a, b));
  }
  if (order === 'lineage') {
    const root = (i) => (i.physical ? null : state.tracks.find((t) => t.trackId === i.track.lineageRootTrackId));
    const key = (i) => [root(i)?.createdAt ?? at(i), i.physical ? 0 : i.track.remixDepth, at(i)];
    return [...items].sort((a, b) => {
      const [x, y] = [key(a), key(b)];
      return cmp(x[0], y[0]) || x[1] - y[1] || cmp(x[2], y[2]);
    });
  }
  return [...items].sort(newest);
}

// Track n of a hall has a semi-major axis of 5 + 3.2n m and holds as many
// 3.6 m booth spacings as fit around it (7.3).
const axis = (n) => 5 + 3.2 * n;
const capacity = (n) => Math.floor((2 * Math.PI * axis(n)) / 3.6);
const GOLDEN = Math.PI * (3 - Math.sqrt(5));
const STORE_EPOCH = Date.parse('2026-10-01T00:00:00Z');
const DAY_MS = 24 * 60 * 60 * 1000;

// Each shop's seat for the day (7.3): the first track with room when it
// opened, the golden angle times its place on that track plus an FNV-1a
// jitter of up to half a booth, then one day's step per day by Kepler's
// third law (the inner track turns once in 28 days). The same for everyone
// all day, and computed here so no two engines' Math.sin disagree. Soft
// repulsion and lineage drift aren't modelled.
function seatShops(shops, day) {
  let track = 0;
  let used = 0;
  for (const shop of shops) {
    while (used >= capacity(track)) {
      track += 1;
      used = 0;
    }
    const spacing = (2 * Math.PI) / capacity(track);
    const start = used * GOLDEN + unitJitter(shop.id, 'booth') * spacing * 0.5;
    const periodDays = 28 * (axis(track) / axis(0)) ** 1.5;
    const phase = (start + (2 * Math.PI * day) / periodDays) % (2 * Math.PI);
    const eccentricity = 0.04 + 0.11 * unitJitter(String(track), 'track');
    shop.seat = { track, phase: Math.round(phase * 1e6) / 1e6, eccentricity: Math.round(eccentricity * 1e4) / 1e4 };
    used += 1;
  }
}

function halls(ctx) {
  const { state } = ctx;
  const viewer = optionalUser(ctx);
  const day = Math.floor((state.now() - STORE_EPOCH) / DAY_MS);
  const items = forSale(state);
  return json(200, {
    asOf: iso(STORE_EPOCH + day * DAY_MS).slice(0, 10),
    halls: HALLS.map(({ medium, family }) => {
      const here = items.filter((i) => (i.physical ? 'fashion' : 'music') === medium);
      const sellers = [...new Set(here.map((i) => i.sellerId))];
      const shops = sellers
        .map((sellerId) => {
          const theirs = here.filter((i) => i.sellerId === sellerId);
          const openedAt = theirs.map((i) => (i.physical ? i.listing.createdAt : i.track.createdAt)).sort()[0];
          return { id: `${family.toLowerCase()}-${sellerId}`, seller: seller(state, sellerId), openedAt, theirs };
        })
        .sort((a, b) => (a.openedAt < b.openedAt ? -1 : a.openedAt > b.openedAt ? 1 : a.seller.id - b.seller.id));
      seatShops(shops, day);
      return {
        medium,
        family,
        shops: shops.map(({ id, seller: who, openedAt, seat, theirs }) => ({
          id,
          seller: who,
          openedAt,
          seat,
          crates: crates(state, theirs, ctx.query.order).map((c) => ({
            ...c,
            items: c.items.map((i) => itemPayload(state, viewer, i)),
          })),
        })),
      };
    }),
  });
}

// One crate is one solar system (7.6); a rail of goods is one crate.
function crates(state, items, order) {
  const groups = new Map();
  for (const item of items) {
    const planet = item.physical ? null : state.planets.find((p) => p.publishedTrackId === item.id);
    const system = planet ? byId(state.systems, planet.systemId) : null;
    const key = item.physical ? 'rail' : (system?.id ?? 'loose');
    if (!groups.has(key)) {
      groups.set(key, { id: key, title: item.physical ? 'Rail' : (system?.title ?? 'Singles'), items: [] });
    }
    groups.get(key).items.push(item);
  }
  return [...groups.values()].map((g) => ({ ...g, items: ordered(state, g.items, order) }));
}

// The New table: the 12 newest records from the whole store (7.13).
function newTable(ctx) {
  const viewer = optionalUser(ctx);
  const items = ordered(ctx.state, forSale(ctx.state), 'newest').slice(0, 12);
  return json(200, { items: items.map((i) => itemPayload(ctx.state, viewer, i)) });
}

// --- the bag ---------------------------------------------------------------------

// How a bag splits into charges: every file in one, each seller's goods in
// their own (7.11).
function payments(state, items) {
  const out = [];
  const files = items.filter((i) => !i.physical);
  if (files.length) out.push({ kind: 'digital', items: files });
  for (const sellerId of [...new Set(items.filter((i) => i.physical).map((i) => i.sellerId))]) {
    out.push({
      kind: 'physical',
      seller: seller(state, sellerId),
      items: items.filter((i) => i.physical && i.sellerId === sellerId),
    });
  }
  return out;
}

function bagOf(state, me) {
  if (!state.bags.has(me.id)) state.bags.set(me.id, []);
  return state.bags.get(me.id);
}

function bagAnswer(state, me) {
  const items = bagOf(state, me).map((b) => itemOf(state, b.type, b.id));
  return json(200, {
    items: items.map((i) => itemPayload(state, me, i)),
    payments: payments(state, items).map((p) => ({
      ...p,
      items: p.items.map((i) => ({ type: i.type, id: i.id })),
      amountCents: p.items.reduce((n, i) => n + i.priceCents, 0),
    })),
    totalCents: items.reduce((n, i) => n + i.priceCents, 0),
  });
}

function getBag(ctx) {
  const me = requireUser(ctx);
  return bagAnswer(ctx.state, me);
}

function addToBag(ctx) {
  const me = requireUser(ctx);
  const item = itemOf(ctx.state, ctx.body.type, ctx.body.id);
  checkBuyable(ctx.state, me, item);
  const bag = bagOf(ctx.state, me);
  if (!bag.some((b) => b.type === item.type && b.id === item.id)) bag.push({ type: item.type, id: item.id });
  return bagAnswer(ctx.state, me);
}

function removeFromBag(ctx) {
  const me = requireUser(ctx);
  const type = ctx.params.type === 'fashion' ? 'fashion' : 'track';
  ctx.state.bags.set(
    me.id,
    bagOf(ctx.state, me).filter((b) => !(b.type === type && b.id === Number(ctx.params.id))),
  );
  return bagAnswer(ctx.state, me);
}

// Every hold is taken before any session opens; if one garment is gone,
// the holds this checkout took are let go and nothing is charged.
function checkoutBag(ctx) {
  const me = requireUser(ctx);
  const { state } = ctx;
  limitCheckouts(state, me);
  const items = bagOf(state, me).map((b) => itemOf(state, b.type, b.id));
  if (!items.length) return error(400, 'Your bag is empty');
  for (const item of items) checkBuyable(state, me, item);
  const held = [];
  try {
    for (const item of items.filter((i) => i.physical)) {
      holdListing(state, me, item);
      held.push(item.listing);
    }
  } catch (refusal) {
    for (const listing of held) Object.assign(listing, { status: 'active', hold: null });
    throw refusal;
  }
  const out = payments(state, items).map((p) => {
    const session = openSession(state, me, p.items, { physical: p.kind === 'physical' });
    return {
      kind: p.kind,
      ...(p.seller ? { seller: p.seller } : {}),
      items: p.items.map((i) => ({ type: i.type, id: i.id })),
      amountCents: session.amountTotal,
      sessionId: session.id,
      url: session.url,
    };
  });
  state.bags.set(me.id, []);
  return json(200, { payments: out });
}

export const routes = [
  ['POST', '/api/purchase/create-checkout', createCheckout],
  ['GET', '/api/purchase/check', check],
  ['GET', '/api/entitlements', entitlements],
  ['GET', '/api/entitlements/:kind/:id/file', entitlementFile],
  ['PUT', '/api/tracks/:trackId/set-price', setPrice],
  ['POST', '/api/connect/onboard', onboard],
  ['GET', '/api/connect/status', connectStatus],
  ['GET', '/api/store/halls', halls],
  ['GET', '/api/store/new', newTable],
  ['GET', '/api/store/bag', getBag],
  ['PUT', '/api/store/bag', addToBag],
  ['DELETE', '/api/store/bag/:type/:id', removeFromBag],
  ['POST', '/api/store/bag/checkout', checkoutBag],
  ['GET', '/__stripe/checkout/:id', checkoutPage],
  ['POST', '/__stripe/checkout/:id', pay],
  ['GET', '/__stripe/onboard/:account', onboardingPage],
  ['POST', '/__stripe/onboard/:account', finishOnboarding],
];
