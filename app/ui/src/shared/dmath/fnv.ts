// FNV-1a over a string's UTF-8 bytes: the one sanctioned source of jitter
// for anything laid out in space. iOS (`StableHash.swift`) and the server
// (`server/utils/stableHash.js`) hash the same way, so a galaxy lands in the
// same place on the iPhone, the server's map and here. Integer arithmetic
// only, so every engine gives the same answer.

const utf8 = new TextEncoder();

const OFFSET_64 = 0xcbf29ce484222325n;
const PRIME_64 = 0x100000001b3n;

export function fnv1a64(s: string): bigint {
  let h = OFFSET_64;
  for (const byte of utf8.encode(s)) {
    h = BigInt.asUintN(64, (h ^ BigInt(byte)) * PRIME_64);
  }
  return h;
}

export function fnv1a32(s: string): number {
  let h = 0x811c9dc5;
  for (const byte of utf8.encode(s)) {
    h = Math.imul(h ^ byte, 0x01000193);
  }
  return h >>> 0;
}

// A number in [0, 1) for (value, salt). The key is the salt, a bar, then
// the value, exactly as iOS builds it; the top 53 bits fill a double's
// mantissa, so the conversion is exact.
export function unitJitter(value: string, salt: string): number {
  return Number(fnv1a64(`${salt}|${value}`) >> 11n) / 2 ** 53;
}
