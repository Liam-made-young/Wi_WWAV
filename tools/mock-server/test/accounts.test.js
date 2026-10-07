// Accounts and sign-in (docs/SPEC.md 2.4, 9.7; PLAN S1.11, server side).
// S1.11 fails if the app sees a password, or the PKCE verifier or state
// isn't checked. The server's half: the password goes only to the
// server's own page, the loopback answer carries only a code and the
// state, and the token endpoint refuses a wrong verifier or state.
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer } from './setup.js';
import { ACCOUNTS } from '../fixtures.js';
import { DESKTOP_CLIENT, advanceClock, call, hiddenField, pkcePair, signInWithoutBrowser } from '../helpers.js';

const REDIRECT = 'http://127.0.0.1:53682/callback';
const DAY = 24 * 60 * 60 * 1000;

function authorizeQuery(over = {}) {
  return new URLSearchParams({
    response_type: 'code',
    client_id: DESKTOP_CLIENT,
    redirect_uri: REDIRECT,
    code_challenge: pkcePair().challenge,
    code_challenge_method: 'S256',
    state: 'state-1',
    ...over,
  });
}

function redeem(ctx, form) {
  return ctx.call('POST', '/oauth/desktop/token', {
    form: { grant_type: 'authorization_code', client_id: DESKTOP_CLIENT, redirect_uri: REDIRECT, ...form },
  });
}

describe('password sign-in and the 7-day JWT', () => {
  const ctx = useServer();

  test('login answers a token pair and the user, without tier', async () => {
    const res = await ctx.call('POST', '/api/auth/login', { body: ACCOUNTS.lmy });
    assert.equal(res.status, 200);
    assert.ok(res.body.token && res.body.refreshToken);
    assert.equal(res.body.user.username, 'LMY');
    assert.equal(res.body.user.tier, undefined);
  });

  test('a wrong password is "Invalid credentials"', async () => {
    const res = await ctx.call('POST', '/api/auth/login', { body: { email: ACCOUNTS.lmy.email, password: 'nope' } });
    assert.equal(res.status, 400);
    assert.deepEqual(res.body, { error: 'Invalid credentials' });
  });

  test('/me answers the account with its tier', async () => {
    const res = await ctx.call('GET', '/api/auth/me', { token: ctx.lmy });
    assert.equal(res.status, 200);
    assert.equal(res.body.username, 'LMY');
    assert.equal(res.body.tier, 'founding');
    assert.equal(res.body.password, undefined);
  });

  test('no token is 401 "No token" on /me and "Unauthorized" elsewhere', async () => {
    assert.deepEqual((await ctx.call('GET', '/api/auth/me')).body, { error: 'No token' });
    const res = await ctx.call('GET', '/api/v2/galaxies/mine');
    assert.equal(res.status, 401);
    assert.deepEqual(res.body, { error: 'Unauthorized' });
  });

  test('a refresh token is refused as an access token, with a 401', async () => {
    const pair = (await ctx.call('POST', '/api/auth/login', { body: ACCOUNTS.ana })).body;
    const res = await ctx.call('GET', '/api/v2/galaxies/mine', { token: pair.refreshToken });
    assert.equal(res.status, 401);
    assert.deepEqual(res.body, { error: 'Invalid Token' });
  });

  test('refresh turns a refresh token into a new pair; an access token is refused there', async () => {
    const pair = (await ctx.call('POST', '/api/auth/login', { body: ACCOUNTS.ana })).body;
    const res = await ctx.call('POST', '/api/auth/refresh', { body: { refreshToken: pair.refreshToken } });
    assert.equal(res.status, 200);
    assert.equal((await ctx.call('GET', '/api/auth/me', { token: res.body.token })).body.username, 'Ana');
    const wrong = await ctx.call('POST', '/api/auth/refresh', { body: { refreshToken: pair.token } });
    assert.equal(wrong.status, 401);
    assert.deepEqual(wrong.body, { error: 'Token is not a refresh token' });
    const missing = await ctx.call('POST', '/api/auth/refresh', { body: {} });
    assert.deepEqual([missing.status, missing.body], [400, { error: 'refreshToken required' }]);
  });

  test('after 7 days the token answers 401 everywhere, and the refresh token still works', async () => {
    const pair = (await ctx.call('POST', '/api/auth/login', { body: ACCOUNTS.lmy })).body;
    await advanceClock(ctx.url, 7 * DAY + 1000);
    for (const [method, path, body] of [
      ['GET', '/api/auth/me'],
      ['GET', '/api/v2/galaxies/mine'],
      ['GET', '/api/v2/galaxies/lmy'],
      ['POST', '/api/publish', { trackId: 'x' }],
      ['GET', '/api/upload/sign?fileType=audio/wav&size=10'],
    ]) {
      const res = await ctx.call(method, path, { token: pair.token, body });
      assert.equal(res.status, 401, `${method} ${path}`);
    }
    const fresh = await ctx.call('POST', '/api/auth/refresh', { body: { refreshToken: pair.refreshToken } });
    assert.equal(fresh.status, 200);
    assert.equal((await ctx.call('GET', '/api/auth/me', { token: fresh.body.token })).status, 200);
  });
});

