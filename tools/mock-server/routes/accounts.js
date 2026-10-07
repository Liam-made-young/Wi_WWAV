// Accounts (docs/SPEC.md 2.4, 9.7): password sign-in as it exists today,
// and the new desktop sign-in. The desktop opens the system browser on
// /oauth/desktop/authorize with a PKCE challenge and waits on a loopback
// address (RFC 8252); the password is typed only into the server's page,
// and the app redeems the code for the account's usual JWT and a refresh
// token. PRANA links with a device code (RFC 8628) entered at /device.
import { createHash, randomBytes, randomInt } from 'node:crypto';
import { escapeHtml, error, html, json, page, redirect } from '../http.js';
import { requireUser, userPayload } from '../auth.js';
import { ACCESS_SECONDS, tokenPair, verifyJwt } from '../jwt.js';

const DESKTOP_CLIENT = 'wi-wwav-desktop';
const DEVICE_CLIENT = 'prana';
const DEVICE_GRANT = 'urn:ietf:params:oauth:grant-type:device_code';
const REQUEST_MS = 15 * 60 * 1000;
const CODE_MS = 5 * 60 * 1000;
const DEVICE_MS = 15 * 60 * 1000;
const INTERVAL_S = 5;

const GUESSES = 5;
const GUESS_MS = 15 * 60 * 1000;

// The server's login lockout (routes/auth.js): five wrong passwords for an
// email, then 429 until 15 minutes after the last one; a right one clears
// the count. Every page that takes a password shares it, so the desktop
// sign-in form and PRANA's /device page can't be used to keep guessing.
// (Not reproduced: the per-IP 20 per 15 minutes, as every test comes from
// one address, and the 30-minute lock after 10 failures in the database.)
// Answers the account, or null for a wrong password; throws the 429.
function signIn(state, email, password, refuse = (status, words) => error(status, words)) {
  const wanted = String(email || '').toLowerCase();
  const now = state.now();
  const tries = state.loginAttempts.get(wanted);
  if (tries && now - tries.last > GUESS_MS) state.loginAttempts.delete(wanted);
  const count = state.loginAttempts.get(wanted)?.count ?? 0;
  if (count >= GUESSES) {
    const minutes = Math.ceil((GUESS_MS - (now - tries.last)) / 60000);
    throw refuse(429, `Too many login attempts. Try again in ${minutes} minutes.`);
  }
  const user = state.users.find((u) => u.email === wanted && u.password === password) ?? null;
  if (user) state.loginAttempts.delete(wanted);
  else state.loginAttempts.set(wanted, { count: count + 1, last: now });
  return user;
}

function login(ctx) {
  const user = signIn(ctx.state, ctx.body.email, ctx.body.password);
  if (!user) return error(400, 'Invalid credentials');
  return json(200, { ...tokenPair(user, ctx.state.now()), user: userPayload(user) });
}

function refresh(ctx) {
  const token = ctx.body.refreshToken;
  if (!token) return error(400, 'refreshToken required');
  const claims = verifyJwt(token, ctx.state.now());
  if (!claims) return error(401, 'Invalid or expired refresh token');
  if (claims.type !== 'refresh') return error(401, 'Token is not a refresh token');
  const user = ctx.state.users.find((u) => u.id === claims.id);
  if (!user) return error(401, 'User no longer exists');
  return json(200, tokenPair(user, ctx.state.now()));
}

// /me reads the header by hand on the server, so its missing-token
// sentence differs from the middleware's.
function me(ctx) {
  if (!ctx.req.headers.authorization) return error(401, 'No token');
  const user = requireUser(ctx);
  return json(200, {
    ...userPayload(user),
    tier: user.tier,
    tierExpiresAt: user.tierExpiresAt,
    foundingMemberNumber: user.foundingMemberNumber,
  });
}

// --- desktop sign-in ---------------------------------------------------------

// RFC 8252 7.3: a loopback IP literal on any port. "localhost" is refused,
// as the RFC advises, because it can be pointed elsewhere.
function isLoopback(uri) {
  let url;
  try {
    url = new URL(uri);
  } catch {
    return false;
  }
  return url.protocol === 'http:' && (url.hostname === '127.0.0.1' || url.hostname === '[::1]') && url.port !== '';
}

function authorizeProblem(q) {
  if (q.client_id !== DESKTOP_CLIENT) return 'This sign-in link is for another app.';
  if (q.response_type !== 'code') return 'This sign-in link asks for something other than a code.';
  if (!isLoopback(q.redirect_uri)) return 'Wi_WWAV signs in through a loopback address, like http://127.0.0.1:53682/.';
  if (q.code_challenge_method !== 'S256') return 'This sign-in link needs an S256 code challenge.';
  if (!/^[A-Za-z0-9_-]{43}$/.test(q.code_challenge || '')) return "This sign-in link's code challenge isn't readable.";
  if (!q.state) return 'This sign-in link is missing its state.';
  return null;
}

