// Space over /api/v2 (docs/SPEC.md 4), in the server's {ok, data} envelope
// and shapes (server.md §4), with what the gates take away taken away:
// no viewCount anywhere, no fog, fuel or gravity wells in the universe, and
// travel is a free camera move (4.11, 4.12; mismatch 11). "Add galaxy" is a
// saved kind, and /since and Newest's filters are new (2.10, 4.10).
import { error, fail, isObject, ok } from '../http.js';
import { optionalUser, requireUser } from '../auth.js';
import { addSun, galaxyPosition } from '../fixtures.js';
import { idempotent, iso, nextId } from '../state.js';

const PLANET_CAP = 21;
const LINK_KINDS = new Set(['influence', 'sample', 'collab', 'cover', 'custom']);
const LINK_TYPES = new Set(['planet', 'sun', 'galaxy']);

// --- lookups -------------------------------------------------------------------

const byId = (rows, id) => rows.find((r) => r.id === Number(id));

function systemOwner(state, system) {
  return byId(state.galaxies, system.galaxyId).userId;
}

function trackOfPlanet(state, planet) {
  return byId(state.tracks, planet.publishedTrackId);
}

function visible(state, system, viewer) {
  return system.status === 'published' || (viewer && systemOwner(state, system) === viewer.id);
}

function slugify(text, fallback) {
  const slug = String(text)
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 60);
  return slug || fallback;
}

function uniqueSlug(base, taken) {
  let slug = base;
  for (let n = 2; taken(slug); n++) slug = `${base}-${n}`;
  return slug;
}

export function ownsPurchase(state, user, type, itemId) {
  return state.purchases.some(
    (p) => p.userId === user.id && p.type === type && p.itemId === Number(itemId) && p.status === 'completed',
  );
}

// --- payloads ------------------------------------------------------------------

function sunSummary(sun) {
  return sun
    ? { id: sun.id, title: sun.title, blockCount: sun.blocks.blocks.length, appearance: sun.appearance }
    : null;
}

function sunDocument(sun) {
  return { id: sun.id, kind: sun.kind, title: sun.title, blocks: sun.blocks, appearance: sun.appearance };
}

const galaxyBrief = (g) => ({ id: g.id, slug: g.slug, displayName: g.displayName });

// A world as the scene draws it. priceCents is for the "$4 · on the shelf"
// tag and `yours` for a work the viewer bought (4.13).
function planetPayload(state, planet, viewer) {
  const track = trackOfPlanet(state, planet);
  return {
    id: planet.id,
    kind: planet.kind,
    orbitIndex: planet.orbitIndex,
    orbitRadius: planet.orbitRadius,
    phaseOffset: planet.phaseOffset,
    appearance: planet.appearance,
    trackId: planet.trackId,
    publishedId: track.id,
    title: track.title,
    artist: track.artist,
    coverArtUrl: track.coverArtUrl,
    duration: track.duration,
    bpm: track.bpm,
    key: track.musicalKey,
    priceCents: track.isForSale ? track.priceCents : null,
    yours: Boolean(viewer && ownsPurchase(state, viewer, 'track', track.id)),
  };
}

function systemSummary(state, s) {
  return {
    id: s.id,
    slug: s.slug,
    title: s.title,
    orbitIndex: s.orbitIndex,
    colorSeed: s.colorSeed,
    posX: s.posX,
    posY: s.posY,
    status: s.status,
    planetCount: state.planets.filter((p) => p.systemId === s.id).length,
    sun: sunSummary(state.suns.find((x) => x.systemId === s.id)),
  };
}

function bySlug(state, slug) {
  const galaxy = state.galaxies.find((g) => g.slug === slug);
  if (!galaxy) throw fail(404, 'not_found', 'No galaxy by that name');
  return galaxy;
}

function systemBySlug(state, galaxy, slug, viewer) {
  const system = state.systems.find((s) => s.galaxyId === galaxy.id && s.slug === slug);
  if (!system || (viewer !== undefined && !visible(state, system, viewer))) {
    throw fail(404, 'not_found', 'No system by that name');
  }
  return system;
}

function ownedSystem(ctx, user) {
  const system = byId(ctx.state.systems, ctx.params.id);
  if (!system) throw fail(404, 'not_found', 'No system there');
  if (systemOwner(ctx.state, system) !== user.id) throw fail(403, 'not_yours', 'That system belongs to someone else');
  return system;
}

function ownedPlanet(ctx, user) {
  const planet = byId(ctx.state.planets, ctx.params.id);
  if (!planet) throw fail(404, 'not_found', 'No planet there');
  if (systemOwner(ctx.state, byId(ctx.state.systems, planet.systemId)) !== user.id) {
    throw fail(403, 'not_yours', 'That planet belongs to someone else');
  }
  return planet;
}

// --- galaxies and systems --------------------------------------------------------