describe('desktop sign-in: authorization code with PKCE on a loopback redirect', () => {
  const ctx = useServer();

  test("the authorize page is the server's own form; the password is posted to the server", async () => {
    const res = await ctx.call('GET', `/oauth/desktop/authorize?${authorizeQuery()}`);
    assert.equal(res.status, 200);
    assert.match(res.headers.get('content-type'), /text\/html/);
    assert.match(res.body, /<form method="post" action="\/oauth\/desktop\/login">/);
    assert.match(res.body, /type="password"/);
  });

  test('the loopback answer carries only the code and the state', async () => {
    const { challenge } = pkcePair();
    const { location } = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.lmy,
      challenge,
      state: 's-42',
      redirectUri: REDIRECT,
    });
    assert.equal(`${location.origin}${location.pathname}`, REDIRECT);
    assert.deepEqual([...location.searchParams.keys()].sort(), ['code', 'state']);
    assert.equal(location.searchParams.get('state'), 's-42');
    assert.ok(!location.href.includes(encodeURIComponent(ACCOUNTS.lmy.password)));
    assert.ok(!location.href.includes(ACCOUNTS.lmy.password));
  });

  test("the right verifier and state get the account's usual JWT and a refresh token", async () => {
    const { verifier, challenge } = pkcePair();
    const { code } = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.ana,
      challenge,
      state: 's1',
      redirectUri: REDIRECT,
    });
    const res = await redeem(ctx, { code, code_verifier: verifier, state: 's1' });
    assert.equal(res.status, 200);
    assert.equal(res.body.token_type, 'Bearer');
    assert.equal(res.body.expires_in, 7 * 24 * 60 * 60);
    assert.equal(res.headers.get('cache-control'), 'no-store');
    const me = await ctx.call('GET', '/api/auth/me', { token: res.body.access_token });
    assert.equal(me.body.username, 'Ana');
    const again = await ctx.call('POST', '/api/auth/refresh', { body: { refreshToken: res.body.refresh_token } });
    assert.equal(again.status, 200);
  });

  test('a wrong verifier is refused', async () => {
    const { challenge } = pkcePair();
    const { code } = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.lmy,
      challenge,
      state: 's2',
      redirectUri: REDIRECT,
    });
    const res = await redeem(ctx, { code, code_verifier: pkcePair().verifier, state: 's2' });
    assert.equal(res.status, 400);
    assert.equal(res.body.error, 'invalid_grant');
  });

  test('a missing verifier is refused', async () => {
    const { challenge } = pkcePair();
    const { code } = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.lmy,
      challenge,
      state: 's3',
      redirectUri: REDIRECT,
    });
    const res = await redeem(ctx, { code, state: 's3' });
    assert.equal(res.status, 400);
    assert.equal(res.body.error, 'invalid_request');
  });

  test('a wrong state is refused', async () => {
    const { verifier, challenge } = pkcePair();
    const { code } = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.lmy,
      challenge,
      state: 's4',
      redirectUri: REDIRECT,
    });
    const res = await redeem(ctx, { code, code_verifier: verifier, state: 'someone-else' });
    assert.equal(res.status, 400);
    assert.equal(res.body.error, 'invalid_grant');
  });

  test('a code works once, and a refused attempt uses it up', async () => {
    const { verifier, challenge } = pkcePair();
    const first = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.lmy,
      challenge,
      state: 's5',
      redirectUri: REDIRECT,
    });
    assert.equal((await redeem(ctx, { code: first.code, code_verifier: verifier, state: 's5' })).status, 200);
    assert.equal((await redeem(ctx, { code: first.code, code_verifier: verifier, state: 's5' })).status, 400);
    const second = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.lmy,
      challenge,
      state: 's6',
      redirectUri: REDIRECT,
    });
    await redeem(ctx, { code: second.code, code_verifier: 'x'.repeat(43), state: 's6' });
    const late = await redeem(ctx, { code: second.code, code_verifier: verifier, state: 's6' });
    assert.equal(late.body.error, 'invalid_grant');
  });

  test('a code redeemed with another redirect address is refused', async () => {
    const { verifier, challenge } = pkcePair();
    const { code } = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.lmy,
      challenge,
      state: 's7',
      redirectUri: REDIRECT,
    });
    const res = await redeem(ctx, { code, code_verifier: verifier, state: 's7', redirect_uri: 'http://127.0.0.1:9/x' });
    assert.equal(res.body.error, 'invalid_grant');
  });

  test('authorize refuses a non-loopback redirect, the plain method, a missing state, or another client', async () => {
    for (const over of [
      { redirect_uri: 'https://evil.example/cb' },
      { redirect_uri: 'http://localhost:53682/cb' },
      { code_challenge_method: 'plain' },
      { state: '' },
      { client_id: 'someone' },
      { code_challenge: 'short' },
    ]) {
      const res = await ctx.call('GET', `/oauth/desktop/authorize?${authorizeQuery(over)}`);
      assert.equal(res.status, 400, JSON.stringify(over));
      assert.equal(res.headers.get('location'), null);
    }
  });

  test('IPv6 loopback with any port is allowed (RFC 8252 7.3)', async () => {
    const res = await ctx.call(
      'GET',
      `/oauth/desktop/authorize?${authorizeQuery({ redirect_uri: 'http://[::1]:61000/cb' })}`,
    );
    assert.equal(res.status, 200);
  });

  test('a wrong password stays on the page; Cancel answers access_denied with the state', async () => {
    const page = await ctx.call('GET', `/oauth/desktop/authorize?${authorizeQuery({ state: 'c1' })}`);
    const request = hiddenField(page.body, 'request');
    const wrong = await ctx.call('POST', '/oauth/desktop/login', {
      form: { request, email: ACCOUNTS.lmy.email, password: 'nope' },
    });
    assert.equal(wrong.status, 401);
    assert.match(wrong.body, /That email and password don&#39;t match\./);
    const cancel = await ctx.call('POST', '/oauth/desktop/login', { form: { request, action: 'cancel' } });
    assert.equal(cancel.status, 302);
    const location = new URL(cancel.headers.get('location'));
    assert.equal(location.searchParams.get('error'), 'access_denied');
    assert.equal(location.searchParams.get('state'), 'c1');
  });

  test('a code expires after 5 minutes', async () => {
    const { verifier, challenge } = pkcePair();
    const { code } = await signInWithoutBrowser(ctx.url, {
      ...ACCOUNTS.lmy,
      challenge,
      state: 's8',
      redirectUri: REDIRECT,
    });
    await advanceClock(ctx.url, 5 * 60 * 1000 + 1);
    const res = await redeem(ctx, { code, code_verifier: verifier, state: 's8' });
    assert.equal(res.body.error, 'invalid_grant');
  });
});

