# Questions for the founder

Every **Open** decision, each with its recommendation (`docs/SPEC.md` 11.4,
with duplicates merged). If a stage reaches one before it has an answer, the
stage builds the recommendation as written and logs it in `docs/DECISIONS.md`,
so no stage waits.

Questions that came up while building are at the end, under "New".

## The whole app

| # | Decision | From | Recommendation |
|---|---|---|---|
| 1 | Hyphen or underscore: Wi-WWAV or Wi_WWAV | 1.5, 10.3 | Use the underscore for the app and the hyphen for the people in letters. Domains keep the hyphen either way, because hostnames can't contain underscores. |
| 2 | Which "v3" the sketch means | 1.5, 4.1 | Read "v3" as "the iPhone app", leave the archive's numbering alone, and give Wi_WWAV its own archive entry. |
| 3 | Copy or reference on import | 2 | Copy songs, films and anything under 2 GB, so the library and its export are complete. Offer "Leave in place" for camera folders and long raw video. |
| 4 | Using the app without an account | 2 | Yes. Heat, the library and the Console are fully local. Space and Unquantized can be viewed, but not travelled, published to or bought from. |
| 5 | Reminders, deadline alerts and Dock badges | 2, 3.18, 10.6 | Never notify or badge about what other people do. Allow one alarm you set yourself on a single task or focus session; it is off by default and fires once. |
| 6 | One sans or two | 8.4 | Lucida Grande (Lucida Sans Unicode on Windows) in the desk and Inter elsewhere; recheck on the Windows build. |
| 7 | Reading size: 17 or 19 pt | 8.9 | Test both with the founder at their own desk, and fix the number before any gate run. |
| 8 | Wi's adjusted stem hues | 8.12 | When the wall becomes the web face, move it to PRANA's exact hexes with ink rings. |

## Heat

| # | Decision | From | Recommendation |
|---|---|---|---|
| 9 | The streak counter | 3.9, 10.6 | By default, a record that only grows ("Done 41 days since August 26"). The counter becomes a per-habit setting, off by default, and the log is no longer pruned at 400 days. |
| 10 | The focus timer's chime | 3.5, 8.7 | Off by default, with the switch beside Start so the choice comes with the press that starts the timer. |
| 11 | Gmail's restricted scope and Google verification | 3.10, 9.8, 10.5 | Stay in testing mode (sign in again every 7 days, 100 test users at most) through the small-group stage. Get verified before the whole school. |
| 12 | Valence or LTI 1.3: when to ask URI | 3.11, 10.5 | Ship on the iCal feed and Gmail. Ask URI's Brightspace admins after the small-group stage, with the group's weekly reviews as evidence. |
| 13 | Control size for dense rows | 3.18, 10.6 | Keep 44 pt for buttons, tabs and orbs. Before testing, write a separate rule for rows and grids: 24 × 24 pt hit areas (WCAG 2.2) and every action on the keyboard. |
| 14 | The "Now making" line | 3.14 | Build it, off until used, and clear it when its task is done or after 7 days. Drop it if it starts to feel like a status to keep up. |
| 15 | Who Heat is for | 3.18 | One account first, with nothing hard-coded to a school, so a classmate can use it next. |
| 16 | Whether grade rows sync | 9.8 | Off by default. When on, encrypt them on the Mac with a Keychain key, so the server holds only ciphertext. |

## Space

| # | Decision | From | Recommendation |
|---|---|---|---|
| 17 | Sky motion while nothing plays | 4.4, 8.6 | Still by default, with "Let the sky turn when it's quiet" in Appearance, off. |
| 18 | Which philosophy governs social features (likes, follows, counts, comments, notifications, popularity) | 4.11, 10.6 | Ship the defaults that pass the gates: Add, Add galaxy, replies as works, pull-only, newest first. If one returns, make it a single "LMY added your galaxy" line, never a number. |
| 19 | The v5 game layer | 4.12, 10.6 | Keep the map and the chance weather (wormholes, random comets, black holes, drift, terraforming). Drop everything paid for with attention. A sun flares once when its system fills its 21st seat. |
| 20 | What plays free | 4.13, 7.6 | The whole work plays in Space and in the shop, and what's sold is the file. A seller may choose a 30-second listen, started by a press. |
| 21 | Unpublishing a work others have forked | 4.13 | It leaves the sky. Earlier forks keep playing its stems, and their trees read "withdrawn by its maker". |
| 22 | "Near you" | 4.10 | After v1, as an opt-in Newest filter on the existing local feed. |
| 23 | Writing's file format | 4.7, 6.11 | Markdown with `wmet` and `wlin` keys in its front matter (`ri: "0.1"`), until a page must carry its images in one file. `.rwav` stays a placeholder. |
| 24 | A fashion reply, and Gi's pattern file | 4.7, 6.11 | For now, a gallery planet with a "styled from" link that waits for consent. Name the pattern file (placeholder `.gwav`) when a Gi_cro_WWAV prototype cuts its first piece. |
| 25 | Younger students | 4.14, 10.7 | No accounts under 13. A teacher plays the class system to the room from their own account. |
| 26 | Challenges (`wwav-2026` is live) | 10.6 | Keep one entry and one vote each, never show vote totals, and name the winner in a letter. |

## Console and files

