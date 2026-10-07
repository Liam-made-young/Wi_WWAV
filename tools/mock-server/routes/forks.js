// ↑ Push (docs/SPEC.md 4.6, 6.8, 9.7): POST /api/tracks/:trackId/fork, as
// server routes/forks.js answers it (server.md §3). A fork is a mix state
// with one parent (two for a v2 project with a second song). It owns no
// audio: it plays its nearest ancestor that does. It joins the family tree
// at once and takes a seat in a galaxy only when placed on a system.
//
// Where the mock differs from the server:
// - an optional Idempotency-Key makes a retried push one fork (mismatch 2:
//   forks have no idempotency, so a retry posts twice);
// - its song id is the platform id of its trackId (6.1, 6.8: "a new id,
//   on the server"), and a file exported from it later becomes its first
//   version;
// - the mock has no RemixAudio imports, so a v1 fork's overdubs are all
//   dropped (the server drops one whose import isn't yours) and a v2
//   project that names an import is refused, as the server refuses an
//   unknown one.
import { error, json } from '../http.js';
import { requireUser } from '../auth.js';
import { platformSongId } from '../hash.js';
import { idempotent, iso, limit, nextId } from '../state.js';
import { lineageNode, ownsAudio, stemsTrackId } from './publish.js';
import { newTrackId } from './uploads.js';

const MAX_REMIX_DEPTH = 64;
const STEM_KINDS = ['vocals', 'drums', 'bass', 'other'];
const IOS_STEM_KEYS = ['vox', 'drum', 'bass', 'synth'];

const isObj = (v) => Boolean(v) && typeof v === 'object';

const num = (v, min, max, fallback) => {
  const n = Number(v);
  return Number.isFinite(n) ? Math.min(Math.max(n, min), max) : fallback;
};

// --- v1: a stem mix ----------------------------------------------------------

function sanitizeStemEffects(raw) {
  if (!isObj(raw)) return null;
  const out = {};
  for (const key of IOS_STEM_KEYS) {
    const s = raw[key];
    if (!isObj(s)) continue;
    out[key] = {
      distortion: num(s.distortion, 0, 1, 0),
      delay: num(s.delay, 0, 1, 0),
      reverb: num(s.reverb, 0, 1, 0),
      tremolo: num(s.tremolo, 0, 1, 0),
      eqLow: num(s.eqLow, -18, 18, 0),
      eqMid: num(s.eqMid, -18, 18, 0),
      eqHigh: num(s.eqHigh, -18, 18, 0),
      pan: num(s.pan, -1, 1, 0),
      ...(isObj(s.compressor) ? { compressor: s.compressor } : {}),
    };
  }
  return Object.keys(out).length ? out : null;
}

function sanitizeMix(mix) {
  if (!isObj(mix)) return null;
  const stems = {};
  for (const [rawKey, s] of Object.entries(isObj(mix.stems) ? mix.stems : {})) {
    if (!isObj(s)) continue;
    // A bare kind ("drums") or a scoped stem id ("track_123-drums").
    const kind = STEM_KINDS.find((k) => rawKey === k || rawKey.endsWith(`-${k}`));
    if (!kind) continue;
    stems[kind] = { level: num(s.level, 0, 1, 0.8), muted: Boolean(s.muted), solo: Boolean(s.solo) };
  }
  if (!Object.keys(stems).length) return null;
  return {
    stems,
    masterPitch: num(mix.masterPitch, -12, 12, 0),
    // A percent offset from rate 1.0: iOS's keylocked 0.5-2.0 is -50…+100.
    masterTime: num(mix.masterTime, -50, 100, 0),
    effects: sanitizeStemEffects(mix.effects),
  };
}

// The four top-level keys the shipped iOS decoder needs, with the mix
// itself under `wwav`.
function snapshotV1(mix, at) {
  return {
    stemEffects: mix.effects || {},
    timePitch: { rate: 1 + mix.masterTime / 100, pitchSemitones: mix.masterPitch },
    overdubs: {},
    capturedAt: at,
    wwav: { v: 1, stems: mix.stems, masterPitch: mix.masterPitch, masterTime: mix.masterTime },
  };
}

// --- v2: an 8-track project -----------------------------------------------------