function claim(ctx) {
  const me = requireUser(ctx);
  const { state } = ctx;
  const existing = state.galaxies.find((g) => g.userId === me.id);
  if (existing) return ok({ galaxy: galaxyBrief(existing), claimed: false });
  const id = nextId(state, 'galaxies');
  const base = slugify(ctx.body.slug || me.username, `galaxy-${me.id}`);
  const galaxy = {
    id,
    userId: me.id,
    slug: uniqueSlug(base, (s) => state.galaxies.some((g) => g.slug === s)),
    displayName: ctx.body.displayName || me.username,
    skySeed: `galaxy-${me.id}`,
    createdAt: iso(state.now()),
  };
  Object.assign(galaxy, galaxyPosition(id, galaxy.skySeed));
  state.galaxies.push(galaxy);
  addSun(state, { kind: 'galaxy', galaxyId: id, title: galaxy.displayName, createdAt: galaxy.createdAt });
  return ok({ galaxy: galaxyBrief(galaxy), claimed: true });
}

function mine(ctx) {
  const me = requireUser(ctx);
  const { state } = ctx;
  const galaxy = state.galaxies.find((g) => g.userId === me.id);
  if (!galaxy) return fail(404, 'no_galaxy', 'You have no galaxy yet');
  const systems = state.systems
    .filter((s) => s.galaxyId === galaxy.id)
    .sort((a, b) => a.orbitIndex - b.orbitIndex)
    .map((s) => ({
      id: s.id,
      slug: s.slug,
      title: s.title,
      status: s.status,
      sun: sunSummary(state.suns.find((x) => x.systemId === s.id)),
    }));
  return ok({
    galaxy: galaxyBrief(galaxy),
    sun: sunSummary(state.suns.find((x) => x.galaxyId === galaxy.id)),
    systems,
  });
}

function galaxyScene(ctx) {
  const viewer = optionalUser(ctx);
  const { state } = ctx;
  const galaxy = bySlug(state, ctx.params.slug);
  const user = byId(state.users, galaxy.userId);
  const systems = state.systems
    .filter((s) => s.galaxyId === galaxy.id && visible(state, s, viewer))
    .sort((a, b) => a.orbitIndex - b.orbitIndex);
  return ok({
    galaxy: {
      ...galaxyBrief(galaxy),
      skySeed: galaxy.skySeed,
      user: {
        id: user.id,
        username: user.username,
        profilePicture: user.profilePicture,
        rocket: user.astronaut?.rocket ?? null,
      },
    },
    sun: sunSummary(state.suns.find((x) => x.galaxyId === galaxy.id)),
    systems: systems.map((s) => systemSummary(state, s)),
  });
}

function galaxySun(ctx) {
  optionalUser(ctx);
  const galaxy = bySlug(ctx.state, ctx.params.slug);
  const sun = ctx.state.suns.find((s) => s.galaxyId === galaxy.id);
  if (!sun) return fail(404, 'no_sun', 'This galaxy has no bio yet');
  return ok({ sun: sunDocument(sun), galaxy: galaxyBrief(galaxy), system: null });
}

// A bio sun's letters (4.8): newest first, ten at a time, then "Show
// older" with nextCursor, until null ("That's everything."). New: the
// server has no such route (mismatch 13: GET /api/devlog answers every
// post at once, as DevlogPost blocks with no greeting or sign-off).
const LETTERS_PAGE = 10;

function letters(ctx) {
  optionalUser(ctx);
  const { state } = ctx;
  const galaxy = bySlug(state, ctx.params.slug);
  const cursor = parseCursor(ctx.query.cursor, 2);
  const rows = state.letters
    .filter((l) => l.userId === galaxy.userId)
    .sort(newestFirst)
    .filter(before(cursor));
  const page = rows.slice(0, LETTERS_PAGE);
  const last = page[page.length - 1];
  return ok({
    letters: page.map(({ id, greeting, blocks, signoff, sentAt, createdAt }) => ({
      id,
      greeting,
      blocks,
      signoff,
      sentAt,
      createdAt,
    })),
    nextCursor: rows.length > page.length ? `${last.createdAt}|${last.id}` : null,
  });
}

function createSystem(ctx) {
  const me = requireUser(ctx);
  const { state } = ctx;
  const galaxy = byId(state.galaxies, ctx.params.id);
  if (!galaxy) return fail(404, 'not_found', 'No galaxy there');
  if (galaxy.userId !== me.id) return fail(403, 'not_yours', 'That galaxy belongs to someone else');
  const title = String(ctx.body.title || '')
    .trim()
    .slice(0, 200);
  if (!title) return fail(400, 'no_title', 'A project needs a name');
  const mine = state.systems.filter((s) => s.galaxyId === galaxy.id);
  const system = {
    id: nextId(state, 'systems'),
    galaxyId: galaxy.id,
    slug: uniqueSlug(slugify(ctx.body.slug || title, 'project'), (s) => mine.some((x) => x.slug === s)),
    title,
    status: ctx.body.status === 'published' ? 'published' : 'draft',
    orbitIndex: mine.length,
    colorSeed: null,
    posX: null,
    posY: null,
    createdAt: iso(state.now()),
  };
  state.systems.push(system);
  addSun(state, { kind: 'system', systemId: system.id, title, createdAt: system.createdAt });
  const { id, slug, status, orbitIndex } = system;
  return ok({ system: { id, slug, title, status, orbitIndex } });
}

