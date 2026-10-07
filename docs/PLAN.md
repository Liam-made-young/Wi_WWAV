# Plan

Milestones in build order (`docs/SPEC.md` 11.1). Each milestone's fail
criteria were written here before its code. A milestone is **passed** only
with evidence (a test name, a log, a measurement); otherwise it is **open**.
Checks that need a Mac or the founder say so and stay open until one is run
there (`docs/SPEC.md` 11.2).

Where each check runs:

- **Linux**: CI or an agent alone.
- **Mac**: needs macOS hardware (AU, CoreAudio, VideoToolbox, signing).
- **Founder**: only the founder can judge.

---

## Work in progress (resume here)

Rules for this work, from the founder (7 Oct 2026): the main agent plus at most one Sonnet subagent at a time while the spec is rewritten; at most two Sonnet subagents at once while coding.

**The scope cut of `docs/SPEC.md`.** The decisions are in `docs/SCOPE_CUT.md` (the brief). Chapters are rewritten one at a time into `docs/spec2/` (one file per chapter group), then assembled into `docs/SPEC.md`.

- [x] Brief written (`docs/SCOPE_CUT.md`)
- [x] 1–2: The idea; One app, three views (`docs/spec2/ch01-02.md`)
- [x] 3: Heat, the profile view (`ch03.md`)
- [x] 4: Space, the social view (`ch04.md`)
- [x] 5: Console, the creation view (`ch05.md`)
- [x] 6–7: Files; Look, sound and feel (`ch06-07.md`)
- [x] 8: Under the hood (`ch08.md`)
- [ ] 9–10: Business and gates; Roadmap, open decisions, glossary (`ch09-10.md`)
- [ ] Assemble: front matter, join, resolve `{{old:X.Y}}` references, contents table
- [ ] Review: every brief decision present, nothing cut left, references resolve
- [ ] Commit and push to `claude/relaxed-cori-x2igz9`
- [ ] Re-scope the milestones below to the new spec (drop Unquantized rows, add MCP and public-view rows)

**Then coding**, in this order: shell and library → Heat with the MCP server → Console audio, VST effects, MIDI → Space in three.js → Console video and `.swav` → Demucs. Partial work for the shell stage sits on branches `build/core`, `build/tauri` and `build/shell` (not merged; check against the new spec first).

---

## Foundation (the format spine and the shared parts)

