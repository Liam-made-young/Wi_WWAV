// Uploads (docs/SPEC.md 2.8, 9.7) and the R2 stand-in they PUT to.
//
// The sign routes keep the server's shapes and lifetimes: /sign lasts
// 300 s, /sign-video and /sign-replace 600 s (server.md mismatch 4: the
// spec says 300 s for all). Two changes follow the spec: whoever signs owns
// the trackId from that moment (mismatch 3: today only /process creates the
// ownership row, so sign, PUT, publish answers 403), and /api/upload/parts
// is new: multipart in 8 MiB parts, each part signed just before it goes up
// (mismatch 5: only Wi's own host has 8 MiB parts, proxied, not presigned).
import { createHmac, randomBytes } from 'node:crypto';
import { bytes, error, json, text } from '../http.js';
import { requireUser } from '../auth.js';
import { etagOf, sha256 } from '../hash.js';
import { SECRET } from '../jwt.js';
import { iso, limit, nextId } from '../state.js';

const MB = 1024 * 1024;
export const PART_SIZE = 8 * MB;
const MAX_AUDIO = 250 * MB;
const MAX_VIDEO = 2048 * MB;
// A .wwav ends at 4 GB, the RIFF limit (6.1).
const MAX_MULTIPART = 4096 * MB;

const AUDIO_MIME = [
  'audio/wav',
  'audio/x-wav',
  'audio/wave',
  'audio/vnd.wave',
  'audio/mpeg',
  'audio/mp3',
  'audio/aac',
  'audio/mp4',
  'audio/x-m4a',
  'audio/m4a',
  'audio/flac',
  'audio/x-flac',
  'audio/ogg',
  'audio/webm',
];
const VIDEO_EXT = ['mp4', 'mov', 'm4v', 'webm'];
const VIDEO_MIME = ['video/mp4', 'video/quicktime', 'video/x-m4v', 'video/webm'];

// --- presigned URLs ----------------------------------------------------------

function signature(method, key, params) {
  const signed = new URLSearchParams(params);
  signed.delete('sig');
  signed.sort();
  return createHmac('sha256', SECRET).update(`${method}\n${key}\n${signed}`).digest('hex');
}

// A URL on the mock's own /r2, good for `seconds`. A PUT must send the
// signed content type, as R2 checks it.
export function presign(state, method, key, { seconds, type, extra = {} }) {
  const params = new URLSearchParams({ ...extra, expires: String(Math.floor(state.now() / 1000) + seconds) });
  if (type) params.set('type', type);
  params.set('sig', signature(method, key, params));
  return `${state.base}/r2/${key}?${params}`;
}

// R2 answers refusals in XML; the mock keeps the code and the message.
function denied(message) {
  return text(403, `AccessDenied: ${message}`);
}

function checkSigned(ctx, method) {
  const key = ctx.params['*'];
  const q = ctx.url.searchParams;
  const sig = q.get('sig') || '';
  if (sig !== signature(method, key, q)) return denied("Signature doesn't match");
  if (Number(q.get('expires')) * 1000 <= ctx.state.now()) return denied('Request has expired');
  const type = q.get('type');
  if (method === 'PUT' && type && (ctx.req.headers['content-type'] || '') !== type) {
    return denied("Signature doesn't match the content type");
  }
  return null;
}

function putObject(ctx) {
  const refused = checkSigned(ctx, 'PUT');
  if (refused) return refused;
  if (ctx.url.searchParams.has('uploadId')) return putPart(ctx);
  const body = ctx.raw;
  const object = {
    bytes: body,
    contentType: ctx.req.headers['content-type'] || '',
    etag: etagOf(body),
    sha256: sha256(body),
  };
  ctx.state.objects.set(ctx.params['*'], object);
  return bytes(200, Buffer.alloc(0), { etag: object.etag });
}

function getObject(ctx) {
  const refused = checkSigned(ctx, 'GET');
  if (refused) return refused;
  const object = ctx.state.objects.get(ctx.params['*']);
  if (!object) return text(404, 'NoSuchKey');
  return bytes(200, object.bytes, {
    'content-type': object.contentType || 'application/octet-stream',
    etag: object.etag,
  });
}

// --- the sign family ---------------------------------------------------------

