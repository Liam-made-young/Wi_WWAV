# Space: every link is a planet

Written 8 Oct 2026 from the founder's brief of that day, and brought up to
his answers of the same day, in two rounds (section 8). Where this file and
`docs/SPEC.md` chapter 4, `docs/SCOPE_CUT.md` ("Space: the social view") or
`docs/PLAN.md` Stage 5 disagree, this file is right and the others are the
old Space.

Three labels are used throughout:

- **Decided**: the founder said it. Don't change it without his word.
- **Proposed**: an agent's recommendation. Build on it only after he agrees,
  or say in your commit that you built on a proposal.
- **Open**: nobody has answered yet. Section 8 lists them.

## 1. What Space is (Decided)

Space is something different from what it was, and new: a space between a
browser and social media, in 3D, that you explore by moving through it. It
is a search engine, a browser and a social medium wrapped into one. **The
search engine and the browser are built first.** The social half comes later,
and there is no posting yet.

**Why that order.** A social medium needs people to get people, and is dull
when nobody is around. So Space has to be worth using with one person in it,
the founder, and get more interesting from there.

**The defining rule.** Every link is a planet. That holds for a link posted
through Wi-WWAV's social side, for a work uploaded straight to the platform,
and for every link on the web. A link that connects many links grows in size
and gains gravity. YouTube connects so many links that it is, in effect, a
sun.

**Size is rough on purpose.** The number of links behind something like
YouTube can't be counted: it changes by hundreds of thousands or millions a
day. So a planet is sized by a rough algorithm, and the algorithm has to be
clear about how it arrived at a size.

**The scale runs galaxy, sun, planet, moon.** A song is a planet or a sun; a
service that plays it is a planet or a moon. An album is a solar system in
its artist's galaxy, and Spotify is a bigger galaxy than any one artist.

**The browser is law.** Space invents no categories. There is no "Music"
galaxy: nothing on the web is called that, and it would be far too big. What
holds what is what the web itself says.

**Where a link appears depends on how you came to it.** Looking for a YouTube
video, you find it with its channel. Looking for the song, you find it with
the song.

**You are an astronaut, seen from your own eyes.** "FPS: first person
space." You float, and you move. Things may move around you.

**A web page is a planet, and you go into it.** Opening a link doesn't leave
Space. The page itself, its real HTML, CSS and JavaScript, is enclosed in the
body. Zoom in and the page grows until it fills the screen, and you read and
search it like any web page. Zoom out and it falls away into space.

**Search moves you.** A search's results appear as planets, and your view is
taken to them. The results are not brought to you. They are also gathered
where you arrive, if that can be made to work with their staying at home
(section 4).

**No feed.** Space is searched, not scrolled.

**It is a desktop.** The old limit of 21 worlds to a solar system came from
the iPhone and is gone.

**Home** is the founder's own galaxy, the whole of the one at
`https://www.mi-wwav.com/summer_26/g/liam-made-young`, for the prototype. It
also holds the history of where he has travelled, in his orbit: every visit
leaves a mark in an outer ring, and keeping one pulls it closer. A mark is a
way back to the body, not a copy of it.

**Staying signed in.** You can sign in to Spotify, Apple Music and the rest
inside Space for whole songs, and Space keeps accounts the way Safari and
Chrome do: signed in to Wi-WWAV, and to every other site once you have signed
in there once.

**People still have galaxies,** but that is not the next move.

**It is a prototype,** not a live product: it has to work as well as it can
for one person, with the fewest moves for the greatest return and no polish
for its own sake. Section 12 says what changes for many.

**Free the way Chrome and Google Search are free,** for now. Nothing in the
prototype is paid for.

**The big platforms come first,** searchable from inside Space. Everything
that can be fitted in through an embed is fitted in through an embed.