Built first because every room reads and writes `.wwav` and `.swav`.

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| F1 | `.wwav` 0.1 in Rust (`crates/wwav-formats`) | An original packed by the Rust writer differs by any byte from `wwav_pack.py pack` of the same folder and `song.txt`; Rust `unpack` then `pack` changes a byte; the Rust verdict differs by any character from `wwav_pack.py info`'s on any file in `tests/corpus/` | Linux | **passed**: `crates/wwav-formats/tests/wwav_pack.rs` packs 14 folders with both tools, byte for byte; unpack-pack identity tested (originals with no creator or splitter, #68) |
| F2 | `.swav` 0.1 in Rust | The Rust packer's bytes differ from `swav_pack.py pack`'s for the same film and `film.txt`; unpacking doesn't return the MP4 byte for byte; `ffprobe -v error` prints anything for a packed file | Linux | **passed**: `crates/wwav-formats/tests/swav_pack.rs` (plain, fast-start, size-0 last box, 64-bit boxes); unpack byte for byte; ffprobe silent |
| F3 | One verdict | Any of the four readers (Python `wwav_pack.py`, PRANA's C++ `core/disc/wwav.cpp`, Wi's `wwav.js`, the app's Rust) gives a different sentence for any file in `tests/corpus/` | Linux | **failed**: the Rust agrees with `wwav_pack.py` on all 45 corpus files, but the reference readers disagree among themselves on 3 (`tools/parity/check.py`; questions #62, #76) |
| F4 | Ids | ULIDs don't sort by creation time as strings; the FNV-1a 128 `song_id` for a track id differs from `prana/web/src/sim/wwavdisc.js`'s; a stable hash differs between two runs or two builds | Linux | **passed**: `crates/wwav-ids` (ULID order over 20,000 ids; FNV-1a 128 against `wwavdisc.js`; FNV vectors) |
| F5 | Undo journal | After a random run of edits, undo-all doesn't return the first state byte for byte, or redo-all the last; ⌘Z acts outside the current room; a label is lost on relaunch; any table has `ON DELETE CASCADE` | Linux | open |
| F6 | `.wwavsession` | Saving the same session twice gives different bytes; save, quit and reopen changes `session.json` by a byte or changes ⌘Z's label; killing the process mid-edit loses a journalled change; a crash mid-write leaves a half-written `session.json` | Linux | open |
| F7 | Tokens | A token is defined anywhere but `design/tokens.json`; the CSS, Rust and C++ outputs disagree on any value; any pair in `docs/SPEC.md` 8.11 marked as text misses its ratio (7:1 body, 4.5:1 secondary) | Linux | open |
| F8 | The wire | A command or event doesn't round-trip through the length-prefixed JSON envelope; the shared-memory layout has a different offset for any field in the C++ header and the Rust mirror; a reader ever sees a torn clock under a writer at full rate | Linux | open |
| F9 | Export maths | The 48 → 44.1 kHz resampler's passband ripple exceeds ±0.1 dB to 20 kHz or its image rejection is under 90 dB; dither is applied more than once; the fold check passes a pair whose difference is over −80 dBFS; frame n's first sample isn't ⌊n × rate ÷ fps⌋; a session over 81 minutes isn't refused with the spec's sentence | Linux | open |

## Stage 0: the thin slice

Done only when all seven pass on a second Mac that has never built the app
(`docs/SPEC.md` 11.3).

| # | Item | Fails if | Runs on | Status |
|---|---|---|---|---|
| S0.1 | The app opens | A cold launch on the reference machine takes over 1.5 s to show the window, or ⌘1–⌘4 doesn't switch between four empty rooms | Linux (switching, in Playwright and the Linux build); Mac (the 1.5 s on the M1 Air) | open |
| S0.2 | The engine starts | `wwav-engine` shows a Dock icon; the app waits on the engine to draw; after `kill -9` during playback the engine isn't back within 2 s with the transport stopped at the same playhead | Linux (restart and playhead, with the real engine on a dummy device); Mac (Dock icon) | open |
| S0.3 | One `.wwav` plays its four stems | The app's verdict differs from `wwav_pack.py info`; a click on a stem light isn't heard within 10 ms; a second click within 250 ms doesn't revert the mute and solo instead | Linux (verdict; mute reaches the audio within one block plus one socket hop, measured in the engine); Mac (heard at the speaker) | open |
| S0.4 | One third-party effect, with its window | A VST3 or AU effect from the founder's plugin folder doesn't load; its window doesn't open as the engine's own; its state isn't restored after a restart; a key it doesn't use (Space, ⌘Z) fails to reach the app | Linux (a VST3 test effect: load, state restore across a restart); Mac and Founder (AU, the founder's own plugins, key routing) | open |
| S0.5 | One synced video lane | The flash-and-click test film shows a flash more than one frame from its click, or drops frames at 1080p | Mac | open |
| S0.6 | `.swav` export | ffprobe reports an error; `swav_pack.py info` can't read its `wmet` and `wlin`; unpacking doesn't return the encoded MP4 byte for byte; the exported flash and click land on different frames | Linux (FFmpeg encode path); Mac (VideoToolbox path) | open |
| S0.7 | Signed and notarized, on a second Mac | Downloaded through a browser onto a Mac that never built it, the app draws a Gatekeeper warning; `spctl --assess` or `codesign --verify --deep --strict` fails; the engine refuses S0.4's plugin | Mac and Founder (Developer ID) | open |

## Stage 1: shell and library

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S1.1 | The window | The title bar isn't 52 pt with switcher, Now strip, search pill and astronaut chip; below 1180 pt the pill isn't a 28 pt magnifier and the strip isn't 440 pt; anything overflows at 1024 × 680 | Linux | open |
| S1.2 | Rooms keep their place | Switching away and back loses a room's scroll, selection, open sheet or half-typed text; a room change isn't a 140 ms cross-fade (a cut under Reduce Motion) | Linux | open |
| S1.3 | The Now strip | Either half shows something other than 2.2's rules; an empty half doesn't read "All clear / Nothing open right now." or "Nothing playing / Select anything and press Space."; stem state is shown by colour alone | Linux | open |
| S1.4 | Stem lights | A click doesn't mute at once; a second click within 250 ms doesn't revert the mute and solo instead; any light's hit area is under 44 × 44 pt | Linux | open: the model passes (`app/ui/src/shared/stems/gesture.test.ts`); the strip itself is not built |
| S1.5 | One grammar | ⌘Z isn't labelled in the menu and a 2.6 s toast; Esc discards typed text; a screen has more than one ⇧Return act; work that left the machine is offered as undoable | Linux | open |
| S1.6 | ⌘K, ⌘⇧N, ⌘L | The palette doesn't search tasks, clips, sessions, settings and actions with the filters in 2.7; capture doesn't keep the panel open on Enter with "N in inbox · captured ✓"; the drawer isn't 280 pt over any room | Linux | open |
| S1.7 | The library | Import names a file anything but a ULID; renaming a song renames a file; a plain WAV comes in as anything but master only, without saying so; a tag isn't lowercase or a clip takes a 13th; a fifth pin fits; a delete removes a file; the verdict in Get Info differs from `wwav_pack.py info` | Linux | open |
| S1.8 | Search | Library search takes over 50 ms at 50,000 clips | Linux | open |
| S1.9 | The upload queue | The queue is anything but `published_at IS NOT NULL AND remote_id IS NULL`; a resumed upload re-sends a finished part; a retry posts twice | Linux (mock server) | open |
| S1.10 | Export everything | The export misses any file, or any file's sha256 differs; it needs a sign-in; `index.html` opened offline doesn't play every song apart and every film | Linux | open |
| S1.11 | Sign-in | The app sees a password; the PKCE verifier or state isn't checked; the token lands anywhere but the Keychain (the OS store) | Linux (mock server, Secret Service); Mac (Keychain) | open |

## Stage 2: Heat

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S2.1 | Heat's maths | The heat value, level or "Warm at / Hot at" table differs from 3.1 for any difficulty; the estimate chain or weekly load differs; any grade, letter or "what it would take" result differs from 3.1 and 3.8 | Linux | **passed** (model): `app/ui/src/heat/model/heat.test.ts`, `estimate.test.ts`, `grades.test.ts`, `review.findings.test.ts` (threshold sweeps) |
| S2.2 | Today | Plan my day uses anything but the written rule (heat order, estimate rounded up to 15 min, capped at 90, first gap that fits, before "Day ends at"); a draft lacks its reason; Return doesn't accept all; Esc doesn't clear them | Linux | open: the rule passes (`plan.test.ts`); Return and Esc in the room are not built |
| S2.3 | Focus | A round or break starts without a press; minutes don't add to the current task's `actualMin`; the timer stops when switching rooms; the chime sounds while off | Linux | open: the state machine passes (`focus.test.ts`); keeping it across rooms is shell work |
| S2.4 | Tasks, Calendar, Grades, Habits, Mail | Any tab lacks its one "+" act and one secondary act from 3.3; a habit 7th fits; the streak counter shows by default; Mail can reply, send or delete | Linux | open |
| S2.5 | Brightspace iCal | Against a stored raw feed, a due item is missed, a non-graded or cancelled item is kept, a UID changes, or an item missing from two syncs is deleted | Linux | open: the parser and rules pass against a synthesised D2L feed (`crates/wi-heat/tests/brightspace_feed.rs`); the founder's real feed is not stored yet |
| S2.6 | Moving in | Importing the artifact's JSON changes any id, so the first sync finds anything new | Linux | open |
| S2.7 | Heat sync | A slower older write overwrites a newer one on any field | Linux (mock server) | **passed**: `crates/wi-heat/tests/heat_sync.rs` (512 proptest cases, three devices, clocks ±10 min off) and `tools/mock-server/test/heat.test.js` |

## Stage 3: Console

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S3.1 | Golden renders | A session in `tests/sessions/` renders to a different SHA-256 on any run; a 44.1 kHz built-ins session misses PRANA's golden hashes | Linux; macOS nightly | open |
| S3.2 | Plugin hosting | A VST3 test instrument doesn't load, take parameters, restore its state and render; its render doesn't null against the last below −96 dBFS; its editor doesn't open and close | Linux (VST3); Mac (AU) | open |
| S3.3 | Delay compensation | `delay-n` summed with a dry copy doesn't null | Linux | open |
| S3.4 | Kill and restart | Of 200 kills at random moments in playback, recording and render (and `crasher`, `hanger`): any takes over 2 s, changes the session hash, loses a take before its last block, or leaves the transport running | Linux, Mac | open |
| S3.5 | Unquantized timing | Recording moves a note; quantize loses `played_at`; ⌥Q doesn't restore it | Linux | open |
| S3.6 | The fold rule | A stem isn't the sum of its role's tracks post-fader; the four stems don't sum to the master before the master chain within −80 dBFS; a clipping stem isn't named | Linux | open |
| S3.7 | `.wwav` export | Fails F1's parity, or `type`, `parent_id`, `root_id` and `generation` break 13's rules | Linux | open |
| S3.8 | Make a disc | A disc has anything but loose `NN Song.wwav` files at the root, or a song that fails PRANA's read rules is written | Linux | open |
| S3.9 | A song finished in the Console | The founder can't finish one | Founder | open |
| S3.10 | Video | Two graded 4K streams drop frames; the grade differs between preview and export | Mac | open |

## Stage 4: Space

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S4.1 | The same sky everywhere | Positions hashed at t = 0 for one catalogue differ between WebKit and Chromium (and later Windows) | Linux | open: Node and Chromium give the same layout hash (`layoutHash.test.ts`, `e2e/space-layout.spec.ts`); WebKit not yet run |
| S4.2 | A still sky | Anything moves while nothing plays (frame-diff over 5 s idle) | Linux | open |
| S4.3 | The planet player | Moon gestures miss 4.6's thresholds (250 ms, 400 ms, 8 pt); a moon's level isn't its distance; ↑ Push stops playback | Linux | open: the thresholds pass in the model (`gesture.test.ts`); the player is not built |
| S4.4 | Systems | A 22nd world is accepted, or the refusal isn't "A solar system holds 21 worlds. Start another one." | Linux (mock server) | open: the model and `tools/mock-server` refuse the 22nd world with the sentence (`space.test.js`, 30 concurrent adds); the room is not built |
| S4.5 | Lineage | A family tree layout differs from v3's ring rule; a link touching someone else's work shows before they agree | Linux | open: the layout passes, bit-identical with a port of v3 over 11,000 families (`lineage.test.ts`); consent display is room work |
| S4.6 | Newest and Since you last looked | Any order but newest; no "That's everything."; any count | Linux | open: the model passes (`newest.test.ts`); the room is not built |

## Stage 5: Unquantized

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S5.1 | The door | Sound or motion starts before the press; the speaker glyph doesn't stop both | Linux | open |
| S5.2 | The floor | A hall has no end wall reading "That's everything."; a shop's seat differs between two reads of the same day | Linux | open |
| S5.3 | List view | Anything in the store can't be reached and bought from List view; it isn't the default under Reduce Motion | Linux | open |
| S5.4 | The counter | Buying needs walking; a price adds a fee at checkout; a count of sales appears on the floor | Linux (mock server) | open: the mock server passes as a test double (`commerce.test.js`); the counter is not built |
| S5.5 | Before launch | Any item in `docs/SPEC.md` 7.20 is open | Linux (server patches, test Postgres) | open |

## Stage 6: Windows

Not started until a Mac v1 exists (Decided).

## Budgets (`docs/SPEC.md` 9.13)

Reference machine: a 2020 M1 MacBook Air with 8 GB. Measured by hand at each
release and written here beside the budget.

| What | Budget | Measured |
|---|---|---|
| Audio callback | 2.67 ms at 128 frames, 48 kHz; DSP load ≤ 70% at p99.9 | — |
| Dropouts | none in an hour of the reference session | — |
| Click to sound | 10 ms at most | — |
| UI | 60 fps; 120 fps on ProMotion for timeline scrolling and the Space camera | — |
| 3D rooms | 60 fps at 2560 × 1600 in a hall of 1,000 shops | — |
| Video | two graded 4K streams, no dropped frames | — |
| Cold launch | Heat usable in 1.5 s; engine opens its device in 0.8 s alongside | — |
| Engine restart | 2 s plus plugin load | — |
| Offline render | 10× real time or faster for built-ins | — |
| Library search | 50 ms at 50,000 clips | — |
| Memory | app 300 MB idle; engine 150 MB empty | — |
