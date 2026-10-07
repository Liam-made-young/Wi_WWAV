// Publishing (docs/SPEC.md 2.8, 6.8, 9.7) and the family tree it builds.
//
// /api/publish upserts by trackId, as the server does. On top of that the
// mock does what the spec says and the server doesn't yet (server.md
// mismatches 2 and 8):
// - settings {origin, clipId} make a retry idempotent even when it re-signed
//   and got a new trackId: the work already published from that clip answers.
// - 6.8's id rules on the file's own song_id: the first upload claims it;
//   the same sha256 again is "Already up"; a higher version from the same
//   account is a new version, kept beside the old; another account's id is
//   refused.
// - lineage comes from the file's wlin (4.13).
// - a re-publish that leaves out isMaster keeps it a master (the server
//   sets it false, which hides the track from lineage and forks).
// - a refused publish changes nothing; an accepted one (including "Already
//   up") takes the body's title, tags and the rest.
// - a work its maker unpublished comes back when they publish it again.
// - each version's bytes are copied to a key named by their sha256, which
//   no sign route hands out, so the file a buyer downloads is the one on the
//   receipt even if a still-valid upload URL is used again (6.12).
import { error, isObject, json } from '../http.js';
import { optionalUser, requireUser } from '../auth.js';
import { platformSongId } from '../hash.js';
import { iso, nextId } from '../state.js';
import { readWwav } from '../wwav.js';

export const versionKey = (sha256) => `files/${sha256}`;

function answer(track, version, extra = {}) {
  return json(200, {
    success: true,
    trackId: track.trackId,
    publishedId: track.id,
    songId: track.songId,
    version: version.version,
    ...extra,
  });
}

// What the body may change on a work that is already up.
function applyBody(track, body) {
  if (typeof body.title === 'string' && body.title) track.title = body.title;
  if (typeof body.album === 'string' && body.album) track.album = body.album;
  if (typeof body.coverArtUrl === 'string') track.coverArtUrl = body.coverArtUrl;
  if (typeof body.isMaster === 'boolean') track.isMaster = body.isMaster;
  if (Number.isFinite(body.duration)) track.duration = body.duration;
  if (Array.isArray(body.tags)) {
    track.tags = body.tags.slice(0, 10).map((t) => String(t).toLowerCase().trim().slice(0, 30));
  }
  if (isObject(body.settings)) track.settings = body.settings;
}

function publish(ctx) {
  const me = requireUser(ctx);
  const { state, body } = ctx;
  const trackId = String(body.trackId || '');
  if (!trackId) return error(400, 'Missing trackId');

  // The object to publish: the one named, else the newest signed for trackId.
  const signed = state.uploads.filter((u) => u.trackId === trackId && (!body.s3Key || u.s3Key === body.s3Key));
  const upload = signed[signed.length - 1];
  if (!upload || upload.userId !== me.id) return error(403, "You don't own this track");
  const object = state.objects.get(upload.s3Key);
  if (!object) return error(409, "The file hasn't finished uploading.");

  const settings = isObject(body.settings) ? body.settings : {};
  const clip = settings.origin && settings.clipId ? { origin: settings.origin, clipId: settings.clipId } : null;
  if (clip) {
    const fromClip = (v) => v.origin === clip.origin && v.clipId === clip.clipId;
    const retried = state.tracks.find((t) => t.uploaderId === me.id && t.versions.some(fromClip));
    if (retried) return alreadyUp(retried, body, retried.versions.find(fromClip));
  }

  const file = readWwav(object.bytes);
  const songId = file?.songId ?? platformSongId(trackId);
  const versionNumber = file?.version ?? 1;
  const kept = { version: versionNumber, origin: clip?.origin, clipId: clip?.clipId };
  const fileName = (title) => `${title}${file ? '.wwav' : '.wav'}`;
  const work = state.tracks.find((t) => t.songId === songId);
  if (work && work.uploaderId !== me.id) {
    const owner = state.users.find((u) => u.id === work.uploaderId);
    return error(409, `This file's id belongs to another work, '${work.title}' by ${owner.username}.`, {
      code: 'id_taken',
    });
  }
  // A platform id (6.1) is FNV-1a of a trackId, so it belongs to whoever
  // signed that trackId, before they publish too.
  if (!work && state.uploads.some((u) => u.userId !== me.id && platformSongId(u.trackId) === songId)) {
    return error(409, "This file's id belongs to another account's upload.", { code: 'id_taken' });
  }
  const atLink = state.tracks.find((t) => t.trackId === trackId);
  if (atLink && atLink !== work) {
    return error(409, `This file is a different work from '${atLink.title}'.`, { code: 'different_work' });
  }

  if (work) {
    const same = work.versions.find((v) => v.sha256 === object.sha256);
    if (same) return alreadyUp(work, body, same);
    // A fork pushed in Space has no version until it is exported (6.8).
    const newest = work.versions.at(-1);
    if (newest && versionNumber <= newest.version) {
      return error(
        409,
        `Version ${versionNumber} of '${work.title}' is already up. Export it again as a new version.`,
        {
          code: 'version_taken',
        },
      );
    }
    applyBody(work, body);
    work.withdrawn = false;
    const version = keepVersion(state, object, { ...kept, fileName: fileName(work.title) });
    work.versions.push(version);
    return answer(work, version, { updated: true });
  }

  // A new work. Its family comes from wlin: the parent and root are found
  // by song_id, when they are on the server. A parent on the server fixes
  // the root and the generation (the parent's + 1), whatever wlin says.
  const parent = file?.parentId ? state.tracks.find((t) => t.songId === file.parentId) : null;
  const root = parent
    ? state.tracks.find((t) => t.trackId === parent.lineageRootTrackId)
    : file?.rootId
      ? state.tracks.find((t) => t.songId === file.rootId)
      : null;
  const track = {
    id: nextId(state, 'tracks'),
    trackId,
    songId,
    uploaderId: me.id,
    title: file?.title || trackId,
    artist: file?.artist || me.username,
    album: 'Single',
    coverArtUrl: null,
    duration: null,
    bpm: file?.bpm ?? null,
    musicalKey: file?.key ?? null,
    isMaster: true,
    settings: {},
    tags: [],
    parentTrackId: parent?.trackId ?? null,
    secondaryParentTrackId: null,
    lineageRootTrackId: root?.trackId ?? trackId,
    remixDepth: parent ? parent.remixDepth + 1 : file ? file.generation : 0,
    inFeed: true,
    remixSnapshot: null,
    priceCents: null,
    isForSale: false,
    withdrawn: false,
    createdAt: iso(state.now()),
    versions: [],
  };
  applyBody(track, body);
  track.versions.push(keepVersion(state, object, { ...kept, fileName: fileName(track.title) }));
  state.tracks.push(track);
  return answer(track, track.versions[0]);
}

