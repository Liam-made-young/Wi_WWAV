// One mock server per test file, on a free port, with LMY and Ana signed in.
import { after, before } from 'node:test';
import { startMockServer } from '../server.js';
import { ACCOUNTS } from '../fixtures.js';
import { call } from '../helpers.js';

export function useServer(options = {}) {
  const ctx = {};
  before(async () => {
    const server = await startMockServer({ port: 0, ...options });
    ctx.server = server;
    ctx.url = server.url;
    ctx.state = server.state;
    ctx.lmy = await login(server.url, ACCOUNTS.lmy);
    ctx.ana = await login(server.url, ACCOUNTS.ana);
    ctx.call = (method, path, opts) => call(server.url, method, path, opts);
  });
  after(() => ctx.server.close());
  return ctx;
}

export async function login(base, { email, password }) {
  const res = await call(base, 'POST', '/api/auth/login', { body: { email, password } });
  if (res.status !== 200) throw new Error(`login answered ${res.status}`);
  return res.body.token;
}