function limitSigning(ctx, user) {
  limit(ctx.state, {
    scope: 'upload_sign',
    key: `u:${user.id}`,
    max: 240,
    windowMs: 60 * 60 * 1000,
    message: 'Too many upload requests. Slow down and try again shortly.',
  });
}

function checkSize(raw, max) {
  if (raw === undefined || raw === '') return null;
  const n = Number(raw);
  if (!Number.isFinite(n) || n < 0) throw error(400, 'Invalid file size');
  if (n === 0) throw error(400, 'File is empty');
  if (n > max) throw error(413, `File too large (max ${Math.round(max / MB)} MB)`);
  return Math.floor(n);
}

function checkMime(raw, allowed, label) {
  if (!raw) throw error(400, `Missing ${label} type`);
  if (!allowed.includes(raw.toLowerCase())) throw error(400, `Unsupported ${label} type: ${raw}`);
  return raw.toLowerCase();
}

// The server mints `track_<ms>_<9 base36>`; the mock counts instead of
// drawing random letters, so runs repeat.
function newTrackId(state) {
  return `track_${state.now()}_${nextId(state, 'trackIds').toString(36).padStart(9, '0')}`;
}

function own(state, user, trackId, s3Key) {
  state.uploads.push({ trackId, userId: user.id, s3Key, createdAt: iso(state.now()) });
}

function ownedTrack(state, user, trackId) {
  if (!trackId) throw error(400, 'Missing trackId');
  const upload = state.uploads.find((u) => u.trackId === trackId);
  if (!upload) throw error(404, 'Upload not found');
  if (upload.userId !== user.id) throw error(403, 'Not authorized');
}

function sign(ctx) {
  const user = requireUser(ctx);
  limitSigning(ctx, user);
  const type = checkMime(ctx.query.fileType || 'audio/wav', AUDIO_MIME, 'audio');
  checkSize(ctx.query.size, MAX_AUDIO);
  const trackId = newTrackId(ctx.state);
  const s3Key = `uploads/${trackId}`;
  own(ctx.state, user, trackId, s3Key);
  return json(200, { signedUrl: presign(ctx.state, 'PUT', s3Key, { seconds: 300, type }), s3Key, trackId });
}

function signVideo(ctx) {
  const user = requireUser(ctx);
  limitSigning(ctx, user);
  const ext = String(ctx.query.ext || 'mp4')
    .replace(/^\./, '')
    .toLowerCase();
  if (!VIDEO_EXT.includes(ext)) throw error(400, `Unsupported video extension: ${ctx.query.ext}`);
  const type = ctx.query.fileType ? checkMime(ctx.query.fileType, VIDEO_MIME, 'video') : 'video/mp4';
  checkSize(ctx.query.size, MAX_VIDEO);
  const s3Key = `videos/${user.id}/${ctx.state.now()}.${ext}`;
  return json(200, { signedUrl: presign(ctx.state, 'PUT', s3Key, { seconds: 600, type }), s3Key });
}

// A new version's object under the same trackId (6.8). Publishing it with
// its s3Key applies the id rules.
function signReplace(ctx) {
  const user = requireUser(ctx);
  limitSigning(ctx, user);
  ownedTrack(ctx.state, user, ctx.query.trackId);
  const type = checkMime(ctx.query.fileType || 'audio/wav', AUDIO_MIME, 'audio');
  checkSize(ctx.query.size, MAX_AUDIO);
  const s3Key = `uploads/${ctx.query.trackId}_v${ctx.state.now()}`;
  own(ctx.state, user, ctx.query.trackId, s3Key);
  return json(200, { signedUrl: presign(ctx.state, 'PUT', s3Key, { seconds: 600, type }), s3Key });
}

// --- multipart ---------------------------------------------------------------

function partLength(upload, n) {
  return n < upload.total ? upload.partSize : upload.size - (upload.total - 1) * upload.partSize;
}

