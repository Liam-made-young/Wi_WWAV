// Answers, routing and bodies. A handler returns (or throws) a Reply, so a
// check deep inside one can end the request with a plain `throw error(...)`.

export class Reply {
  constructor(status, headers, body, value) {
    this.status = status;
    this.headers = headers;
    this.body = body;
    this.value = value; // the JSON value, kept for the onResponse hook
  }
}

export function json(status, value, headers = {}) {
  const body = Buffer.from(JSON.stringify(value));
  return new Reply(status, { 'content-type': 'application/json; charset=utf-8', ...headers }, body, value);
}

export function html(status, text, headers = {}) {
  return new Reply(status, { 'content-type': 'text/html; charset=utf-8', ...headers }, Buffer.from(text));
}

export function text(status, words) {
  return new Reply(status, { 'content-type': 'text/plain; charset=utf-8' }, Buffer.from(words));
}

export function bytes(status, buffer, headers = {}) {
  return new Reply(status, headers, buffer);
}

export function redirect(location) {
  return new Reply(302, { location }, Buffer.alloc(0));
}

// The legacy routes' error: { error: "a sentence" }, sometimes with a code.
export function error(status, message, extra = {}) {
  return json(status, { error: message, ...extra });
}

// /api/v2's envelope (server routes/v2/_shared.js).
export function ok(data) {
  return json(200, { ok: true, data });
}

export function fail(status, code, message) {
  return json(status, { ok: false, error: { code, message } });
}

const ENTITIES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

export function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (ch) => ENTITIES[ch]);
}

// A page of the mock's own HTML: the sign-in forms and the fake Stripe.
export function page(title, inner) {
  return `<!doctype html><html lang="en"><meta charset="utf-8"><title>${escapeHtml(title)}</title>
<body style="font: 17px/1.6 system-ui; max-width: 30em; margin: 3em auto">
<h1>${escapeHtml(title)}</h1>
${inner}
</body></html>`;
}

// Routes are [method, path, handler]. A path segment ":name" is a
// parameter, and a final "*" takes the rest of the path (an R2 key).
export function router(table) {
  const routes = table.map(([method, path, handler]) => ({ method, path, parts: path.split('/'), handler }));
  return {
    names: routes.map((r) => `${r.method} ${r.path}`),
    match(method, pathname) {
      const segments = pathname.split('/');
      for (const route of routes) {
        if (route.method !== method) continue;
        let params;
        try {
          params = matchParts(route.parts, segments);
        } catch (e) {
          // A broken percent-escape, such as %E0%A4%A.
          if (e instanceof URIError) throw error(400, "That path isn't readable");
          throw e;
        }
        if (params) return { route, params };
      }
      return null;
    },
  };
}

function matchParts(parts, segments) {
  const params = {};
  for (let i = 0; i < parts.length; i++) {
    if (parts[i] === '*') {
      params['*'] = segments.slice(i).map(decodeURIComponent).join('/');
      return params;
    }
    if (i >= segments.length) return null;
    if (parts[i].startsWith(':')) params[parts[i].slice(1)] = decodeURIComponent(segments[i]);
    else if (parts[i] !== segments[i]) return null;
  }
  return parts.length === segments.length ? params : null;
}

export async function readBody(req, limit) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > limit) throw error(413, 'Request body is too large');
    chunks.push(chunk);
  }
  return Buffer.concat(chunks);
}

// JSON for JSON, an object for a form, nothing for anything else (raw
// bodies such as an R2 PUT are read from ctx.raw).
export function parseBody(req, raw) {
  const type = (req.headers['content-type'] || '').split(';')[0].trim();
  if (type === 'application/json') {
    if (raw.length === 0) return {};
    let value;
    try {
      value = JSON.parse(raw.toString('utf8'));
    } catch {
      throw error(400, "Request body isn't JSON");
    }
    // Objects and arrays only, as express.json()'s strict mode takes them:
    // a body of null, 3 or "x" would otherwise reach handlers as ctx.body.
    if (value === null || typeof value !== 'object') throw error(400, 'Request body must be a JSON object');
    return value;
  }
  if (type === 'application/x-www-form-urlencoded') {
    return Object.fromEntries(new URLSearchParams(raw.toString('utf8')));
  }
  return {};
}

export function isObject(v) {
  return v !== null && typeof v === 'object' && !Array.isArray(v);
}