const MAX_CLIPS = 500;
const MAX_IMPORTS = 12;
const MAX_TIMELINE_SEC = 3600;
const TRACK_COUNT = 8;

function sanitizeTrack(raw) {
  const t = isObj(raw) ? raw : {};
  const eq = isObj(t.eq) ? t.eq : {};
  const comp = isObj(t.comp) ? t.comp : {};
  const fx = isObj(t.fx) ? t.fx : {};
  const block = (b, extra) => {
    const src = isObj(b) ? b : {};
    return { on: Boolean(src.on), mix: num(src.mix, 0, 100, 30), ...extra(src) };
  };
  return {
    gain: num(t.gain, 0, 1, 0.8),
    pan: num(t.pan, -1, 1, 0),
    muted: Boolean(t.muted),
    solo: Boolean(t.solo),
    pitch: num(t.pitch, -12, 12, 0),
    rate: num(t.rate, 0.5, 2, 1),
    eq: {
      on: Boolean(eq.on),
      low: num(eq.low, -12, 12, 0),
      mid: num(eq.mid, -12, 12, 0),
      high: num(eq.high, -12, 12, 0),
    },
    comp: {
      on: Boolean(comp.on),
      threshold: num(comp.threshold, -60, 0, -24),
      ratio: num(comp.ratio, 1, 20, 4),
      attack: num(comp.attack, 0, 1, 0.01),
      gain: num(comp.gain, 0, 24, 0),
    },
    fx: {
      reverb: block(fx.reverb, (s) => ({ size: num(s.size, 0.1, 8, 2), decay: num(s.decay, 0.1, 10, 2) })),
      delay: block(fx.delay, (s) => ({ time: num(s.time, 0.01, 2, 0.35), feedback: num(s.feedback, 0, 0.9, 0.35) })),
      distortion: block(fx.distortion, (s) => ({ drive: num(s.drive, 0, 100, 30), output: num(s.output, 0, 2, 1) })),
      tremolo: block(fx.tremolo, (s) => ({ rate: num(s.rate, 0.1, 20, 4), depth: num(s.depth, 0, 1, 0.6) })),
    },
  };
}

const pitchRate = (raw) => {
  const g = isObj(raw) ? raw : {};
  return { pitch: num(g.pitch, -12, 12, 0), rate: num(g.rate, 0.5, 2, 1) };
};

// A source has to resolve to a node that owns audio, or every listener
// would replay silence.
function ownedSource(state, trackId) {
  const node = state.tracks.find((t) => t.trackId === trackId && t.isMaster && !t.withdrawn);
  const resolved = node && stemsTrackId(state, node);
  if (!resolved || !ownsAudio(state, resolved)) throw error(422, `Source ${trackId} has no audio`);
  return { trackId: resolved };
}

