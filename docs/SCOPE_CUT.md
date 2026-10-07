# Scope cut for docs/SPEC.md — the founder's decisions (7 Oct 2026)

These were settled in conversation with the founder today. In the rewritten
spec they carry the label **Decided**. Everything else keeps the label it has
in the current spec (Decided, Proposed, Open, Exists today).

## The shape

- The app has **three views**, not four rooms. The founder's words: "Learn ->
  profile view, Space -> social view, Console -> creation view".
  - **Learn** (⌘1) is the profile view.
  - **Space** (⌘2) is the social view.
  - **Console** (⌘3) is the creation view.
  - Keep the names Learn, Space and Console; describe each as the profile,
    social or creation view.
- **Unquantized is cut entirely.** "Eventually the space will hold commerce
  instead of having that be a 4th place." So commerce is **later**, inside
  Space: a short section in Space says so, and nothing about a walkable shop,
  booths, halls, the counter, the bag, Stripe Checkout, payouts, entitlements,
  fashion holds, the clerk or the back room stays in the spec. Connected
  selling and connected distribution go. The `$4 · on the shelf` tag goes.
- **This is not going out for a while** (the founder's words). Privacy
  defaults stay conservative, but nothing needs to be built for a public
  launch now.

## Learn: the profile view

- Keep Learn as the spec has it now (chapter 3: Today with the time column,
  Plan my day and the Pomodoro; Tasks; Calendar; Grades; Habits; Mail; spaces;
  capture, notes and the weekly review). It stays "supremely useful".
- **Privacy model (Decided):** "Learn is like a facebook typa thing. By default
  everything will be a private PKM, with a simple version appearing public, but
  I can toggle anything to be public, including grades." So:
  - Every record (task, project, milestone, habit, note, course, grade, focus
    record) is private by default.
  - A simple public version exists by default: what replaces 3.14's "Now
    making" line and project timeline is "a simple version appearing public" —
    keep those two as the default public items.
  - Any item can be toggled public, including grades. Grades are FERPA
    education records when a school holds them; here the student chooses to
    show their own, the default is private, and the toggle says so plainly.
- **Two profile views (Decided):** your private Learn (the profile view, ⌘1),
  and your **public Learn view**, which is what anyone sees when they open "the
  sun in the middle of your galaxy" in Space. The bio sun therefore opens your
  public Learn view (plus your bio blocks).
- **AI through MCP (Decided):** Wi_WWAV is an **MCP server**. Claude
  (claude.ai, Claude Desktop, Claude Code) connects to it and calls tools such
  as list_tasks, add_task, update_task (difficulty, estimate, reason),
  plan_day, get_grades, add_pending_grade, log_focus, record_mail_thread.
  Consequences to write through the spec:
  - The app holds no Anthropic key and calls no model; `/api/assist/:task`,
    the per-account daily limit and the in-app consent sheets go. Claude's own
    tool-permission prompts replace them. "Claude estimates and drafts, never
    decides" still holds: every Claude action is a visible, labelled, undoable
    tool call ("Undo Claude's estimate").
  - School mail is read by Claude through Claude's own Gmail connector, which
    then calls Wi_WWAV's tools; the app never asks for Gmail's restricted
    scope, so Google verification and testing-mode sign-ins drop out. The Mail
    tab shows the threads Claude recorded (record_mail_thread) with their
    state, and "Open in Gmail".
  - Calendars: the Brightspace iCal feed stays, and other calendars come in the
    same way, through their private iCal addresses (Google Calendar has one),
    so the app needs no Google sign-in at all.
  - A local MCP server (stdio, for Claude Desktop and Claude Code) first; a
    remote one through mi-wwav.com for claude.ai later, since it needs Learn
    sync.
  - Recommendation to record where it matters: MCP tools act on the local
    library through the same journal as the UI, so ⌘Z undoes them.

## Space: the social view

- **Real 3D is a necessity (Decided)**, with three.js in the web view (WebGPU
  where available, WebGL 2 otherwise). Keep 4.5's "text stays flat" DOM labels.
- **Keep:** the universe → galaxy → solar system → planet model, the 21-world
  cap, moving through it (zoom tiers, camera, the same sky on every machine,
  Kepler motion only while something plays), the planet player with **level,
  mute and solo** on the four moons (distance is volume; click mutes; second
  click within 250 ms reverts and solos), **↑ Push** (fork the mix: a few
  kilobytes of mix state, owning no audio), and a new **Open in Console**
  button (⌘E) on any work, which opens it in the Console as a session to "tear
  it apart completely".
- **Keep all four media as worlds:** film (the ringed planet, the screening
  room for watching), writing (the page planet, the reading room), fashion
  (the gallery planet: looking, spinning, stepping through photos). Viewing
  only.
- **Cut all editing in Space besides those stem controls (Decided):** "remixing
  is Mi-WWAV and Console, not Space." So cut: the FX moons and the 400 ms hold
  that blooms them, the tempo and pitch dials, reverse and beat-repeat, the
  remix deck (fx, eq, comp, time, rec), ＋ add a song (the figure-eight), the
  video synth, film grade forks and new-soundtrack forks in Space, quote-and-
  reply forks of writing, "styled from" fashion replies, the freeform sun
  layout editor, Export from the player.
- **Cut the extraneous (Decided):** constellations, universe drift (gravity
  drift and binary pairs), the whole game layer (fog of war, fuel, travel cost,
  wormholes, comets, supernovas, black holes, terraforming), the astronaut and
  the rocket (so the title bar's astronaut chip becomes a chip of your own
  planet or galaxy; Users.astronaut is simply not used), "Near you".
- **Keep the social model** that passes the gates: lineage as the only social
  graph (fork edges as facts, links as claims that wait for consent, the family
  tree view), Add (private save), Add galaxy (private), Newest, "Since you last
  looked", the message door, letters on a sun (read-only; the devlog), and the
  social-features-and-gates table, with what was cut removed. Publishing is a
  drop.
- School/class systems: move to "Later" (one line), not designed now.
- **Commerce, later (Decided):** one short section at the end of Space: works
  in Space will one day carry a price and be bought where they are; nothing of
  it is designed or built now; Unquantized's walkable shop is gone.

## Console: the creation view

- **Keep:** one timeline for audio and video; the Console's metal-and-DMG look;
  audio tracks; stem groups (any .wwav as four lanes); stem roles, the four
  stem buses and the fold rule; recording audio takes, one take per pass
  (**no comping**, no camera takes); **VST3 and AU plugin hosting** in the
  separate engine process with crash recovery; delay compensation; export as
  .wwav (with "As settings" or "Baked" for remixes), .swav and plain WAV;
  **local Demucs splitting**; versions and Push to Space; opening any file
  (including from Space's Open in Console) as a session to tear apart.
- **Effects (Decided):** third-party VST/AU effects **plus a few built-ins**:
  PRANA's reverb, delay, distortion, tremolo, filter and the master limiter, as
  PRANA has them (one amount each). No built-in EQ or compressor, no four-knob
  extensions.
- **MIDI (Decided):** keep MIDI and the piano roll, but **instruments are
  third-party VST/AU only** — no built-in Sampler, no custom instruments.
  **Quantize applies to MIDI only, not audio** (keep played_at so ⌥Q returns
  to played). Audio is never time-stretched to a grid: cut "Follow tempo" /
  "Follow what I played" and audio quantize. Keep the metronome (off by default).
- **Video (Decided): basic.** Video tracks with cutting clips only: cut at
  playhead, trim, move, ripple delete, slip and roll if cheap. **No automation,
  no effects:** no grade, no RGB curves, no varispeed, no titles, no
  generators/video synth, no transitions. Keep proxies for 4K and the viewer
  on the engine's clock.
- **Cut (Decided):** Make a disc and everything PRANA-disc ("PRANA disc not
  ready yet, so definitely not writing software yet"), device parity /
  "Matches PRANA" / PRANA golden-hash parity for sessions, the PRANA view, the
  gamepad mapping, the Planet mixer view (strips only), captions from lyrics,
  "Ask for feedback", freeze and flatten, MIDI learn and "Capture what I just
  played" (record it like any take instead), camera takes, the clip-launch
  question (closed: no), second songs / Match to session.

## Everywhere else

- Chapter 6 (files): keep .wwav and .swav byte-level description; the session
  package (minus video grade/titles); stem buses; sample rate; versions,
  remixes and forks; thin remixes; .swav 0.2's wgrd goes (no grade); words and
  garments stay Open (writing and fashion are kept); "Files in the store" goes
  (commerce is later).
- Chapter 8 (look): two registers, the desk (Learn, the Console's chrome) and
  the night (Space); the interior register goes; room sound and house music
  go; the astronaut goes.
- Chapter 9 (engineering): keep the two processes, the wire and clock, crash
  recovery, audio, storage, sync, security, shipping; video stays but basic
  (decode, proxies, a plain composite with no grade, present, encode); add the
  MCP server; remove commerce endpoints, Google OAuth and Gmail scope work, the
  assist endpoint, Unquantized budgets. Keep "Exists today" references that
  still apply.
- Chapter 10 (business): commerce is later, inside Space, so the store fee is
  not a revenue line yet; what remains is Pro (hosting, cloud splits),
  Founding and split packs; say plainly that gate 1.4 has no route until
  commerce returns. Keep the gates table with rows for cut features removed.
- Chapter 11 (roadmap): a new build order for three views; drop Unquantized,
  Make a disc and video-effects stages; keep Windows last; keep the thin slice
  (its seven items still apply: video lane and .swav export stay); prune open
  decisions to those still relevant and renumber; prune the glossary.
- Keep the document's voice exactly: plain, short sentences, present tense, no
  exclamation marks, real numbers, every omission with its reason. Every
  chapter keeps a "Left out, and why" table, now including what this cut
  removed, each with its one-line reason (usually: "cut on 7 Oct 2026 to
  narrow v1", or the founder's own words).

## Later on 7 Oct 2026

The founder settled six more things. They carry the label **Decided** in the spec.

- **No commerce until the founder says so.** "commerce won't exist until I say it does." The app takes no money: no Pro, no Founding seats, no split packs, no store fee, no `tier` entitlement, and no plan bought in the browser. Pro, Founding and split packs were the iPhone app's plans; they stay there, and the desktop app neither sells nor reads them. Gate 1.4 stays open with no route.
- **Galleries are made by uploading photos.** "lets just have people upload images for fashion gallery." Drop photos on a system and they become a gallery planet. An image editor "somewhere between photoshop and kidpics" comes after v1, "not yet".
- **Plain export is WAV only for now.** "I really want .wwav and .swav to work but keep em out for now." MP3 and a plain MP4 wait.
- **Learn's maths moves to the Rust core.** "heat math is fine thats actually better rust I trust more than typescript for this."
- **The dates.** "heat 100% done by the end of today" (7 Oct), the app "80-90% done by this weekend" (11 Oct), and "a working version of wi-wwav by january" (2027).
- **Syllabus import and the Claude-drafted weekly note** were left to the build ("sounds good on any direction u feel comfortable on this"). They stay out of Learn's first version; each can come back as one more MCP tool.