// The same file (or the same clip) again: no new version, but the body
// still applies, and a work its maker withdrew is back, so it isn't
// "Already up" after all.
function alreadyUp(work, body, version) {
  applyBody(work, body);
  if (!work.withdrawn) return answer(work, version, { updated: true, message: 'Already up' });
  work.withdrawn = false;
  return answer(work, version, { updated: true });
}

// A new version, its bytes copied to their sha256's key, where no upload
// URL can reach them.
function keepVersion(state, object, fields) {
  const s3Key = versionKey(object.sha256);
  state.objects.set(s3Key, object);
  return { ...fields, sha256: object.sha256, bytes: object.bytes.length, s3Key, createdAt: iso(state.now()) };
}

// Open #21 at its recommendation: the work leaves the sky, and its family
// keeps it, marked withdrawn. (The server destroys the row.)
function unpublish(ctx) {
  const me = requireUser(ctx);
  const track = ctx.state.tracks.find((t) => t.trackId === ctx.body.trackId && !t.withdrawn);
  if (!track) return error(404, 'Not published');
  if (track.uploaderId !== me.id) return error(403, 'Unauthorized');
  track.withdrawn = true;
  track.isForSale = false;
  ctx.state.planets = ctx.state.planets.filter((p) => p.publishedTrackId !== track.id);
  return json(200, { success: true });
}

// The server's audio rule (utils/audioSource.js): a node owns audio when
// it has a file of its own (on the server, a UserUpload row), and plays the
// nearest ancestor that does, itself included. A fork owns none.
export function ownsAudio(state, trackId) {
  return Boolean(state.tracks.find((t) => t.trackId === trackId)?.versions.length);
}

export function stemsTrackId(state, track) {
  let at = track;
  for (let hops = 0; at && !ownsAudio(state, at.trackId) && hops < 64; hops++) {
    at = state.tracks.find((t) => t.trackId === at.parentTrackId);
  }
  return at && ownsAudio(state, at.trackId) ? at.trackId : track.lineageRootTrackId || track.trackId;
}