| Order | Platform | What the founder asked for |
|---|---|---|
| 1 | YouTube | The big one. YouTube search and the YouTube embed run Space: search a video, the results are planets, you are moved to them |
| 2 | Spotify | Search a song and the same thing happens. The song plays through the embed |
| with 2 | Bandcamp, SoundCloud, and any service added later | A song search returns the YouTube link and the Spotify link both, and a link for every other service supported. The embed works with all of them and the results work with all of them |
| soon | Wikipedia, Substack | The next two to sweep through |
| also | RSS blogs, podcasts | Their embeds should work |
| also | Apple Music | Should work through WebKit |
| also | The rest of the web | Through a general web search provider, a free one, chosen to suit Space (section 5) |
| wanted | X | Without paying for its API. No feed. Searchable posts, maybe. See section 6 |

"This will get better and better as we build."

## 2. Words, and where a link sits

- **Link**: one canonical address. Two addresses that lead to the same thing
  (a tracking parameter, `youtu.be` and `youtube.com/watch`) are one link.
- **Body**: what a link is drawn as. One link is one body, with one mass and
  one mark in your history, wherever it is shown.
- **Mass**: how many links a link connects, as an order of magnitude.
- **Orbit**: where a body is shown, around the thing you reached it through.
  One body can be met in more than one orbit (Decided: it depends on how you
  came).
- **Native platform**: one Space can search and play by itself (section 5).

**The scale.** The Music column is the founder's. The others are Proposed by
the same pattern, and every name in the table is something the web itself
has: a site, a page on it, a link on that page.

| Size | Music | YouTube | Writing | Any other site |
|---|---|---|---|---|
| A massive galaxy | A service: Spotify, bigger than any one artist | YouTube | Substack; Wikipedia | A very large site |
| A galaxy | An artist | A channel | A publication | A site |
| A solar system | An album, in its artist's galaxy | A playlist | A series or section | A section of the site |
| A planet | A song | A video | A post or an article | A page |
| A moon | Each service's link to the song | The links in its description | The links in it | The links on it |

Which of these a thing is drawn as also follows its mass (section 3): a
channel with ten videos is not a galaxy yet.

**One rule under all of it (Proposed).** Where you are is the centre, and
what it links to orbits it. A site's own structure gives the big steps
(service, artist, album, song), and the links on a page give the rest. Go to
one of the orbiting bodies and it becomes the centre. That is how a song is
"a planet or a sun": seen from its album it is a planet, and when you are at
it, it is the sun of its services.

**The same orbit is always in the same place (Proposed).** A body's position
around a given centre comes from a stable hash of the two links. So a context
looks the same every time and on every machine, even though one link can be
met in several contexts.

## 3. How a link gets its size (Proposed)

The founder's rule is that connecting many links makes a link bigger, and
that the sizing must be rough but clear. This is one way to do that.

1. **Mass is a magnitude, not a count.** A link's magnitude is the power of
   ten of the links it connects: about 10 links is 1, about 10,000 is 4. A
   whole number, 0 and up. Nothing in Space claims an exact count.
2. **Where the number comes from,** taking the first that applies:
   - the platform's own figure, when it gives one: a channel's video count, a
     playlist's or an album's length, a Wikipedia article's link count, a
     feed's item count, a podcast's episode count;
   - for a whole site, its place in a public ranking of the web's link graph,
     shipped with the app as a table and refreshed now and then;
   - for any other page, the links found on it when it is fetched, plus the
     links to it that Space already knows;
   - for the native platforms themselves, a short table written by hand, in
     magnitudes, so YouTube is as big as it should be from the first launch.
3. **A body can say why it is its size,** in one line: "About 10,000 links:
   12,300 videos on this channel." That line is the clarity the founder asked
   for, and it is shown wherever a body's details are.
4. **Size on screen and gravity both come from the magnitude,** and from
   nothing else.
5. **A magnitude moves a whole step or not at all,** so a body doesn't
   flicker between sizes as its count drifts.

**Gravity (Decided, answer 3).** It bends and slows the traveller near big
bodies and holds orbits together. It never moves a body, so a link is in the
same place for everyone.

## 4. Search (rule Decided, mechanics Proposed)

Decided: results are planets, and you are moved to them. Results both stay at
home and are gathered, if that works.