| # | Decision | From | Recommendation |
|---|---|---|---|
| 27 | A clip-launch view | 5.2 | Leave it out of v1. Decide once one song has been finished in the Console from start to end. |
| 28 | Metronome on for new sessions | 5.4 | Off. A free-time recording finds its tempo afterwards with **Follow what I played**. |
| 29 | One process per plugin | 5.7, 9.3 | Not in v1. Log a month of engine crashes per plugin, then decide. |
| 30 | What a local split costs | 5.10, 10.4 | Free and unmetered. The paid cloud split stays for phones and the web. |
| 31 | PRANA as a USB MIDI controller | 5.6 | Specify it after Beta 1, mapping its four faders onto the four roles. |
| 32 | A PRANA view in the app | 5.15 | After v1, built natively on `prana/core`, so the preview is the device and not a likeness of it. |
| 33 | Versions and identity | 5.17, 6.8 | Keep versions on the server under one `song_id`. Add an optional `version` to `wmet` and a `parent_version` to `wlin` in 0.2. |
| 34 | A 48 kHz or 24-bit `.wwav` | 5.13, 6.7 | Keep 44.1 kHz, 16-bit for all of 0.x and convert on export, saying so. Decide both changes together for 1.0, once PRANA's hardware is measured. |
| 35 | Thin remixes | 6.9 | Thick files wherever a file leaves. Thin storage inside the library and R2, keyed by sha256. |
| 36 | Film stems for Si_WWAV | 6.10 | Build them after one film has been cut in the Console and someone has asked to take a film apart. |
| 37 | Selling one version on its own | 6.12 | No. A purchase covers every version, and a new work gets a new id. |

## Unquantized

| # | Decision | From | Recommendation |
|---|---|---|---|
| 38 | Ambiance | 7.2, 10.6 | The door press starts room sound and motion together, and one switch stops both. |
| 39 | Infinite or finite | 7.4, 10.6 | Finite and continuous. Every hall ends at "That's everything.", and the daily drift gives the sense of endlessness. |
| 40 | Strangers in the store | 7.5, 10.6 | Makers at their booths, as portraits, and your own party. Never strangers, and never a count. |
| 41 | Selling cosmetics | 7.5 | Don't. A cosmetics market invites people to compare each other. |
| 42 | Arbor Vitae | 7.9 | A Gi booth that links out to its Shopify checkout until its stock moves onto `FashionListing`. |
| 43 | Drop progress | 7.10, 10.6 | Show "Funded", or "Not funded yet · ends Feb 1", and never a pledge count or a bar. |
| 44 | Minimum price | 7.11 | $2 for paid digital items, because Stripe's 30¢ is more than the 10% fee at $1. Stems come inside the `.wwav` price. |
| 45 | Royalties to ancestors | 7.14, 4.9 | Each listing sets remix rights. A remix license carries a lineage share of 0–30% (default 10%) of the remixer's 90%, paid one level up, with shares under $1 carried forward. Songs and films first. |
| 46 | Paid reach | 7.15, 10.6 | Sell none. The window stays chance, with equal odds per shop. |
| 47 | House music | 8.7 | Use the founder's `site-music` layers under the room-sound switch, stopping when a record plays. Drop them if they compete with the records. |

## Engineering and shipping

| # | Decision | From | Recommendation |
|---|---|---|---|
| 48 | `allow-unsigned-executable-memory` for copy-protected plugins | 9.9 | Add it only if a plugin in the test set fails without it. |
| 49 | Intel-only plugins | 9.9 | Offer "Open the engine under Rosetta" per session, as a bridge. |
| 50 | Minimum macOS | 9.9 | macOS 13. |
| 51 | A self-hosted Mac mini CI runner | 9.11 | Add one once macOS runner minutes pass about $50 a month. |
| 52 | The `formats/` submodule | 9.12 | A sparse checkout of Mi-WWAV now. Split the four folders out only if clone times start to hurt. |

## Business and school

| # | Decision | From | Recommendation |
|---|---|---|---|
| 53 | Kickstarter or the store's own preorders | 10.2 | Kickstarter for roadmap step 4, and the plinth for every pledge after the campaign closes. |
| 54 | One desktop price list | 10.4 | A free plan (all local features, 10 GB hosted, 10 cloud splits); Pro at $7.99 a month or $69.99 a year; Founding at $199.99 once, seats 1–500; a split pack at $9.99 for 50; one `tier` entitlement everywhere. |
| 55 | Expiring split packs | 10.4 | Packs keep until used. |
| 56 | Commissions for every maker | 10.4 | After v1, the same commission card with each maker's own price sheet, at the 10% fee. |
| 57 | The device's price | 10.4 | Write a price ceiling into the campaign plan before step 4, and announce it in a letter to Wi-WWAV. |
| 58 | Money from the school | 10.5 | No-equity grants and competitions, leading with gate 4. Read the university's IP policy for student work first. |
| 59 | A fail criterion for gate 1.4 | 10.6 | Adopt it: the gate fails if, 12 months after the store's first payment, the store fee, Pro, Founding and packs don't cover server running costs, or if any line depends on a count of attention. |


## New

Questions raised while building. Each says what was built meanwhile.

| # | Question | Built meanwhile |
|---|---|---|
| 60 | This repo is public and `formats/` is a submodule of the private Mi-WWAV repo. CI on GitHub needs a token to check it out. Add a read-only deploy key or a fine-grained token as the secret `MI_WWAV_TOKEN`? | CI checks the submodule out with `MI_WWAV_TOKEN` when the secret exists, and skips the parity and golden jobs (marked open, never passed) when it doesn't. |
| 61 | The new server routes (`/api/entitlements`, `/api/upload/parts`, `/api/heat/changes`, `/api/assist/:task`, `/api/store/*`, desktop sign-in, `/desktop/latest.json`) and the 7.20 fixes belong in Mi-WWAV's `server/`. Should they land there through a pull request on Mi-WWAV? | The desktop talks to them through `crates/wi-core`'s clients, tested against a mock server in `tools/mock-server` that follows the routes as specified. |