function signInPage(request, problem = '') {
  return page(
    'Sign in to Wi_WWAV',
    `${problem ? `<p role="alert">${escapeHtml(problem)}</p>` : ''}
<form method="post" action="/oauth/desktop/login">
<input type="hidden" name="request" value="${escapeHtml(request)}">
<p><label>Email <input name="email" type="email" autocomplete="username"></label></p>
<p><label>Password <input name="password" type="password" autocomplete="current-password"></label></p>
<p><button name="action" value="allow">Sign in</button> <button name="action" value="cancel">Cancel</button></p>
</form>`,
  );
}

function authorize(ctx) {
  const problem = authorizeProblem(ctx.query);
  // Nothing redirects until the redirect address has been checked.
  if (problem) return html(400, page("Sign-in can't start", `<p>${escapeHtml(problem)}</p>`));
  const request = randomBytes(18).toString('base64url');
  ctx.state.oauth.requests.set(request, {
    redirectUri: ctx.query.redirect_uri,
    challenge: ctx.query.code_challenge,
    state: ctx.query.state,
    expiresAt: ctx.state.now() + REQUEST_MS,
  });
  return html(200, signInPage(request));
}

function answer(redirectUri, params) {
  const url = new URL(redirectUri);
  for (const [k, v] of Object.entries(params)) url.searchParams.set(k, v);
  return redirect(url.toString());
}

function loginForm(ctx) {
  const { requests, codes } = ctx.state.oauth;
  const request = requests.get(ctx.body.request);
  if (!request || request.expiresAt <= ctx.state.now()) {
    return html(400, page('Sign-in expired', '<p>This sign-in link has expired. Start again from Wi_WWAV.</p>'));
  }
  if (ctx.body.action === 'cancel') {
    requests.delete(ctx.body.request);
    return answer(request.redirectUri, { error: 'access_denied', state: request.state });
  }
  const user = signIn(ctx.state, ctx.body.email, ctx.body.password, (status, words) =>
    html(status, signInPage(ctx.body.request, words)),
  );
  if (!user) return html(401, signInPage(ctx.body.request, "That email and password don't match."));
  requests.delete(ctx.body.request);
  const code = randomBytes(32).toString('base64url');
  codes.set(code, { ...request, userId: user.id, expiresAt: ctx.state.now() + CODE_MS });
  return answer(request.redirectUri, { code, state: request.state });
}

// RFC 6749 5.2: { error, error_description }, and tokens are never cached.
function oauthError(status, code, description) {
  return json(status, { error: code, error_description: description }, { 'cache-control': 'no-store' });
}

function tokens(state, user) {
  const pair = tokenPair(user, state.now());
  return json(
    200,
    { access_token: pair.token, token_type: 'Bearer', expires_in: ACCESS_SECONDS, refresh_token: pair.refreshToken },
    { 'cache-control': 'no-store' },
  );
}

function s256(verifier) {
  return createHash('sha256').update(verifier).digest('base64url');
}

// The code grant. The token request repeats the state, so the server
// checks it against the authorization request as well as the app does
// against the loopback answer.
function redeemCode(ctx) {
  const b = ctx.body;
  if (b.client_id !== DESKTOP_CLIENT) return oauthError(400, 'invalid_client', 'Unknown client.');
  if (!b.code || !b.redirect_uri || !b.state) {
    return oauthError(400, 'invalid_request', 'A code, its redirect address and its state are needed.');
  }
  if (!/^[A-Za-z0-9._~-]{43,128}$/.test(b.code_verifier || '')) {
    return oauthError(400, 'invalid_request', 'A code verifier of 43 to 128 characters is needed.');
  }
  const { codes } = ctx.state.oauth;
  const grant = codes.get(b.code);
  // One attempt per code, right or wrong, so a stolen code can't be tried twice.
  codes.delete(b.code);
  if (!grant || grant.expiresAt <= ctx.state.now()) {
    return oauthError(400, 'invalid_grant', 'That code is used up or has expired.');
  }
  if (grant.redirectUri !== b.redirect_uri) {
    return oauthError(400, 'invalid_grant', "The redirect address doesn't match.");
  }
  if (s256(b.code_verifier) !== grant.challenge)
    return oauthError(400, 'invalid_grant', "The code verifier doesn't match.");
  if (grant.state !== b.state) return oauthError(400, 'invalid_grant', "The state doesn't match.");
  return tokens(
    ctx.state,
    ctx.state.users.find((u) => u.id === grant.userId),
  );
}

// --- device code -------------------------------------------------------------

// RFC 8628 6.1: consonants only, so a code never spells a word.
const LETTERS = 'BCDFGHJKLMNPQRSTVWXZ';