Proposed, as the way to have both:

- A result is shown in the orbit that fits what you searched for (section
  2): a video search shows a video with its channel, a song search shows it
  with the song.
- When the results share a centre (a YouTube search, a song and its
  services), you are flown there and the bodies that match are lit. Home and
  gathered are the same place.
- When they are scattered (a web search), you are flown to the best one, and
  the rest stand around you as beacons: a light and a name in the direction
  of each. Choosing a beacon flies you there. A beacon is the same body seen
  from far off, not a copy.
- Arriving at a planet and going on in opens it (section 11).
- Every search is a core command (`space.search`), so Claude can call what a
  person can click.

## 5. The platforms

The list is Decided. The rest of each row was written from memory on 8 Oct
2026 and, except where a row gives a date, **has not been checked against the
platform's current terms**. The first agent to build on a row checks it,
corrects it here, and dates the correction.

| Platform | Finding | Playing or reading | A size signal | Needs, and what to check |
|---|---|---|---|---|
| YouTube | Data API v3 search | IFrame Player | A channel's video count; a playlist's length | A free API key. The default quota is small: a search costs 100 units of 10,000 a day, so about 100 searches a day for one key until more is granted. **Checked 8 Oct 2026:** the player answers "Error 153" in a WebKit view when the page holding it sends no referrer, which an app's own page does not. The known cure is to hold the player in a page served from a real web address, with `referrerpolicy="strict-origin"` on the frame. Prove it in the Mac app first |
| Spotify | Web API search, with a registered app | The embed, and its iFrame API | An album's or a playlist's length | Whole songs play only for a listener signed in to Spotify in that same web view; otherwise a preview. A registered app in development mode serves only a few listeners |
| Apple Music | iTunes Search API, no key | The `embed.music.apple.com` embed | An album's length | Previews unless signed in. Whether signing in and protected playback work in the app's WebKit view is untested |
| Bandcamp | No public search | Its embedded player, by the id on the album's or track's page | An album's length | Finding comes from a pasted link or the general web search |
| SoundCloud | oEmbed for any link; search needs a registered app | The widget, and its Widget API | A playlist's length | |
| Wikipedia | Its search, no key | The page itself, or text with the reader the Wiki tab already has (`crates/wi-wiki`) | An article's links out and links in: the closest fit to the defining rule | A contact in the User-Agent, which the Wiki tab already sends |
| Substack | No official API. Every publication has a feed at `/feed` | The post's page, or the feed's text | Posts in the feed | Finding comes from a pasted link or the general web search |
| RSS blogs | The feed's address, found from the page | The item's page or text | Items in the feed | |
| Podcasts | iTunes Search API (no key), or Podcast Index (free key) | The episode's audio file plays as it is | Episodes in the show | |
| X | Nothing without paying | The post's embed, by its address, no key | | Section 6 |
| One song across services | A matching service that takes one link and answers the others, or the recording's ISRC | | | Rate limits and terms |

**The general web search (Decided: free the way Chrome and Google Search are
free, so nothing paid and no card. Which way: Proposed).**

| Way | What it is | Cost, checked 8 Oct 2026 | Fit |
|---|---|---|---|
| The search engine's own page | Google or DuckDuckGo opened as a page in Space, like any page. The results page is a body, and Space reads the links on the page in front of you and draws them around it, as it does for every page (section 2) | Nothing: no key, no quota, no card | Proposed first (question 23). It is "free as Chrome" exactly, it covers the whole web, Bandcamp, Substack and X included, and it needs nothing built that Space doesn't need anyway. Fine for one person browsing; the engine's terms are read before anyone else uses it (section 12) |
| Marginalia Search, by its API | An independent, open-source search engine for the small, hand-made, text-first web | Free. A key for non-commercial use is given on request by email; the shared key `public` works at once but is often rate-limited | Proposed second: a different sky, the part of the web no platform covers, with clean results. Its code can be run by us if Space grows |
| Brave Search, by its API | An independent index of the whole web | $5 of credit a month, about 1,000 searches, but a card is required to use it | Out. It is not free in the founder's sense |

