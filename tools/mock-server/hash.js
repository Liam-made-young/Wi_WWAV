// The stable hashes the server and the clients share, and the digests a
// receipt carries.
import { createHash } from 'node:crypto';

const MASK64 = 0xffffffffffffffffn;
const MASK128 = (1n << 128n) - 1n;

// 64-bit FNV-1a over UTF-8 (server utils/stableHash.js, iOS StableHash).
export function fnv1a64(s) {
  let hash = 0xcbf29ce484222325n;
  for (const byte of Buffer.from(String(s), 'utf8')) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & MASK64;
  }
  return hash;
}

// A jitter in [0, 1) for (value, salt), as unitJitter on the server.
export function unitJitter(s, salt) {
  return Number(fnv1a64(`${salt}|${s}`) >> 11n) / 2 ** 53;
}

// A platform track's song_id: FNV-1a 128 of "wwav-track:" + trackId, so a
// song has the same id everywhere (docs/SPEC.md 6.1, server wiDisc.js).
export function platformSongId(trackId) {
  let hash = 0x6c62272e07bb014262b821756295c58dn;
  for (const byte of Buffer.from(`wwav-track:${trackId}`, 'utf8')) {
    hash ^= BigInt(byte);
    hash = (hash * 0x0000000001000000000000000000013bn) & MASK128;
  }
  return hash.toString(16).padStart(32, '0');
}

export function sha256(buffer) {
  return createHash('sha256').update(buffer).digest('hex');
}

// R2 and S3 answer a PUT with the MD5 of the bytes, in quotes, as its ETag.
export function etagOf(buffer) {
  return `"${createHash('md5').update(buffer).digest('hex')}"`;
}

// JSON with sorted keys, so two equal bodies hash the same (the server's
// idempotency middleware compares bodies this way).
export function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (value && typeof value === 'object') {
    const keys = Object.keys(value).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonical(value[k])}`).join(',')}}`;
  }
  return JSON.stringify(value) ?? 'null';
}