function systemScene(ctx) {
  const viewer = optionalUser(ctx);
  const { state } = ctx;
  const galaxy = bySlug(state, ctx.params.slug);
  const system = systemBySlug(state, galaxy, ctx.params.system, viewer);
  const planets = state.planets
    .filter((p) => p.systemId === system.id)
    .sort((a, b) => a.orbitIndex - b.orbitIndex || (a.createdAt < b.createdAt ? -1 : 1));
  const { id, slug, title, orbitIndex, colorSeed, posX, posY, status } = system;
  return ok({
    galaxy: galaxyBrief(galaxy),
    system: { id, slug, title, orbitIndex, colorSeed, posX, posY, status },
    sun: sunSummary(state.suns.find((s) => s.systemId === system.id)),
    planets: planets.map((p) => planetPayload(state, p, viewer)),
  });
}

function systemSun(ctx) {
  optionalUser(ctx);
  const galaxy = bySlug(ctx.state, ctx.params.slug);
  const system = systemBySlug(ctx.state, galaxy, ctx.params.system);
  const sun = ctx.state.suns.find((s) => s.systemId === system.id);
  if (!sun) return fail(404, 'no_sun', 'This project has no description yet');
  return ok({
    sun: sunDocument(sun),
    galaxy: galaxyBrief(galaxy),
    system: { id: system.id, slug: system.slug, title: system.title },
  });
}

function patchSystem(ctx) {
  const system = ownedSystem(ctx, requireUser(ctx));
  const b = ctx.body;
  if (b.status !== undefined && b.status !== 'draft' && b.status !== 'published') {
    return fail(400, 'bad_status', 'A system is draft or published');
  }
  for (const field of ['title', 'colorSeed', 'status']) if (b[field] !== undefined) system[field] = b[field];
  if (b.orbitIndex !== undefined) system.orbitIndex = Number(b.orbitIndex) || 0;
  // Both or neither: half a coordinate is not a place.
  if (b.posX !== undefined || b.posY !== undefined) {
    if (b.posX === null && b.posY === null) {
      system.posX = null;
      system.posY = null;
    } else {
      const [x, y] = [Number(b.posX), Number(b.posY)];
      const inside = (v) => Number.isFinite(v) && v >= 0 && v <= 5000;
      if (!inside(x) || !inside(y)) return fail(400, 'bad_position', 'That is not a place in this galaxy');
      system.posX = x;
      system.posY = y;
    }
  }
  if (typeof b.slug === 'string' && b.slug.trim()) {
    const base = slugify(b.slug, '');
    if (!base) return fail(400, 'bad_slug', 'That name leaves nothing to put in a URL');
    const others = ctx.state.systems.filter((s) => s.galaxyId === system.galaxyId && s !== system);
    system.slug = uniqueSlug(base, (s) => others.some((x) => x.slug === s));
  }
  const { id, slug, title, status, orbitIndex, posX, posY } = system;
  return ok({ system: { id, slug, title, status, orbitIndex, posX, posY } });
}

// The system and its sun go; the works it held stay published.
function deleteSystem(ctx) {
  const system = ownedSystem(ctx, requireUser(ctx));
  const { state } = ctx;
  state.planets = state.planets.filter((p) => p.systemId !== system.id);
  state.suns = state.suns.filter((s) => s.systemId !== system.id);
  state.systems = state.systems.filter((s) => s !== system);
  return ok({ deleted: true });
}

// "21 worlds to a system, the length of the first record" (4.2). The count
// and the insert happen in one turn of JavaScript, so two adds at once
// can't make 22 (mismatch 16: the server counts, then creates, unlocked).
function addPlanet(ctx) {
  const me = requireUser(ctx);
  const system = ownedSystem(ctx, me);
  const { state } = ctx;
  const count = state.planets.filter((p) => p.systemId === system.id).length;
  if (count >= PLANET_CAP) {
    return fail(409, 'system_full', `A solar system holds ${PLANET_CAP} worlds. Start another one.`);
  }
  // The mock holds songs only; it has no films or galleries to place.
  if (ctx.body.kind === 'film') return fail(404, 'no_film', 'No film by that id');
  if (ctx.body.kind === 'gallery') return fail(404, 'no_gallery', 'No gallery by that id');
  const track = state.tracks.find((t) => t.trackId === String(ctx.body.trackId || '') && !t.withdrawn);
  if (!track) return fail(404, 'no_track', 'No track by that id');
  if (track.uploaderId !== me.id) return fail(403, 'not_yours', 'That track belongs to someone else');
  if (state.planets.some((p) => p.publishedTrackId === track.id)) {
    return fail(409, 'already_placed', 'That song is already a planet somewhere');
  }
  const planet = {
    id: nextId(state, 'planets'),
    systemId: system.id,
    kind: 'song',
    trackId: track.trackId,
    publishedTrackId: track.id,
    orbitIndex: count,
    orbitRadius: null,
    phaseOffset: null,
    appearance: null,
    createdAt: iso(state.now()),
  };
  state.planets.push(planet);
  return ok({
    planet: { id: planet.id, kind: 'song', trackId: track.trackId, filmId: null, galleryId: null, orbitIndex: count },
    remaining: PLANET_CAP - (count + 1),
  });
}