## 6. X, without paying for its API (Open)

What is free: showing any post whose address you have, through X's own
embed, and opening x.com itself as a page (section 11), signed in as
yourself. What is not free: searching or reading posts through the API.

| Way | What you get | What it costs |
|---|---|---|
| A. Links only | Any X link put into Space is a planet that opens as the post. Space's search finds the posts Space has been given | Nothing. No discovery of posts nobody has brought in |
| B. The general web search, kept to x.com | Posts found by the search provider, each opened as the post | The provider's allowance; results are partial and late |
| C. X as a page | The real site, signed in as the person, as a body you go into. Space reads nothing from it | Nothing. It is browsing, not searching |
| Not recommended | Scraping the site or a mirror of it | Breaks often, is against X's terms, and risks the person's account |

Recommended: A and C first, B as a trial. Bluesky and Mastodon have open
search and would give "searchable posts" for nothing, if the founder wants
posts from somewhere now.

## 7. The old Space

Until 8 Oct 2026 Space was the social view of `docs/SPEC.md` chapter 4:
people's galaxies holding solar systems of at most 21 worlds, each world a
work published to mi-wwav.com. What is in the repository from it is a frame
(`app/ui/src/space/SpaceView.tsx`, `sky.ts`) drawing a sample sky, a tested
model beside it (`app/ui/src/space/model/`), and the mock server's `/api/v2`
routes (`tools/mock-server/routes/space.js`).

Kept, because the new Space needs the same things (Proposed):

- the sky in three.js with flat labels over it;
- placement from a stable hash, so a link sits in the same place on every
  machine (the hash is of the link, where it was of an id);
- the orbit, Kepler, camera, label and hit-testing maths;
- the stem gesture (`app/ui/src/shared/stems/gesture.ts`) for works that come
  apart;
- works published to mi-wwav.com: each is a link like any other, so each is a
  planet, and a fork is a link between two of them. The founder's galaxy read
  from `/api/v2/galaxies/liam-made-young` is home (section 1).

Gone (Decided): the 21-world cap, and with it the refusal sentence and
milestone S5.4. The rule that nothing moves while nothing plays, and the
drawing only on a change that kept it: you move now, so the sky is drawn as
you do.

Set aside with the social half, not deleted (follows from answers 4 and 11):
Newest and "Since you last looked", lineage links and their consent, letters
on a sun, Add and Add galaxy, the message door. The other Stage 5 milestones
are the old Space's and are not targets.

Don't delete old model code until what replaces it is in and the founder has
seen it. A test goes in the same commit as the code it tests, with a line
saying why.

## 8. Questions

### Answered by the founder, 8 Oct 2026

| # | Question | His answer |
|---|---|---|
| 1 | Do results stay in their home orbit, lit, or get gathered where you arrive? | "Both if u think that works." Section 4 proposes how |
| 2 | A song on four services: one planet with four moons, or four planets? | A song is a planet or sun, a playback service is a planet or moon. The scale is galaxy, sun, planet, moon. The hierarchy for any one link is left to be proposed (section 2) |
| 3 | What does gravity do to the traveller? | As proposed: it bends and slows the camera near big bodies and holds orbits, and never moves a body |
| 4 | Do people still have galaxies? | Yes, but not the most important move yet. Search engine and browser first. Maybe whole web pages enclosed in planets |
| 5 | Does the 21-world cap still apply? | No. "Mega defunct." This is a desktop, not an iPhone |
| 6 | A link with no embed: open the real page? | Yes, as a 3D view of the page: the page is shown as a moon, planet or sun; zoom in until it fills the screen, zoom out into space |
| 7 | May Space use a general web search provider? | Yes, and first. A cool one that suits this software, and free |
| 8 | Whose API keys? | It is a prototype for one person. Make it work as well as it can for him, and say in a doc where it changes for scale (section 12) |
| 9 | Signing in inside the app's web view for whole songs? | Yes. And account management like Safari's or Chrome's: signed in to Wi-WWAV and to every other site after signing in once |
| 10 | Does "nothing moves while nothing plays" still hold? | No. The app is the first-person view of an astronaut floating in space; movement is fine |
| 11 | Is posting a link placing a planet? | Not yet. Browser and search engine for now; no posting |
| 12 | What is in the sky before you have searched? | His own solar system as home, taken from mi-wwav.com, and the history of where he has travelled, in his orbit |