function userCode() {
  const pick = () => LETTERS[randomInt(LETTERS.length)];
  return `${Array.from({ length: 4 }, pick).join('')}-${Array.from({ length: 4 }, pick).join('')}`;
}

function normalCode(raw) {
  const letters = String(raw || '')
    .toUpperCase()
    .replace(/[^A-Z]/g, '');
  return letters.length === 8 ? `${letters.slice(0, 4)}-${letters.slice(4)}` : '';
}

function deviceCode(ctx) {
  if (ctx.body.client_id !== DEVICE_CLIENT) return oauthError(400, 'invalid_client', 'Unknown client.');
  const { state } = ctx;
  const device = {
    deviceCode: randomBytes(32).toString('base64url'),
    userCode: userCode(),
    status: 'pending',
    userId: null,
    interval: INTERVAL_S,
    lastPoll: null,
    expiresAt: state.now() + DEVICE_MS,
  };
  state.oauth.devices.set(device.deviceCode, device);
  return json(200, {
    device_code: device.deviceCode,
    user_code: device.userCode,
    verification_uri: `${state.base}/device`,
    verification_uri_complete: `${state.base}/device?user_code=${device.userCode}`,
    expires_in: DEVICE_MS / 1000,
    interval: INTERVAL_S,
  });
}

function devicePage(code = '', problem = '') {
  return page(
    'Link PRANA',
    `${problem ? `<p role="alert">${escapeHtml(problem)}</p>` : ''}
<form method="post" action="/device">
<p><label>The code on PRANA <input name="user_code" value="${escapeHtml(code)}"></label></p>
<p><label>Email <input name="email" type="email" autocomplete="username"></label></p>
<p><label>Password <input name="password" type="password" autocomplete="current-password"></label></p>
<p><button name="action" value="allow">Link</button> <button name="action" value="cancel">Cancel</button></p>
</form>`,
  );
}

function deviceForm(ctx) {
  return html(200, devicePage(normalCode(ctx.query.user_code)));
}

function deviceApprove(ctx) {
  const code = normalCode(ctx.body.user_code);
  const device = [...ctx.state.oauth.devices.values()].find(
    (d) => d.userCode === code && d.status === 'pending' && d.expiresAt > ctx.state.now(),
  );
  if (!device) return html(400, devicePage(code, "That code isn't one we gave out, or it has expired."));
  const user = signIn(ctx.state, ctx.body.email, ctx.body.password, (status, words) =>
    html(status, devicePage(code, words)),
  );
  if (!user) return html(401, devicePage(code, "That email and password don't match."));
  if (ctx.body.action === 'cancel') {
    device.status = 'denied';
    return html(200, page('Not linked', '<p>PRANA was not linked. You can close this page.</p>'));
  }
  device.status = 'approved';
  device.userId = user.id;
  return html(200, page('Linked', `<p>PRANA is linked to ${escapeHtml(user.username)}. You can close this page.</p>`));
}

function redeemDevice(ctx) {
  if (ctx.body.client_id !== DEVICE_CLIENT) return oauthError(400, 'invalid_client', 'Unknown client.');
  const { devices } = ctx.state.oauth;
  const device = devices.get(ctx.body.device_code);
  if (!device) return oauthError(400, 'invalid_grant', "That device code isn't one we gave out.");
  const now = ctx.state.now();
  if (device.expiresAt <= now) return oauthError(400, 'expired_token', 'The code expired. Start again on PRANA.');
  if (device.status === 'denied') return oauthError(400, 'access_denied', 'The link was refused.');
  // RFC 8628 3.5: polling faster than the interval earns 5 s more of it.
  const early = device.lastPoll !== null && now - device.lastPoll < device.interval * 1000;
  device.lastPoll = now;
  if (early) {
    device.interval += 5;
    return oauthError(400, 'slow_down', `Ask at most every ${device.interval} s.`);
  }
  if (device.status === 'pending') {
    return oauthError(400, 'authorization_pending', 'Waiting for someone to enter the code.');
  }
  devices.delete(ctx.body.device_code);
  return tokens(
    ctx.state,
    ctx.state.users.find((u) => u.id === device.userId),
  );
}

function token(ctx) {
  if (ctx.body.grant_type === 'authorization_code') return redeemCode(ctx);
  if (ctx.body.grant_type === DEVICE_GRANT) return redeemDevice(ctx);
  return oauthError(400, 'unsupported_grant_type', 'Use authorization_code or the device code grant.');
}

export const routes = [
  ['POST', '/api/auth/login', login],
  ['POST', '/api/auth/refresh', refresh],
  ['GET', '/api/auth/me', me],
  ['GET', '/oauth/desktop/authorize', authorize],
  ['POST', '/oauth/desktop/login', loginForm],
  ['POST', '/oauth/desktop/token', token],
  ['POST', '/oauth/device/code', deviceCode],
  ['GET', '/device', deviceForm],
  ['POST', '/device', deviceApprove],
];