function patchPlanet(ctx) {
  const planet = ownedPlanet(ctx, requireUser(ctx));
  const b = ctx.body;
  if (b.orbitIndex !== undefined) {
    const index = Number(b.orbitIndex);
    if (!Number.isInteger(index) || index < 0 || index > 200) {
      return fail(400, 'bad_orbit', 'That is not a place in an orbit');
    }
    planet.orbitIndex = index;
  }
  if (b.orbitRadius !== undefined) {
    const r = Number(b.orbitRadius);
    if (b.orbitRadius !== null && (!Number.isFinite(r) || r < 400 || r > 1800)) {
      return fail(400, 'bad_radius', 'An orbit fits between 400 and 1800');
    }
    planet.orbitRadius = b.orbitRadius === null ? null : r;
  }
  if (b.phaseOffset !== undefined) {
    const p = Number(b.phaseOffset);
    if (b.phaseOffset !== null && (!Number.isFinite(p) || p < 0 || p > Math.PI * 2)) {
      return fail(400, 'bad_phase', 'A phase is 0 to 2π');
    }
    planet.phaseOffset = b.phaseOffset === null ? null : p;
  }
  const { id, orbitIndex, orbitRadius, phaseOffset, appearance } = planet;
  return ok({ planet: { id, orbitIndex, orbitRadius, phaseOffset, appearance } });
}

// The world leaves the system; the work stays published.
function deletePlanet(ctx) {
  const planet = ownedPlanet(ctx, requireUser(ctx));
  ctx.state.planets = ctx.state.planets.filter((p) => p !== planet);
  return ok({ deleted: true });
}

// --- suns: blocks v1 -------------------------------------------------------------

const MAX_BLOCKS = 200;
const MAX_TEXT = 20000;
const MAX_FIELDS = 40;
const TEXT_STYLES = new Set(['heading', 'body', 'quote']);
const BLOCK_ID = /^[A-Za-z0-9_-]{1,64}$/;

const str = (v, max) => (typeof v === 'string' ? v.slice(0, max) : '');

// v4's codec (routes/v2/blocks.js), changed as QUESTIONS #73 says: a block
// keeps keys the server doesn't know (a quote's source, a photo's width),
// and a block of a type it doesn't know is kept too, so a newer client's
// page isn't flattened (mismatch 12). Still: every block needs an id, text
// is capped, and a photo's key is rebuilt, never trusted.
function sanitizeBlock(raw, sunId) {
  if (!isObject(raw) || !BLOCK_ID.test(str(raw.id, 64))) return null;
  const block = { ...raw };
  if (raw.type === 'text') {
    block.text = str(raw.text, MAX_TEXT);
    if (!block.text.trim()) return null;
    block.style = TEXT_STYLES.has(raw.style) ? raw.style : 'body';
  } else if (raw.type === 'photo') {
    block.imageKey = `suns/${sunId}/${raw.id}.jpg`;
    block.alt = str(raw.alt, 300);
    const aspect = Number(raw.aspect);
    if (Number.isFinite(aspect) && aspect > 0.05 && aspect < 20) block.aspect = aspect;
    else delete block.aspect;
  } else if (raw.type === 'details') {
    if (!Array.isArray(raw.fields)) return null;
    block.fields = raw.fields
      .filter(isObject)
      .slice(0, MAX_FIELDS)
      .map((f) => ({ label: str(f.label, 120), value: str(f.value, 600) }))
      .filter((f) => f.label || f.value);
    if (!block.fields.length) return null;
  }
  return block;
}

function sanitizeBlocks(doc, sunId) {
  if (!isObject(doc)) throw new Error('A blocks document is expected');
  if (!Array.isArray(doc.blocks)) throw new Error('A blocks document needs a blocks array');
  if (doc.blocks.length > MAX_BLOCKS) throw new Error(`A sun holds at most ${MAX_BLOCKS} blocks`);
  const seen = new Set();
  const blocks = [];
  for (const raw of doc.blocks) {
    const block = sanitizeBlock(raw, sunId);
    if (!block || seen.has(block.id)) continue;
    seen.add(block.id);
    blocks.push(block);
  }
  const { blocks: _blocks, v: _v, ...rest } = doc;
  return { ...rest, v: 1, blocks };
}

function sunOwner(state, sun) {
  const galaxyId = sun.galaxyId ?? byId(state.systems, sun.systemId)?.galaxyId;
  return byId(state.galaxies, galaxyId)?.userId ?? null;
}

function sunAnswer(state, sun) {
  const system = sun.systemId ? byId(state.systems, sun.systemId) : null;
  const galaxy = byId(state.galaxies, sun.galaxyId ?? system.galaxyId);
  return ok({
    sun: sunDocument(sun),
    galaxy: galaxyBrief(galaxy),
    system: system ? { id: system.id, slug: system.slug, title: system.title } : null,
  });
}

function getSun(ctx) {
  optionalUser(ctx);
  const sun = byId(ctx.state.suns, ctx.params.id);
  if (!sun) return fail(404, 'not_found', 'No sun there');
  return sunAnswer(ctx.state, sun);
}

function putBlocks(ctx) {
  const me = requireUser(ctx);
  const sun = byId(ctx.state.suns, ctx.params.id);
  if (!sun) return fail(404, 'not_found', 'No sun there');
  if (sunOwner(ctx.state, sun) !== me.id) return fail(403, 'not_yours', 'That sun belongs to someone else');
  try {
    sun.blocks = sanitizeBlocks(ctx.body.blocks, sun.id);
  } catch (e) {
    return fail(400, 'bad_blocks', e.message);
  }
  if (typeof ctx.body.title === 'string') sun.title = ctx.body.title.slice(0, 200);
  return sunAnswer(ctx.state, sun);
}

