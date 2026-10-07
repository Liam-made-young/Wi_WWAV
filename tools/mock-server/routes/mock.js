// Test controls, under /__mock so nothing in the app's contract can reach
// them by accident. Another language's tests drive the mock with these the
// way Node tests use `state` directly.
import { error, html, isObject, json, page } from '../http.js';
import { seed } from '../fixtures.js';
import { reset } from '../state.js';

// The seed again, and the clock back to its start.
function resetAll(ctx) {
  reset(ctx.state);
  seed(ctx.state);
  return json(200, { reset: true });
}

function clock(ctx) {
  const ms = Number(ctx.body.advanceMs);
  if (!Number.isFinite(ms) || ms < 0) return error(400, 'advanceMs is a number of milliseconds');
  ctx.state.offset += ms;
  return json(200, { now: new Date(ctx.state.now()).toISOString() });
}

// The next `times` requests to {method, path} fail: with `status` and no
// work done, by dropping the connection before any work ("before"), or by
// doing the work and then dropping the connection, so the answer is lost
// ("after").
function fault(ctx) {
  const { method, path, status, drop, times = 1 } = ctx.body;
  if (typeof method !== 'string' || typeof path !== 'string') return error(400, 'Say which method and path');
  if (drop !== undefined && drop !== 'before' && drop !== 'after') return error(400, 'drop is "before" or "after"');
  if (drop === undefined && !Number.isInteger(status)) return error(400, 'Give a status or a drop');
  ctx.state.faults.push({ method: method.toUpperCase(), path, status, drop, times });
  return json(200, { queued: true });
}

function setLatest(ctx) {
  if (!isObject(ctx.body)) return error(400, 'Send a manifest');
  ctx.state.latest = ctx.body;
  return json(200, ctx.body);
}

// Everything, as JSON: Maps become objects and file bytes become their size.
function snapshot(ctx) {
  const text = JSON.stringify(ctx.state, function (key, value) {
    const raw = this[key];
    if (Buffer.isBuffer(raw)) return { size: raw.length };
    if (raw instanceof Map) return Object.fromEntries(raw);
    if (typeof raw === 'function') return undefined;
    return value;
  });
  return json(200, JSON.parse(text));
}

function home() {
  return html(200, page('mi-wwav.com (mock)', "<p>A stand-in for mi-wwav.com, for Wi_WWAV's tests.</p>"));
}

export const routes = [
  ['GET', '/', home],
  ['POST', '/__mock/reset', resetAll],
  ['POST', '/__mock/clock', clock],
  ['POST', '/__mock/fail', fault],
  ['PUT', '/__mock/latest', setLatest],
  ['GET', '/__mock/state', snapshot],
];
