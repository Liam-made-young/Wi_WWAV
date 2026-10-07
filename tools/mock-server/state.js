// The mock's whole world, in memory. Tests read it directly (it is what
// startMockServer returns as `state`); /__mock/state shows it over HTTP.
// fixtures.js fills it.
import { canonical, sha256 } from './hash.js';
import { error, json, Reply } from './http.js';

// The clock runs from `startAt` (default: the real time at start) at real
// speed, plus whatever /__mock/clock has added, so expiry is testable.
export function createState({ startAt } = {}) {
  const state = { base: '' };
  const started = Date.now();
  const origin = startAt ?? started;
  state.offset = 0;
  state.now = () => origin + (Date.now() - started) + state.offset;
  reset(state);
  return state;
}

export function reset(state) {
  state.offset = 0;
  state.counters = {};
  state.users = [];
  state.galaxies = [];
  state.systems = [];
  state.suns = [];
  state.planets = [];
  state.tracks = []; // PublishedTrack rows, each with its kept versions
  state.uploads = []; // who signed which object: ownership starts at sign time
  state.objects = new Map(); // the R2 stand-in: key -> { bytes, contentType, etag, sha256 }
  state.multipart = new Map();
  state.links = [];
  state.saved = [];
  state.letters = [];
  state.listings = [];
  state.purchases = [];
  state.sessions = new Map(); // fake Stripe Checkout sessions
  state.transfers = [];
  state.payouts = [];
  state.bags = new Map();
  state.heat = new Map();
  state.assist = new Map();
  state.oauth = { requests: new Map(), codes: new Map(), devices: new Map() };
  state.loginAttempts = new Map(); // email -> { count, last }: wrong passwords
  state.limits = new Map();
  state.idempotency = new Map();
  state.faults = [];
  state.latest = null;
}

export function nextId(state, table) {
  state.counters[table] = (state.counters[table] ?? 0) + 1;
  return state.counters[table];
}

export function iso(ms) {
  return new Date(ms).toISOString();
}

// The server's in-memory limiter (middleware/rateLimit.js): a fixed window
// per key, and a 429 with Retry-After once it is used up.
export function limit(state, { scope, key, max, windowMs, message }) {
  const id = `${scope}|${key}`;
  const now = state.now();
  let entry = state.limits.get(id);
  if (!entry || now - entry.start > windowMs) {
    entry = { used: 0, start: now };
    state.limits.set(id, entry);
  }
  entry.used += 1;
  if (entry.used > max) {
    const retryAfterSeconds = Math.max(1, Math.ceil((entry.start + windowMs - now) / 1000));
    throw json(
      429,
      { error: message, code: 'rate_limited', scope, retryAfterSeconds },
      { 'retry-after': String(retryAfterSeconds) },
    );
  }
}

const KEY_RE = /^[A-Za-z0-9_\-:.]{8,120}$/;
const DAY_MS = 24 * 60 * 60 * 1000;

// The server's Idempotency-Key middleware: the same key and body replay the
// stored answer; the same key with another body is a conflict.
export async function idempotent(ctx, user, scope, required, run) {
  const key = ctx.req.headers['idempotency-key'] || ctx.req.headers['x-idempotency-key'];
  if (!key) {
    if (required) {
      throw error(400, 'Idempotency-Key header is required for this action', { code: 'idempotency_key_required' });
    }
    return run();
  }
  if (!KEY_RE.test(key)) {
    throw error(400, 'Invalid Idempotency-Key. Use 8–120 chars, [A-Za-z0-9_-.:]', { code: 'idempotency_key_invalid' });
  }
  const { state } = ctx;
  const id = `${user.id}|${scope}|${key}`;
  const hash = sha256(canonical(ctx.body));
  const prior = state.idempotency.get(id);
  if (prior && state.now() - prior.at < DAY_MS) {
    if (prior.hash !== hash) {
      throw error(409, 'Idempotency-Key already used with a different request body', { code: 'idempotency_conflict' });
    }
    const r = prior.reply;
    return new Reply(r.status, { ...r.headers, 'idempotent-replay': 'true' }, r.body, r.value);
  }
  let reply;
  try {
    reply = await run();
  } catch (thrown) {
    if (!(thrown instanceof Reply)) throw thrown;
    reply = thrown;
  }
  if (reply.status < 500) state.idempotency.set(id, { hash, reply, at: state.now() });
  return reply;
}