// --- lineage links: claims wait for consent ----------------------------------

function endpointOwner(state, { type, id }) {
  if (type === 'galaxy') return byId(state.galaxies, id)?.userId;
  if (type === 'planet') {
    const planet = byId(state.planets, id);
    return planet ? systemOwner(state, byId(state.systems, planet.systemId)) : undefined;
  }
  const sun = byId(state.suns, id);
  return sun ? sunOwner(state, sun) : undefined;
}

function linkPayload(state, link) {
  const by = byId(state.users, link.declaredById);
  return {
    id: link.id,
    from: link.from,
    to: link.to,
    kind: link.kind,
    note: link.note,
    status: link.status,
    declaredById: link.declaredById,
    declaredBy: { id: by.id, username: by.username },
    createdAt: link.createdAt,
  };
}

const touches = (state, link, user) =>
  endpointOwner(state, link.from) === user.id || endpointOwner(state, link.to) === user.id;

function declareLink(ctx) {
  const me = requireUser(ctx);
  const { state, body } = ctx;
  const from = { type: String(body.from?.type || ''), id: Number(body.from?.id) };
  const to = { type: String(body.to?.type || ''), id: Number(body.to?.id) };
  const kind = LINK_KINDS.has(body.kind) ? body.kind : 'influence';
  if (!LINK_TYPES.has(from.type) || !LINK_TYPES.has(to.type) || !from.id || !to.id) {
    return fail(400, 'bad_endpoints', 'A link needs two things to join');
  }
  if (from.type === to.type && from.id === to.id) return fail(400, 'self_link', 'A thing cannot descend from itself');
  const [fromOwner, toOwner] = [endpointOwner(state, from), endpointOwner(state, to)];
  if (fromOwner === undefined || toOwner === undefined) return fail(404, 'no_endpoint', 'One end does not exist');
  if (fromOwner !== me.id && toOwner !== me.id) {
    return fail(403, 'not_yours', 'A link has to touch something of yours');
  }
  const same = (a, b) => a.type === b.type && a.id === b.id;
  if (state.links.some((l) => same(l.from, from) && same(l.to, to) && l.kind === kind)) {
    return fail(409, 'already_linked', 'That line is already drawn');
  }
  const link = {
    id: nextId(state, 'links'),
    from,
    to,
    kind,
    note: typeof body.note === 'string' ? body.note.slice(0, 280) : null,
    // Wholly yours: nothing to agree to. Otherwise it waits.
    status: fromOwner === me.id && toOwner === me.id ? 'accepted' : 'pending',
    declaredById: me.id,
    createdAt: iso(state.now()),
  };
  state.links.push(link);
  return ok({ link: linkPayload(state, link) });
}

function pendingLinks(ctx) {
  const me = requireUser(ctx);
  const links = ctx.state.links
    .filter((l) => l.status === 'pending' && l.declaredById !== me.id && touches(ctx.state, l, me))
    .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1))
    .slice(0, 100);
  return ok({ links: links.map((l) => linkPayload(ctx.state, l)) });
}

function acceptLink(ctx) {
  const me = requireUser(ctx);
  const link = byId(ctx.state.links, ctx.params.id);
  if (!link) return fail(404, 'not_found', 'No link there');
  if (link.declaredById === me.id) return fail(403, 'your_own', 'You cannot accept your own claim');
  if (!touches(ctx.state, link, me)) return fail(403, 'not_yours', 'That link does not touch anything of yours');
  link.status = 'accepted';
  return ok({ link: linkPayload(ctx.state, link) });
}

// Retract your own claim, or Refuse one on your work.
function deleteLink(ctx) {
  const me = requireUser(ctx);
  const link = byId(ctx.state.links, ctx.params.id);
  if (!link) return fail(404, 'not_found', 'No link there');
  if (link.declaredById !== me.id && !touches(ctx.state, link, me)) {
    return fail(403, 'not_yours', 'That link does not touch anything of yours');
  }
  ctx.state.links = ctx.state.links.filter((l) => l !== link);
  return ok({ deleted: true });
}

// Accepted links only: a claim on someone else's work shows nowhere until
// they agree (4.9).
function listLinks(ctx) {
  optionalUser(ctx);
  const target = { type: ctx.query.type, id: Number(ctx.query.id) };
  if (!LINK_TYPES.has(target.type) || !target.id) return fail(400, 'bad_target', 'Ask about one thing');
  const at = (e) => e.type === target.type && e.id === target.id;
  const links = ctx.state.links
    .filter((l) => l.status === 'accepted' && (at(l.from) || at(l.to)))
    .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1))
    .slice(0, 200);
  return ok({ links: links.map((l) => linkPayload(ctx.state, l)) });
}

// --- saved: private, never counted, never told ------------------------------

const SAVE_TARGETS = { song: 'publishedTrackId', system: 'systemId', galaxy: 'galaxyId', film: 'filmId' };

