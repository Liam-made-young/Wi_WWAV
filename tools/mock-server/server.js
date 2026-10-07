// A stand-in for mi-wwav.com that follows docs/SPEC.md's contract, for the
// desktop app's tests: `node tools/mock-server/server.js --port 8787`, or
// `startMockServer({ port })` from a test. README.md lists every endpoint.
import http from 'node:http';
import { pathToFileURL } from 'node:url';
import { Reply, error, parseBody, readBody, router } from './http.js';
import { createState } from './state.js';
import { seed } from './fixtures.js';
import { routes as accounts } from './routes/accounts.js';
import { routes as uploads } from './routes/uploads.js';
import { routes as publish } from './routes/publish.js';
import { routes as forks } from './routes/forks.js';
import { routes as space } from './routes/space.js';
import { routes as heat } from './routes/heat.js';
import { routes as updates } from './routes/updates.js';
import { routes as mock } from './routes/mock.js';

const table = router([
  ...accounts,
  ...uploads,
  ...publish,
  ...forks,
  ...space,
  ...heat,
  ...updates,
  ...mock,
]);

// JSON bodies stay small, as the server's parsers keep them (1 MB on
// /api/v2, 2 MB on the PKM); R2 PUTs carry whole parts and files.
const JSON_LIMIT = 2 * 1024 * 1024;

function takeFault(state, method, path) {
  const fault = state.faults.find((f) => f.method === method && f.path === path);
  if (!fault) return null;
  fault.times -= 1;
  if (fault.times <= 0) state.faults.splice(state.faults.indexOf(fault), 1);
  return fault;
}

async function handle(state, req, res, onResponse) {
  const url = new URL(req.url, state.base);
  const fault = takeFault(state, req.method, url.pathname);
  if (fault?.drop === 'before') return req.socket.destroy();
  let route = null;
  let reply;
  try {
    const found = table.match(req.method, url.pathname);
    if (found) route = `${found.route.method} ${found.route.path}`;
    if (fault && !fault.drop) throw error(fault.status, 'Injected failure');
    if (!found) throw error(404, 'Not found');
    const raw = await readBody(req, url.pathname.startsWith('/r2/') ? Infinity : JSON_LIMIT);
    const ctx = {
      state,
      req,
      url,
      params: found.params,
      query: Object.fromEntries(url.searchParams),
      raw,
      body: parseBody(req, raw),
    };
    reply = await found.route.handler(ctx);
  } catch (thrown) {
    if (!(thrown instanceof Reply)) {
      console.error(thrown);
      thrown = error(500, 'Something went wrong');
    }
    reply = thrown;
  }
  if (fault?.drop === 'after') return req.socket.destroy();
  onResponse?.({ route, status: reply.status, body: reply.value });
  res.writeHead(reply.status, { ...reply.headers, 'content-length': reply.body.length });
  res.end(reply.body);
}

// Starts the mock on `port` (0 picks a free one) and answers once it is
// listening. `startAt` fixes where the mock's clock starts; `onResponse`
// sees every answer as {route, status, body}.
export async function startMockServer({ port = 8787, host = '127.0.0.1', startAt, onResponse } = {}) {
  const state = createState({ startAt });
  seed(state);
  const server = http.createServer((req, res) => {
    handle(state, req, res, onResponse).catch((e) => {
      console.error(e);
      res.destroy();
    });
  });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(port, host, resolve);
  });
  const url = `http://${host}:${server.address().port}`;
  state.base = url;
  return {
    url,
    state,
    routes: table.names,
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections();
        server.close(resolve);
      }),
  };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const at = process.argv.indexOf('--port');
  const port = at === -1 ? 8787 : Number(process.argv[at + 1]);
  const { url } = await startMockServer({ port });
  console.log(`mock mi-wwav.com on ${url}`);
}