// A node as routes/lineage.js answers it, plus `songId` and `withdrawn`.
// The global forest leaves out the snapshot, as the server does.
export function lineageNode(state, track, { snapshot = true } = {}) {
  const uploader = state.users.find((u) => u.id === track.uploaderId);
  return {
    id: track.id,
    trackId: track.trackId,
    songId: track.songId,
    title: track.title,
    artist: track.artist,
    coverArtUrl: track.coverArtUrl,
    uploaderId: track.uploaderId,
    inFeed: track.inFeed,
    uploader: { id: uploader.id, username: uploader.username, profilePicture: uploader.profilePicture },
    parentTrackId: track.parentTrackId,
    secondaryParentTrackId: track.secondaryParentTrackId,
    lineageRootTrackId: track.lineageRootTrackId,
    remixDepth: track.remixDepth,
    ...(snapshot ? { remixSnapshot: track.remixSnapshot } : {}),
    duration: track.duration,
    bpm: track.bpm,
    musicalKey: track.musicalKey,
    created_at: track.createdAt,
    ownsAudio: ownsAudio(state, track.trackId),
    stemsTrackId: stemsTrackId(state, track),
    withdrawn: track.withdrawn,
  };
}

const byDepth = (a, b) => a.remixDepth - b.remixDepth || (a.createdAt < b.createdAt ? -1 : 1);

// GET /api/tracks/:trackId/lineage, as routes/lineage.js answers it.
function lineage(ctx) {
  optionalUser(ctx);
  const { state } = ctx;
  const byTrackId = (id) => state.tracks.find((t) => t.trackId === id);
  const current = byTrackId(ctx.params.trackId);
  if (!current || !current.isMaster) return error(404, 'Track not found');
  const root = byTrackId(current.lineageRootTrackId) ?? current;
  const ancestors = [];
  for (let t = byTrackId(current.parentTrackId); t && ancestors.length < 64; t = byTrackId(t.parentTrackId)) {
    ancestors.unshift(t);
  }
  const descendants = state.tracks
    .filter((t) => t.lineageRootTrackId === root.trackId && t !== root && t.isMaster)
    .sort(byDepth);
  return json(200, {
    root: lineageNode(state, root),
    ancestors: ancestors.map((t) => lineageNode(state, t)),
    descendants: descendants.map((t) => lineageNode(state, t)),
    current: current.trackId,
  });
}

// GET /api/lineage/global: every family as one flat list. Roots are the
// originals, oldest first (the server orders by album track order, then
// date; the mock has no albums, and no films to add). Withdrawn works
// stay in their families, marked (Open #21). The server caches it for
// 30 s; the mock is always fresh, and sends the server's header unless
// ?fresh=1.
function globalLineage(ctx) {
  optionalUser(ctx);
  const { state } = ctx;
  const masters = state.tracks.filter((t) => t.isMaster);
  const roots = masters
    .filter((t) => t.remixDepth === 0 && t.lineageRootTrackId === t.trackId)
    .filter((t) => t.inFeed || masters.some((d) => d.remixDepth > 0 && d.lineageRootTrackId === t.trackId))
    .sort((a, b) => (a.createdAt < b.createdAt ? -1 : 1))
    .map((t) => t.trackId);
  const nodes = masters.filter((t) => roots.includes(t.lineageRootTrackId)).sort(byDepth);
  const headers = ctx.query.fresh === '1' ? {} : { 'cache-control': 'public, max-age=15' };
  return json(200, { roots, nodes: nodes.map((t) => lineageNode(state, t, { snapshot: false })) }, headers);
}

// GET /api/tracks/:trackId/is-published: how the server hands out the
// numeric id. The row leaves out playCount (gate 1.3); price is the
// DECIMAL string Postgres returns. A withdrawn work isn't published (the
// server deletes its row).
function isPublished(ctx) {
  const viewer = optionalUser(ctx);
  const track = ctx.state.tracks.find((t) => t.trackId === ctx.params.trackId && !t.withdrawn);
  if (!track) return json(200, { isPublished: false, publishedTrack: null, canUnpublish: false });
  const publishedTrack = {
    id: track.id,
    trackId: track.trackId,
    songId: track.songId,
    uploaderId: track.uploaderId,
    title: track.title,
    artist: track.artist,
    album: track.album,
    coverArtUrl: track.coverArtUrl,
    isMaster: track.isMaster,
    inFeed: track.inFeed,
    settings: track.settings,
    tags: track.tags,
    price: track.priceCents === null ? null : (track.priceCents / 100).toFixed(2),
    isForSale: track.isForSale,
    duration: track.duration,
    bpm: track.bpm,
    musicalKey: track.musicalKey,
    parentTrackId: track.parentTrackId,
    secondaryParentTrackId: track.secondaryParentTrackId,
    lineageRootTrackId: track.lineageRootTrackId,
    remixDepth: track.remixDepth,
    created_at: track.createdAt,
  };
  return json(200, { isPublished: true, publishedTrack, canUnpublish: viewer?.id === track.uploaderId });
}

export const routes = [
  ['POST', '/api/publish', publish],
  ['POST', '/api/unpublish', unpublish],
  ['GET', '/api/tracks/:trackId/lineage', lineage],
  ['GET', '/api/tracks/:trackId/is-published', isPublished],
  ['GET', '/api/lineage/global', globalLineage],
];
