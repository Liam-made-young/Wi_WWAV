# Plan

Milestones in build order (`docs/SPEC.md` 10.1). Each milestone's fail
criteria were written here before its code. A milestone is **passed** only
with evidence (a test name, a log, a measurement); otherwise it is **open**.
Checks that need a Mac or the founder say so and stay open until one is run
there (`docs/SPEC.md` 10.2).

Where each check runs:

- **Linux**: CI or an agent alone.
- **Mac**: needs macOS hardware (AU, CoreAudio, VideoToolbox, signing).
- **Founder**: only the founder can judge.

---

## Work in progress (resume here)

Rules for this work, from the founder (7 Oct 2026): the main agent plus at most two Sonnet subagents at once while coding.

**Dates** (Decided, `docs/SPEC.md` 10.1): Learn 100% by the end of 7 Oct 2026; the app 80–90% by Sunday 11 Oct, which counts everything an agent can build and check on Linux; a working version by January 2027.

The spec rewrite is done: `docs/SPEC.md`, with the decisions in `docs/SCOPE_CUT.md`. Edit the spec directly from now on.

**Where 7 Oct ended** (the founder asked to wrap up; everything below is merged and pushed on `claude/relaxed-cori-x2igz9`):

- [x] A. Learn's maths in Rust (`crates/wi-heat/src/model/`): 242 tests mirroring the TS ones, and 59,067 generated vectors where Rust and TS agree byte for byte (S2.1)
- [x] B. The shell branches merged and brought to the spec: three views on ⌘1–⌘3, the galaxy chip, no Unquantized, first launch and Settings → Account per 2.13–2.14, the review findings fixed
- [x] C. Learn's store and commands: `crates/wi-heat-store` and every `heat.*` command in wi-core, the journal's `actor`/`tool`/`reason`, the 500 ms watcher, calendars with addresses in the Keychain, Learn sync of public copies, export (`heat.json` and Obsidian markdown), and Learn running with no audio engine (S2.2–S2.6, S2.9–S2.11 through the commands: 43 core tests)
- [x] D. `wi-mcp`: the eight tools over stdio; 8.12's suite passes (S2.8). Ten more since (S2.14)
- [x] E, part 1. Learn's screens: the frame, Today (time column, Plan my day, Pomodoro, the five widgets), Tasks, Calendar, Grades, Habits, Mail, Get Info with the Public switch, capture and inbox triage
- [x] The frames of Space (the sky in three.js, a sample catalogue marked as one) and the Console (its regions per 5.2, the real library, the engine's state)
- [x] An end-to-end check through the real core: Claude's helper adds a task from its own process while the app runs, it shows as Claude's, and ⌘Z takes it back (`app/ui/e2e/heat/claude.spec.ts`)
- [ ] E, part 2. Not built yet in the UI, though every command behind them exists: Settings → Learn (the School sheet and calendars), Settings → Claude (the config lines, the switches, recent changes) and Settings → Privacy; dropping a task on "Your galaxy" and the public Learn view page; the space sheet; notes; the weekly review; hiding widgets from the View menu
- [ ] Mac checks: the Tauri build itself has not been run on a Mac or under Xvfb with today's code (the container's disk couldn't hold it), nor the WebKit pass

**Green on the last merge (520fe28):** `cargo clippy --workspace --all-targets -D warnings` clean; wi-core, wi-store, wi-heat-store and wi-mcp tests all pass (97 in wi-core); `tsc` clean; Vitest 703 pass; the Claude end-to-end check passes. The agents' own runs: `cargo test --workspace` 900 pass, mock server 128, Playwright 29 (shell) and 30 (Learn on the fake core).

**Then, to the weekend:** Console audio, VST effects and MIDI → Space in three.js → Console video and `.swav` → Demucs.

---

## Foundation (the format spine and the shared parts)

Built first because every view reads and writes `.wwav` and `.swav`.

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| F1 | `.wwav` 0.1 in Rust (`crates/wwav-formats`) | An original packed by the Rust writer differs by any byte from `wwav_pack.py pack` of the same folder and `song.txt`; Rust `unpack` then `pack` changes a byte; the Rust verdict differs by any character from `wwav_pack.py info`'s on any file in `tests/corpus/` | Linux | open: packing matches `wwav_pack.py` byte for byte on 14 folders (`tests/wwav_pack.rs`) and the verdicts match on all 49 corpus files; unpack then pack fails only where `wwav_pack.py` fails its own round trip (a bpm of 127.9988; a bpm of 20.001 written as `20.00` and then dropped; folder titles with spaces), and those three tests are ignored with the question (`tests/review_findings.rs`) |
| F2 | `.swav` 0.1 in Rust | The Rust packer's bytes differ from `swav_pack.py pack`'s for the same film and `film.txt`; unpacking doesn't return the MP4 byte for byte; `ffprobe -v error` prints anything for a packed file | Linux | **passed**: `crates/wwav-formats/tests/swav_pack.rs` (plain, fast-start, size-0 last box, 64-bit boxes); unpack byte for byte; ffprobe silent |
| F3 | One verdict | Any of the four readers (Python `wwav_pack.py`, PRANA's C++ `core/disc/wwav.cpp`, Wi's `wwav.js`, the app's Rust) gives a different sentence for any file in `tests/corpus/` | Linux | **failed**: the Rust agrees with `wwav_pack.py` on all 49 corpus files, but the reference readers disagree among themselves on 5: Python and PRANA on `wmet-almost-json` and `dup-wmet-newer`, Python and Wi's JS on `wmet-nan`, `version-number` and `version-digits` (`tools/parity/check.py --allow-known`; questions #62, #76) |
| F4 | Ids | ULIDs don't sort by creation time as strings; the FNV-1a 128 `song_id` for a track id differs from `prana/web/src/sim/wwavdisc.js`'s; a stable hash differs between two runs or two builds | Linux | **passed**: `crates/wwav-ids` (ULID order over 20,000 ids; FNV-1a 128 against `wwavdisc.js`; FNV vectors) |
| F5 | Undo journal | After a random run of edits, undo-all doesn't return the first state byte for byte, or redo-all the last; ⌘Z acts outside the current view; a label is lost on relaunch; any table has `ON DELETE CASCADE` | Linux | **passed**: `crates/wi-store/tests/journal.rs` (12 tests, random edit runs undone and redone byte for byte) and `tests/review.rs` (21) |
| F6 | `.wwavsession` | Saving the same session twice gives different bytes; save, quit and reopen changes `session.json` by a byte or changes ⌘Z's label; killing the process mid-edit loses a journalled change; a crash mid-write leaves a half-written `session.json` | Linux | **passed**: `crates/wwav-session/tests/f6.rs` (11), `crash.rs` (3, killed mid-edit and mid-write), `package.rs` (20), `review.rs` (16; one ignored by decision: `session.json`'s numbers are I-JSON, `docs/DECISIONS.md` 2026-10-07) |
| F7 | Tokens | A token is defined anywhere but `design/tokens.json`; the CSS, Rust and C++ outputs disagree on any value; any pair in `docs/SPEC.md` 7.11 marked as text misses its ratio (7:1 body, 4.5:1 secondary) | Linux | open: the compiler, parity and contrast tests pass (`crates/wwav-tokens/tests/parity.rs`, `contrast.rs`, `key_color.rs`); two contrast findings and one question wait on the founder (`tests/review.rs`, ignored with their reasons: desk ink on dark case metal is 5.6:1; night ink at 50% on `night.clay` is 4.46:1; F major's glow on v4's key-tinted sky is 3.90:1) |
| F8 | The wire | A command or event doesn't round-trip through the length-prefixed JSON envelope; the shared-memory layout has a different offset for any field in the C++ header and the Rust mirror; a reader ever sees a torn clock under a writer at full rate | Linux | **passed**: `crates/wwav-wire` (39 unit tests, `client.rs` 14, `process.rs` 5, `shm_stress.rs` 2, `review_findings.rs` 3) |
| F9 | Export maths | The 48 → 44.1 kHz resampler's passband ripple exceeds ±0.1 dB to 20 kHz or its image rejection is under 90 dB; dither is applied more than once; the fold check passes a pair whose difference is over −80 dBFS; frame n's first sample isn't ⌊n × rate ÷ fps⌋; a session over 81 minutes isn't refused with the spec's sentence | Linux | **passed**: `crates/wwav-dsp/tests/resampler.rs`, `dither.rs`, `fold.rs`, `film.rs`, `sheet.rs`, `review_findings.rs` |

## Stage 0: the thin slice

Done only when all seven pass on a second Mac that has never built the app
(`docs/SPEC.md` 10.3). Target: the Linux half by 11 Oct, all seven by January 2027.

| # | Item | Fails if | Runs on | Status |
|---|---|---|---|---|
| S0.1 | The app opens | A cold launch on the reference machine takes over 1.5 s to show the window, or ⌘1–⌘3 doesn't switch between three empty views | Linux (switching, in Playwright and the Linux build); Mac (the 1.5 s on the M1 Air) | open |
| S0.2 | The engine starts | `wwav-engine` shows a Dock icon; the app waits on the engine to draw; after `kill -9` during playback the engine isn't back within 2 s with the transport stopped at the same playhead | Linux (restart and playhead, with the real engine on a dummy device); Mac (Dock icon) | open |
| S0.3 | One `.wwav` plays its four stems | The app's verdict differs from `wwav_pack.py info`; a click on a stem light isn't heard within 10 ms; a second click within 250 ms doesn't revert the mute and solo instead | Linux (verdict; mute reaches the audio within one block plus one socket hop, measured in the engine); Mac (heard at the speaker) | open |
| S0.4 | One third-party effect, with its window | A VST3 or AU effect from the founder's plugin folder doesn't load; its window doesn't open as the engine's own; its state isn't restored after a restart; a key it doesn't use (Space, ⌘Z) fails to reach the app | Linux (a VST3 test effect: load, state restore across a restart); Mac and Founder (AU, the founder's own plugins, key routing) | open |
| S0.5 | One synced video lane | The flash-and-click test film shows a flash more than one frame from its click, or drops frames at 1080p | Linux (frame-indexed maths, FFmpeg software decode); Mac (VideoToolbox, the presenter) | open |
| S0.6 | `.swav` export | ffprobe reports an error; `swav_pack.py info` can't read its `wmet` and `wlin`; unpacking doesn't return the encoded MP4 byte for byte; the exported flash and click land on different frames | Linux (FFmpeg encode path); Mac (VideoToolbox path) | open |
| S0.7 | Signed and notarized, on a second Mac | Downloaded through a browser onto a Mac that never built it, the app draws a Gatekeeper warning; `spctl --assess` or `codesign --verify --deep --strict` fails; the engine refuses S0.4's plugin | Mac and Founder (Developer ID) | open |

## Stage 1: shell and library

Target: 7 Oct 2026, with Learn. Partial work is on `build/core`, `build/tauri` and `build/shell` (item B above).

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S1.1 | The window | The title bar isn't 52 pt with the view switcher, Now strip, search pill and galaxy chip; below 1180 pt the pill isn't a 28 pt magnifier and the strip isn't 440 pt; anything overflows at 1024 × 680 | Linux | open |
| S1.2 | Views keep their place | Switching away and back loses a view's scroll, selection, open sheet or half-typed text; a view change isn't a 140 ms cross-fade (a cut under Reduce Motion) | Linux | open |
| S1.3 | The Now strip | Either half shows something other than 2.2's rules; an empty half doesn't read "All clear / Nothing open right now." or "Nothing playing / Select anything and press Space."; stem state is shown by colour alone | Linux | open |
| S1.4 | Stem lights | A click doesn't mute at once; a second click within 250 ms doesn't revert the mute and solo instead; any light's hit area is under 44 × 44 pt | Linux | open: the model passes (`app/ui/src/shared/stems/gesture.test.ts`); the strip itself is on `build/shell` |
| S1.5 | One grammar | ⌘Z isn't labelled in the menu and a 2.6 s toast; Esc discards typed text; a screen has more than one ⇧Return act; work that left the machine is offered as undoable | Linux | open |
| S1.6 | ⌘K, ⌘⇧N, ⌘L | The palette doesn't search tasks, clips, sessions, settings and actions with the filters in 2.7; capture doesn't keep the panel open on Enter with "N in inbox · captured ✓"; the drawer isn't 280 pt over any view | Linux | open |
| S1.7 | The library | Import names a file anything but a ULID; renaming a song renames a file; a plain WAV comes in as anything but master only, without saying so; a tag isn't lowercase or a clip takes a 13th; a fifth pin fits; a delete removes a file; the verdict in Get Info differs from `wwav_pack.py info` | Linux | open |
| S1.8 | Search | Library search takes over 50 ms at 50,000 clips | Linux | **passed**: `crates/wi-store/tests/search_speed.rs` (12–13.5 ms at 50,000 clips) |
| S1.9 | The upload queue | The queue is anything but `published_at IS NOT NULL AND remote_id IS NULL`; a resumed upload re-sends a finished part; a retry posts twice | Linux (mock server) | open |
| S1.10 | Export everything | The export misses any file, or any file's sha256 differs; it needs a sign-in; `index.html` opened offline doesn't play every song apart and every film, or show every Learn record | Linux | open |
| S1.11 | Sign-in | The app sees a password; the PKCE verifier or state isn't checked; the token lands anywhere but the Keychain (the OS store) | Linux (mock server, Secret Service); Mac (Keychain) | open |

## Stage 2: Learn, with the MCP server

Target: 100% by the end of 7 Oct 2026 (Decided). Learn's maths lives in the Rust core (Decided); the TS model in `app/ui/src/heat/model` is the reference it is ported from.

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S2.1 | Learn's maths, in Rust | The heat value, level or "Warm at / Hot at" table differs from 3.1 for any difficulty; the estimate chain or weekly load differs; any grade, letter or "what it would take" result differs from 3.1 and 3.8; for any generated input, the Rust result differs from the TS model's | Linux | **passed**: `crates/wi-heat/tests/model_*.rs` (242 tests, 240 pass, 2 ignored for TS bugs, QUESTIONS #175 and #176) and `model_vectors.rs` (59,067 TS-generated cases over 115 functions, equal byte for byte) |
| S2.2 | Today | Plan my day uses anything but the written rule (heat order, estimate rounded up to 15 min, capped at 90, first gap that fits, before "Day ends at"); a draft lacks its reason; Return doesn't accept all; Esc doesn't clear them | Linux | open: the rule passes in TS (`plan.test.ts`) |
| S2.3 | Focus | A round or break starts without a press; minutes don't add to the current task's `actualMin`; the timer stops when switching views; the chime sounds while off | Linux | open: the state machine passes in TS (`focus.test.ts`) |
| S2.4 | Tasks, Calendar, Grades, Habits, Mail | Any tab lacks its one "+" act and one secondary act from 3.3; a habit 7th fits; the streak counter shows by default; Mail can reply, send or delete; Mail shows anything but the threads Claude recorded | Linux | open |
| S2.5 | Calendars by iCal | Against a stored raw feed, a due item is missed, a non-graded or cancelled item is kept, a UID changes, or an item missing from two syncs is deleted; an iCal address is logged or stored outside the Keychain | Linux | open: the parser and rules pass against a synthesised D2L feed (`crates/wi-heat/tests/brightspace_feed.rs`); the founder's real feed is not stored yet |
| S2.6 | Moving in | Importing the artifact's JSON changes any id, so the first sync finds anything new | Linux | open: passes in TS (`importArtifact.test.ts`) |
| S2.7 | Learn sync | A slower older write overwrites a newer one on any field | Linux (mock server) | **passed**: `crates/wi-heat/tests/heat_sync.rs` (512 proptest cases, three devices, clocks ±10 min off) and `tools/mock-server/test/heat.test.js` |
| S2.8 | The MCP server | Any of the eight tools in 3.13 is missing or takes other arguments; a write tool adds anything but exactly one journal entry with `actor` `claude`, its fixed label and its result's `undo_label`; undoing it doesn't return the library byte for byte; a read or `plan_day` adds an entry; a repeated `source_id` or `thread_id` makes a second row; a tool switched off is listed or answers; any argument sets `done`, a score or `public`; of 200 kills at random moments, a call leaves its change without its entry or the reverse; with the app running, 1,000 interleaved writes lose one | Linux; Mac (Claude Desktop starting the helper from the bundle) | **passed** on Linux: `crates/wi-mcp/tests/stdio.rs` (every tool over real stdio; one entry per write by Claude with its label and undo returning the library byte for byte; reads and plan_day journal nothing; repeats make no second row; a switched-off tool is missing and refused; 200 SIGKILLs mid-call; 1,000 interleaved writes from two processes) and `tests/protocol.rs` (11); the Mac half (Claude Desktop starting the helper) is open |
| S2.9 | Public and private | A private record, grade or note appears in `/api/heat/public/:userId` or the preview; a public record shows more than the fields 3.15 lists; switching it back doesn't remove its copy at the next sync; a Now making line outlives its `clearsAt`; any response carries a count or a total; a grade's switch doesn't say what it does | Linux (mock server) | open |
| S2.10 | Capture, notes and the weekly review | Capture loses a line on Enter; a note can't be switched public on its own; the weekly review shows a score, a streak or a comparison; it writes the note for you | Linux | open |
| S2.11 | Settings → Claude and Privacy | Settings → Claude doesn't show the exact lines for Claude Desktop and Claude Code, with a switch per tool; Settings → Privacy doesn't list every public item with its switch | Linux | open |
| S2.12 | Learn used for a week | The founder stops opening it | Founder | open |
| S2.14 | Ten more tools for Claude, under the same rule | A new tool can mark anything done, tick a habit, triage a capture, accept a draft, set a score or a Public switch, or edit or delete what the person made; a read or `draft_block` adds a journal entry or a record; a record Claude adds lacks `source: "claude"` or its reason, or makes more than one entry; undoing it doesn't return the library byte for byte | Linux; Mac | **passed** on the Mac, 7 Oct 2026: `crates/wi-mcp/tests/stdio.rs` (`the_reads_change_nothing_and_the_drafts_are_marked_as_claudes`, and the four new writes in `every_tool_answers_and_each_write_is_one_entry_by_claude`) and `tests/protocol.rs`. Open: the views don't show whose a project, milestone, note or capture is yet, and Settings → Claude's pane isn't built |
| S2.15 | Calendars and mail, set up from the window | Settings → Learn can't save the School sheet, the Brightspace link or another calendar's iCal address through the core; a saved address is shown again, or is anywhere but the Keychain; Settings → Claude doesn't show the core's own lines, a switch per tool and Claude's changes with Undo; the school mail prompt names a tool that doesn't exist, asks for a score, a guessed due date or a message body | Linux; Mac | **passed** on the Mac, 7 Oct 2026, against the fake core and the real one: `app/ui/src/shell/SchoolSheet.test.tsx`, `ClaudePane.test.tsx`, `crates/wi-core/tests/core/heat_calendars.rs` (the sheet in the snapshot), `crates/wi-mcp/tests/protocol.rs` (the prompt). Open: nobody has pasted a real Brightspace link or run the prompt on real mail yet; spaces in Settings; the WebKit pass |
| S2.13 | Heat becomes Learn in the code | Anything a person or Claude reads still calls the view Heat; after the code rename, a library made before it doesn't open with every record, or a device on the old names can't sync with one on the new | Linux; Mac | open. The founder renamed the view on 7 Oct 2026: Learn, in full WWL (Wi-WWAV-Learn). **Done:** every visible name (the switcher, menus, Settings, sentences, the MCP tools' descriptions, the docs). A task's score stays its heat (Hot, Warm, heat order, `is:hot`). **Not done, on purpose, until the parallel build branches have merged:** the code's names (`heat.*` commands and the `heat` event, `Room::Heat` and its stored `heat`, `crates/wi-heat` and `wi-heat-store`, `app/ui/src/heat`, `docs/HEAT.md`, `/api/heat/*`). That rename needs a migration for stored rooms and a server that answers both paths for a while |

## Stage 3: Console, audio

Target: 80–90% by 11 Oct 2026.

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S3.1 | Golden renders | A session in `tests/sessions/` renders to a different SHA-256 on any run, on arm64 or x86_64 | Linux; macOS nightly | open |
| S3.2 | Plugin hosting | A VST3 test instrument doesn't load, take parameters, restore its state and render; its render doesn't null against the last below −96 dBFS; its editor doesn't open and close | Linux (VST3); Mac (AU) | open |
| S3.3 | Delay compensation | `delay-n` summed with a dry copy doesn't null | Linux | open |
| S3.4 | Kill and restart | Of 200 kills at random moments in playback, recording and render (and `crasher`, `hanger`): any takes over 2 s, changes the session hash, loses a take before its last block, or leaves the transport running | Linux, Mac | open |
| S3.5 | Quantize, MIDI only | Recording moves a note; quantize loses `played_at`; ⌥Q doesn't restore it; quantize or any tempo change moves or stretches an audio clip | Linux | open |
| S3.6 | The fold rule | A stem isn't the sum of its role's tracks post-fader; the four stems don't sum to the master before the master chain within −80 dBFS; a clipping stem isn't named | Linux | open |
| S3.7 | `.wwav` export | Fails F1's parity; `type`, `parent_id`, `root_id` and `generation` break 5.13's rules; "As settings" writes a `wrmx` PRANA can't read, or "Baked" writes one at all | Linux | open |
| S3.8 | Built-in effects | Any of the six differs from PRANA's at the same amount (reverb, delay, distortion, tremolo, filter, the master limiter), or takes more than one amount | Linux | open |
| S3.9 | Takes | A pass makes more than one take; a take loses audio before its last block when the engine dies | Linux | open |
| S3.10 | A song finished in the Console | The founder can't finish one | Founder | open |

## Stage 4: Console, video

Target: 80–90% by 11 Oct 2026. Basic: cutting only, no effects (Decided).

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S4.1 | Cutting | Cut at playhead, trim, move or ripple delete changes any frame outside the edit, or isn't one labelled ⌘Z | Linux | open |
| S4.2 | On the engine's clock | The viewer shows a frame more than one frame from the audio's playhead; the flash-and-click film exports with any offset | Linux (maths, FFmpeg software path); Mac (VideoToolbox, the presenter) | open |
| S4.3 | Proxies | Footage over 1080p doesn't scrub on a proxy; export reads the proxy instead of the original | Linux | open |
| S4.4 | `.swav` export | Fails F2 or S0.6 | Linux; Mac | open |
| S4.5 | A 4K stream with cuts | It drops frames on the reference machine | Mac | open |

## Stage 5: Space

Target: 80–90% by 11 Oct 2026. Real 3D in three.js (Decided).

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S5.1 | The same sky everywhere | Positions hashed at t = 0 for one catalogue differ between WebKit and Chromium (and later Windows) | Linux | open: Node and Chromium give the same layout hash (`layoutHash.test.ts`, `e2e/space-layout.spec.ts`); WebKitGTK not yet run |
| S5.2 | A still sky | Anything moves while nothing plays (frame-diff over 5 s idle) | Linux | open |
| S5.3 | The planet player | Moon gestures miss 4.6's thresholds (250 ms, 8 pt); a moon's level isn't its distance; ↑ Push stops playback or owns audio | Linux | open: the thresholds pass in the model (`gesture.test.ts`) |
| S5.4 | Systems | A 22nd world is accepted, or the refusal isn't "A solar system holds 21 worlds. Start another one." | Linux (mock server) | open: the model and `tools/mock-server` refuse the 22nd world with the sentence (`space.test.js`, 30 concurrent adds) |
| S5.5 | Lineage | A family tree layout differs from v3's ring rule; a link touching someone else's work shows before they agree | Linux | open: the layout passes, bit-identical with a port of v3 over 11,000 families (`lineage.test.ts`) |
| S5.6 | Newest and Since you last looked | Any order but newest; no "That's everything."; any count | Linux | open: the model passes (`newest.test.ts`) |
| S5.7 | Open in Console | ⌘E on any song or film doesn't open it as a session whose lanes are its stems; it changes the work in Space | Linux | open |
| S5.8 | The public Learn view on the sun | Opening someone's sun shows anything S2.9 forbids, or a count | Linux (mock server) | open |
| S5.9 | Galleries from photos | Dropping photos on a system doesn't make a gallery planet of up to 40; a 41st is accepted; the app changes a photo's bytes | Linux (mock server) | open |
| S5.10 | 60 fps | Under 60 fps at 2560 × 1600 with 2,000 galaxies as instanced points | Mac | open |

## Stage 6: Demucs splitting

Target: 80–90% by 11 Oct 2026.

| # | Milestone | Fails if | Runs on | Status |
|---|---|---|---|---|
| S6.1 | Split into stems | ⌃⌘S runs in the audio engine instead of its own worker; progress shows anything but the model's real segments; the stem group isn't made in place with the source kept muted; Undo doesn't read "Undo split into stems" | Linux | open |
| S6.2 | Stems that sum | The four stems differ from htdemucs's own output for the same file; the export doesn't record `splitter: "demucs"` | Linux | open |
| S6.3 | The CPU fallback | A machine without a supported GPU starts a split without saying it will be slower | Linux | open |

## Stage 7: Windows

Not started until a Mac v1 exists (Decided).

## Budgets (`docs/SPEC.md` 8.14)

Reference machine: a 2020 M1 MacBook Air with 8 GB. Measured by hand at each
release and written here beside the budget.

| What | Budget | Measured |
|---|---|---|
| Audio callback | 2.67 ms at 128 frames, 48 kHz; DSP load ≤ 70% at p99.9 | — |
| Dropouts | none in an hour of the reference session | — |
| Click to sound | 10 ms at most | — |
| UI | 60 fps; 120 fps on ProMotion for timeline scrolling and the Space camera | — |
| Space | 60 fps at 2560 × 1600 with up to 2,000 galaxies as instanced points | — |
| Video | a 4K stream with cuts, no dropped frames; over 1080p scrubs on proxies | — |
| Cold launch | Learn usable in 1.5 s; engine opens its device in 0.8 s alongside | — |
| Engine restart | 2 s plus plugin load | — |
| Offline render | 10× real time or faster for built-ins | — |
| Library search | 50 ms at 50,000 clips | 12–13.5 ms on the Linux CI box (not the reference machine) |
| Memory | app 300 MB idle; engine 150 MB empty | — |