function createParts(ctx) {
  const user = requireUser(ctx);
  limitSigning(ctx, user);
  const size = checkSize(ctx.body.size, MAX_MULTIPART);
  if (size === null) throw error(400, 'Invalid file size');
  let trackId = ctx.body.trackId;
  let s3Key;
  if (trackId) {
    ownedTrack(ctx.state, user, trackId);
    s3Key = `uploads/${trackId}_v${ctx.state.now()}`;
  } else {
    trackId = newTrackId(ctx.state);
    s3Key = `uploads/${trackId}`;
  }
  own(ctx.state, user, trackId, s3Key);
  const upload = {
    uploadId: `mpu_${randomBytes(9).toString('base64url')}`,
    userId: user.id,
    trackId,
    s3Key,
    size,
    contentType: ctx.body.fileType || '',
    partSize: PART_SIZE,
    total: Math.ceil(size / PART_SIZE),
    parts: new Map(), // n -> { bytes, etag, sends }
    done: null,
  };
  ctx.state.multipart.set(upload.uploadId, upload);
  return json(200, { uploadId: upload.uploadId, trackId, s3Key, partSize: PART_SIZE, parts: upload.total });
}

function ownedUpload(ctx) {
  const user = requireUser(ctx);
  const upload = ctx.state.multipart.get(ctx.params.uploadId);
  if (!upload) throw error(404, 'No upload there');
  if (upload.userId !== user.id) throw error(403, 'Not authorized');
  return { user, upload };
}

function signPart(ctx) {
  const { user, upload } = ownedUpload(ctx);
  limitSigning(ctx, user);
  const n = Number(ctx.params.n);
  if (!Number.isInteger(n) || n < 1 || n > upload.total) throw error(400, `There is no part ${ctx.params.n}`);
  if (upload.done) throw error(409, 'That upload is finished');
  const signedUrl = presign(ctx.state, 'PUT', upload.s3Key, {
    seconds: 300,
    extra: { uploadId: upload.uploadId, partNumber: String(n) },
  });
  return json(200, { signedUrl, n, size: partLength(upload, n) });
}

// Every PUT of a part is counted in `sends`, so a test can see a finished
// part sent again (PLAN S1.9).
function putPart(ctx) {
  const q = ctx.url.searchParams;
  const upload = ctx.state.multipart.get(q.get('uploadId'));
  const n = Number(q.get('partNumber'));
  if (!upload || upload.s3Key !== ctx.params['*']) return text(404, 'NoSuchUpload');
  if (upload.done) return text(409, 'That upload is finished');
  const want = partLength(upload, n);
  if (ctx.raw.length !== want) return text(400, `Part ${n} should be ${want} bytes`);
  const sends = (upload.parts.get(n)?.sends ?? 0) + 1;
  const part = { bytes: ctx.raw, etag: etagOf(ctx.raw), sends };
  upload.parts.set(n, part);
  return bytes(200, Buffer.alloc(0), { etag: part.etag });
}

const unquote = (etag) => String(etag || '').replace(/"/g, '');

function completeParts(ctx) {
  const { upload } = ownedUpload(ctx);
  if (!upload.done) {
    const given = new Map((Array.isArray(ctx.body.parts) ? ctx.body.parts : []).map((p) => [Number(p?.n), p?.etag]));
    for (let n = 1; n <= upload.total; n++) {
      const part = upload.parts.get(n);
      if (!part) throw error(400, `Part ${n} hasn't arrived`);
      if (unquote(given.get(n)) !== unquote(part.etag)) throw error(400, `Part ${n}'s etag doesn't match what arrived`);
    }
    const whole = Buffer.concat([...upload.parts.keys()].sort((a, b) => a - b).map((n) => upload.parts.get(n).bytes));
    const object = { bytes: whole, contentType: upload.contentType, etag: etagOf(whole), sha256: sha256(whole) };
    ctx.state.objects.set(upload.s3Key, object);
    upload.done = { trackId: upload.trackId, s3Key: upload.s3Key, size: whole.length, sha256: object.sha256 };
    for (const part of upload.parts.values()) part.bytes = null;
  }
  return json(200, upload.done);
}

export const routes = [
  ['GET', '/api/upload/sign', sign],
  ['GET', '/api/upload/sign-video', signVideo],
  ['GET', '/api/upload/sign-replace', signReplace],
  ['POST', '/api/upload/parts', createParts],
  ['GET', '/api/upload/parts/:uploadId/:n', signPart],
  ['POST', '/api/upload/parts/:uploadId/complete', completeParts],
  ['PUT', '/r2/*', putObject],
  ['GET', '/r2/*', getObject],
];
