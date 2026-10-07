// A ULID's first ten characters are the time it was made, in milliseconds
// (docs/SPEC.md 2.5). A task Claude added keeps no date of its own, so Get
// Info reads "Claude, Oct 6 8:41 AM" from the id the core gave it.

const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

/** When a ULID was made, or null for an id that isn't one (an imported UID, a hand-made id). */
export function ulidTime(id: string): number | null {
  if (!/^[0-9A-HJKMNP-TV-Z]{26}$/i.test(id)) return null;
  let ms = 0;
  for (const c of id.slice(0, 10).toUpperCase()) ms = ms * 32 + ALPHABET.indexOf(c);
  // 2001 to 2100: anything else is a lookalike.
  return ms > 978_307_200_000 && ms < 4_102_444_800_000 ? ms : null;
}

/** A ULID for `ms`, with a fixed tail, for fixtures. */
export function ulidAt(ms: number, tail = '0'.repeat(16)): string {
  let time = '';
  for (let i = 0, t = ms; i < 10; i++, t = Math.floor(t / 32)) time = ALPHABET[t % 32] + time;
  return time + tail;
}