function saveTarget(ctx, refusal) {
  const kind = ctx.body.kind;
  const id = Number(ctx.body[SAVE_TARGETS[kind]]);
  if (!SAVE_TARGETS[kind] || !id) throw fail(400, 'bad_target', refusal);
  const { state } = ctx;
  if (kind === 'song' && !state.tracks.some((t) => t.id === id && !t.withdrawn)) {
    throw fail(404, 'no_track', 'No song by that id');
  }
  if (kind === 'film') throw fail(404, 'no_film', 'No film by that id');
  if (kind === 'system' && !byId(state.systems, id)) throw fail(404, 'no_system', 'No system by that id');
  if (kind === 'galaxy' && !byId(state.galaxies, id)) throw fail(404, 'no_galaxy', 'No galaxy by that id');
  return { kind, targetId: id };
}

function save(ctx) {
  const me = requireUser(ctx);
  const target = saveTarget(ctx, 'Say what to save');
  const exists = ctx.state.saved.some(
    (s) => s.userId === me.id && s.kind === target.kind && s.targetId === target.targetId,
  );
  if (!exists) ctx.state.saved.push({ userId: me.id, ...target, createdAt: iso(ctx.state.now()) });
  return ok({ saved: true });
}

function unsave(ctx) {
  const me = requireUser(ctx);
  const target = saveTarget(ctx, 'Say what to let go');
  ctx.state.saved = ctx.state.saved.filter(
    (s) => !(s.userId === me.id && s.kind === target.kind && s.targetId === target.targetId),
  );
  return ok({ saved: false });
}

function placeOf(state, track) {
  const planet = state.planets.find((p) => p.publishedTrackId === track.id);
  const system = planet && byId(state.systems, planet.systemId);
  const galaxy = system && byId(state.galaxies, system.galaxyId);
  return {
    planet,
    galaxy: galaxy ? { slug: galaxy.slug, displayName: galaxy.displayName } : null,
    system: system ? { slug: system.slug, title: system.title } : null,
  };
}

function savedShelf(ctx) {
  const me = requireUser(ctx);
  const { state } = ctx;
  const rows = state.saved.filter((s) => s.userId === me.id).sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1));
  const of = (kind) => rows.filter((r) => r.kind === kind);
  const songs = of('song').flatMap((r) => {
    const track = byId(state.tracks, r.targetId);
    if (!track || track.withdrawn) return [];
    const { planet, galaxy, system } = placeOf(state, track);
    return [
      {
        type: 'planet',
        kind: 'song',
        id: planet?.id ?? 0,
        createdAt: r.createdAt,
        title: track.title,
        artist: track.artist,
        coverArtUrl: track.coverArtUrl,
        key: track.musicalKey,
        bpm: track.bpm,
        duration: track.duration,
        trackId: track.trackId,
        publishedId: track.id,
        galaxy,
        system,
      },
    ];
  });
  const systems = of('system').flatMap((r) => {
    const s = byId(state.systems, r.targetId);
    if (!s) return [];
    const galaxy = byId(state.galaxies, s.galaxyId);
    const planetCount = state.planets.filter((p) => p.systemId === s.id).length;
    return [{ id: s.id, title: s.title, slug: s.slug, galaxySlug: galaxy.slug, planetCount }];
  });
  const galaxies = of('galaxy').flatMap((r) => {
    const g = byId(state.galaxies, r.targetId);
    return g ? [galaxyBrief(g)] : [];
  });
  return ok({ songs, films: [], systems, galaxies });
}

function addedUserIds(state, user) {
  return new Set(
    state.saved
      .filter((s) => s.userId === user.id && s.kind === 'galaxy')
      .map((s) => byId(state.galaxies, s.targetId)?.userId),
  );
}

// --- catalog and Newest ------------------------------------------------------

// Keyset paging, newest first: "<iso>|<id>" (catalog) or "<iso>|planet|<id>"
// (feed). Unlike the server, rows sharing the cursor's second aren't
// skipped: the id breaks the tie.
function parseCursor(raw, parts) {
  if (!raw) return null;
  const bits = String(raw).split('|');
  const at = new Date(bits[0]);
  if (bits.length !== parts || Number.isNaN(at.getTime())) throw fail(400, 'bad_cursor', 'That cursor is not readable');
  return { at: at.toISOString(), id: Number(bits[parts - 1]) || 0 };
}

const newestFirst = (a, b) => (a.createdAt === b.createdAt ? b.id - a.id : a.createdAt < b.createdAt ? 1 : -1);
const before = (cursor) => (row) =>
  !cursor || row.createdAt < cursor.at || (row.createdAt === cursor.at && row.id < cursor.id);

function pageSize(raw, fallback, max) {
  return Math.min(max, Math.max(1, Number(raw) || fallback));
}

function catalog(ctx) {
  optionalUser(ctx);
  const { state } = ctx;
  const cursor = parseCursor(ctx.query.cursor, 2);
  const limitTo = pageSize(ctx.query.limit, 40, 100);
  const search = String(ctx.query.search || '')
    .trim()
    .toLowerCase();
  const rows = state.tracks
    .filter((t) => t.isMaster && t.remixDepth === 0 && !t.withdrawn)
    .filter((t) => !search || t.title.toLowerCase().includes(search) || t.artist.toLowerCase().includes(search))
    .sort(newestFirst)
    .filter(before(cursor));
  const page = rows.slice(0, limitTo);
  const last = page[page.length - 1];
  return ok({
    items: page.map((t) => {
      const uploader = byId(state.users, t.uploaderId);
      return {
        trackId: t.trackId,
        publishedId: t.id,
        title: t.title,
        artist: t.artist,
        coverArtUrl: t.coverArtUrl,
        duration: t.duration,
        bpm: t.bpm,
        musicalKey: t.musicalKey,
        uploader: { id: uploader.id, username: uploader.username },
      };
    }),
    nextCursor: rows.length > page.length ? `${last.createdAt}|${last.id}` : null,
  });
}

