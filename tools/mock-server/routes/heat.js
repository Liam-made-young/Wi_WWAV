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

// A cursor is a place in the account's log. One past its end (from before
// a reset or a restore) would hide the next changes, so it is refused and
// the device pulls again from 0.
function readCursor(log, raw) {
  const cursor = Number(raw ?? 0);
  if (!Number.isInteger(cursor) || cursor < 0) throw error(400, "That cursor isn't readable");
  if (cursor > log.entries.length) {
    throw error(409, "That cursor is past the end of this account's changes. Pull again from 0.", {
      code: 'cursor_ahead',
    });
  }
  return cursor;
}

const isCurrent = (log, e) => log.current.get(fieldKey(e)) === e;

// POST {device, cursor?, changes: [{kind, id, field, value, seq}]} →
// {cursor, kept}. `kept` lists every pushed field where the server already
// held something newer, with that newer value, so the device can take it.
// The answer's cursor is the one the device sent (0 if none), moved past
// the device's own changes and any overwritten since, and no further: a
// change another device made in between still waits in front of it.
function push(ctx) {
  const me = requireUser(ctx);
  const { device, changes } = ctx.body;
  if (typeof device !== 'string' || !NAME.test(device)) return error(400, 'Say which device this is');
  if (!Array.isArray(changes)) return error(400, 'Send a list of changes');
  if (changes.length > MAX_BATCH) return error(413, `Send at most ${MAX_BATCH} changes at a time`);
  const incoming = changes.map(readChange);
  const log = logOf(ctx.state, me);
  let cursor = readCursor(log, ctx.body.cursor);
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
  for (const e of log.entries.slice(cursor)) {
    if (e.device !== device && isCurrent(log, e)) break;
    cursor = e.n;
  }
  return json(200, { cursor, kept });
}

// GET ?cursor=n&limit= → {changes, cursor, more}. Only each field's current
// value comes back: a value overwritten since is skipped, not sent twice.
function pull(ctx) {
  const me = requireUser(ctx);
  const log = logOf(ctx.state, me);
  const from = readCursor(log, ctx.query.cursor);
  const limitTo = Math.min(MAX_PULL, Math.max(1, Number(ctx.query.limit) || MAX_PULL));
  const out = [];
  let cursor = from;
  for (const entry of log.entries.slice(from)) {
    if (!isCurrent(log, entry)) {
      cursor = entry.n;
      continue;
    }
    if (out.length === limitTo) break;
    out.push(entryPayload(entry));
    cursor = entry.n;
  }
  const more = log.entries.slice(cursor).some((e) => isCurrent(log, e));
  return json(200, { changes: out, cursor, more });
}

// The public Heat view (docs/SPEC.md 3.15, 8.7): what anyone who opens a
// person's sun sees. Built from the same log, so it is the synced copies
// and nothing else. A record shows only when its `public` field is true, and
// only the fields 3.15 lists for its kind. Grades reach the server only once
// they are switched public, and switching back deletes the copy, which
// arrives here as `deleted`. Never a total, a count or a comparison.
const SHOWN = {
  task: ['title', 'due', 'done'],
  project: ['title', 'status', 'targetDate'],
  milestone: ['title', 'date', 'done'],
  habit: ['title', 'log'],
  note: ['title', 'markdown'],
  dailyNote: ['date', 'markdown'],
  course: ['code', 'name'],
  grade: ['title', 'score', 'outOf'],
  focusSession: ['startedAt', 'focusMin'],
};

function records(log) {
  const out = new Map();
  for (const e of log.current.values()) {
    const key = `${e.kind}\u0000${e.id}`;
    if (!out.has(key)) out.set(key, { kind: e.kind, id: e.id, fields: {} });
    out.get(key).fields[e.field] = e.value;
  }
  for (const [key, r] of out) if (r.fields.deleted === true) out.delete(key);
  return [...out.values()];
}

function shown(r, all) {
  const item = { id: r.id };
  for (const name of SHOWN[r.kind]) if (name in r.fields) item[name] = r.fields[name];
  // A grade names its course, and a focus record its task, by title: the
  // course's code and the task's title, never anything else of theirs.
  const find = (kind, id) => all.find((o) => o.kind === kind && o.id === id);
  if (r.kind === 'grade') item.course = find('course', r.fields.courseId)?.fields.code ?? null;
  if (r.kind === 'focusSession') item.task = find('task', r.fields.taskId)?.fields.title ?? null;
  return item;
}

function publicView(ctx) {
  const id = Number(ctx.params.userId);
  if (!Number.isSafeInteger(id) || !ctx.state.users.some((u) => u.id === id)) return error(404, 'No such account');
  const log = ctx.state.heat.get(id) ?? { entries: [], current: new Map() };
  const all = records(log);
  const now = ctx.state.now();
  const shares = all.filter((r) => r.kind === 'profileShare');
  const live = (r) => !(typeof r.fields.clearsAt === 'number' && r.fields.clearsAt <= now);
  const nowLine = shares.find((r) => r.fields.kind === 'now' && live(r) && typeof r.fields.text === 'string');
  const timelines = shares
    .filter((r) => r.fields.kind === 'timeline')
    .map((r) => {
      const project = all.find((o) => o.kind === 'project' && o.id === r.fields.sourceId);
      const beads = all
        .filter((o) => o.kind === 'milestone' && o.fields.projectId === r.fields.sourceId)
        .sort((a, b) => String(a.fields.date).localeCompare(String(b.fields.date)) || (a.fields.order ?? 0) - (b.fields.order ?? 0))
        .map((o) => ({ id: o.id, title: o.fields.title, date: o.fields.date, done: o.fields.done === true }));
      return { projectId: r.fields.sourceId, targetId: r.fields.targetId, title: project?.fields.title ?? null, milestones: beads };
    })
    .filter((t) => t.title !== null);
  const items = {};
  for (const kind of Object.keys(SHOWN)) {
    const list = all.filter((r) => r.kind === kind && r.fields.public === true).map((r) => shown(r, all));
    if (list.length) items[kind] = list;
  }
  return json(200, { now: nowLine ? { text: nowLine.fields.text } : null, timelines, items });
}

export const routes = [
  ['GET', '/api/heat/changes', pull],
  ['POST', '/api/heat/changes', push],
  ['GET', '/api/heat/public/:userId', publicView],
];