describe('device code for PRANA (RFC 8628)', () => {
  const ctx = useServer();
  const GRANT = 'urn:ietf:params:oauth:grant-type:device_code';

  async function start() {
    const res = await ctx.call('POST', '/oauth/device/code', { form: { client_id: 'prana' } });
    assert.equal(res.status, 200);
    return res.body;
  }

  function poll(deviceCode) {
    return ctx.call('POST', '/oauth/desktop/token', {
      form: { grant_type: GRANT, device_code: deviceCode, client_id: 'prana' },
    });
  }

  test('pending, then slow_down when polled too fast, then tokens once a person approves', async () => {
    const device = await start();
    assert.match(device.user_code, /^[BCDFGHJKLMNPQRSTVWXZ]{4}-[BCDFGHJKLMNPQRSTVWXZ]{4}$/);
    assert.equal(device.verification_uri, `${ctx.url}/device`);
    assert.equal(device.interval, 5);
    assert.equal((await poll(device.device_code)).body.error, 'authorization_pending');
    assert.equal((await poll(device.device_code)).body.error, 'slow_down');

    const page = await ctx.call('GET', `/device?user_code=${device.user_code}`);
    assert.match(page.body, new RegExp(device.user_code));
    const approve = await ctx.call('POST', '/device', {
      form: { user_code: device.user_code.toLowerCase().replace('-', ''), ...ACCOUNTS.lmy },
    });
    assert.equal(approve.status, 200);
    assert.match(approve.body, /PRANA is linked to LMY\./);

    await advanceClock(ctx.url, 11 * 1000);
    const res = await poll(device.device_code);
    assert.equal(res.status, 200);
    assert.equal((await ctx.call('GET', '/api/auth/me', { token: res.body.access_token })).body.username, 'LMY');
    assert.equal((await poll(device.device_code)).body.error, 'invalid_grant');
  });

  test('a wrong code on the page, and an expired device code', async () => {
    const wrong = await ctx.call('POST', '/device', { form: { user_code: 'BBBB-BBBB', ...ACCOUNTS.lmy } });
    assert.equal(wrong.status, 400);
    const device = await start();
    await advanceClock(ctx.url, 15 * 60 * 1000 + 1);
    assert.equal((await poll(device.device_code)).body.error, 'expired_token');
  });

  test('a person who refuses gives access_denied', async () => {
    const device = await start();
    await ctx.call('POST', '/device', { form: { user_code: device.user_code, ...ACCOUNTS.ana, action: 'cancel' } });
    await advanceClock(ctx.url, 6 * 1000);
    assert.equal((await poll(device.device_code)).body.error, 'access_denied');
  });
});

describe('a desktop token from another client id', () => {
  const ctx = useServer();
  test("the device grant is PRANA's and the code grant the desktop's", async () => {
    const res = await call(ctx.url, 'POST', '/oauth/device/code', { form: { client_id: DESKTOP_CLIENT } });
    assert.equal(res.status, 400);
    assert.equal(res.body.error, 'invalid_client');
  });
});
