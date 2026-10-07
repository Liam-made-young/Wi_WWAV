// Helpers for tests that drive the mock server over HTTP: a small fetch
// wrapper, the PKCE pair, the browser's half of desktop sign-in, and the
// test-only controls under /__mock (clock, Stripe webhook).
import { createHash, randomBytes } from 'node:crypto';

// One request. `body` goes as JSON, `form` as a urlencoded form; the reply's
// body is parsed as JSON when it is JSON, and is text otherwise.
export async function call(base, method, path, { token, body, form, headers = {} } = {}) {
  const init = { method, headers: { ...headers }, redirect: 'manual' };
  if (token) init.headers.authorization = `Bearer ${token}`;
  if (body !== undefined) {
    init.headers['content-type'] = 'application/json';
    init.body = JSON.stringify(body);
  } else if (form !== undefined) {
    init.headers['content-type'] = 'application/x-www-form-urlencoded';
    init.body = new URLSearchParams(form).toString();
  }
  const res = await fetch(new URL(path, base), init);
  const text = await res.text();
  const json = (res.headers.get('content-type') || '').includes('application/json');
  return { status: res.status, headers: res.headers, body: json && text ? JSON.parse(text) : text };
}

// RFC 7636: a random verifier and its S256 challenge.
export function pkcePair() {
  const verifier = randomBytes(32).toString('base64url');
  const challenge = createHash('sha256').update(verifier).digest('base64url');
  return { verifier, challenge };
}

export const DESKTOP_CLIENT = 'wi-wwav-desktop';

// What the system browser does during desktop sign-in: open the authorize
// page, fill in the form, and follow the answer to the loopback address.
// Returns the redirect and the code and state it carries.
export async function signInWithoutBrowser(base, { email, password, challenge, state, redirectUri }) {
  const query = new URLSearchParams({
    response_type: 'code',
    client_id: DESKTOP_CLIENT,
    redirect_uri: redirectUri,
    code_challenge: challenge,
    code_challenge_method: 'S256',
    state,
  });
  const page = await call(base, 'GET', `/oauth/desktop/authorize?${query}`);
  if (page.status !== 200) throw new Error(`authorize answered ${page.status}: ${page.body}`);
  const request = hiddenField(page.body, 'request');
  const answer = await call(base, 'POST', '/oauth/desktop/login', { form: { request, email, password } });
  if (answer.status !== 302) throw new Error(`sign-in answered ${answer.status}: ${answer.body}`);
  const location = new URL(answer.headers.get('location'));
  return { location, code: location.searchParams.get('code'), state: location.searchParams.get('state') };
}

// The whole desktop sign-in, for tests that only need an account's tokens.
export async function desktopTokens(base, { email, password }) {
  const { verifier, challenge } = pkcePair();
  const redirectUri = 'http://127.0.0.1:53682/callback';
  const state = randomBytes(16).toString('base64url');
  const { code } = await signInWithoutBrowser(base, { email, password, challenge, state, redirectUri });
  const res = await call(base, 'POST', '/oauth/desktop/token', {
    form: {
      grant_type: 'authorization_code',
      code,
      code_verifier: verifier,
      redirect_uri: redirectUri,
      client_id: DESKTOP_CLIENT,
      state,
    },
  });
  if (res.status !== 200) throw new Error(`token answered ${res.status}: ${JSON.stringify(res.body)}`);
  return res.body;
}

export function hiddenField(html, name) {
  const match = html.match(new RegExp(`name="${name}" value="([^"]*)"`));
  if (!match) throw new Error(`no hidden field ${name}`);
  return match[1];
}

// Stripe's checkout.session.completed webhook, as if the buyer had paid.
export function completeCheckout(base, sessionId) {
  return call(base, 'POST', '/__mock/stripe/complete', { body: { sessionId } });
}

export function advanceClock(base, ms) {
  return call(base, 'POST', '/__mock/clock', { body: { advanceMs: ms } });
}

// PUT bytes to a presigned URL, as the app does straight to R2.
export async function putSigned(url, bytes, contentType) {
  const headers = contentType ? { 'content-type': contentType } : {};
  const res = await fetch(url, { method: 'PUT', headers, body: bytes });
  return { status: res.status, etag: res.headers.get('etag'), text: await res.text() };
}
