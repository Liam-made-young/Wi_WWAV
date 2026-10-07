// Who is asking. The spec's contract (2.4, QUESTIONS #69): a missing token
// is 401 "Unauthorized", and a bad or expired one, or a refresh token used
// as an access token, is 401 "Invalid Token", so the app refreshes after a
// 401. server.md §1 and mismatch 1: today authenticateUser answers 403 for
// a bad token and accepts refresh tokens, /api/publish answers 500, and the
// v2 reads quietly treat a bad token as nobody.
import { error } from './http.js';
import { verifyJwt } from './jwt.js';

function bearer(ctx) {
  const [scheme, token] = (ctx.req.headers.authorization || '').split(' ');
  return scheme === 'Bearer' && token ? token : null;
}

export function requireUser(ctx) {
  const token = bearer(ctx);
  if (!token) throw error(401, 'Unauthorized');
  const claims = verifyJwt(token, ctx.state.now());
  const user = claims && claims.type !== 'refresh' && ctx.state.users.find((u) => u.id === claims.id);
  if (!user) throw error(401, 'Invalid Token');
  return user;
}

// Public reads that show more to the owner (their drafts). A token that is
// sent must still be good.
export function optionalUser(ctx) {
  return bearer(ctx) ? requireUser(ctx) : null;
}

// The user as login answers it (server routes/auth.js), less the fields
// the desktop never reads: no plan, no payout account, no shipping address.
export function userPayload(user) {
  return {
    id: user.id,
    email: user.email,
    username: user.username,
    bio: user.bio,
    profilePicture: user.profilePicture,
    astronaut: user.astronaut,
    stemPlayerCustomization: user.stemPlayerCustomization,
  };
}