// Newest's medium filter takes the words the rest of the API uses: a
// medium as the store's halls name it, its family (4.7), or a planet kind.
// The mock's worlds are all songs.
const MEDIA = {
  music: 'music',
  mi: 'music',
  song: 'music',
  film: 'film',
  si: 'film',
  writing: 'writing',
  ri: 'writing',
  page: 'writing',
  fashion: 'fashion',
  gi: 'fashion',
  gallery: 'fashion',
};

// Newest (4.10): planets in published systems, newest first, filtered by
// medium, "Galaxies I've added", key and BPM. A card says what a thing is
// and nothing about how it did. The server's feed also mixes in suns; the
// desktop's Newest lists works, so this one doesn't.
function feed(ctx) {
  const viewer = optionalUser(ctx);
  const { state, query } = ctx;
  const cursor = parseCursor(query.cursor, 3);
  const limitTo = pageSize(query.limit, 12, 40);
  const media = query.medium
    ? String(query.medium)
        .split(',')
        .map((m) => MEDIA[m.trim().toLowerCase()])
    : null;
  if (query.added && !viewer) return error(401, 'Unauthorized');
  const added = query.added ? addedUserIds(state, viewer) : null;
  const bpmMin = query.bpmMin === undefined ? -Infinity : Number(query.bpmMin);
  const bpmMax = query.bpmMax === undefined ? Infinity : Number(query.bpmMax);
  const cards = state.planets
    .map((planet) => {
      const system = byId(state.systems, planet.systemId);
      const galaxy = byId(state.galaxies, system.galaxyId);
      const track = trackOfPlanet(state, planet);
      return { planet, system, galaxy, track, createdAt: planet.createdAt, id: planet.id };
    })
    .filter((c) => c.system.status === 'published' && !c.track.withdrawn)
    .filter((c) => !media || media.includes('music'))
    .filter((c) => !added || added.has(c.galaxy.userId))
    .filter((c) => !query.key || c.track.musicalKey === query.key)
    .filter(
      (c) =>
        (query.bpmMin === undefined && query.bpmMax === undefined) || (c.track.bpm >= bpmMin && c.track.bpm <= bpmMax),
    )
    .sort(newestFirst)
    .filter(before(cursor));
  const page = cards.slice(0, limitTo);
  const last = page[page.length - 1];
  return ok({
    items: page.map(({ planet, system, galaxy, track }) => ({
      type: 'planet',
      kind: 'song',
      medium: 'music',
      id: planet.id,
      createdAt: planet.createdAt,
      title: track.title,
      artist: track.artist,
      coverArtUrl: track.coverArtUrl,
      key: track.musicalKey,
      bpm: track.bpm,
      duration: track.duration,
      trackId: track.trackId,
      publishedId: track.id,
      priceCents: track.isForSale ? track.priceCents : null,
      galaxyId: galaxy.id,
      galaxy: { slug: galaxy.slug, displayName: galaxy.displayName },
      system: { slug: system.slug, title: system.title },
    })),
    nextCursor: cards.length > page.length ? `${last.createdAt}|planet|${last.id}` : null,
  });
}

// --- the universe and travel -------------------------------------------------

// Every galaxy where it sits, with no fog and no fuel (4.12). Drift and
// weather aren't simulated: binaryPairs and events stay empty.
function universe(ctx) {
  optionalUser(ctx);
  const galaxies = ctx.state.galaxies.map((g) => ({
    id: g.id,
    x: g.x,
    y: g.y,
    slug: g.slug,
    displayName: g.displayName,
    skySeed: g.skySeed,
  }));
  return ok({ galaxies, binaryPairs: [], events: [] });
}

const round2 = (v) => Math.round(v * 100) / 100;

// A free flight, never captured (4.12). The server's Idempotency-Key stays
// required, so a retried flight is the same flight.
function travel(ctx) {
  const me = requireUser(ctx);
  return idempotent(ctx, me, 'travel', true, () => {
    const { state } = ctx;
    const home = state.galaxies.find((g) => g.userId === me.id);
    if (!home) return fail(400, 'no_galaxy', 'Claim a galaxy before you fly');
    const toId = Number(ctx.body.toGalaxyId);
    if (!Number.isInteger(toId) || toId < 1) return fail(400, 'bad_target', 'Aim at a galaxy');
    const originId = me.currentGalaxyId || home.id;
    if (originId === toId) return fail(400, 'already_there', 'You are already in that galaxy');
    const target = byId(state.galaxies, toId);
    if (!target) return fail(404, 'not_found', 'No galaxy out there');
    const from = byId(state.galaxies, originId);
    const [dx, dy] = [target.x - from.x, target.y - from.y];
    const theta = Number.isFinite(ctx.body.launchAngle) ? ctx.body.launchAngle : Math.atan2(dy, dx);
    const reach = 0.5 * Math.hypot(dx, dy);
    const control = { x: from.x + Math.cos(theta) * reach, y: from.y + Math.sin(theta) * reach };
    const path = Array.from({ length: 64 }, (_, i) => {
      const t = i / 63;
      const u = 1 - t;
      return [
        round2(u * u * from.x + 2 * u * t * control.x + t * t * target.x),
        round2(u * u * from.y + 2 * u * t * control.y + t * t * target.y),
      ];
    });
    me.currentGalaxyId = target.id;
    const { id, slug, displayName, skySeed, x, y } = target;
    return ok({ outcome: 'arrived', landedGalaxy: { id, slug, displayName, skySeed, x, y }, path });
  });
}