### Answered by the founder, 8 Oct 2026, second round

| # | Question | His answer |
|---|---|---|
| 13 | Is home his whole galaxy, or one of its two solar systems? | The whole galaxy |
| 14 | Does every visit leave a mark around home, or only what he keeps? | As proposed: every visit leaves a mark in an outer ring, keeping one pulls it closer, and a mark is a way back, not the body |
| 15 | A song's YouTube video: with the song, or with its channel? | It depends on the context. Looking for the video, it comes from the channel; looking for the song, it comes from the song |
| 16 | Is Music its own galaxy, and where is an album? | No. "No fake categories, the browser is law"; Music would be far too big a galaxy. An album is a solar system orbiting in its artist's galaxy, and Spotify is a bigger galaxy than any one artist. Context decides here too |
| 17 | What is a Wikipedia article's one parent? | He asked for the question to be made clearer. It is now question 22 |
| 18 | Flying by keys and mouse or by trackpad; and what leaves a page? | "You're getting the idea nicely": both ways to fly; inside a page, pinch out or Esc pushes off, and plain scrolling and typing are the page's |
| 19 | One page live at a time, the rest as pictures? | Yes for the prototype, not for production. Fewest moves for the greatest return |
| 20 | Is a card on file acceptable for a "free" search provider? | He asked for this to be made clearer too, and said everything should be as free as Chrome and Google Search for now. So no card: Brave is out (section 5). Question 23 follows from it |
| 21 | How much of a browser in the prototype? | Back, forward and staying signed in. Polish is not the priority |

### New, from those answers (Open)

| # | Question | Recommendation |
|---|---|---|
| 22 | Question 17 again, plainly. A YouTube video has an obvious thing to orbit, its channel. A Wikipedia article doesn't: "Saturn" is linked from thousands of articles and belongs to none of them. So when you search Wikipedia for Saturn and arrive, what is around you? | The article is the centre and the articles it links to orbit it, with Wikipedia as the galaxy all of it sits in. Fly to one and it becomes the centre. Under "where you are is the centre" (section 2) this stops being a special case |
| 23 | "Free as Google Search" can be had literally: open Google's or DuckDuckGo's own results page inside Space and draw the links on it as planets, with no key at all. Do you want that as the general web search, and which engine first? | Yes. Try Google first, since it is the one you named, and DuckDuckGo if Google's page keeps asking whether you are a robot inside the app |
| 24 | The same could be done for YouTube: its own results page in place of its API's search, which lifts the limit of about 100 searches a day. | The API first: it is free, clean, and enough for one person. The results page only if the limit is ever hit |
| 25 | A page can hold hundreds of links (a Wikipedia article, about 500). How many orbit it? | The biggest few dozen by mass, and the rest as a faint belt you can ask to see |
| 26 | An artist has a page on every service, like a song. Reached through a song search, is the artist one galaxy joined across services, or the artist as the service you came through shows them? Songs can be matched across services reliably; artists only by name, which can be wrong | For the prototype, the artist as that service shows them, and only songs are joined |

## 9. Protocol for any Claude or Codex instance working on Space

**Before anything**

