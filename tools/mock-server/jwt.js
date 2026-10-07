// HS256 JSON Web Tokens, the server's account tokens (routes/auth.js):
// an access token lasts 7 days and a refresh token 30.
import { createHmac, timingSafeEqual } from 'node:crypto';

export const SECRET = 'wi-wwav-mock-secret';
export const ACCESS_SECONDS = 7 * 24 * 60 * 60;
const REFRESH_SECONDS = 30 * 24 * 60 * 60;

const HEADER = Buffer.from(JSON.stringify({ alg: 'HS256', typ: 'JWT' })).toString('base64url');

function mac(input) {
  return createHmac('sha256', SECRET).update(input).digest();
}

export function signJwt(claims) {
  const body = Buffer.from(JSON.stringify(claims)).toString('base64url');
  return `${HEADER}.${body}.${mac(`${HEADER}.${body}`).toString('base64url')}`;
}

// The claims, or null when the token is malformed, forged or expired.
export function verifyJwt(token, nowMs) {
  const [head, body, sig] = String(token).split('.');
  if (head !== HEADER || !body || !sig) return null;
  const want = mac(`${head}.${body}`);
  const got = Buffer.from(sig, 'base64url');
  if (got.length !== want.length || !timingSafeEqual(got, want)) return null;
  let claims;
  try {
    claims = JSON.parse(Buffer.from(body, 'base64url').toString('utf8'));
  } catch {
    return null;
  }
  return typeof claims.exp === 'number' && claims.exp * 1000 > nowMs ? claims : null;
}

// signTokenPair on the server: {id, email} for access, plus type "refresh".
export function tokenPair(user, nowMs) {
  const iat = Math.floor(nowMs / 1000);
  return {
    token: signJwt({ id: user.id, email: user.email, iat, exp: iat + ACCESS_SECONDS }),
    refreshToken: signJwt({ id: user.id, email: user.email, type: 'refresh', iat, exp: iat + REFRESH_SECONDS }),
  };
}
