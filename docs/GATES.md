# Wi_WWAV — the four gates

The founder's design audit (`formats/wi/GATES.md`, the gates as Wi ran them),
applied to the whole desktop app. Chapter 10.6 of `docs/SPEC.md` gives the
desktop wording used here.

Rule 2 says to write down what a fail looks like before testing. This file was
committed with the fail criteria and no results, before any code was written.
Results are added in later commits, each with its evidence. A check with no
evidence is marked **open**. It is never marked passed.

---

## Gate 1: Corruption audit

Does the success of the app depend on the net harm of living things?

| # | Question | Fails if… | How it's tested |
|---|---|---|---|
| 1.1 | Users are healthier | Anything stretches use past what the person came for: autoplay, a list or floor without an end, notifications, badges, streak counters, "up next", a focus round or break that starts on its own | Drive every room in Playwright: nothing makes a sound or moves until a press, every list ends "That's everything.", the store ends at a wall. Search the code for OS notifications, Dock badges, timers that reach out, and autoplay |
| 1.2 | Users are freer | Any file can't leave as its exact bytes, or the whole account can't leave in one action, signed in or out | Export everything (⌘⇧E) while signed out; sha256 every file in `media/` against the library; open the export's `index.html` offline |
| 1.3 | Users aren't addicted | Anyone, owners included, is shown a count of other people's attention (plays, likes, followers, views, sales on a shelf), or anything is ordered by engagement or sales | Read every field each client and mock API returns and everything each room renders; grep the UI for counts of other people |
| 1.4 | The business works if 1.1–1.3 are true | 12 months after the store takes its first payment, the store fee, Pro, Founding and split packs together don't cover the server's running costs (Heroku, Postgres, R2, Replicate, the Anthropic API, Apple's developer program); or any revenue line depends on a count of attention | Can't be tested before the store takes money. Stays **open** |

## Gate 2: Good design audit (must pass all 6)

| # | Question | Fails if… | How it's tested |
|---|---|---|---|
| 2.1 Complexity | What verb does each room simplify? Steps before vs after | For any room's verb, steps after ≥ steps before | Count by doing, at each room's first build. Heat: *knowing what to do next*. Space: *hearing a song apart*. Console: *finishing a song that comes apart*, and *cutting a film to its score on one clock*. Unquantized: *buying a file from the person who made it* |
| 2.2 Flourishing | What can the person do after a month that they couldn't before? | No positive answer after a month | The answer: *a term planned and kept, a song finished that comes apart, and a shelf that pays*. Heat's own records are the month of evidence. Needs the founder |
| 2.3 Freedom | Can they leave with all outputs? Does it work without the company? | (a) A downloaded or exported file differs from the original by any byte. (b) An exported `.wwav` or `.swav` fails in tools that know nothing of Wi_WWAV (`ffprobe`/`ffmpeg`, `wwav_pack.py`, `swav_pack.py`). (c) The export, opened offline, doesn't play its songs apart and its films. (d) A session, export or purchase needs WWAV's server to open | Byte compare. `ffmpeg -v error … -f null -`. Both pack tools' `info`. Open `index.html` from disk with the network off. Open a session and a purchase with the network off |
| 2.4 Being & body | Does it respect attention, eyes, hands, posture, sleep? | **Eyes:** body text under 7:1 or secondary under 4.5:1, in light or dark, in any register. **Hands:** a button, tab or orb under 44 × 44 pt; a dense row or grid cell under 24 × 24 pt (rule in 3.18); any gesture without a key. **Posture:** text read for more than a line under 17 pt; anything under 11 pt. **Sleep and attention:** anything moves while nothing plays, or anything plays without a press | Measure at 1024 × 680 and 1280 × 800, at default size and at ⌘+'s 20 pt, in light and dark, in all four rooms: computed contrast and bounding boxes for every control, and a frame-diff over 5 s of idle |
| 2.5 Craft | Would you show the inside to someone you respect? | An independent review finds a bug that is left unfixed; the tests aren't green; one plugin can take the app down | Every suite green in CI. An adversarial review whose findings are each fixed or answered. 200 random engine kills with the app still up |
| 2.6 Wisdom | Did you decide what not to build, and why? | No written list of what was left out and the reason for each | `docs/SPEC.md` 10.7 and every chapter's "Left out, and why" table |

## Gate 3: Great design audit (scores 1–10, not pass/fail)

| # | Question | How it's scored |
|---|---|---|
| 3.1 Immortal | Would this still make sense in 50 years? What dies first? | List the parts by lifespan: the files (RIFF/WAVE 1991, ISO BMFF 2001, JSON) last longest; the web UI dies first, then plugin formats |
| 3.2 Invisible in use | Seconds spent thinking about the tool | **Unscored** until a person is watched. Proxy: presses from opening a `.wwav` in Finder to hearing it apart (target 2: double-click, Space) |
| 3.3 Paradigm shifting | What behavior stops? | A song sold as a sealed, flattened file; a label standing between an artist and a buyer |
| 3.4 Categorically fresh | What category does it adopt, and how far outside it does it sit? | It adopts the DAW, the social network and the store, and moves outside them by sharing one file across all three |
| 3.5 Anti-entropy | What accumulates and what decays? | Lineage links and Heat's records accumulate; signed links and plugin compatibility decay |

## Gate 4: Mission audit

Did someone who doesn't consider themselves an artist make something with it?

**Fails** until it happens. The first chance is the small-group stage
(`docs/SPEC.md` 10.5): each person is asked once, in person, before they start,
"Do you consider yourself an artist?", and the answers go here with each
person's permission. The gate passes the first time someone who answered no
makes something and keeps it.

---

## Results

None yet. Each result is added with its evidence.
