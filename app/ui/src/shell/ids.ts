// ULIDs for the records the shell writes, as wwav-ids makes them in Rust:
// 48 bits of milliseconds then 80 random bits, in Crockford's base 32, so
// they sort by creation as plain strings. Within one millisecond, or when
// the clock steps back, the last id is counted up instead (docs/DECISIONS.md).

const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

let lastTime = -1;
let lastRandom: number[] = [];

export function ulid(now = Date.now()): string {
  let random: number[];
  if (now <= lastTime) {
    now = lastTime;
    random = countUp(lastRandom);
  } else {
    random = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b % 32);
  }
  lastTime = now;
  lastRandom = random;
  let time = '';
  for (let i = 0, t = now; i < 10; i++, t = Math.floor(t / 32)) time = ALPHABET[t % 32] + time;
  return time + random.map((d) => ALPHABET[d]).join('');
}

// The 16 random digits plus one, carrying leftward.
function countUp(digits: number[]): number[] {
  const next = [...digits];
  for (let i = next.length - 1; i >= 0; i--) {
    if (next[i] < 31) {
      next[i]++;
      return next;
    }
    next[i] = 0;
  }
  return next;
}
