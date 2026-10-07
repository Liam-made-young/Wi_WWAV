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
import { error, isObject, json } from '../http.js';
import { requireUser } from '../auth.js';
import { platformSongId } from '../hash.js';
import { iso, nextId } from '../state.js';
import { readWwav } from '../wwav.js';

function newestVersion(track) {
  return track.versions[track.versions.length - 1];
}

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
    if (retried) return answer(retried, retried.versions.find(fromClip), { updated: true, message: 'Already up' });
  }

  const file = readWwav(object.bytes);
  const songId = file?.songId ?? platformSongId(trackId);
  const versionNumber = file?.version ?? 1;
  const work = state.tracks.find((t) => t.songId === songId);
  if (work && work.uploaderId !== me.id) {
    const owner = state.users.find((u) => u.id === work.uploaderId);
    return error(409, `This file's id belongs to another work, '${work.title}' by ${owner.username}.`, {
      code: 'id_taken',
    });
  }
  const atLink = state.tracks.find((t) => t.trackId === trackId);
  if (atLink && atLink !== work) {
    return error(409, `This file is a different work from '${atLink.title}'.`, { code: 'different_work' });
  }

  const version = {
    version: versionNumber,
    sha256: object.sha256,
    bytes: object.bytes.length,
    s3Key: upload.s3Key,
    fileName: '',
    origin: clip?.origin,
    clipId: clip?.clipId,
    createdAt: iso(state.now()),
  };

  if (work) {
    applyBody(work, body);
    const same = work.versions.find((v) => v.sha256 === object.sha256);
    if (same) return answer(work, same, { updated: true, message: 'Already up' });
    if (versionNumber <= newestVersion(work).version) {
      return error(
        409,
        `Version ${versionNumber} of '${work.title}' is already up. Export it again as a new version.`,
        {
          code: 'version_taken',
        },
      );
    }
    version.fileName = `${work.title}${file ? '.wwav' : '.wav'}`;
    work.versions.push(version);
    return answer(work, version, { updated: true });
  }

  // A new work. Its family comes from wlin: the parent and root are found
  // by song_id, when they are on the server.
  const parent = file?.parentId ? state.tracks.find((t) => t.songId === file.parentId) : null;
  const root = file?.rootId ? state.tracks.find((t) => t.songId === file.rootId) : null;
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
    remixDepth: file ? file.generation : 0,
    priceCents: null,
    isForSale: false,
    withdrawn: false,
    createdAt: iso(state.now()),
    versions: [version],
  };
  applyBody(track, body);
  version.fileName = `${track.title}${file ? '.wwav' : '.wav'}`;
  state.tracks.push(track);
  return answer(track, version);
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

export function lineageNode(state, track) {
  const uploader = state.users.find((u) => u.id === track.uploaderId);
  return {
    id: track.id,
    trackId: track.trackId,
    songId: track.songId,
    title: track.title,
    artist: track.artist,
    coverArtUrl: track.coverArtUrl,
    uploaderId: track.uploaderId,
    uploader: { id: uploader.id, username: uploader.username, profilePicture: uploader.profilePicture },
    parentTrackId: track.parentTrackId,
    secondaryParentTrackId: track.secondaryParentTrackId,
    lineageRootTrackId: track.lineageRootTrackId,
    remixDepth: track.remixDepth,
    duration: track.duration,
    bpm: track.bpm,
    musicalKey: track.musicalKey,
    created_at: track.createdAt,
    withdrawn: track.withdrawn,
  };
}

// GET /api/tracks/:trackId/lineage, as routes/lineage.js answers it.
function lineage(ctx) {
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
    .filter((t) => t.lineageRootTrackId === root.trackId && t !== root)
    .sort((a, b) => a.remixDepth - b.remixDepth || (a.createdAt < b.createdAt ? -1 : 1));
  return json(200, {
    root: lineageNode(state, root),
    ancestors: ancestors.map((t) => lineageNode(state, t)),
    descendants: descendants.map((t) => lineageNode(state, t)),
    current: current.trackId,
  });
}

export const routes = [
  ['POST', '/api/publish', publish],
  ['POST', '/api/unpublish', unpublish],
  ['GET', '/api/tracks/:trackId/lineage', lineage],
];
