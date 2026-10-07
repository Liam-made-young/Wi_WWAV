import { describe, expect, test } from 'vitest';
import { fnv1a32, fnv1a64, unitJitter } from './fnv';

describe('FNV-1a 64 over UTF-8', () => {
  // The three parity pins shared by the server (server/utils/stableHash.js)
  // and iOS (ios_v4/WWAVTests/KeyColorTests.swift), plus the offset basis.
  test('matches the iOS and server vectors', () => {
    expect(fnv1a64('')).toBe(0xcbf29ce484222325n);
    expect(fnv1a64('sky|galaxy-1')).toBe(0x3a54cc2cbfcbfdccn);
    expect(fnv1a64('rotation|demo')).toBe(0x9f304bef9e3c2fdan);
    expect(fnv1a64('ring|galaxy-42|3')).toBe(0x77d9944dd12bf887n);
  });

  test('matches the published FNV vectors', () => {
    expect(fnv1a64('a')).toBe(0xaf63dc4c8601ec8cn);
    expect(fnv1a64('foobar')).toBe(0x85944171f73967e8n);
  });

  // Hashed as UTF-8 bytes, not UTF-16 code units (web v4's mistake):
  // pinned against the server's Buffer-based port.
  test('hashes non-ASCII text as UTF-8', () => {
    expect(fnv1a64('key|Café ♯ 🌍')).toBe(0x6b9ff285685383f6n);
  });
});

describe('unitJitter', () => {
  test('is salt, a bar, then the value, as on iOS and the server', () => {
    expect(unitJitter('galaxy-1', 'sky')).toBe(0.22785640804777063);
    expect(unitJitter('demo', 'rotation')).toBe(0.6218306980137959);
    expect(unitJitter('3', 'ring|galaxy-42')).toBe(0.4681637468321894);
    expect(unitJitter('Café ♯ 🌍', 'key')).toBe(0.42040935284633285);
  });

  test('stays in [0, 1) and differs by salt', () => {
    const j = unitJitter('track_123', 'angle');
    expect(j).toBeGreaterThanOrEqual(0);
    expect(j).toBeLessThan(1);
    expect(unitJitter('track_123', 'radius')).not.toBe(j);
  });
});

describe('FNV-1a 32 over UTF-8', () => {
  test('matches the published FNV vectors', () => {
    expect(fnv1a32('')).toBe(0x811c9dc5);
    expect(fnv1a32('a')).toBe(0xe40c292c);
    expect(fnv1a32('foobar')).toBe(0xbf9cf968);
  });

  test('is unsigned', () => {
    expect(fnv1a32('sky|galaxy-1')).toBeGreaterThanOrEqual(0);
  });
});
