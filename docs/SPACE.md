# Space: every link is a planet

Written 8 Oct 2026 from the founder's brief of that day. Where this file and
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
browser and social media, in 3D, that you explore by moving through it.

**The defining rule.** Every link is a planet. That holds for a link posted
through Wi-WWAV's social side, for a work uploaded straight to the platform,
and for every link on the web. A link that connects many links grows in size
and gains gravity. YouTube connects so many links that it is, in effect, a
sun.

**Size is rough on purpose.** The number of links behind something like
YouTube can't be counted: it changes by hundreds of thousands or millions a
day. So a planet is sized by a rough algorithm, and the algorithm has to be
clear about how it arrived at a size.

**Search moves you.** A search's results appear as planets, and your view is
taken to them. The results are not brought to you.

**No feed.** Space is searched, not scrolled.

**The big platforms come first,** searchable from inside Space and drawn as
solar systems, planets and moons. Everything that can be fitted in through an
embed is fitted in through an embed.

| Order | Platform | What the founder asked for |
|---|---|---|
| 1 | YouTube | The big one. YouTube search and the YouTube embed run Space: search a video, the results are planets, you are moved to them |
| 2 | Spotify | Search a song and the same thing happens. The song plays through the embed |
| with 2 | Bandcamp, SoundCloud, and any service added later | A song search returns the YouTube link and the Spotify link both, and a link for every other service supported. The embed works with all of them and the results work with all of them |
| soon | Wikipedia, Substack | The next two to sweep through |
| also | RSS blogs, podcasts | Their embeds should work |
| also | Apple Music | Should work through WebKit |
| wanted | X | Without paying for its API. No feed. Searchable posts, maybe. See section 6 |

"This will get better and better as we build."

## 2. Words (Proposed)

- **Link**: one canonical address. Two addresses that lead to the same thing
  (a tracking parameter, `youtu.be` and `youtube.com/watch`) are one link.
- **Body**: what a link is drawn as. Moon, planet or sun is a matter of its
  size, not of what kind of thing it is.
- **Mass**: how many links a link connects, as an order of magnitude.
- **Parent**: the bigger link a link orbits. A video orbits its channel, and
  the channel orbits YouTube.
- **Native platform**: one Space can search and play by itself (section 5).

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
     magnitudes, so YouTube is a sun from the first launch.
3. **A body can say why it is its size,** in one line: "About 10,000 links:
   12,300 videos on this channel." That line is the clarity the founder asked
   for, and it is shown wherever a body's details are.
4. **Size on screen and gravity both come from the magnitude,** and from
   nothing else.
5. **A magnitude moves a whole step or not at all,** so a body doesn't
   flicker between sizes as its count drifts.
6. **Moon, planet and sun are ranges of magnitude.** Where the lines fall is
   set by looking at a real sky, not decided here.
7. **A link orbits the nearest bigger link that holds it:** a video its
   channel, a channel its platform, an article its wiki, a post its
   publication, an episode its show.

What gravity does to the person travelling (pulls the camera, slows you
near a sun, holds orbits together) is Open.

## 4. Search (rule Decided, mechanics Proposed)

Decided: results are planets, and you are moved to them.

Proposed:

- A result has a home, which is its orbit around its parent. A YouTube search
  takes you to YouTube's system, and the planets that match are the ones lit.
- Arriving at a planet opens what it holds, flat and facing you: the
  platform's embed, drawn as the platform requires (not covered, not hidden,
  not shrunk below its minimum). Text stays flat, as `docs/SPEC.md` 4.5 said.
- A song found on several services is one work with a link on each service.
  Whether that is one planet with a moon per service, or a planet per link,
  is Open (section 8).
- Every search is a core command (`space.search`), so Claude can call what a
  person can click.

## 5. The platforms

The list is Decided. The rest of each row was written from memory on 8 Oct
2026 and **has not been checked against the platform's current terms**. The
first agent to build on a row checks it, corrects it here, and dates the
correction.

| Platform | Finding | Playing or reading | A size signal | Needs, and what to check |
|---|---|---|---|---|
| YouTube | Data API v3 search | IFrame Player | A channel's video count; a playlist's length | A free API key. The default quota is small: a search costs 100 units of 10,000 a day, so about 100 searches a day for one key until more is granted. The player may refuse to play in a page with no web address of its own, which the app's window is. Prove playback in the Mac app first |
| Spotify | Web API search, with a registered app | The embed, and its iFrame API | An album's or a playlist's length | Whole songs play only for a listener signed in to Spotify inside that same web view; otherwise a preview. A registered app in development mode serves only a few listeners |
| Apple Music | iTunes Search API, no key | The `embed.music.apple.com` embed | An album's length | Previews unless signed in. Whether signing in and protected playback work in the app's WebKit view is untested |
| Bandcamp | No public search | Its embedded player, by the id on the album's or track's page | An album's length | Finding has to come from a pasted link or a general web search |
| SoundCloud | oEmbed for any link; search needs a registered app | The widget, and its Widget API | A playlist's length | |
| Wikipedia | Its search, no key | Text, with the reader the Wiki tab already has (`crates/wi-wiki`) | An article's links out and links in: the closest fit to the defining rule | A contact in the User-Agent, which the Wiki tab already sends |
| Substack | No official API. Every publication has a feed at `/feed` | The post's embed, or the feed's text | Posts in the feed | Finding has to come from a pasted link or a general web search |
| RSS blogs | The feed's address, found from the page | The item's text | Items in the feed | |
| Podcasts | iTunes Search API (no key), or Podcast Index (free key) | The episode's audio file plays as it is | Episodes in the show | |
| X | Nothing without paying | The post's embed, by its address, no key | | Section 6 |
| One song across services | A matching service that takes one link and answers the others, or the recording's ISRC | | | Rate limits and terms |