1. Read this file to the end, and all of the shared `COORDINATION.md` (the
   copy at the root of the main checkout, `Wi-WWAV/COORDINATION.md`; a
   worktree's copy can be behind it).
2. Read `docs/SPEC.md` 4.3 to 4.5 for the parts of the old Space that are
   kept. Treat the rest of chapter 4 as history.

**Where the work happens**

3. Space work is done on the branch `space`, in the worktree
   `Desktop/Mi-WWAV journey/Wi-WWAV-space`, started from
   `claude/focus-ask-notes` (the branch the installed app is built from; `main`
   once PR #2 is merged). Never in the main checkout, `Wi-WWAV`. The Space
   section of `COORDINATION.md` says whether the worktree exists yet.
4. Two instances at once take one piece each. Each works on its own branch
   off `space`, named `claude/space-<piece>` or `codex/space-<piece>`, in its
   own worktree `../Wi-WWAV-space-<piece>`, and merges into `space` when its
   tests pass.
5. A worktree needs `app/ui/node_modules` linked to the main checkout's, with
   that path in the repository's `info/exclude`, and a build folder of its
   own off the Desktop, which is in iCloud:
   `CARGO_TARGET_DIR=~/Library/Developer/wi-wwav-build/target-space` (or
   `target-space-<piece>`).

**What is yours and what is shared**

6. Space's files: `app/ui/src/space/`, `crates/wi-core/src/space/` and its
   tests, `tools/mock-server/routes/space.js` and `test/space.test.js`,
   `app/ui/e2e/space-*.spec.ts`, and this file.
7. Before editing, write under the Space section of the shared
   `COORDINATION.md` the piece you are taking and the files you expect to
   touch. Don't edit a file another section claims: write a request under
   that section and work around it. If the Space section is missing from the
   shared file, another agent's rewrite dropped it: put it back from your
   branch's copy and leave everything else as you found it.
8. In shared files (`crates/wi-core/src/lib.rs`, the shell, the schema,
   `docs/COMMANDS.md`, `app/src-tauri/`): add, don't rename or remove, and
   list each addition in the Space section.

**Rules of the build**

9. Everything a person can do in Space is a core command (`space.*`), so
   Claude can call it and ⌘Z can undo it.
10. No key, token or cookie in the repository or in the web bundle. Keys live
    in the Keychain, as the calendar addresses do. What a person is signed in
    to, and where they have travelled, stays on their Mac.
11. Before building on a platform, check its current terms and limits, and
    correct its row in section 5 with the date. Draw an embed the way its
    platform requires.
12. No scraping behind a sign-in, and no paid API, without the founder's
    word.
13. It is a prototype for one person. Where you build something that only
    works for one, add its row to section 12.
14. Only the founder makes something Decided. What you choose while building
    goes in `docs/DECISIONS.md`; what you can't choose goes in section 8.

**Checking and handing over**

15. Run what covers your change and say what you ran. From `app/ui`:
    `npx vitest run src/space`. From `tools/mock-server`:
    `node --test test/space.test.js`. On 8 Oct 2026 these passed 119 and 22
    (at `8f4ea3a`). Failing tests that are not Space's are listed in
    `COORDINATION.md`.
16. In a browser, use installed Chrome (`channel: 'chrome'`); Playwright's
    own browser is not installed. Check a port with `lsof` before using it,
    and never stop a server you did not start. What depends on the Mac app's
    WebKit view (embeds, sign-in, pages) is proved in the Mac app, not in
    Chrome.
17. Commit after every piece of work and push your branch. `main` moves by a
    pull request the founder merges; an agent's push to `main` is refused.
18. Don't install a build over the app on this Mac unless asked. If asked,
    build from a branch that holds everything in `claude/focus-ask-notes`, or
    the Focus layout, the prompt box, Database, Wiki, commitments and notes go
    away.
19. When you stop, bring the Space section of `COORDINATION.md` up to date
    (what is built, what is not, requests), and copy the shared file into
    your branch with your last commit.

## 10. A first order of work (Proposed)

Nothing here is started. The founder is still deciding (section 8).

1. **Prove the web views in the Mac app.** A YouTube video, a Spotify song
   and an Apple Music song each playing inside the installed app; a real page
   (Wikipedia, then x.com) opened as a web view of its own; signing in once
   and still being signed in after a restart. Everything else rests on this.
2. **Home and flight.** The founder's galaxy from mi-wwav.com as the place
   you start, and moving through it in first person.
3. **YouTube search to planets.** `space.search`, results as bodies around
   their channels, the flight to them, going in opens the player.
4. **A page as a planet.** The picture from afar, the live page up close, and
   the way out.
5. **Mass and parents,** with YouTube channels and Wikipedia articles, which
   both give a count for nothing.
6. **One song, every service.**
7. **The general web search,** then Wikipedia, feeds and podcasts, Substack.
8. **The history of where you have been,** around home.
9. **X, by link and as a page.**

## 11. A page inside a planet: how (Proposed)

Decided: a page is a body, with its real HTML, CSS and JavaScript; zoom in
until it fills the screen, zoom out into space; and you stay signed in.

What follows was written from memory on 8 Oct 2026 and is unchecked except
where dated. Step 1 of section 10 is there to check it.

- **Most sites refuse to be drawn inside another page.** YouTube's, Google's
  and X's own pages can't be put in a frame in Space's window. So a real page
  needs a web view of its own, which the Mac app can open beside its main one
  (in Tauri this is still marked unstable).
- **A web view is a flat rectangle.** It can't be wrapped around a sphere or
  sit behind other bodies. So a body has two states: from afar it is a
  sphere in the sky wearing a picture of its page; as you close in and it
  turns to face you, the picture gives way to the live page, which grows
  until it fills the screen. Zooming out does the reverse.
- **Embeds are pages too.** A platform's embed address can be the whole page
  of a web view. That matters for signing in: WebKit keeps a site's cookies
  from a frame inside someone else's page, so a Spotify embed framed in
  Space's own window would not know you are signed in to Spotify. As the top
  page of its own web view it does.
- **One store of sign-ins for every Space web view,** kept on the Mac and
  surviving restarts, gives "sign in once, signed in everywhere". Filling in
  saved passwords, as Safari does, is a separate piece and is not in the
  prototype.
- **Google may refuse to sign in inside an app's web view.** YouTube plays
  without signing in, so this blocks nothing at first.
- **Live pages are heavy.** The prototype keeps one live and the rest as
  pictures (Decided for the prototype, answer 19).
- **Reading the links on the page in front of you** is what makes any page a
  centre with bodies around it. A small script in the page's own web view
  lists its links to the core. Nothing is fetched that the person did not
  open.

## 12. The prototype, and what changes at scale

Decided: build for the founder as the only user, and say here where that
stops being enough. Agents add a row when they build something that only
works for one.

| Piece | For one person, now | For many |
|---|---|---|
| API keys (YouTube, Spotify, search) | His own keys, in the Keychain on his Mac | The server holds the keys, makes the searches and caches them. Nobody's key ships in the app |
| YouTube's quota | About 100 searches a day is enough | A cache in front of it, and a request to YouTube for more, which they audit |
| Spotify's app | Development mode, a handful of listeners | Their review for wider access |
| The general web search | The engine's own results page, read as he browses; a free non-commercial key | The engine's terms read first. If they don't allow it: a commercial agreement, a paid plan, or running the open-source engine ourselves |
| Live pages | One at a time, the rest as pictures | Several, with their memory managed |
| Browser chores | Back, forward, staying signed in | Downloads, pop-ups, blocking, filling passwords, profiles |
| Mass and the link graph | Worked out on his Mac from what he has seen | Worked out on the server, so everyone sees the same sky |
| Home | Fixed to `liam-made-young` | Each person's own galaxy |
| Travel history | A file on his Mac | Still private to each person; sent nowhere unless they choose |
| Sign-ins | Cookies in the app's own store on his Mac | The same, never sent anywhere; several people on one Mac need profiles; filling passwords |
| The page that holds an embed | Whatever real address makes the players accept it | A fixed address on wi-wwav.com that the platforms can be told about |
| Platform terms | Personal use | Each platform's terms for a product given to others, read before release |
| Where a page takes you | He chooses where to go | Blocking of harmful pages, pop-ups, and what is shown to whom |
| The Mac's WebKit | The only target | Windows draws pages with a different engine; every web-view rule here is re-proved there |
