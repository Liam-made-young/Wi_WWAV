// Heat sync (docs/SPEC.md 2.8, 3.15, 9.7): private to each account, pulled
// with a cursor and pushed in batches. Every change is one field of one
// record with a sequence number, and the server keeps the higher one field
// by field, so a slow older write never overwrites a newer one.
//
// A device's counter runs above every sequence number it has pulled (a
// Lamport clock), so "higher" means "later" across devices too. Equal
// numbers from two devices settle by the higher device id, so every
// replica picks the same winner. A deleted record is a field `deleted`
// set to true.
import { error, isObject, json } from '../http.js';
import { requireUser } from '../auth.js';

const MAX_BATCH = 500;
const MAX_PULL = 500;
const NAME = /^[A-Za-z0-9_.:-]{1,64}$/;

function logOf(state, user) {
  if (!state.heat.has(user.id)) state.heat.set(user.id, { entries: [], current: new Map() });
  return state.heat.get(user.id);
}

const fieldKey = (c) => `${c.kind}\u0000${c.id}\u0000${c.field}`;

function newer(change, held) {
  return change.seq > held.seq || (change.seq === held.seq && change.device > held.device);
}

function readChange(raw) {
  if (!isObject(raw)) throw error(400, 'A change is an object');
  for (const name of ['kind', 'id', 'field']) {
    if (typeof raw[name] !== 'string' || !NAME.test(raw[name])) throw error(400, `A change needs a ${name}`);
  }
  if (!Number.isSafeInteger(raw.seq) || raw.seq < 1) throw error(400, 'A sequence number is a whole number from 1');
  if (!('value' in raw)) throw error(400, 'A change needs a value');
  return { kind: raw.kind, id: raw.id, field: raw.field, value: raw.value, seq: raw.seq };
}

function entryPayload(e) {
  return { kind: e.kind, id: e.id, field: e.field, value: e.value, seq: e.seq, device: e.device };
}

// POST {device, changes: [{kind, id, field, value, seq}]} → {cursor, kept}.
// `kept` lists every pushed field where the server already held something
// newer, with that newer value, so the device can take it.
function push(ctx) {
  const me = requireUser(ctx);
  const { device, changes } = ctx.body;
  if (typeof device !== 'string' || !NAME.test(device)) return error(400, 'Say which device this is');
  if (!Array.isArray(changes)) return error(400, 'Send a list of changes');
  if (changes.length > MAX_BATCH) return error(413, `Send at most ${MAX_BATCH} changes at a time`);
  const incoming = changes.map(readChange);
  const log = logOf(ctx.state, me);
  const kept = [];
  for (const change of incoming) {
    const key = fieldKey(change);
    const held = log.current.get(key);
    if (held && !newer({ ...change, device }, held)) {
      if (held.seq !== change.seq || held.device !== device) kept.push(entryPayload(held));
      continue;
    }
    const entry = { n: log.entries.length + 1, ...change, device };
    log.entries.push(entry);
    log.current.set(key, entry);
  }
  return json(200, { cursor: log.entries.length, kept });
}

// GET ?cursor=n&limit= → {changes, cursor, more}. Only each field's current
// value comes back: a value overwritten since is skipped, not sent twice.
function pull(ctx) {
  const me = requireUser(ctx);
  const log = logOf(ctx.state, me);
  const from = Number(ctx.query.cursor ?? 0);
  if (!Number.isInteger(from) || from < 0) return error(400, "That cursor isn't readable");
  const limitTo = Math.min(MAX_PULL, Math.max(1, Number(ctx.query.limit) || MAX_PULL));
  const out = [];
  let cursor = from;
  for (const entry of log.entries.slice(from)) {
    if (log.current.get(fieldKey(entry)) !== entry) {
      cursor = entry.n;
      continue;
    }
    if (out.length === limitTo) break;
    out.push(entryPayload(entry));
    cursor = entry.n;
  }
  const more = log.entries.slice(cursor).some((e) => log.current.get(fieldKey(e)) === e);
  return json(200, { changes: out, cursor, more });
}

export const routes = [
  ['GET', '/api/heat/changes', pull],
  ['POST', '/api/heat/changes', push],
];