## 6. X, without paying for its API (Open)

What is free: showing any post whose address you have, through X's own
embed. What is not free: searching or reading posts through the API.

| Way | What you get | What it costs |
|---|---|---|
| A. Links only | Any X link put into Space is a planet that opens as the post's embed. Space's search finds the posts Space has been given | Nothing. No discovery of posts nobody has brought in |
| B. A web search kept to x.com | Posts found by a general search engine's API, each opened through the embed | A search provider's free allowance; results are partial and late |
| C. X in its own web view | The real site, signed in as the person, in a browser pane. Space reads nothing from it | Nothing. It is a browser, not a search |
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

Proposed to keep, because the new Space needs the same things:

- the sky in three.js with flat labels over it, drawn only when something
  changes;
- placement from a stable hash, so a link sits in the same place on every
  machine (the hash is of the link, where it was of an id);
- the orbit, Kepler, camera, label and hit-testing maths;
- the stem gesture (`app/ui/src/shared/stems/gesture.ts`) for works that come
  apart;
- works published to mi-wwav.com: each is a link like any other, so each is a
  planet, and a fork is a link between two of them.

Open, because the brief doesn't say (section 8): people's galaxies, the
21-world cap, suns as a person's page and letters, Newest and "Since you last
looked" (which are close to a feed), and the Stage 5 milestones as written.

Don't delete old model code until what replaces it is in and the founder has
seen it. A test goes in the same commit as the code it tests, with a line
saying why.

## 8. Open questions for the founder

| # | Question | Recommendation |
|---|---|---|
| 1 | Does a result stay in its home orbit and get lit, or are results gathered in front of you where you arrive? | Home orbit, lit. A place you can go back to is what makes it a space and not a results page |
| 2 | A song on four services: one planet with four moons, or four planets? | One planet for the work, a moon per service. Each link is still a body |
| 3 | What does gravity do to the traveller? | It bends and slows the camera near big bodies and holds orbits. It never moves a body, so the sky stays the same for everyone |
| 4 | Do people still have galaxies? Where does a person's page and public Learn view live? | Undecided. A person could be a link too: their page is a body that grows with what they have posted |
| 5 | Does the 21-world cap still apply to anything? | Drop it. A channel has thousands of videos |
| 6 | A link with no embed: does Space open the real page in a browser pane? | Yes. That is the browser half of "between a browser and social media" |
| 7 | Bandcamp, Substack and X have no search of their own that Space can use. May Space use a general web search provider? | Yes, on a free allowance, with the provider named in Settings |
| 8 | Whose API keys? One key bundled with the app shares one quota among everyone (about 100 YouTube searches a day) | The founder's own keys in the Keychain while it is "for myself first"; later, searches go through the server, which caches them |
| 9 | Signing in to Spotify or Apple Music inside the app's web view, for whole songs: acceptable? | Try it in the first spike and decide on what it shows |
| 10 | The old rule that nothing moves while nothing plays: does it hold now that you travel? | The sky is still when you are; it moves when you move |
| 11 | Posting through the social side: is posting a link the same as placing a planet, and whose orbit is it in? | Undecided |
| 12 | What is in the sky before you have searched anything? | The native platforms as suns and a few thousand well-known sites. Everything else appears when it is found |

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
   that section and work around it.
8. In shared files (`crates/wi-core/src/lib.rs`, the shell, the schema,
   `docs/COMMANDS.md`): add, don't rename or remove, and list each addition
   in the Space section.

**Rules of the build**

9. Everything a person can do in Space is a core command (`space.*`), so
   Claude can call it and ⌘Z can undo it.
10. No key, token or cookie in the repository or in the web bundle. Keys live
    in the Keychain, as the calendar addresses do.
11. Before building on a platform, check its current terms and limits, and
    correct its row in section 5 with the date. Draw an embed the way its
    platform requires.
12. No scraping behind a sign-in, and no paid API, without the founder's
    word.
13. Only the founder makes something Decided. What you choose while building
    goes in `docs/DECISIONS.md`; what you can't choose goes in section 8.

**Checking and handing over**

14. Run what covers your change and say what you ran. From `app/ui`:
    `npx vitest run src/space`. From `tools/mock-server`:
    `node --test test/space.test.js`. On 8 Oct 2026 these passed 119 and 22
    (at `8f4ea3a`). Failing tests that are not Space's are listed in
    `COORDINATION.md`.
15. In a browser, use installed Chrome (`channel: 'chrome'`); Playwright's
    own browser is not installed. Check a port with `lsof` before using it,
    and never stop a server you did not start.
16. Commit after every piece of work and push your branch. `main` moves by a
    pull request the founder merges; an agent's push to `main` is refused.
17. Don't install a build over the app on this Mac unless asked. If asked,
    build from a branch that holds everything in `claude/focus-ask-notes`, or
    the Focus layout, the prompt box, Database, Wiki, commitments and notes go
    away.
18. When you stop, bring the Space section of `COORDINATION.md` up to date
    (what is built, what is not, requests), and copy the shared file into
    your branch with your last commit.

## 10. A first order of work (Proposed)

1. **Prove the embeds in the Mac app.** A YouTube video, a Spotify song and
   an Apple Music song each playing inside the installed app's window. If
   YouTube refuses the app's page, find what page it accepts. Everything else
   rests on this.
2. **YouTube search to planets.** `space.search`, results as bodies around
   their channels, the camera taken to them, arriving opens the player.
3. **Mass and parents,** with YouTube channels and Wikipedia articles, which
   both give a count for nothing.
4. **One song, every service.**
5. **Wikipedia, feeds and podcasts, Substack.**
6. **X, by link.**