function sanitizeProject(state, project) {
  if (!isObj(project)) throw error(400, 'Missing project');
  const sources = isObj(project.sources) ? project.sources : {};
  const primaryId = sources.primary?.trackId;
  if (typeof primaryId !== 'string' || !primaryId) throw error(400, 'Project has no primary source');
  const primary = ownedSource(state, primaryId);
  const secondary =
    typeof sources.secondary?.trackId === 'string' ? ownedSource(state, sources.secondary.trackId) : null;

  const rawImports = Array.isArray(sources.imports) ? sources.imports : [];
  if (rawImports.length > MAX_IMPORTS) throw error(400, 'Too many imports');
  for (const raw of rawImports) {
    const id = isObj(raw) ? raw.importId : raw;
    if (typeof id === 'string') throw error(422, `Import ${id} does not exist`);
  }

  const rawClips = Array.isArray(project.clips) ? project.clips : [];
  if (!rawClips.length) throw error(400, 'Project has no clips');
  if (rawClips.length > MAX_CLIPS) throw error(400, 'Too many clips');
  const clips = rawClips.map((raw, i) => {
    if (!isObj(raw) || !isObj(raw.src)) throw error(400, 'Malformed clip');
    if (raw.src.type === 'import') throw error(400, 'Clip references an unknown import');
    if (raw.src.type !== 'stem') throw error(400, 'Unknown clip source type');
    if (raw.src.of !== 'primary' && raw.src.of !== 'secondary') {
      throw error(400, 'Clip stem source must be primary or secondary');
    }
    if (raw.src.of === 'secondary' && !secondary) {
      throw error(400, 'Clip references a secondary source that is not set');
    }
    if (!STEM_KINDS.includes(raw.src.kind)) throw error(400, 'Unknown stem kind');
    return {
      id: typeof raw.id === 'string' ? raw.id.slice(0, 32) : `c${i}`,
      track: Math.min(Math.max(Math.floor(Number(raw.track) || 0), 0), TRACK_COUNT - 1),
      startTime: num(raw.startTime, 0, MAX_TIMELINE_SEC, 0),
      clipDuration: num(raw.clipDuration, 0.05, MAX_TIMELINE_SEC, 1),
      bufferOffset: num(raw.bufferOffset, 0, MAX_TIMELINE_SEC, 0),
      src: { type: 'stem', of: raw.src.of, kind: raw.src.kind },
    };
  });

  const rawTracks = Array.isArray(project.tracks) ? project.tracks : [];
  return {
    duration: num(project.duration, 1, MAX_TIMELINE_SEC, 60),
    sources: { primary, secondary, imports: [] },
    tracks: Array.from({ length: TRACK_COUNT }, (_, i) => sanitizeTrack(rawTracks[i])),
    groups: { A: pitchRate(project.groups?.A), B: pitchRate(project.groups?.B) },
    master: pitchRate(project.master),
    clips,
  };
}

function snapshotV2(project, at) {
  return {
    stemEffects: {},
    timePitch: { rate: project.master.rate, pitchSemitones: project.master.pitch },
    overdubs: {},
    capturedAt: at,
    wwav: { v: 2, project },
  };
}

// --- the route -----------------------------------------------------------------

function fork(ctx) {
  const me = requireUser(ctx);
  limit(ctx.state, {
    scope: 'fork',
    key: `u:${me.id}`,
    max: 20,
    windowMs: 60 * 1000,
    message: 'Slow down — too many forks in a short window.',
  });
  return idempotent(ctx, me, 'fork', false, () => {
    const { state, body } = ctx;
    // v2 wins when both are sent.
    const project = body.project ? sanitizeProject(state, body.project) : null;
    const mix = project ? null : sanitizeMix(body.mix);
    if (!project && !mix) return error(400, 'Missing or invalid mix state');

    // A withdrawn work has left the sky: its earlier forks keep playing it,
    // but no new one starts from it (Open #21).
    const parent = state.tracks.find((t) => t.trackId === ctx.params.trackId && t.isMaster && !t.withdrawn);
    if (!parent) return error(404, 'Track not found');
    if (parent.remixDepth + 1 > MAX_REMIX_DEPTH) {
      return error(422, 'Lineage is too deep to extend', { code: 'max_depth_reached' });
    }

    const at = iso(state.now());
    const trackId = newTrackId(state, 'fork');
    const title = typeof body.title === 'string' && body.title.trim() ? body.title.trim().slice(0, 200) : '';
    const track = {
      id: nextId(state, 'tracks'),
      trackId,
      songId: platformSongId(trackId),
      uploaderId: me.id,
      title: title || `${parent.title} (fork)`,
      artist: me.username,
      album: 'Single',
      coverArtUrl: parent.coverArtUrl,
      duration: project ? project.duration : parent.duration,
      bpm: parent.bpm,
      musicalKey: parent.musicalKey,
      isMaster: true,
      settings: {},
      tags: [],
      parentTrackId: parent.trackId,
      secondaryParentTrackId: project?.sources.secondary?.trackId ?? null,
      lineageRootTrackId: parent.lineageRootTrackId || parent.trackId,
      remixDepth: parent.remixDepth + 1,
      // Off Newest by default, as on the server: a fork lives in its tree.
      inFeed: body.inFeed === true,
      remixSnapshot: project ? snapshotV2(project, at) : snapshotV1(mix, at),
      withdrawn: false,
      createdAt: at,
      versions: [],
    };
    state.tracks.push(track);
    return json(201, lineageNode(state, track));
  });
}

export const routes = [['POST', '/api/tracks/:trackId/fork', fork]];
