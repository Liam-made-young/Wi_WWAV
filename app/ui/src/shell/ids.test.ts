import { describe, expect, it } from 'vitest';
import { ulid } from './ids';

// Records the shell writes (a capture) take a ULID, as every record does
// (docs/COMMANDS.md). What a fail looks like: an id that isn't 26 Crockford
// characters, or ids that don't sort by creation as plain strings, even
// within one millisecond.

describe('ulid', () => {
  it('is 26 Crockford base-32 characters, the time first', () => {
    const id = ulid(Date.UTC(2026, 9, 7));
    expect(id).toMatch(/^[0-9A-HJKMNP-TV-Z]{26}$/);
    expect(id.slice(0, 10)).toBe('01M49THV00');
  });

  it('sorts by creation, within a millisecond too', () => {
    const ids = [ulid(1000), ulid(1000), ulid(1000), ulid(999), ulid(2000)];
    expect(ids[0] < ids[1] && ids[1] < ids[2]).toBe(true);
    expect(ids[3] > ids[2]).toBe(true); // a clock step back still sorts after the last
    expect(ids[4] > ids[3]).toBe(true);
  });
});
