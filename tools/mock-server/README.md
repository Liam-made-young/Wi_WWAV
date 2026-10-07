# The mock mi-wwav.com

A stand-in for mi-wwav.com that the desktop app's tests run against. It
follows the contract `docs/SPEC.md` intends, not the live server's bugs: where
the two differ, the mock follows the spec and says which mismatch in the
server digest (`server.md`, "Mismatches with the spec", numbered 1–21) it
encodes. This file is also the contract the server patch implements
(`docs/QUESTIONS.md` #61, #69, #70, #71, #73).

Node 22, ES modules, no npm dependencies, everything in memory.

```
node tools/mock-server/server.js --port 8787     # run it
node --test tools/mock-server                    # its tests
```

From a test: `startMockServer({ port: 0 })` answers `{ url, state, routes, close }`.
`state` is the whole world (tables, the R2 stand-in, the clock); `routes` lists
every route as `"METHOD /path"`. Options: `startAt` (where the clock starts,
in ms), `onResponse({ route, status, body })` (sees every answer).
`helpers.js` has the client side: `call`, `pkcePair`, `signInWithoutBrowser`,
`desktopTokens`, `completeCheckout`, `advanceClock`, `putSigned`.

## The seed

The same world on every start and after `POST /__mock/reset`.

| What | Detail |
|---|---|
| LMY | `lmy@mi-wwav.com` / `WeWave-lmy1`, id 1, tier `founding`, Founding #1, payouts ready (`acct_lmy`) |
| Ana | `ana@example.com` / `WeWave-ana1`, id 2, tier `free`, payouts ready (`acct_ana`) |
| Galaxies | `lmy` (systems "World Ending" and "Covers"), `ana` (system "Borrowed bass") |
| Works | World Ending (LMY, E minor, 92 BPM, gen 0, $9); Low Tide (LMY, A minor, 86 BPM, gen 1, remix of World Ending, $4); glass hours (Ana, A minor, 128 BPM, gen 2, remix of Low Tide, $4). Each is a real `.wwav` in the R2 stand-in; `fixtures.js` exports their `trackId`s and `song_id`s as `WORKS` |
| Planets | World Ending and Low Tide in "World Ending"; glass hours in "Borrowed bass"; "Covers" empty |
| Fashion | "Waxed chore jacket", Ana's, $120, M, like new, one of one, ships from Providence, RI |
| Links | Ana's claim that glass hours is a `sample` of World Ending, pending for LMY |
| Saved | Ana has added LMY's galaxy; LMY has saved glass hours |
| Money | Ana bought World Ending on Oct 5 (completed); LMY was paid out $8.10 on Oct 6 |
| Letter | LMY's "Dear Wi-WWAV, / Hardware is hard…", Oct 3 |

`addAccount(state, { username, onboarded })` in `fixtures.js` adds a signed-in
account for a test (password `WeWave-test1`).

## Conventions

- **Legacy routes** answer bare objects, and errors as `{ error: "sentence" }`,
  sometimes with a `code`. **`/api/v2`** answers `{ ok: true, data }` and
  `{ ok: false, error: { code, message } }`; its sign-in failures are legacy
  shaped, as on the server.
- **Tokens.** `Authorization: Bearer <jwt>`, HS256. Access tokens last 7 days,
  refresh tokens 30. No token: 401 `{error: "Unauthorized"}`. A bad or
  expired token, or a refresh token used as an access token: 401
  `{error: "Invalid Token"}`, on every route that reads one, the public reads
  included (catalog, suns, lineage, the store's floor). *Mismatch 1:* the
  server answers 403 there, 500 on `/api/publish`, treats a bad token as
  nobody on public reads, and accepts refresh tokens anywhere.
- **Bodies.** JSON bodies are objects or arrays, as `express.json()` takes
  them; `null`, a number or a string is 400 "Request body must be a JSON
  object". A path with a broken percent-escape is 400 "That path isn't
  readable".
- **Rate limits** answer 429 `{error, code: "rate_limited", scope, retryAfterSeconds}`
  with `Retry-After`, as `middleware/rateLimit.js` does.
- **Idempotency-Key** (8–120 of `[A-Za-z0-9_-.:]`) replays the stored answer
  with `Idempotent-Replay: true`; another body under the same key is 409
  `idempotency_conflict`.
- **Counts.** No answer carries a count of anyone's attention: no `viewCount`,
  likes, plays, followers, sales or fuel (gate 1.3; `test/gate.test.js` walks
  every key of every answer). The only `*Count` keys are `planetCount` and
  `blockCount`, which count a system's worlds and a sun's blocks.
- Not reproduced: the global 1200/min limit, CORS (*mismatch 21*), the XSS
  sanitizer on legacy bodies, compression.

## Accounts and sign-in (2.4, 9.7)

| Endpoint | Body / query | Answer | Notes |
|---|---|---|---|
| `POST /api/auth/login` | `{email, password}` | `{token, refreshToken, user}`; 400 "Invalid credentials"; 429 "Too many login attempts. Try again in N minutes." | `user` has no `tier`, as today. The desktop never uses this: it signs in through the browser. Five wrong passwords for an email lock it until 15 minutes after the last, as `routes/auth.js` does; a right one clears the count. The count is shared with the desktop and PRANA sign-in pages. Not reproduced: the per-IP 20 per 15 minutes (every test comes from one address) and the 30-minute lock after 10 failures |
| `POST /api/auth/refresh` | `{refreshToken}` | `{token, refreshToken}`; 400 "refreshToken required"; 401 "Invalid or expired refresh token", "Token is not a refresh token" | A new pair; the old refresh token stays good until it expires, as today |
| `GET /api/auth/me` | — | the user plus `tier, tierExpiresAt, foundingMemberNumber`; 401 "No token" / "Invalid Token" | |
| `GET /oauth/desktop/authorize` | `response_type=code, client_id=wi-wwav-desktop, redirect_uri, code_challenge, code_challenge_method=S256, state` | 200 HTML sign-in form; 400 HTML for an unknown client, a redirect that isn't `http://127.0.0.1:<port>` or `http://[::1]:<port>` (RFC 8252 7.3; `localhost` refused), a method other than S256, a challenge that isn't 43 base64url characters, or no state. Never redirects before the redirect address is checked | **New.** *Mismatch 20:* the server's OAuth checks only the devlog password |
| `POST /oauth/desktop/login` | form `{request, email, password}` or `{request, action: "cancel"}` | 302 to `redirect_uri?code=…&state=…` (nothing else in the query); Cancel: 302 `?error=access_denied&state=…`; 401 HTML "That email and password don't match."; 429 HTML "Too many login attempts. Try again in N minutes." (the login count above); 400 HTML once the request is 15 minutes old | **New.** The page the system browser shows; the password never reaches the app |
| `POST /oauth/desktop/token` | form or JSON. Code grant: `grant_type=authorization_code, code, code_verifier, redirect_uri, client_id, state` | `{access_token, token_type: "Bearer", expires_in: 604800, refresh_token}`, `Cache-Control: no-store`. 400 `{error, error_description}` (RFC 6749 5.2): `invalid_request` (no verifier, or not 43–128 characters), `invalid_grant` (code unknown, used or over 5 minutes old; wrong verifier; wrong redirect; wrong state), `invalid_client`, `unsupported_grant_type` | **New.** The access token is the account's usual 7-day JWT and the refresh token its usual one, so `/api/auth/refresh` refreshes it. A code is good for one attempt: a refused attempt uses it up. The request repeats `state`, which the server checks as well as the app |
| `POST /oauth/device/code` | form or JSON `{client_id: "prana"}` | `{device_code, user_code: "WDJB-MJHT", verification_uri: <base>/device, verification_uri_complete, expires_in: 900, interval: 5}` | **New**, RFC 8628, for PRANA's account link. User codes use consonants only |
| `GET /device` | `?user_code=` | HTML form | **New** |
| `POST /device` | form `{user_code, email, password, action?}` | 200 HTML "PRANA is linked to LMY. You can close this page."; 400 "That code isn't one we gave out, or it has expired."; 401 wrong password; 429 as login | The code may be typed in any case, with or without the hyphen |
| `POST /oauth/desktop/token` | device grant: `grant_type=urn:ietf:params:oauth:grant-type:device_code, device_code, client_id=prana` | tokens as above; 400 `authorization_pending`, `slow_down` (polled sooner than `interval`, which then grows by 5 s), `expired_token`, `access_denied`, `invalid_grant` once redeemed | |

## Uploads (2.8, 9.7) and the R2 stand-in

Whoever signs an upload owns its `trackId` from that moment, so sign → PUT →
publish works with no `/process` step (*mismatch 3*).

| Endpoint | Query / body | Answer | Notes |
|---|---|---|---|
| `GET /api/upload/sign` | `fileType` (default `audio/wav`), `size` | `{signedUrl, s3Key: "uploads/<trackId>", trackId}`; 413 "File too large (max 250 MB)"; 400 "File is empty", "Unsupported audio type: …" | URL good for 300 s. `trackId` is `track_<ms>_<9 random base36>`, as on the server: it names a platform song id (6.1), so it mustn't be guessable |
| `GET /api/upload/sign-video` | `ext` (mp4, mov, m4v, webm), `fileType`, `size` | `{signedUrl, s3Key: "videos/<userId>/<ms>.<ext>"}`; 2 GB cap | 600 s (*mismatch 4:* the spec says 300 s for every sign). *Mismatch 6:* films use `/api/films/sign-upload` today; the mock has no film routes |
| `GET /api/upload/sign-replace` | `trackId`, `fileType`, `size` | `{signedUrl, s3Key: "uploads/<trackId>_v<ms>"}`; 400 "Missing trackId"; 404 "Upload not found"; 403 "Not authorized" | 600 s. Publish with that `s3Key` to make it a new version (*mismatch 7:* today the link keeps serving the first upload) |
| `POST /api/upload/parts` | `{size, fileType?, trackId?}` | `{uploadId, trackId, s3Key, partSize: 8388608, parts}` | **New.** For files over 250 MB, up to 4 GB. With a `trackId` you own, the object is a new version's (`uploads/<trackId>_v<ms>`). Creating one counts against `upload_sign`, as every sign route does: 240 an hour per account, then 429 |
| `GET /api/upload/parts/:uploadId/:n` | — | `{signedUrl, n, size}`; 400 "There is no part n"; 403; 404 "No upload there"; 409 "That upload is finished" | **New.** Sign each part just before it goes up: the URL lasts 300 s. Part signatures don't count against `upload_sign` (creating the upload did), so a 4 GB file's 512 parts sign without a 429 |
| `POST /api/upload/parts/:uploadId/complete` | `{parts: [{n, etag}]}` | `{trackId, s3Key, size, sha256}`; 400 "Part n hasn't arrived", "Part n's etag doesn't match what arrived" | **New.** Idempotent. *Mismatch 5:* Wi's 8 MiB parts are proxied through the server on its own host, not presigned |
| `PUT /r2/<key>?…` | the bytes; `Content-Type` must be the signed one | 200 with `ETag: "<md5>"`; 403 "Request has expired", "Signature doesn't match"; for a part, 400 "Part n should be N bytes" | Every part's PUTs are counted (`state.multipart.get(id).parts.get(n).sends`), so a test sees a finished part sent again (PLAN S1.9). A PUT can replace an upload's object while its URL lasts, but never a published version: publishing copies the bytes to `files/<sha256>`, which no sign route hands out |
| `GET /r2/<key>?…` | — | the bytes | Only through a signed URL (downloads, 60 s) |

Not here: `/api/upload/process`, `/process-stems`, `/sign-stem` and
`/:trackId/replace-audio` (the desktop splits locally, 5.10).

## Publishing and ids (2.8, 6.8)

`POST /api/publish` `{trackId, s3Key?, title?, album?, coverArtUrl?, isMaster?, settings?, tags?, duration?}`
reads the uploaded file itself (the one named by `s3Key`, else the newest
signed for `trackId`) and answers
`{success: true, trackId, publishedId, songId, version, updated?, message?}`.

| Case | Answer |
|---|---|
| A new `song_id` | claims it for this account: a new work, version `wmet.version` (absent means 1) |
| `settings: {origin, clipId}` already published by this account | that work, `message: "Already up"`, even under a re-signed `trackId` (*mismatch 2:* nothing reads `settings` today) |
| The same sha256 as a version already up | `message: "Already up"`: no new version, but the body's `title`, `album`, `coverArtUrl`, `isMaster`, `duration`, `tags` and `settings` apply, since a publish is how a work is renamed |
| Any of these on a work this account unpublished | the work is back (`withdrawn` clears) and the answer has no "Already up". It is in no system and not for sale until it is placed and priced again |
| The same id, this account, a higher version | a new version, kept beside the old ones; `trackId` stays the first one's, so the link plays the newest |
| The same id, this account, the same or a lower version with other bytes | 409 `{error: "Version 2 of 'Fixed' is already up. Export it again as a new version.", code: "version_taken"}` |
| Another account's id | 409 `{error: "This file's id belongs to another work, 'Low Tide' by LMY.", code: "id_taken"}` |
| The platform id of a `trackId` another account signed, not yet published | 409 `{error: "This file's id belongs to another account's upload.", code: "id_taken"}` |
| A file without `wmet` (a plain WAV) | `song_id` is FNV-1a 128 of `"wwav-track:" + trackId` (6.1) |
| Someone else's upload | 403 "You don't own this track" |
| Signed but not yet PUT | 409 "The file hasn't finished uploading." |

A refused publish (any 403 or 409) changes nothing; an accepted one applies
the body. Each version keeps its bytes at `files/<sha256>`, and downloads
are signed from there (6.12).

*Mismatch 8:* none of the id rules exist on `/api` today. *Mismatch 9:*
lineage comes from the file's `wlin`, not from JSON the client sends: the
parent is found by `song_id`, and when it is on the server it fixes the
root and the generation (the parent's `remixDepth` + 1, whatever `wlin`
says); without it, `wlin`'s root and generation stand.
A re-publish that leaves out `isMaster` keeps it (*mismatch 2*: the server sets
it false). The answer names the ids (the server answers no id today).

| Endpoint | Answer | Notes |
|---|---|---|
| `POST /api/unpublish` `{trackId}` | `{success: true}`; 404 "Not published"; 403 "Unauthorized" | Open #21 at its recommendation: the planet leaves the sky, and the work stays in its family marked `withdrawn` (the server deletes the row) |
| `GET /api/tracks/:trackId/lineage` | `{root, ancestors (root first), descendants, current}`; 404 "Track not found" | Node: `{id, trackId, songId, title, artist, coverArtUrl, uploaderId, inFeed, uploader, parentTrackId, secondaryParentTrackId, lineageRootTrackId, remixDepth, remixSnapshot, duration, bpm, musicalKey, created_at, ownsAudio, stemsTrackId, withdrawn}`, as `routes/lineage.js` plus `songId` and `withdrawn`. A node owns audio when it has a file (the server: a UserUpload row); `stemsTrackId` is the nearest ancestor that does, itself included |
| `GET /api/lineage/global` | `{roots: [trackId], nodes: [Node without remixSnapshot]}` with `Cache-Control: public, max-age=15` (none with `?fresh=1`) | Every family: the roots are originals that are `inFeed` or have descendants, oldest first; nodes by depth, then age. Withdrawn works stay, marked. The server caches it 30 s and orders roots by album track order; the mock has no albums or films |
| `GET /api/tracks/:trackId/is-published` | `{isPublished, publishedTrack, canUnpublish}`; `{isPublished: false, publishedTrack: null, canUnpublish: false}` when it isn't (or was withdrawn) | How the numeric `publishedTrack.id` is read (server.md §2). The row leaves out `playCount` (gate 1.3); `price` is the DECIMAL string Postgres answers, e.g. `"4.00"`. `canUnpublish` is true for the uploader |
| `POST /api/tracks/:trackId/fork` | `{title?, mix, inFeed?}` (v1) or `{title?, project}` (v2 wins); optional `Idempotency-Key` | 201 Node, with `trackId: "fork_<ms>_<9>"`, `ownsAudio: false`, `remixSnapshot: {stemEffects, timePitch, overdubs: {}, capturedAt, wwav: {v: 1, stems, masterPitch, masterTime} \| {v: 2, project}}`. 400 "Missing or invalid mix state", "Missing project", "Project has no primary source", "Project has no clips", "Too many clips", "Too many imports", "Malformed clip", …; 404 "Track not found" (also for a withdrawn work); 422 "Source X has no audio", "Import X does not exist", `{error: "Lineage is too deep to extend", code: "max_depth_reached"}` past depth 64; 429 `fork` after 20 a minute | ↑ Push (4.6), sanitized as `routes/forks.js`: stems keyed vocals/drums/bass/other with level 0–1, pitch ±12, time −50…100 %, FX clamped; a v2 project's 8 tracks, groups and master clamped, its sources resolved to audio-owning nodes. A fork owns no audio, inherits cover, BPM, key and duration, has `inFeed: false` unless sent true, and takes a seat (and a place in Newest) only when placed on a system. Its `songId` is the platform id of its `trackId`; a file later exported from it becomes its first version, and until then it can't be priced. The mock has no RemixAudio imports, so v1 overdubs are dropped and v2 imports refused. *Mismatch 2:* the server's fork takes no Idempotency-Key, so a retried push posts twice |

Not here: film forks (`POST /api/films/:filmId/fork`), as the mock has no
films.

## Space: `/api/v2` (4)

Shapes as `server.md` §4, with `viewCount` removed everywhere (*mismatch 11*).

| Endpoint | Answer `data` | Notes |
|---|---|---|
| `POST /galaxies` `{slug?, displayName?}` | `{galaxy: {id, slug, displayName}, claimed}` | Idempotent |
| `GET /galaxies/mine` | `{galaxy, sun: SunSummary, systems: [{id, slug, title, status, sun}]}`; 404 `no_galaxy` | |
| `GET /galaxies/:slug` | `{galaxy: {id, slug, displayName, skySeed, user: {id, username, profilePicture, rocket}}, sun, systems: [{id, slug, title, orbitIndex, colorSeed, posX, posY, status, planetCount, sun}]}` | Drafts only for the owner |
| `GET /galaxies/:slug/sun` | `{sun: {id, kind, title, blocks, appearance}, galaxy, system: null}` | |
| `GET /galaxies/:slug/letters?cursor=` | `{letters: [{id, greeting, blocks, signoff, sentAt, createdAt}], nextCursor}`; 404 `not_found` | **New** (4.8): a bio sun's letters, newest first, ten at a time; `nextCursor` (`"<iso>\|<id>"`) is "Show older", null is "That's everything." *Mismatch 13:* the server has no such route; `GET /api/devlog` answers every post at once, with no greeting or sign-off |
| `POST /galaxies/:id/systems` `{title, slug?, status?}` | `{system: {id, slug, title, status, orbitIndex}}`; 400 `no_title` "A project needs a name" | New systems are drafts unless `status: "published"` |
| `GET /galaxies/:slug/systems/:system` | `{galaxy, system, sun, planets: [Planet]}` | Planet: `{id, kind, orbitIndex, orbitRadius, phaseOffset, appearance, trackId, publishedId, title, artist, coverArtUrl, duration, bpm, key, priceCents, yours}`. `priceCents` is set while it's for sale ("$4 · on the shelf"); `yours` once the viewer has bought it (4.13) |
| `GET /galaxies/:slug/systems/:system/sun` | `{sun, galaxy, system}` | |
| `PATCH /systems/:id` | `{system: {id, slug, title, status, orbitIndex, posX, posY}}`; 400 `bad_status`, `bad_position` "That is not a place in this galaxy", `bad_slug` | `posX`/`posY` both or neither, 0–5000 |
| `DELETE /systems/:id` | `{deleted: true}` | Its planets and sun go; the works stay published |
| `POST /systems/:id/planets` `{kind: "song", trackId}` | `{planet: {id, kind, trackId, filmId, galleryId, orbitIndex}, remaining}`; **409 `system_full` "A solar system holds 21 worlds. Start another one."**; 404 `no_track`; 403 `not_yours`; 409 `already_placed` | The 22nd world is refused (PLAN S4.4). Counting and inserting can't interleave (*mismatch 16:* the server's count-then-create is racy). The mock has no films or galleries: those kinds answer 404 `no_film` / `no_gallery` |
| `PATCH /planets/:id` `{orbitIndex?, orbitRadius?, phaseOffset?}` | `{planet: {id, orbitIndex, orbitRadius, phaseOffset, appearance}}`; 400 `bad_orbit`, `bad_radius` "An orbit fits between 400 and 1800", `bad_phase` | |
| `DELETE /planets/:id` | `{deleted: true}` | The work survives |
| `GET /suns/:id` | `{sun, galaxy, system}` | |
| `PUT /suns/:id/blocks` `{blocks: {v: 1, blocks: […], …}, title?}` | as GET; 400 `bad_blocks`; 403 `not_yours` "That sun belongs to someone else" | Blocks v1 that **keeps what it doesn't know**: extra keys on a block (a quote's `source`, a photo's `width`), blocks of new types, and top-level keys (`kind`, `greeting`, `layout`…) all survive (*mismatch 12*, QUESTIONS #73). A block still needs an `id` matching `[A-Za-z0-9_-]{1,64}`; a photo's `imageKey` is always rebuilt as `suns/<sunId>/<blockId>.jpg` |
| `POST /lineage-links` `{from: {type, id}, to: {type, id}, kind, note?}` | `{link: Link}`; 400 `bad_endpoints`, `self_link`; 404 `no_endpoint`; 403 `not_yours` "A link has to touch something of yours"; 409 `already_linked` | `accepted` when you own both ends, else `pending` until the other owner agrees |
| `GET /lineage-links/pending` | `{links}` | Claims by others on your things |
| `POST /lineage-links/:id/accept` | `{link}`; 403 `your_own`, `not_yours` | **Agree** |
| `DELETE /lineage-links/:id` | `{deleted: true}` | **Refuse**, or retract your own |
| `GET /lineage-links?type=&id=` | `{links}` | Accepted links only: a claim shows nowhere until it is agreed |
| `PUT /saved` / `DELETE /saved` `{kind: "song", publishedTrackId}` · `{kind: "system", systemId}` · `{kind: "galaxy", galaxyId}` | `{saved: true}` / `{saved: false}` | Private: never counted, never shown to the maker. `galaxy` is **new**: Add galaxy (4.11) |
| `GET /saved` | `{songs, films: [], systems, galaxies}` | |
| `GET /catalog?cursor=&limit=&search=` | `{items: [{trackId, publishedId, title, artist, coverArtUrl, duration, bpm, musicalKey, uploader}], nextCursor}` | Originals (depth 0) |
| `GET /feed?cursor=&limit=&medium=&added=1&key=&bpmMin=&bpmMax=` | `{items: [{type: "planet", kind, medium: "music", id, createdAt, title, artist, coverArtUrl, key, bpm, duration, trackId, publishedId, priceCents, galaxyId, galaxy, system}], nextCursor}` | **Newest** (4.10): newest first, the only order; `nextCursor: null` is the end ("That's everything."). The filters are **new**: `medium` takes a comma list of media (`music`, `film`, `writing`, `fashion`), their families (`Mi`, `Si`, `Ri`, `Gi`, any case) or planet kinds (`song`, `page`, `gallery`); the mock's worlds are all music. Ties within a second are broken by id rather than skipped. Sun cards are left out: Newest lists works |
| `GET /universe` | `{galaxies: [{id, x, y, slug, displayName, skySeed}], binaryPairs: [], events: []}` | No fog, fuel, travel costs or gravity wells (4.12, *mismatch 11*). Drift and weather aren't simulated |
| `POST /travel` `{toGalaxyId, launchAngle?}` + `Idempotency-Key` | `{outcome: "arrived", landedGalaxy: {id, slug, displayName, skySeed, x, y}, path: [[x, y] × 64]}`; 400 `idempotency_key_required`, `no_galaxy`, `bad_target`, `already_there`; 404 | Free and never captured (4.12): no `fuelSpent` |
| `GET /since?after=<ISO>` | `{events: [SinceEvent]}`, newest first | **New** (2.10). `fork {at, who, work, fork}` for forks of your works; `link {at, who, linkId, link: {from, to, kind}}` for every claim still waiting on you, however old; `sale {at, work, priceCents}`; `payout {at, amountCents}`; and from galaxies you've added, `work {at, who, title}` and `letter {at, who, greeting, opening}`. One event per thing, nothing summed |

Not here: `/galaxies/:slug/lineage`, `/lineage-web` and `/constellations`
(9.7 doesn't list them), galleries, films, page planets (4.7 names no API for them),
`/api/v2/views`, the leaderboard, fuel, comets and wormholes.

## Buying (6.12, 7.10–7.14, 7.20)

| Endpoint | Body / query | Answer | Notes |
|---|---|---|---|
| `PUT /api/tracks/:trackId/set-price` | `{price}` in dollars | `{success: true, isForSale: true, price}`; `{success: true, isForSale: false}` for 0 or none; 400 "Price cannot exceed $2000"; 400 `{error: "A paid file costs at least $2.", code: "price_too_low"}`; 400 `{error: "This fork has no file of its own yet. Export it to sell it.", code: "no_file"}`; 400 "Selling needs a payout account. Set it up once."; 403 "Not authorized" | A shelf opens only once payouts are ready (7.12, *mismatch 17*). The $2 minimum is Open #44 at its recommendation (the server has none) |
| `POST /api/purchase/create-checkout` | `{type: "track" \| "fashion", id}`; optional `Idempotency-Key` | `{url, sessionId}`; 400 "Cannot purchase your own content" / "…own listing", "Already purchased", "Track not for sale"; 404; 409 "Seller payout setup incomplete"; **409 "Just sold or being purchased"**; 429 after 20 an hour | One item. The charge is the price exactly, its line item reading "Purchase on WWAV (10% platform fee included)". The session expires in 30 minutes, with the one-of-one hold (server.md §11: today the session lives 24 h while an hourly sweep frees the hold, so a garment can sell twice; QUESTIONS #71). A garment's sale is a destination charge on the seller's behalf, shipped in the US only. `sessionId` is new. Asking again for what you're already checking out answers that open checkout: the one holding your garment (even one opened by a bag checkout whose answer was lost), or the open one for exactly this file. *Mismatch 18:* refusals stay 400, a pending purchase doesn't block a second checkout on the server, and the 20 an hour is the mock's own bucket (the server shares it with splits and Founding claims) |
| `GET /api/purchase/check` | `type, id` | `{purchased}` | Only a completed purchase counts (7.20: today a pending one does). `type` is `track` (the default) or `fashion`; the mock sells no albums, films or tickets, so those are never owned (the server maps any other type to a track) |
| `GET /api/entitlements` | — | `{items: [{kind: "track", id, trackId, songId, title, artist, purchasedAt, versions: [{version, fileName, bytes, sha256}]}]}` | **New.** A purchase covers every version (6.12) |
| `GET /api/entitlements/:kind/:id/file` | `version?` (default the newest) | `{url, expiresIn: 60, fileName, bytes, sha256, version}`; 403 `{error: "You haven't bought this yet.", code: "not_entitled"}` | **New.** The exact bytes the maker uploaded, through a URL good for 60 s, only after a completed purchase |
| `POST /api/connect/onboard` | — | `{url}` to the fake Stripe onboarding page | |
| `GET /api/connect/status` | — | `{connected, onboarded, chargesEnabled, payoutsEnabled, detailsSubmitted}`, or `{connected: false, onboarded: false, message: "No Stripe account linked"}` | |
| `GET /api/store/halls?order=` | `order`: `newest` (default), `artist`, `lineage`, `alphabet` | `{asOf: "YYYY-MM-DD", halls: [{medium, family, shops: [{id, seller, openedAt, seat: {track, phase, eccentricity}, crates: [{id, title, items: [Item]}]}]}]}` | **New** (7.3, 7.13, 7.17): the whole store in one answer, so List view reaches and buys everything. A crate is a solar system; goods hang on one rail. `seat` is the day's place: the first track with room, the golden angle per place plus an FNV-1a jitter, one day's Kepler step per day; the same all day for everyone. Soft repulsion and lineage drift aren't modelled |
| `GET /api/store/new` | — | `{items}` | **New.** The 12 newest |
| `GET /api/store/bag` · `PUT /api/store/bag {type, id}` · `DELETE /api/store/bag/:type/:id` | — | `{items: [Item & {available}], payments: [{kind: "digital", items, amountCents} \| {kind: "physical", seller, items, amountCents}], totalCents}` | **New.** `payments` is how the bag will charge, for "1 payment for 3 files, 1 for Ana's jacket." Adding refuses what checkout refuses. A thing that left the shop while in the bag (withdrawn, off sale, sold, bought another way) stays in `items` with `available: false`, so the buyer sees which one and takes it out; `payments` and `totalCents` count only what's available |
| `POST /api/store/bag/checkout` | optional `Idempotency-Key` | `{payments: [{kind, seller?, items, amountCents, sessionId, url}]}`; 409 "Just sold or being purchased" if a garment is held; 409 `{error: "Nothing in your bag can be bought now.", code: "unavailable"}`; 400 "Your bag is empty"; 429 as create-checkout | **New.** Every available file in one charge, transferred to each seller (90%) under one `transfer_group`; each seller's goods in a destination charge of their own. Holds are all taken before any session opens, and let go if one fails. What was charged leaves the bag; unavailable rows stay. Send an `Idempotency-Key`: the retry of a lost answer gets the same sessions |

Item: a record `{type: "track", id, medium: "music", title, artist, seller, priceCents, gets: ".wwav, master and four stems", key, bpm, durationSeconds, provenance: {generation, parent: {title, artist} | null}, yours, createdAt}`; a garment `{type: "fashion", id, medium: "fashion", title, seller, priceCents, size, condition, category, measurements, material, brand, color, photos, oneOfOne, shipsFrom, shipsTo: "US", status, hold: {until, yours} | null, createdAt}`. No item says how it sold.

Not here: the back room (`/api/purchases/*`, `/connect/balance`,
`/request-payout`), `/api/fashion-listings`, orders and events, sizes as
variants (7.20), and the Made Archive.

## The fake Stripe

| Endpoint | What it does |
|---|---|
| `GET /__stripe/checkout/:id` | The hosted page: each line, its "(10% platform fee included)", the total, and **Pay** |
| `POST /__stripe/checkout/:id` | Pays: `checkout.session.completed`, then 302 to `<base>/?purchase=success`. 410 once the session has expired |
| `GET`/`POST /__stripe/onboard/:account` | Express onboarding; the POST marks details submitted, charges and payouts enabled |

Completing a session completes its purchases, marks a garment sold, and
records the transfers in `state.transfers`. A file the buyer already owns by
then (paid for through another checkout while this one was open) is refunded
instead: its purchase reads `refunded`, `session.refundedCents` grows, and
its seller is sent nothing for it. Each item's 10% is rounded on its own and
`session.feeCents` is their sum, so the fee and the transfers add up to the
charge to the cent. A session or hold past its 30
minutes expires before any request is handled, and the garment is back on its
rail.

## Heat sync (2.8, 3.15, 9.7): new

| Endpoint | Body / query | Answer |
|---|---|---|
| `POST /api/heat/changes` | `{device, cursor?, changes: [{kind, id, field, value, seq}]}`, at most 500 | `{cursor, kept: [{kind, id, field, value, seq, device}]}`; 413 over 500; 400 for a bad change or no device; 409 `cursor_ahead` (nothing is stored) |
| `GET /api/heat/changes` | `cursor` (default 0), `limit` (default and max 500) | `{changes: [{kind, id, field, value, seq, device}], cursor, more}`; 409 `{error, code: "cursor_ahead"}` for a cursor past the end of the log (from before a reset or a restore): pull again from 0 |
| `GET /api/heat/public/:userId` | — | `{now: {text} \| null, timelines: [{projectId, targetId, title, milestones: [{id, title, date, done}]}], items: {<kind>: [{id, ...only 3.15's fields}]}}`; 404 for an unknown account | **New** (3.15, 8.7): the public Heat view, built from the synced copies. A record shows only when its `public` field is true, with only the fields 3.15 lists; a grade adds its course's code and a focus record its task's title. The Now making line is gone at its `clearsAt`. No count or total in any answer. Open to anyone |

Private to the account. The server keeps, field by field, the change with the
higher `seq`, so a slower older write never overwrites a newer one (PLAN
S2.7). A device keeps its counter above every `seq` it has pulled (a Lamport
clock), so higher means later across devices too; equal `seq`s go to the
higher device id. `kept` lists the pushed fields where the server already
held something newer, with that value. A push's `cursor` is the one the
device sent (0 if none), moved past the device's own changes and any
overwritten since, and no further, so pulling from it never skips another
device's change: send the cursor from your last pull. A pull returns each
field's current value once: a value overwritten since isn't sent. A deleted
record is a field `deleted` set to true.

## Claude (2.11, 3.12, 7.16, 9.8, 10.4): new

`POST /api/assist/:task` with the task's body answers `{result}`, the same
every time for the same input.

| Task | Body | `result` |
|---|---|---|
| `score` | `{title, type?, notes?, averages?}` | `{difficulty, minutes, reason}` |
| `score-batch` | `{items: [{index, title, type?}]}` | `{items: [{index, difficulty, minutes}]}`, minutes 5–600 |
| `read-mail` | `{messages (≤ 8), today, zone, titles}` | `{tasks: [{title, messageId}]}` |
| `syllabus` | `{text}` | `{categories: [{name, weight}], scale, keywords}` |
| `review-note` | `{facts}` | `{draft}`, restating only the facts |
| `release-plan` | `{title, releaseDate}` | `{phases: {pre, launch, post}}` |
| `feedback` | `{question, analysis}` | `{answer}` |
| `clerk` | `{record: {title, artist, notes?}, question}` | `{answer}`: "I don't know. Neither the file nor Ana's notes say." without notes |

401 signed out; 404 "No such task"; 400 for a missing input; 429
`rate_limited` "Too many requests. Wait a minute, then try again." past 10 a
minute; 429 `{error: "Claude's 50 calls for today are used. They come back at midnight.", code: "daily_limit", retryAfterSeconds}`
past 50 a day (UTC). Only answered calls count toward the 50.

## Updates (9.9): new

`GET /desktop/latest.json`: a Tauri updater manifest,
`{version, notes, pub_date, platforms: {"darwin-aarch64" | "darwin-x86_64" | "linux-x86_64": {signature, url}}}`.
The signatures are empty until a test sets real ones (`PUT /__mock/latest` or
`state.latest`).

## Test controls

| Endpoint | Body | What it does |
|---|---|---|
| `POST /__mock/reset` | — | The seed again; the clock back to its start |
| `POST /__mock/clock` | `{advanceMs}` | Moves the clock on: tokens, codes, presigned URLs, holds and sessions expire by it |
| `POST /__mock/fail` | `{method, path, status?, drop?: "before" \| "after", times?}` | The next `times` (default 1) requests to that exact path fail: with `status` and nothing done; dropped before anything is done; or done and then dropped, so the answer is lost (a retry test) |
| `POST /__mock/stripe/complete` | `{sessionId}` | The `checkout.session.completed` webhook; 409 "This checkout has expired." |
| `PUT /__mock/latest` | a manifest | What `/desktop/latest.json` answers |
| `GET /__mock/state` | — | The whole state as JSON (file bytes as their size) |
| `GET /` | — | A page saying what this is; where Checkout's success URL lands |

`helpers.js`'s `signInWithoutBrowser` is the browser's half of desktop
sign-in: it opens the authorize page, posts the form, and returns the loopback
redirect with its `code` and `state`. A test in another language can do the
same with two requests: the page's form carries the request as
`<input type="hidden" name="request" value="…">`.