// --- Since you last looked (2.10) --------------------------------------------

// What happened to you and the galaxies you added since `after`: forks of
// your works, sales, payouts, new works and letters; plus every link still
// waiting on you, however old. Each event is one line; nothing is summed.
function since(ctx) {
  const me = requireUser(ctx);
  const { state } = ctx;
  const after = new Date(ctx.query.after ?? 0);
  if (Number.isNaN(after.getTime())) return fail(400, 'bad_after', "That time isn't readable");
  const cut = after.toISOString();
  const name = (id) => byId(state.users, id).username;
  const events = [];
  for (const t of state.tracks) {
    const parent = state.tracks.find((p) => p.trackId === t.parentTrackId);
    if (parent?.uploaderId === me.id && t.uploaderId !== me.id && !t.withdrawn && t.createdAt > cut) {
      events.push({ kind: 'fork', at: t.createdAt, who: name(t.uploaderId), work: parent.title, fork: t.title });
    }
  }
  for (const l of state.links) {
    if (l.status === 'pending' && l.declaredById !== me.id && touches(state, l, me)) {
      events.push({
        kind: 'link',
        at: l.createdAt,
        who: name(l.declaredById),
        linkId: l.id,
        link: { from: l.from, to: l.to, kind: l.kind },
      });
    }
  }
  for (const p of state.purchases) {
    if (p.sellerId === me.id && p.status === 'completed' && p.completedAt > cut) {
      events.push({ kind: 'sale', at: p.completedAt, work: itemTitle(state, p), priceCents: p.amountCents });
    }
  }
  for (const p of state.payouts) {
    if (p.userId === me.id && p.at > cut) events.push({ kind: 'payout', at: p.at, amountCents: p.amountCents });
  }
  const added = addedUserIds(state, me);
  for (const t of state.tracks) {
    if (added.has(t.uploaderId) && t.isMaster && !t.withdrawn && t.createdAt > cut) {
      events.push({ kind: 'work', at: t.createdAt, who: name(t.uploaderId), title: t.title });
    }
  }
  for (const letter of state.letters) {
    if (added.has(letter.userId) && letter.createdAt > cut) {
      const opening = letter.blocks.find((b) => b.type === 'text')?.text.split('\n')[0] ?? '';
      events.push({
        kind: 'letter',
        at: letter.createdAt,
        who: name(letter.userId),
        greeting: letter.greeting,
        opening,
      });
    }
  }
  events.sort((a, b) => (a.at < b.at ? 1 : a.at > b.at ? -1 : 0));
  return ok({ events });
}

export function itemTitle(state, purchase) {
  const rows = purchase.type === 'track' ? state.tracks : state.listings;
  return byId(rows, purchase.itemId).title;
}

export const routes = [
  ['POST', '/api/v2/galaxies', claim],
  ['GET', '/api/v2/galaxies/mine', mine],
  ['GET', '/api/v2/galaxies/:slug', galaxyScene],
  ['GET', '/api/v2/galaxies/:slug/sun', galaxySun],
  ['GET', '/api/v2/galaxies/:slug/letters', letters],
  ['POST', '/api/v2/galaxies/:id/systems', createSystem],
  ['GET', '/api/v2/galaxies/:slug/systems/:system', systemScene],
  ['GET', '/api/v2/galaxies/:slug/systems/:system/sun', systemSun],
  ['PATCH', '/api/v2/systems/:id', patchSystem],
  ['DELETE', '/api/v2/systems/:id', deleteSystem],
  ['POST', '/api/v2/systems/:id/planets', addPlanet],
  ['PATCH', '/api/v2/planets/:id', patchPlanet],
  ['DELETE', '/api/v2/planets/:id', deletePlanet],
  ['GET', '/api/v2/suns/:id', getSun],
  ['PUT', '/api/v2/suns/:id/blocks', putBlocks],
  ['POST', '/api/v2/lineage-links', declareLink],
  ['GET', '/api/v2/lineage-links/pending', pendingLinks],
  ['GET', '/api/v2/lineage-links', listLinks],
  ['POST', '/api/v2/lineage-links/:id/accept', acceptLink],
  ['DELETE', '/api/v2/lineage-links/:id', deleteLink],
  ['PUT', '/api/v2/saved', save],
  ['DELETE', '/api/v2/saved', unsave],
  ['GET', '/api/v2/saved', savedShelf],
  ['GET', '/api/v2/catalog', catalog],
  ['GET', '/api/v2/feed', feed],
  ['GET', '/api/v2/universe', universe],
  ['POST', '/api/v2/travel', travel],
  ['GET', '/api/v2/since', since],
];
