# Wi_WWAV

*One desktop app where you plan your time, make songs and films on one clock, live among other people's work as a galaxy, and sell what you make from a shop you can walk through.*

- **Working name:** Wi_WWAV, said "we wave".
- **Date:** 6 October 2026.
- **Status:** a description written before any code. Parts of it already exist in other WWAV products, and each of those parts says where.
- **Platform:** Mac first, as a signed and notarized direct download. Windows comes later.

Four labels mark how settled each part is:

| Label | Means |
|---|---|
| **Decided** | settled in conversation with the founder |
| **Proposed** | this document's design for the new app, which is most of it |
| **Open** | a decision the founder still has to make, given with a recommendation |
| **Exists today** | already built in the repo or in Heat, with where it lives |

**Contents**

| # | Chapter | What it covers |
|---|---|---|
| 1 | [The idea](#1-the-idea) | what the app is, the sketch translated, the four connections, the name, one day with it |
| 2 | [One app, four rooms](#2-one-app-four-rooms) | the window, the Now strip, the library, one account, the shared grammar |
| 3 | [Heat](#3-heat) | planning time: Today, tasks by heat, focus sessions, grades, habits, school mail |
| 4 | [Space](#4-space) | the universe of galaxies, the planet player, the four media as worlds, lineage |
| 5 | [Console](#5-console) | one timeline for audio and video, with plugins, MIDI, splits, export and discs |
| 6 | [Files: .wwav, .swav and the session](#6-files-wwav-swav-and-the-session) | the formats byte by byte, the session package, versions and forks |
| 7 | [Unquantized](#7-unquantized) | the walkable shop: halls, booths, the counter, provenance, selling |
| 8 | [Look, sound and feel](#8-look-sound-and-feel) | three visual registers, type, motion, sound, words, tokens |
| 9 | [Under the hood](#9-under-the-hood) | two processes, the clock, audio, video, storage, sync, shipping, tests |
| 10 | [Business, community and the gates](#10-business-community-and-the-gates) | money, Wi-WWAV, school, and the whole app against the gates |
| 11 | [Roadmap, open decisions and glossary](#11-roadmap-open-decisions-and-glossary) | build order, the first slice, every open decision, every term |

## 1. The idea

### 1.1 What Wi_WWAV is

Wi_WWAV is one native desktop app with four rooms (**Decided**).

| Room | Key | What you do there |
|---|---|---|
| **Heat** | ⌘1 | plan your time: today's plan, tasks ranked by heat, a focus timer, grades, habits, school mail |
| **Space** | ⌘2 | live among people and their work: every person is a galaxy, every project a solar system, every work a planet |
| **Console** | ⌘3 | make: one timeline for audio and video, with plugins, exporting `.wwav` songs and `.swav` films |
| **Unquantized** | ⌘4 | sell and buy, in a shop you walk through where each maker stands at a booth |

The window around the rooms never changes. Its brushed-metal title bar holds the room switcher and a Now strip that shows the task you're on beside the track you're hearing (see 2).

WWAV stands for "World-Wide Audio-Visual", and it rests on one idea: "every song comes apart". A song is a `.wwav`, an ordinary WAV whose master plays anywhere, with four stems inside in PRANA's order: vocals, drums, other, bass. The same file comes apart in every room. It shows as four lights in the title bar, four moons in Space, four lanes in the Console, and a planet you hold in the shop. A film is a `.swav`, an ordinary MP4 that carries its identity and its parent. Both formats **exist today** (see 6).

The archive gives every WWAV family one model: "a device, a format, and a marketplace. You make the work on the device, it lives in a WWAV format, and creators can sell it." Wi_WWAV is that model in software, for four media at once:

- **The device** is the Console.
- **The format** is the pair `.wwav` and `.swav`.
- **The marketplace** is Unquantized.
- **Space** carries work between people.
- **Heat** holds the hours all of it takes.

Some things are left out on purpose: a feed ranked by popularity, likes and follower counts, notifications, autoplay, and anything paid for with attention. The founder's own gates (`wi/GATES.md`) fail each of them. Where the sketch asks for one, this document makes the passing design the default, names the conflict, and marks it **Open** (see 10.6).

### 1.2 The sketch, translated

The founder's sketch draws four boxes and four lines between them. Each box becomes a room.

| Sketch box | Room | What it is | Reuses (**Exists today**) | Family |
|---|---|---|---|---|
| The organizer, in a window labelled "Wi-WWAV" | Heat | Today with a time column, Plan my day and a Pomodoro timer; tasks, calendar, grades, habits, mail | Heat, a claude.ai artifact of 1,481 lines; the PKM (`portfolio/src/pkm/`) | none; it holds the time behind all four |
| "souped up version of WWAV app v3", an "intergalactic 3D social media for music, film, writing & fashion" | Space | a universe you zoom through; the player is the planet; replies are forks | v3's player, deck and video synth (`ios_v3/`); v4's galaxy (`wwav/`); v5's orbits and gallery planets (`ios_v4/`, `server/routes/v2/`) | Mi, Si, Ri, Gi as four kinds of world |
| The "Mi-WWAV console": one timeline for audio and video, "the best parts of Premiere and Ableton smashed together" | Console | a JUCE engine hosting VST3 and AU, MIDI, local splits, and video on the engine's clock; it writes PRANA discs | `prana/core`; MI-WWAV-OS's timeline and undo-journal designs (ideas, not code); WWAV Push (`vst_plugin/`) | Mi (Mi_pro_WWAV's software) and Si |
| "unquantized", a walkable 3D video and record store, "where you can walk around infinitely and purchase digital goods" | Unquantized | a lit shop at dusk: a counter and four halls of drifting booths | PRISMON's engine; the Works hub's orbits (`Hub.jsx`); `purchase.js`, `connect.js`, `fashion.js` | Mi, Si, Ri, Gi as four halls |

Zi_WWAV (furniture) stays out, because the sketch names four media and nothing in the app makes a chair.

### 1.3 The four connections

```
Heat ── connected profiles ── Space ── connected distribution ── Unquantized
                                │                                     │
                        connected uploads                     connected selling
                                │                                     │
                                └───────────── Console ───────────────┘
```

**Connected profiles (Heat and Space).** Heat is private. Only what you drop on your galaxy crosses: an opt-in "Now making" line on your sun, and a project's milestone beads on its solar system. Hours, grades, habits and every count stay home, because a productivity number on a profile invites comparing people (3.14).

**Connected uploads (Console and Space).** Dropping an export on a solar system publishes it. The server keeps the exact bytes and reads the work's family from the file's `wlin` chunk. Going back, any planet dragged onto "Console" opens as a session with four stem lanes, ready to fork (4.13, 5.17).

**Connected selling (Console and Unquantized).** You drop an export on your shelf and set a price. The buyer downloads exactly those bytes, DRM-free, with the sha256 on the receipt, and you receive 90% through Stripe Connect. The sale shows in your back room, never as a number on the shelf (7.11, 7.12).

**Connected distribution (Space and Unquantized).** A work for sale wears "$4 · on the shelf" in the sky. Clicking it opens the shop at that shelf, so walking is never required. A share link opens on www.wi-wwav.com with **Play** and **Buy**, so a friend on a phone installs nothing (7.17).

### 1.4 Why one app, and why a desktop

**One app**, because the boxes share everything that matters: one file format, one account, one library, one undo journal, and one audio engine. That engine's playhead is the clock for the Console, the Now strip, Space's orbits and the shop's listening booths. As four apps, every line in the sketch would be a trip through Finder.

**A desktop**, because the founder's test for hardware holds for software too: "I need to build something irreplaceable by the iphone." Several of the app's jobs need a computer:

- hosting other developers' plugins;
- cutting 4K film against four stems on one clock;
- walking a store with a keyboard, mouse or gamepad.

It is a direct download, because the Mac App Store's sandbox blocks plugin hosting, and purchases go through Stripe with no Apple cut (**Decided**). That is the founder's "taking another bite out of apple".

**A business**, in the founder's words: "I am creating technology for artists and selling it", "an artist first technology company". The money comes from selling files, clothes, tickets, devices, storage and time, never attention (see 10). "Wwav is about everybody" sets the floor: the free app is a whole product, and the performance target is a 2020 M1 MacBook Air.

The devlog pulls the other way twice. Post 1 wonders about "One medium: music; one screen?", and post 34's roadmap is all hardware. Wi_WWAV doesn't replace the devices; it is the software layer above them. The Console is Mi_pro_WWAV's desktop half ("Makes discs"): it writes PRANA discs and brings their remixes home (5.15). It does compete for one founder's hours, so it is built in stages that each stand alone (see 11).

### 1.5 The name

**Wi_WWAV** is said "we wave". The sketch labels the Heat window "Wi-WWAV", and the second devlog letter coins the word: "I think Wi-Wwav (we wave) is a perfect name for the people who are customers and fans of wwav, we are the wwav, so just shorten it and its Wi-WWAV." The app takes the name of the people it is for. The underscore follows the archive's rule for family names, "never a hyphen" (`archive/src/content.js:281`).

**Exists today:** www.wi-wwav.com is already Wi_WWAV, a private wall of `.wwav` and `.swav` files that one account can sign in to (`wi/`). The rest of this document calls that site **Wi**, or the wall, so "Wi_WWAV" always means the app. **Proposed:** it becomes the app's web face, serving share pages, downloads, and each person's wall as their public shelf (9.7).

**Open: hyphen or underscore.** *Recommendation:* use the underscore for the app and the hyphen for the people, as letters already do ("Dear Wi-WWAV,"). Domains keep the hyphen either way, because a hostname can't contain an underscore.

**On version numbers.** The sketch's "WWAV app v3" is, in the archive, DISCMAN, the spring 2026 iPhone app "for music, image, text and video posts", while the galaxy is v4 (web) and v5 (iOS), SOLAR SYSTEM. Space is built from both, and which one "v3" means is **Open** (4.1).

### 1.6 A day with Wi_WWAV

Tuesday, October 6. All of this is **Proposed**, and every name, key and number is the one the later chapters specify.

**8:40 AM, Heat.** The app opens on Today and syncs, because the last sync was over 15 minutes ago: "Synced 8:41 AM: 2 new tasks, 1 date change". One task came from the Brightspace calendar feed: "Grammar quiz 4", JPN 201, due Wednesday 11:59 PM. Claude scored it at difficulty 2: "Claude's estimate: 45m. It read the title, the notes and your past averages." Its tube is amber, which means Warm. At difficulty 2 the runway is 5 days, so it will turn Hot with 1.5 days left, at 11:59 this morning.

**Plan my day** drafts dashed blocks into the time column in heat order, each with its reason: "Due tomorrow 11:59 PM, Warm." Return accepts them: "4 blocks · 3h 10m planned · 2 due today".

At 9:00 you press C on the quiz to make it current, then F. The LCD reads "24:59 · Focus 1 of 4 · Grammar quiz 4". The space bar starts World Ending, the album's title song. You click its vocals light, the voice drops out within one audio block, and the band plays on to study to. When the round ends: "Focus done. 25m logged to Grammar quiz 4." Then it waits: "Break 5:00. Press F to start it."

At 11:59, while you're in class, the tube fills past 0.70 and turns red. The strip reads "Hot: Grammar quiz 4". Nothing pings.

**2:00 PM, Console.** The next block, "Mix the second verse", is linked to the session Low Tide, A minor, 86 BPM. You press ⌘3, and the Now strip still reads "Mix the second verse · focus 24:12 left".

- **Keys.** You add an instrument track (⌘⇧T) with an AU piano. "KeyStep connected · channel 1 → selected track." The chords land where you played them. You quantize only the low notes, 1/8 at 50%: "Undo quantize 11 notes".
- **A crash.** A third-party delay, "Tape Echo", crashes, and only the engine falls: "The audio engine stopped. 'Tape Echo' on the track 'Keys' was running when it did. Restarting…" Two seconds later you press **Try it again**. The focus countdown never stopped.
- **A split.** You drag `break.wav` from Purchases onto a lane and press ⌃⌘S: "Splitting 'break.wav' · segment 4 of 12". About 20 s later there are four lanes. You keep the drums and play one hit on the Sampler as a pad.
- **Picture.** You drop in a 4K phone clip of the shore and cut on bar lines with ⇧Return. The grade is saturation 140 and contrast 115. A prism generator at 30% follows the drums.

You select OUTPUT and press Return with `.wwav` and `.swav` ticked. The sheet reads "This session is 48 kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered.", "Stems sum to the master." and "3:58 → about 210 MB." A minute later both files exist, and the film's parent is the song.

**4:30 PM, Space.** You drag the export onto "Space" and drop it on the solar system World Ending. The planet condenses in while a ring traces the upload: "Up. Low Tide is in your galaxy." It is brick red, hsl(0, 58%, 42%), because A minor's hue is 0, and the ringed film hangs from it. You also drop `Low Tide.wwav` on "Unquantized" at $4, ".wwav, master and four stems". In the sky it now wears "$4 · on the shelf".

**6:10 PM, Ana.** Ana has added your galaxy, so Low Tide tops her Newest. She mutes the vocals moon, holds the drums moon 400 ms until the FX moons bloom, pulls delay out, and presses ↑ Push. A comet spark leaves the planet, and "Low Tide (fork)" joins your song's family at generation 1, owning no audio of its own. Nobody tells you.

**8:15 PM, Unquantized.** ⌘4 puts you outside the shop at dusk, in a still frame: oak and glass, gold leaf, "Press E to open." The press opens the door, and the room comes up over 1.2 s with air, the espresso machine and a cup set down. The only figures are makers at their booths. Low Tide is on the New table.

At Ana's booth you point at a sleeve: "glass hours / Ana · Song · A minor · 128 BPM / gen 2 · remix of World Ending by LMY / $4". E lifts it, and it becomes a lit planet 22 cm across in your hands while the wall warms to its key colour. In a glass listening booth, the space bar plays it. You mute the drums to hear what's left, then press ⇧Return to save it for later.

On the fashion rails, a tag reads "Waxed chore jacket / M · like new · $120 · one of one". The fitting mirror says "This shows colour, not fit. Check the measurements." You press B: "Held for you · 29:41", the only countdown in the store. Then you pay in Stripe Checkout.

The back room behind your booth has one new row: Low Tide.wwav, $4.00, fee $0.40, net $3.60.

**10:30 PM, Since you last looked.** At the top of Space you find Ana's fork, the sale, two new works from galaxies you've added, and a letter that opens "Dear Wi-WWAV,". The list ends "That's everything since 4:30 PM." You quit. Nothing reaches out before morning, when the app opens on Today with the quiz at the top, Hot.


## 2. One app, four rooms

This chapter covers the shell, which is everything that stays put when you change rooms. Everything here is **Proposed** unless it carries another label. The rooms themselves are chapters 3 (Heat), 4 (Space), 5 (Console) and 7 (Unquantized).

### 2.1 The window

There is one main window. It opens at 1280 × 800 pt and can shrink to 1024 × 680.

| Band | Height | Contents, left to right |
|---|---|---|
| Title bar | 52 pt | traffic lights · room switcher · Now strip · search pill · astronaut chip |
| Room | the rest | the current room |
| Status bar | 22 pt | the room's own count · sync state · save state |

The title bar is brushed metal in every room, in light and dark. As MI-WWAV-OS puts it, "Aqua is the case; the Game Boy is inside it." The case never changes, so the controls never move. What is inside the case does change: striped Aqua lists in Heat, the night in Space, metal with DMG-green screens in the Console, and warm interior light in Unquantized (see 8).

The title bar holds four controls:

- **Room switcher.** A four-segment gel control, 76 pt per segment, reading "Heat", "Space", "Console", "Unquantized", on ⌘1–⌘4. The selected segment wears the deepened blue gel (`#336dcc→#1B4C8C`), so its white label holds 5.0:1 (8.2). Changing rooms is a 140 ms cross-fade, the MI-WWAV-OS "beat", and a cut under Reduce Motion.
- **Traffic lights.** Real window controls. Heat's are only decorative today.
- **Search pill.** Opens ⌘K.
- **At narrow widths.** Below 1180 pt the search pill becomes a 28 pt magnifier and the Now strip narrows to 440 pt, so the title bar fits at 1024 pt.
- **Astronaut chip.** A 28 pt portrait of your astronaut. It opens "Your galaxy", "Your shelf", "Settings…", "Export everything…" and "Sign out".

### 2.2 The Now strip

**Exists today:** Heat's toolbar LCD. It is pale olive `#f2f4e4→#dfe3c6` with ink `#262a17`, shows two lines over a 6 px meter, and becomes phosphor `#d7e0a8` on `#20241a` in dark mode.

**Proposed:** the LCD grows to 520 × 44 pt and is split by an etched 1 px divider. The task sits on the left and the track on the right, so one glance answers "what am I doing" and "what am I hearing".

| Half | Line 1 | Line 2 | Meter | Click |
|---|---|---|---|---|
| Task | "Hot: Grammar quiz 4" | "Today 4:00 PM, JPN 201 · focus 18:40 left" | that task's heat, cool→warm→hot | opens Heat with the task selected |
| Track | 18 pt key-coloured planet, "World Ending" | four stem lights, "1:42 / 3:58" | the playhead | expands the player |

The task half shows the current task (set with C or by starting focus; see 3.5), or else the hottest open task, as Heat's LCD does now. The track half shows whatever the audio engine is playing; in the Console it shows the session: "BAR 42.3 · 128.00 BPM · REC ARMED". The engine is the master clock and publishes its playhead in shared memory (see 9), so the strip and the video viewer read the same value and cannot disagree.

**Stem lights.** There are four 8 pt lights, in PRANA's order and colours: vocals `#D23C2A`, drums `#F0B90B`, other `#2E9A55`, bass `#1F4E9E`.

- **State is shown by shape, not colour.** Filled means audible. A hollow ring means muted. Filled with a 2 pt outer ring means soloed.
- **A click works like clicking a moon.** It mutes at once. A second click within 250 ms reverts the mute and solos instead.
- **Hit area.** Each light has a 44 × 44 pt hit area.

**When a half is empty**, the strip reads "All clear / Nothing open right now." and "Nothing playing / Select anything and press Space."

### 2.3 Rooms keep their place

Each room is built once and lives until you quit, so switching back finds its camera, scroll, selection, open sheet and half-typed text where you left them. **Exists today:** v4's player "mounts once, outside the routes, and never unmounts" (`wwav/src/components/PlayerShell.jsx`).

**The player is a state, not a screen.** There is one listening player, and the audio engine owns it.

- **Expanded,** it is the planet player: over Space, or as a sheet dropped from the title bar in any other room.
- **Collapsed,** it is the planet in the strip. In v4's words, "the bar is just the planet seen from far away" (`wwav/src/planet/MiniBar.jsx`).
- **To collapse it,** press Esc or pinch out with two fingers.

| You are | You do | What happens |
|---|---|---|
| Listening | Switch rooms | It keeps playing. |
| Listening | Press play in the Console | Listening pauses and the strip reads "Paused for the Console". Stopping the Console doesn't resume it. |
| Listening | Open a film | "The planet is paused while this plays." (v4) |
| In an Unquantized listening booth | Walk out | The booth stops: "Leave, and your place is kept." (PRISMON) |
| In a Heat focus session | Switch rooms | The countdown continues in the strip. It is silent unless you turned the chime on. |

Nothing resumes on its own. The Console can tear its video viewer or mixer off onto a second display. Plugin windows belong to the audio engine (see 5).

### 2.4 One account, one you

You have one WWAV account: the existing `Users` row at mi-wwav.com. Its 7-day JWT is kept in the macOS Keychain and refreshed after a 401. WWAV Push already refreshes on a 401, though it keeps its tokens in a properties file.

| Part | What it is | Stored as | Seen by | Status |
|---|---|---|---|---|
| Galaxy | your place in Space | `Galaxy`, one per user (slug, `skySeed`) | everyone | Exists today (v4) |
| Bio sun | your page | blocks v1 `{v:1, blocks:[…]}` | everyone | Exists today (`wwav/src/sun/blocks.js`) |
| Astronaut | your body in Space and Unquantized; the title-bar chip | `Users.astronaut` JSONB: race, complexion, physique, hair {style, color}, ≤ 8 cosmetics, rocket {preset, hull, accent, flame} | everyone | Exists today (`ios_v4/WWAV/Models/AstronautModels.swift`) |
| Stem player | your instrument's skin, wrapped from a photo or painted | `/api/user/stem-player-customization` | visitors to your shop's booth | Exists today (v3) |
| Heat | your time | local SQLite, synced privately | only you | Proposed |

**The astronaut stores IDs, never art.** The placeholder vectors can therefore become Unquantized's Mii-like characters without touching a single stored user (see 7).

**The stem player.** v3's tutorial says "the user's stem player IS their account". In Wi_WWAV the astronaut is who you are and the stem player is what you hold: your skin wraps the strip's planet when nothing is loaded, and the player in your shop's booth. A loaded song always wears its own key colour.

### 2.5 The library: files first

Wi's rule carries over: "A post is a file." The library is a folder you can open in Finder:

```
~/Music/Wi_WWAV/
  library.sqlite   clips, tags, sequences, Heat records, the undo journal
  media/           01JA2B7X9Q4M8K3T5V6W0YHZRC.wwav   (ULID-named)
  sessions/        Console sessions (see 6)
  purchases/       every file you bought, as delivered
  trash/           deleted media, kept until you empty it
```

**File names are ULIDs.** They sort by creation time as plain strings (**Exists today:** `MI-WWAV-OS/engine/src/ids.rs`). A song's title lives inside its file, in `wmet`. Renaming a song never renames anything on disk, and two songs called "untitled" never collide.

**Three models, from MI-WWAV-OS** (**Exists today:** `MI-WWAV-OS/engine/src/model.rs`):

- **Clip** is any one thing: a `.wwav`, a `.swav`, plain audio or video, an image, or text. A note is a zero-duration text clip.
- **Tag** is `user`, `card` or `system`. System tags are places: your solar systems and your shelves. Dropping a clip on one publishes it there.
- **Sequence** is an edit list, never media. A Console session is a sequence. Rendering it makes a new clip that points back through `from_sequence`, so "the edit stays editable forever."

**Organisation, from v3's library** (**Exists today:** `ios_v3/WWAV/Models/Track.swift`, `LibraryFolder.swift`):

- **Tags** are lowercase, with at most 12 per clip.
- **Colour labels** come in six, for sorting only: red `#E0383E`, orange `#F08A2C`, yellow `#E8C73A`, green `#3CAA68`, blue `#3D7DD5`, purple `#9359C9`.
- **Pins** fill a row of 4 slots.
- **Smart folders** are AND rules over tags, colour, a BPM range, key and kind, such as "128–132 BPM · minor · tag:live-drums".
- **Bulk edits** cover tag, colour, move and delete.

**Where the library appears.** ⌘L slides a 280 pt source-list drawer over any room. The Console's browser, Space's publish drawer and your Unquantized shelf are all filtered views of this same library.

A plain WAV comes in as master only and says so. Nothing is converted without a press (see 6).

**Open: copy or reference.** Should imports be copied into `media/` or left where they are? Recommendation: copy songs, films and anything under 2 GB, so the library and its export are complete. Offer "Leave in place" for camera folders and long raw video.

### 2.6 The four connections

| Connection | Rooms | What crosses | What never crosses |
|---|---|---|---|
| Connected profiles | Heat ↔ Space | a project's milestone timeline and an opt-in "Now making" line, each only after you drop it on your galaxy (3.14) | task lists, hours, grades, habits, any count |
| Connected uploads | Console ↔ Space | exported `.wwav`/`.swav` as exact bytes, with lineage read from `wlin`; forks back into the Console | unexported takes, unpublished sessions |
| Connected selling | Console ↔ Unquantized | an export, its price and terms, which becomes a record on your shelf; payouts via Stripe Connect | sales counts on any shelf |
| Connected distribution | Space ↔ Unquantized | a for-sale planet linked to its shelf and back; purchases landing in your library | ranking by sales or plays |

**Connected profiles: a timeline on a sun (see 3.14).**

1. In Heat's WWAV space, set the project "EP" to point at the solar system "World Ending" (Get Info → Project). Space doesn't change, because the link is private.
2. Drag the project onto "World Ending" under "Your galaxy" in Heat's sidebar. The sheet reads "This timeline will show on your galaxy", and **Show** adds the milestone beads (titles, dates, reached or not) to the system's sun. ⌘Z reads "Undo show timeline".
3. Ticking **Reached** on "EP v1 mixed" later fills its bead and publishes nothing else. In the other direction, your project sun's secondary act, **Plan in Heat**, creates a linked milestone.

**Connected uploads: from a friend's planet to your fork.**

1. In Space, drag a friend's planet "glass hours" onto the "Console" segment, or press ⌘E. A session opens with four stem lanes and the parent's remix snapshot applied, under the header "fork of glass hours · gen 3".
2. Record a vocal take, mute the original vocals and export a `.wwav`. It is written with `wmet.type` `remix`, `wlin.parent_id` set to the parent's `song_id`, `generation` 3 (the parent's + 1) and `creator` you. How N lanes fold into four stems is in 5 and 6.
3. Drag the export onto "Space" and choose the system "Covers". Once it lands, the fork edge shows in the parent's lineage. A human link such as "sample" waits until the other owner presses **Agree** (**Exists today:** v4 lineage consent).

**Connected selling: from an export to a shelf.**

1. Drag "World Ending.wwav" onto "Unquantized", or onto your shelf while you stand in your shop. The drop sheet asks for:
   - **Price** ($9).
   - **What they get**: ".wwav, master and four stems", ".wwav, master only" or ".swav film".
   - **Crate** ("LMY · new"), one of your solar systems on your booth (7.6).
2. Without a payout account, the sheet says "Selling needs a payout account. Set it up once." and offers **Set up payouts**, which starts Stripe Connect Express (**Exists today:** `server/routes/connect.js`).
3. The buyer pays through Stripe Checkout and downloads the exact bytes you dropped, DRM-free, with the sha256 on the receipt. The sale shows in "Since you last looked", never as a number on the shelf.

**Connected distribution: from a planet to the counter.**

1. In Space, a planet that is for sale wears a tag: "$4 · on the shelf". Clicking it opens Unquantized at that shelf with the record pulled forward. Walking is for browsing, never required.
2. Press to listen in the booth, then **Buy**. The file lands in Library → Purchases, the planet reads "yours", and its stems open in the Console.
3. A share link opens the same record on a phone at www.wi-wwav.com with **Play** and **Buy**, and nothing to install.

### 2.7 One grammar in every room

These rules come from MI-WWAV-OS. When a feature breaks one, the feature changes.

**Undo is ⌘Z everywhere, always free, and always labelled.** Every change is a transaction that snapshots the affected rows before and after (**Exists today:** `MI-WWAV-OS/engine/src/store.rs`).

- **Labels.** The Edit menu reads "Undo move clip", and a toast says "Undone — move clip" for 2.6 s. ⌘⇧Z redoes.
- **Scope.** There is one journal, but ⌘Z acts on the room you are in.
- **Deleting.** A delete removes rows, never media. Files leave only when you empty the trash.
- **Work that has left the machine.** It can't be undone, and the label says so. ⌘Z cancels an upload that is still queued. After the server has it, the menu reads "Can't undo a publish. Unpublish 'World Ending'…".

**Esc and back never destroy committed work.** Esc on a sheet keeps the draft you typed for the next time it opens.

**Each screen has exactly one secondary act.** It is ⇧Return on the selection (or ⇧-click), and the status bar names it.

| Screen | Return | ⇧Return |
|---|---|---|
| Heat tab | edit the selected task | the tab's secondary action (Today: Plan my day; Tasks: Triage inbox; see 3.3) |
| Space sky | dive in, or open a work's player | Add (a private save) |
| Planet player | play / pause | ↑ Push (fork the mix) |
| Your sun | open | Plan in Heat |
| Console timeline | select clip at playhead | Cut at playhead |
| Unquantized record | listen | Save for later |
| Library | open | add to selection |

**Publishing is a drop.** You publish by dragging onto a system, a shelf or a room segment, wherever a target can be shown.

**⌘K is one palette** over tasks, clips, sessions, galaxies, shelves, settings and actions.

- **Results.** Local fuzzy matches come first and server results follow after 200 ms (**Exists today:** `portfolio/src/pkm/palette/CommandPalette.jsx`). Filters: `tag:`, `key:`, `bpm:`, `is:hot`, `is:remix`, `@name`.
- **Opening.** ⌘Return opens a result in its other room: a song in the Console, a person in their shop.

**⌘⇧N is quick capture.** It opens a 420 × 160 pt panel from any room, or from any app if you turn that on.

- **Capturing.** Type, paste a link, drop a file, or hold R to record a voice memo. Enter saves and keeps the panel open. The footer reads "3 in inbox · captured ✓" (**Exists today:** `portfolio/src/pkm/capture/CaptureModal.jsx`).
- **Triage.** The inbox sits at the top of Heat → Today, with **→ task**, **→ note**, **→ project** and **→ upload** on each item.

| Keys | Does | Where |
|---|---|---|
| ⌘1 · ⌘2 · ⌘3 · ⌘4 | Heat · Space · Console · Unquantized | everywhere |
| Space | play / pause | wherever media is, outside text |
| ⌘K · ⌘⇧N · ⌘L | palette · quick capture · library drawer | everywhere |
| ⌘Z · ⌘⇧Z | undo · redo, labelled | everywhere |
| Esc | close, collapse, deselect; never deletes | everywhere |
| ⇧Return | the screen's one secondary act | everywhere |
| ← → ↑ ↓ | move selection; level ±5% on a focused stem | lists; planet, Console lanes |
| M · S | mute · solo the focused stem | planet, Console lanes, strip |
| ⌘E · ⌘⇧E | open in Console · Export everything… | a selected work · everywhere |

### 2.8 Offline first

What is local is the truth. Heat, the library and the Console work with no network for as long as you like. Space and Unquantized show what you have already visited, marked "Seen Oct 4". Buying, travelling somewhere new, and Claude each need a connection, and each says so on its button.

**The upload queue is a query, not a list** (**Exists today:** `MI-WWAV-OS/proto/README.md`). A drop sets `published_at` at once, and `remote_id` fills in when the server acknowledges.

- **What counts as waiting.** Anything with `published_at IS NOT NULL AND remote_id IS NULL`. The queue survives a crash with no queue file, and unpublishing early cancels the upload with no extra logic.
- **Uploading.** Each upload signs just before it starts, because presigned URLs last 300 s. It goes up in 8 MiB parts, as Wi's uploads do, and resumes from the last finished part.
- **No double posts.** The publish body carries `settings: {origin: "wi_wwav", clipId: <ULID>}`, so a retry never posts twice.

The status bar says only real stages:

1. "Offline. 2 works wait to go up; they leave when you're back."
2. "Uploading World Ending · part 14 of 27"
3. "Up. World Ending is in your galaxy."

**Heat records sync privately** to your account, with a monotonic sequence number per field, so a slow older write never overwrites a newer one (**Exists today:** the PKM's `useAutosave.js`; see 9).

### 2.9 You can leave with everything

File → **Export everything…** (⌘⇧E) is one action. It writes a folder or a zip, is never behind a paywall, and works while you are signed out.

| In the export | What it is |
|---|---|
| `media/` + `manifest.json` | every library file byte for byte, with its sha256 |
| `sessions/` | every Console session, with the plugin state it saved |
| `purchases/` + `receipts.json` | every file you bought, as delivered |
| `heat.json` | tasks, milestones, habits, courses, grades, focus sessions |
| `notes/` | Markdown with frontmatter and `[[wikilinks]]`; opens in Obsidian |
| `galaxy.json` | your galaxy, systems, suns as blocks, lineage links |
| `index.html` | plays every song (stems apart) and every film from disk |

**Exists today:** Wi's one-press export with an offline `index.html` (`wi/README.md`), and the PKM's Obsidian zip.

The test is written before it is run: export, turn Wi-Fi off, and open `index.html`. Every song must come apart, and every film must play.

### 2.10 Notifications: pull only

**Proposed default:** nothing reaches out. There are no OS notifications, no Dock badge, no sounds, and no unread dots on the room switcher.

**"Since you last looked"** is a list at the top of Space, built when you open it. It holds forks of your works, links waiting on you ("*name* says their planet is an influence of your planet", with **Agree** and **Refuse**), sales and payouts, and new works from galaxies you have added. It ends with "That's everything since Oct 4." Jobs you started yourself, such as splits, exports and uploads, report in the status bar and finish quietly.

**Open: reminders and badges.** The sketch's planner implies "due at 4 PM" reminders, and gate 1.1 fails anything that reaches out. Recommendation: never notify or badge about what other people do. Allow one kind of OS notification: an alarm you set yourself on one task or focus session, off by default.

### 2.11 Claude in the app

Claude estimates and drafts, and never decides. Nothing it suggests applies without a press, and each suggestion carries a one-sentence reason. Ripple Creator's rule holds throughout: "never invent metrics."

Calls go through mi-wwav.com, which holds the founder's Anthropic key and rate-limits each account; the key never ships inside the app binary. Heat's two tiers carry over, "quick" for scoring and "default" for reading and drafting, and the server maps each tier to a model, so a model change needs no app update. Ripple uses `claude-sonnet-4-6` today (**Exists today:** `server/services/claudeService.js`).

| Feature | Room | Tier | Sends | When it fails |
|---|---|---|---|---|
| Score a task | Heat | quick | title, type, notes, your average minutes per type | type average, else difficulty × 20 min |
| Read school email | Heat | default | ≤ 8 messages per sync, date, time zone, ≤ 80 existing titles | "More emails left, they'll come in on the next sync." |
| Feedback on your work | Console | default | the work's analysis and your question; answers use [MM:SS], never Hz/dB/LUFS | "Feedback isn't available right now." |
| The clerk | Unquantized | default | one record's metadata and the seller's notes | "The clerk can't answer right now." |

**Consent.** Each feature is off until its first use. The first use shows exactly what will be sent: "Heat will send this task's title, type and notes, and your average minutes per type. Nothing else." The buttons are **Turn on scoring** and **Not now**. Reading email has its own switch.

**Failures** follow Heat's sample handling:

- **Not granted.** The button hides, with the message "Claude scoring is off. Set difficulty yourself."
- **Rate-limited.** "Too many requests. Wait a minute, then try again."
- **Offline.** The button reads "Needs a connection".
- **Daily limit.** "Claude's 50 calls for today are used. They come back at midnight." (10.4)
- **Retryable errors.** These get one retry after 1500 ms plus up to 800 ms of jitter.

### 2.12 Accessibility bars

These are gate 2.4's bars, written down before testing. A desktop rule is added because the gate's posture rule covers phones only.

- **Contrast and text.** Body text 7:1 and secondary 4.5:1, in light and dark, in all four registers. Body text is at least 13 pt; ⌘+ scales it to 20.
- **Targets and keys.** Hit areas are at least 44 × 44 pt, and every gesture has a key. "Go to shelf" in ⌘K means Unquantized never requires walking.
- **Motion and state.** Nothing moves while nothing plays, and state is never colour alone. VoiceOver reads a moon as "Vocals, 70 percent, audible".

The existing tokens measured against these bars, and the fixes they need (LCD dim text to `#4b5034`, selected rows filled `#1B4C8C` with white text), are in 8.2 and 8.11.

### 2.13 Settings

Settings is a classic Mac preferences window with an Aqua icon toolbar.

| Pane | Contents |
|---|---|
| Account | galaxy address, "your astronaut & rocket", stem player skin, sign out, delete account (type DELETE) |
| Library | location, watched folders, copy or leave in place, Empty trash |
| Heat | spaces, school and time zone, Brightspace calendar link, Google account |
| Audio & MIDI · Video | devices, buffer 64–1024 samples, plugin folders; hardware encode, proxy media |
| Claude | one switch per feature and what each sends |
| Selling · Privacy | payout account; one table of everything public |
| Appearance · Keyboard | Light, Dark or Match system (Space is always night), text size; every shortcut |

### 2.14 First launch

There are five steps. Each has exactly one secondary action, "Skip for now", and without an import the whole flow takes under three minutes.

1. **Sign in.** The button **Sign in or create an account** opens mi-wwav.com in your browser (9.7). The form there asks for email, username, password, a birthdate for the 13+ gate, and an invite key when one is required (**Exists today:** v4 registration). The browser hands the sign-in back to the app.

   **Open:** should "Not now" let you in without an account? Recommendation: yes. Heat, the library and the Console are fully local. Space and Unquantized can be looked at but not travelled, published to or bought from, which is how the signed-out universe already works.
2. **Make your astronaut.** The pages are race · body · hair & marks · rocket · ready, as in v5. Skipping gives you `AstronautDoc.starter`.
3. **Claim your galaxy.** The copy is v4's: "You have no galaxy yet. A galaxy is yours. Projects orbit it as solar systems, and each song or film is a world inside one. The sun at the centre is where you say who you are." The button reads **Make my galaxy**. An optional first line on your sun has the placeholder "Say it plainly".
4. **Import your folder.** Before anything is copied, you see what the folder holds: "214 files: 38 .wwav, 12 .swav, 160 plain audio, 4 other. Plain audio comes in as master only." The button reads **Bring them in**. It runs in the background, and pressing again picks up after an interruption.
5. **Connect your school calendar.** Choose one of:
   - "Paste your Brightspace calendar link": the per-student iCal feed, which works now with no approval.
   - "Connect Google": Gmail and Calendar.
   - "Skip".

   The school-approved route (Valence and LTI 1.3) is in chapter 3.

The app then opens on Heat → Today. The strip reads "All clear" on the left and "Nothing playing" on the right.


## 3. Heat

Heat is the room where you plan your time. It opens with ⌘1 and comes first in the room switcher, because the day starts there (see 2. One app, four rooms). The founder already uses it: today it is a single-file artifact inside claude.ai. Section 3.1 describes that file exactly, and all of it is **Exists today** (Heat, one HTML file of 1,481 lines). From 3.2 on, everything is **Proposed** unless it is marked **Decided** or **Open**.

### 3.1 Heat today

Heat is a personal task tracker that ranks work by urgency, which it calls heat. Around that ranking it keeps milestones, daily habits, grades, and a Brightspace sync that runs through Gmail and Google Calendar. Claude estimates difficulty and time. It runs on four claude.ai capabilities: `db` and `user` (live storage), `sample` (Claude) and `mcp` (connectors). Each one fails soft; without `db`, it shows "Saved in this browser".

A segmented control at the top switches between three workspaces, each with its own Claude persona and types (plus Other in each): **Classes** (group label "Course": Homework, Quiz, Listening, Reading, Lab, Project, Exam prep), **WWAV** ("Milestone": Hardware, Software, Design, Music, Business, Content) and **Personal** ("Area": Errand, Admin, Money, Health, Home, Social). In Heat, "WWAV" means the handheld hardware and an album, not the social app. Habits exist only in Personal, Grades only in Classes, and the milestone timeline only in WWAV.

#### The heat algorithm

```
done         -> no heat ("Done")
no due date  -> v = 0.05, "Cool"
days   = (due - now) / 86_400_000
days < 0     -> v = 1.1, "Overdue"
runway = difficulty * 2 + 1           // difficulty 1..5 -> 3, 5, 7, 9, 11 days
v      = clamp(1 - days / runway, 0, 1)
v >= 0.70 -> "Hot";  v >= 0.34 -> "Warm";  else "Cool" (v floored at 0.05)
```

Harder work heats up sooner. A task turns Warm or Hot when this many days are left:

| Difficulty | 1 | 2 | 3 | 4 | 5 |
|---|---|---|---|---|---|
| Warm at | 1.98 d | 3.3 d | 4.62 d | 5.94 d | 7.26 d |
| Hot at | 0.9 d | 1.5 d | 2.1 d | 2.7 d | 3.3 d |

The level colours are Overdue `#8f1d16`, Hot `#e0402c`, Warm `#efa431` and Cool `#4f9be6`. The badge is a 46×11 px glossy tube filled to `v`. Open tasks sort by heat descending, then by due date, with undated tasks last. The window re-renders every 60 seconds, so heat and phrases like "3h overdue" stay current.

When you check a task off, a sheet asks "Time it took", with the hint "This trains your time averages for this type of task." A task's estimate is its own `estMin` if it has one, else the average for its type in that workspace, else difficulty × 20 minutes. Weekly load is the sum of estimates for open tasks due within 7 days.

#### The window

| Region | What it holds |
|---|---|
| Toolbar | Round "+" gel; View switch (List / Calendar / Habits / Grades); centre LCD; Sync; pill search |
| Sidebar, 190 px | "Library" (All open, Hot, Due this week, Done) with counts; Courses / Milestones / Areas; "Your average time", e.g. "Homework 1h 15m (6)" |
| List | 26 px striped rows: checkbox, Heat, Task, group, Type, Due ("Today 4:00 PM", "2d overdue"), Diff, Time |
| Calendar | Month grid from Sunday; 3 pills per cell with a 3 px heat border, "N more"; today in a red circle |
| Get Info, 290 px | Appears only while a task is selected: every field, the source ("Brightspace calendar"), Edit, "Mark done" |
| Sheets | One sheet that drops from the top in 0.22 s; validation in one line: "Give the task a name first." |
| LCD | Two lines and a meter: "Hot: Grammar quiz 4" / "Tomorrow 11:59 PM, JPN 102. This week: 3h 20m across 5 tasks" |
| Timeline (WWAV) | A blue groove filled to a red "Today" marker, with 16 px glass beads; click a bead to filter, double-click to edit |
| Habits (Personal) | Up to 6 habits, each with a 34 px gel orb, a 14-day grid of 13 px squares, and "N-day streak". The streak survives until midnight. The log is pruned at 400 days |
| Grades (Classes) | One card per course under "Fall 2026 grades": the %, a letter pill, "Based on X% of the course so far", yellow "New grade posted" banners, Stickies |

The grade maths stays exactly as it is:

```
category % = Σ score / Σ outOf   (items not dropped, outOf > 0, scored)
current %  = Σ (weight × category %) / Σ weight   (graded categories only)
decided %  = graded weight / total weight × 100
letter     = ≥93 A, ≥90 A-, ≥87 B+, ≥83 B, ≥80 B-, ≥77 C+, ≥73 C, ≥70 C-, ≥67 D+, ≥60 D, else F
```

A course can bring its own scale. Letter pills are green for A, blue for B, amber for C and red for D or F.

#### Brightspace sync and Claude

- **Calendar path.** Heat reads the Google calendar named like `/university of rhode island/i`, from 2 hours ago to 70 days ahead, and keeps titles ending " - Due". It takes the course from the location ("MTH 142") and the type from keywords. It drops a duplicate when the due day and course match and the titles share at least 60% of their words.
- **Gmail path.** Heat searches `brightspace newer_than:Nd`, with N between 2 and 14. Grade notices become **pending grades**, with no score and a link to `brightspace.uri.edu`. Up to 8 announcements per sync go to Claude, which pulls out only items with a future deadline. The last 400 message ids are remembered.
- **Rhythm.** Heat syncs on open if the last sync was over 15 minutes ago, then hourly, and reports "Synced 3:41 PM: 2 new tasks, 1 date change, 1 new grade posted".
- **Claude.** "Ask Claude to score" returns `{difficulty, minutes, reason}`, and the reason shows as the field's hint. If it fails, Heat says why in one line ("Too many requests. Wait a minute, then try again."), or hides the button when scoring isn't granted. Batch scoring of synced items fails silently and keeps the per-type defaults.

The look is classic Mac OS X: brushed metal on a slate desk, gel buttons, Aqua stripes, selection in `#3875d7`, an olive iTunes LCD (`#f2f4e4`→`#dfe3c6`), and Lucida Grande at 13 px. Dark mode turns the LCD dark olive with phosphor ink.

**What it can't do yet:** there is no timer, no time-blocking, no plan for the day and no "current task". Mail is only a sync source. Courses can't be edited in the UI. The school, time zone, term name and category rules are hard-coded to URI. There are no arrow keys and no drag and drop.

### 3.2 From one file to a room

| Today | In Wi_WWAV |
|---|---|
| Three fixed workspaces as top tabs | User-defined **spaces**, which filter every tab |
| List / Calendar / Habits / Grades | **Today / Tasks / Calendar / Grades / Habits / Mail** |
| Get Info only while a task is selected | A right column of widgets: **Now, Habits, Hot tasks, Mail, Grades** |
| Minutes typed in after finishing | Focus sessions write `actualMin` as you work |
| claude.ai connectors, hard-coded to URI | The app's own Google sign-in and a Brightspace iCal feed, set per school |
| Habits in Personal, Grades in Classes | Both global |

The heat algorithm, the estimate chain, the grade maths, the LCD, the sheets and the copy ("Heat will rank it.") carry over unchanged.

### 3.3 The room's window

Heat fills the room area of the one main window (see 2; **Decided**: one desktop app, Mac first): 1280 × 726 pt at the default window, 1024 × 606 at the smallest. Heat's LCD moves up into the title bar as the Now strip's task half (see 2), so from left to right Heat's toolbar holds "+", the six tabs as one segmented control, Sync and search. Below sit the 190 px sidebar, the main view, and a 290 px right column. Below 1240 pt of window width, the right column folds into a 44 px strip of widget icons that open as popovers.

Each tab has one primary action ("+" or N) and exactly one secondary action:

| Tab | "+" adds | Secondary action |
|---|---|---|
| Today | A task straight into the plan | Plan my day |
| Tasks | A task | Triage inbox (only while the inbox has items) |
| Calendar | A task due 11:59 PM on the selected day | Today |
| Grades | A grade | Add course |
| Habits | A habit (at 6: "Habit limit reached") | Show the year |
| Mail | (hidden) | Sync now |

⌘Z undoes anything, and the menu names what it will undo: "Undo mark done", "Undo move block". Esc closes a sheet or drawer, or clears the selection. It never discards saved work.

### 3.4 Spaces

Workspaces become **spaces**: named filters that apply to Today, Tasks, Calendar and Mail. The sidebar opens with All, then each space with its hue dot and open count, then "New space…". Grades, Habits and calendar events ignore the filter, because they belong to the person rather than to a project.

A space keeps the settings the old workspaces had built in, now editable in a sheet: name, hue, group kind (course, milestone or free text), group label, types, placeholders and Claude persona. Classes, WWAV and Personal are created with today's values. In Classes, the group kind is "course", so a task's course points at a Grades course record and each course is defined once. The WWAV persona is rewritten to cover the app: "a solo founder building WWAV: the PRANA handheld (Teensy 4.1, C++ firmware, PCBs), the Wi_WWAV desktop app, and an album."

### 3.5 Today

Today is the sketch's "today's plan", and Heat opens on it. The main view puts a 300 px time column on the left and the plan list on the right, with a 112 px Pomodoro panel across the bottom. The header reads "Today, Tuesday, October 6", with the subtitle "4 blocks · 3h 10m planned · 2 due today".

#### The time column

The time column reuses the PKM calendar's scale (`portfolio/src/pkm/calendarUtils.js`): 44 px per hour, a 15-minute snap and 30-minute default blocks. It runs from 7 AM to midnight and scrolls so that now sits a third of the way down. A 1 px red line marks the current minute.

- **Blocks** have a 3 px left border in the task's heat colour, the title and the length ("45m"). The current block gets the `#3875d7` ring, and finished blocks dim to 50% with a check.
- **Events** from Google Calendar are grey and hatched, and read-only. They sit behind blocks, so a clash shows as an overlap.
- **Drag** a task onto the column to make a block as long as its estimate, rounded up to 15 minutes. Drag a block's bottom edge to resize it. Resizing changes the block, never the estimate. A task can have several blocks.
- **P** puts the selected task into the next free gap after now.

#### The plan list

The plan list has four sections, and any empty one is hidden: **Planned** (in time order, with start times), **Due today, not planned**, **Recurring today ↻**, and **Hot, not planned** (up to 5 suggestions). Below them, the **daily note** sits collapsed to one line until clicked: "How's the day going? Markdown + [[wikilinks]] welcome."

**Plan my day** fills the time between now and "Day ends at" (11 PM by default) with a written rule, not Claude. It takes unplanned open tasks in heat order and gives each a block the length of its estimate, rounded up to 15 minutes and capped at 90 ("45m left to plan"), in the first gap that fits. The drafts appear dashed, each with a reason: "Due tomorrow 11:59 PM, Hot." Return accepts all of them, a click accepts one, and Esc clears them. The empty state says "Nothing planned yet. Drag a task onto the time column, or press Plan my day."

#### The Pomodoro timer

The Pomodoro timer is an olive LCD panel. It shows 32 px tabular digits ("24:59"), a line such as "Focus 1 of 4 · Mix the second verse", and a meter that drains.

- **Lengths.** Focus defaults to 25 minutes, with 50 minutes or a custom 10–90 as options. Breaks are 5 minutes, and every fourth break is 15.
- **Keys.** F starts or pauses. ⇧F stops and logs. I marks "Pulled away", which pauses the timer and records an interruption. The space bar stays play/pause for media, because music under a focus session is the usual case.
- **Ending.** The LCD reads "Focus done. 25m logged to Mix the second verse." It is silent unless you turned on Heat's chime, which is off by default (8.7). The break waits for you: "Break 5:00. Press F to start it." Nothing starts without a press.
- **Other rooms.** The timer keeps running when you switch rooms. The Now strip's task half shows "focus 18:42 left" (see 2); the room switcher stays plain.

Every focus session belongs to the **current task**, and its minutes add to that task's `actualMin`. If a task has logged time, checking it completes it at once, and the status bar says "Done. Took 1h 15m across 3 focus sessions." with an Undo. Get Info's "Took" field adjusts the time by hand. Only a task with no logged time still asks "Time it took". The per-type averages now come mostly from measured time.

#### The right column

These are the sketch's five widgets, in its order. Each is a brushed-metal panel with a small grey heading, and each can be hidden from the View menu.

| Widget | Shows | Actions | Empty |
|---|---|---|---|
| **Now** | The current task, its space dot and heat tube, "Block ends 3:30 PM", and the timer state | Start focus (F), Done (⌘↩), Open link | "Nothing is current. Pick a task and press C, or drag one here." |
| **Habits** | Today's orbs; "3 of 5 done" | Click an orb to toggle it | "No habits yet." |
| **Hot tasks** | Up to 5 Hot or Overdue tasks from all spaces | Select; drag onto the time column | "Nothing is hot." |
| **Mail** | The 3 newest unhandled school threads | Open in Mail | "No new school mail." |
| **Grades** | The lowest course and its letter; "2 new grades to enter" | Open Grades | Hidden until a course exists |

One task is current at a time. You set it with C, by dragging a task onto Now, or by starting focus on a selection. While a task is selected, Get Info slides over the right column in 280 ms (`fade`, 8.6), and Esc slides it back.

### 3.6 Tasks

Tasks is Heat's List view with the gaps filled.

- **When.** A new **When** column (the scheduled date) brings in the PKM's rule: "scheduled (when I'll work on it) is distinct from due (deadline)". Heat still comes only from the due date.
- **Sidebar.** Inbox, All open, Hot, Due this week, Scheduled, Someday, Done. Below them, Projects with their milestones, or Courses, or Areas, then "Your average time".
- **Subtasks** indent under a disclosure triangle. A parent's estimate is the sum of its open children.
- **Recurrence** is stored as an RRULE, as in the PKM ("Every weekday", "Custom…"). Each finished occurrence is a row of its own, and the series never flips to done. A recurring task's heat comes from its next occurrence. These rows show ↻.
- **Timeline.** With a milestone space selected, the bead timeline sits above the list as it does today.
- **Keys.** ↑ and ↓ move the selection. Return edits, ⌘I opens Get Info, ⌘↩ marks done, and ⌫ deletes with Undo.
- **Dragging a row:** onto a Calendar day to schedule it, onto Today's column to make a block, onto a milestone to link it, or onto "Your galaxy" to share it (3.14).

The empty state keeps Heat's line: "Add your first WWAV task and Heat will rank it."

### 3.7 Calendar

The month grid stays as built. Week and Day views are added on the PKM's grid. M, W and D switch views, ← and → page, and T jumps to today. In Week view, an all-day strip holds due pills and milestone beads. A deadline shows as a small heat-coloured flag on the right edge of its column ("due 11:59 PM"), so a block and its deadline read on one line. An unscheduled tray on the left lists this week's open tasks that have no block, in heat order, ready to drag in. Brightspace items are tasks, so they never draw as grey events.

### 3.8 Grades

Grades is global. The header reads the current term from a Term record ("Fall 2026 grades") instead of hard-coded text.

- **Course editor.** Code, name, categories with weights, scale and notes. If the weights don't add up, it says so: "Weights add to 95%. The other 5% is unassigned."
- **Syllabus import.** Drop a syllabus PDF on a course card, and Claude drafts the categories, weights, scale and keywords as a list to check: "Claude found 5 categories adding to 100%. Check them against the syllabus." Nothing is saved until you press Save.
- **Category keywords.** Today the rules that guess a grade's category (exit ticket, Edfinity, kanji, Lab 5a…) are hard-coded. They move onto each category as editable keywords, starting from the current rules.
- **What it would take.** This is plain arithmetic: "To finish with a B (83%), you need 78.4% on the remaining 35%." When the target can't be reached, it says so: "A B is out of reach; the highest possible is 81.2% (B-)."

Pending grades keep their yellow "Enter score" banners. Grades never leave the person's account and never appear in Space. Claude sees a syllabus, never a score.

### 3.9 Habits

Habits is global. It keeps the limit of 6, the orbs, the 14-day grid, the grace until midnight, and the advice: "Keep them small enough that you never skip." The log stops being pruned at 400 days, since a year of daily keys is about 6 KB. "Show the year" opens a 53 × 7 grid. A habit can have a length ("Practise kanji, 20m"). A habit with a length appears under Recurring in Today, can be blocked like a task, and ticks itself when a focus session on it reaches that length.

**Open: the streak counter.** The sketch keeps Heat's "12-day streak". Gate 1.1 names them as a fail: "Anything stretches use past what the person came for: autoplay, infinite scroll, notifications, streaks, 'up next'". The gate-passing default replaces the counter with a record that only grows ("Done 41 days since August 26"), with the grids as the picture. Nothing breaks, so nothing pulls you back to protect a number. *Recommendation:* make the growing record the default, and keep the counter as a per-habit setting that is off by default.

### 3.10 Mail

Mail is a reader for school mail. The main view splits into a 320 px thread list and a reading pane. The sidebar sources are **Brightspace** (Heat's query), **School** (a Gmail label you pick) and up to 5 saved searches. Each row shows the sender and time, then the subject, with chips for the course code and the thread's state: **Grade posted**, **Task made**, or **Nothing to do** (Claude found no deadline). HTML mail renders in a sandboxed view with remote images blocked until you press "Load images".

Each message has one action bar. **Make a task** (T) runs Heat's extraction prompt on that message and opens the task sheet already filled in, with notes that start "From mail:" and link back to the thread. A message with several deadlines gives a short list of drafts with checkboxes. With Claude off, the subject becomes the title and the body becomes the notes. **Open in Gmail** is the secondary action. Mail has no reply, send or delete, because those need wider permissions and Gmail already does them.

Mail uses the app's own Google sign-in with `gmail.readonly`, which Google classes as restricted. While the Google project is in testing mode, sign-in lapses every 7 days, and the app says so plainly: "Google asks you to sign in again every 7 days while Heat is in testing."

### 3.11 Brightspace in the app

Nothing is hard-coded to one school. A School sheet holds the name, the Brightspace host (`brightspace.uri.edu`), the iCal link, the course-code pattern (default `/^([A-Z]{3})\s?(\d{3})/`) and the term dates. The time zone comes from the system, which gives America/New_York for URI without naming it.

| Route | Gives | Needs | When |
|---|---|---|---|
| Brightspace calendar **iCal feed** | Due dates for every course | The student's private link, kept in the Keychain; no approval | Now |
| **Gmail** (`gmail.readonly`) | Grade notices and announcements | The app's Google sign-in | Now |
| **Google Calendar** (`calendar.readonly`) | Other events for the time column | The same sign-in | Now |
| D2L **Valence REST API** or an **LTI 1.3** tool | Real scores, exact due dates, course lists, gradebook weights | Registration by the school's Brightspace admin | Later, with the school |

The iCal path keeps Heat's filters (titles ending " - Due"; skip `/non-graded/i` and cancelled items). They were written against the feed as Google Calendar shows it, so they are checked against a raw feed before shipping. The VEVENT `UID` becomes the task id, and tasks that arrived through Google Calendar are matched by Heat's duplicate test and take it on. An item missing from two syncs in a row gets a grey "No longer in Brightspace" tag and is never deleted on its own. The rhythm and status line stay. Errors stop pointing at claude.ai: "Google sign-in has expired. Sign in again to read Brightspace mail."

Grades are education records under FERPA. Whichever route brings them in, they stay private by default.

### 3.12 Claude in Heat

Claude estimates and drafts. It never decides. Every result arrives as a draft for you to accept, shows its one-line reason, and has a fallback that works without Claude. Calls go through mi-wwav.com (`/api/assist/:task`), which holds the founder's key (see 2 and 9.8).

| Use | Sends | Returns | Without Claude |
|---|---|---|---|
| Score a task (**Exists today**) | Persona, task, your average minutes by type | Difficulty, minutes, reason | The estimate chain |
| Batch-score synced tasks (**Exists today**) | An indexed list | Difficulty and minutes, clamped 5–600 | Per-type defaults |
| Extract tasks from mail (**Exists today**) | The message, today's date and zone, up to 80 existing titles | `{"tasks":[…]}` | Grade notices still parse |
| Syllabus to course | The syllabus text | Categories, weights, scale, keywords | Type them in |
| Weekly review note | Facts Heat computed | A short draft | The facts alone |
| Release plan (3.14) | Title, release date, linked work | Tasks for each phase | An empty three-phase template |

The review prompt carries Ripple's rule, "NEVER invent metrics": Claude may only restate numbers that Heat passed in. Heat does not use Ripple Creator's hype-coach voice. It speaks plainly.

### 3.13 Capture, notes and the weekly review

These come from the PKM (`portfolio/src/pkm/`), which already runs them for one person.

- **Quick capture (⌘⇧N, in every room).** A sheet that stays open for rapid entry and shows "4 in inbox · captured ✓". From the Console, a capture also stores the session and the playhead, so "fix the snare at 1:32" opens at 1:32. Captures wait in the Inbox until they are triaged "→ task", "→ note", "→ project" or "→ upload".
- **Song / Project Brief.** A project can start from the PKM's template: "Vibe / references · Tempo / key · Status: sketch · Sections · Stems / instrumentation · To finish". On Save, Heat offers "Make 4 tasks from To finish?". When the project is linked to a Console session, the tempo and key fill in from it, and the project's dot takes the work's key colour.
- **Weekly review.** A five-step wizard adapted from the PKM's (Inbox → Active projects → Someday → Schedule next week → Done), with Ripple's weekly-report questions folded into the last step:

| Step | What happens |
|---|---|
| 1 Inbox | Triage every capture, ending at "Inbox zero ✓" |
| 2 Last week | Facts only: tasks done and focus time per space, milestones reached, and estimate accuracy ("Homework: estimated 1h 15m, took 1h 32m across 4") |
| 3 Projects | Each active project's next milestone; set it to active, on hold, someday or archived |
| 4 Next week | Drag tasks onto the next 7 days |
| 5 Note | Claude drafts "What moved / What slipped / Next week's one thing" from step 2; you edit it and press "Review complete ✓" |

- **Export.** Heat's share of the whole-app export (see 2) is Obsidian-ready markdown with `[[wikilinks]]`, plus `heat.json` with every record.

### 3.14 Connections

#### Connected profiles

Heat is private. Nothing in it appears in Space unless you put it there, one item at a time:

| Item | Where | Viewers see | Never shown |
|---|---|---|---|
| **Now making** | One line under your name on your bio sun | The text you approved: "Now making: the second verse of More Love" | When it was set, time spent, the task, the space |
| **Project timeline** | The project's sun | Milestone beads with titles, dates, and reached or not | Open-task counts, tasks, estimates |

Sharing is a drop. Drag a task onto "Your galaxy" in the sidebar, edit the line in the sheet ("This line will show on your galaxy"), and press Show. Drag a project onto its solar system, listed under "Your galaxy", to show its timeline. Get Info has the same switch for keyboard use. The Now-making line clears quietly when its task is done, or after 7 days.

Heat never sends Space any productivity number: no focus minutes, done counts, heat, habit records or grades, and no comparison between people. Wi_WWAV left counts out because a count "invites checking and comparing" (gate 1). A milestone timeline describes the work, not the person.

#### Console and Space

- **Links.** A task, milestone or project can point at a Console session, a solar system or a single work (`link: {kind, id}`). Double-clicking the link opens it in its room.
- **Focus follows you.** Start focus on a task linked to a session, and Now offers "Open session". Over the Console, the Now strip keeps reading "Mix the second verse · focus 18:42 left" (see 2), and that time counts toward the same session.
- **Milestones in Space.** A WWAV milestone linked to a solar system draws its bead with a small planet glyph in the system's colour. Reaching the milestone publishes nothing. Publishing stays a drop (see 4. Space).
- **Release plans become projects.** Ripple Creator (`server/routes/rippleCreator.js`, **Exists today**) generates plans with tasks in `pre`, `launch` and `post` phases. In Heat, "New release plan" makes three milestones (Pre-release at −28 days, Release day, Post-release at +28 days) and Claude drafts each phase's tasks. Existing plans import as projects (draft → someday, active → active, completed and archived → archived).
- **Selling.** If a work is dropped on a shelf before payouts are set up, Heat adds a WWAV task: "Finish payout setup so your shelf can open" (see 7. Unquantized).

### 3.15 Data

Heat's records live in the app's local database and sync to the person's account on mi-wwav.com (see 9). Grade rows are the exception: they stay on the Mac unless you turn on encrypted sync (Open, 9.8). The server tables are new, because the PKM's tables have no `userId`. Writes stay optimistic, and the status bar says where things stand: "Saved on this Mac · Synced 3:41 PM".

```ts
Space        { id, name, hue, groupKind: "course"|"milestone"|"free", groupLabel, types[], persona }
Task         { id, spaceId, title, type, courseId?, projectId?, milestoneId?, parentTaskId?,
               due, scheduledDate?, rrule?, difficulty, estMin, adjustMin, notes, link?,
               done, doneAt, source }      // actualMin = Σ FocusSession.focusMin + adjustMin
TaskOccurrence { id, taskId, date, doneAt }
TimeBlock    { id, taskId?|habitId?, date, start, minutes, origin: "you"|"plan" }
FocusSession { id, taskId?|habitId?, startedAt, endedAt, focusMin, interruptions, room }
HeatState    { currentTaskId?, timer: { phase: "focus"|"break"|"idle", round, endsAt } }
Project      { id, spaceId, title, status: "active"|"on_hold"|"someday"|"archived", targetDate?, link? }
Milestone    { id, spaceId, projectId?, title, date, done, order, link? }
Habit        { id, title, minutes?, log: { "YYYY-MM-DD": true }, showCounter: false }
Term / Course / Grade   // as today, plus termId, category keywords, source "valence"
MailThread   { id, source, subject, from, receivedAt, course?, state: "new"|"task"|"grade"|"nothing" }
Capture      { id, text, link?, triagedAt?, resultType?, resultId? }
DailyNote    { date, markdown }
ProfileShare { id, kind: "now"|"timeline", sourceId, text?, targetId, clearsAt? }
```

**Moving in.** The artifact has no export today, so step 0 is a "Download JSON" button on it. The import maps each workspace to a space and keeps every id (event ids, the `em-` and `gp-` hashes, the 400 processed Gmail ids), so the first sync in the app finds nothing new instead of everything twice.

### 3.16 Keyboard

| Key | Action |
|---|---|
| ⌘1 / 1–6 | Heat; then Today, Tasks, Calendar, Grades, Habits, Mail (no field focused) |
| N, ⌘⇧N, ⌘K | New item; quick capture; command palette |
| ↑ ↓, Return, ⌘I | Move; edit; Get Info |
| ⌘↩, ⌫ | Mark done; delete (with Undo) |
| C, P | Make current; plan into the next free gap |
| ⇧Return | The tab's secondary action (3.3): Plan my day, Triage inbox, Today, Add course, Show the year, Sync now |
| F, ⇧F, I | Focus start or pause; stop and log; pulled away |
| M W D T ← → | Calendar views, today, page |
| T (in Mail), ⌘R, ⌘F | Make a task; sync; search |
| ⌘Z, ⇧⌘Z, Esc | Labelled undo and redo; close without losing saved work |

### 3.17 Left out, and why

| Left out | Why |
|---|---|
| Notifications, dock badges, reminders that fire on their own | Anything that reaches out fails gate 1.1; Heat shows what is due when it is open |
| Auto-starting the next round | An "up next" keeps a person past what they came for |
| Ripple's momentum score and daily missions | A productivity score invites checking, and the weekly facts cover the same ground |
| Replying or sending in Mail | It needs wider Gmail permissions, and Gmail already does it |
| Submitting to Brightspace | Writing back needs the school's approval, and planning doesn't need it |
| The PKM's kanban and graph views | The sketch names six tabs; the Obsidian export gives a graph to anyone who wants one |

### 3.18 The look, and open decisions

Heat keeps its Aqua and brushed-metal skin (see 8). Its LCD stays iTunes olive while the Console's screens are DMG green, so the screen's colour says which room you're in. A check against gate 2.4 (body text 7:1, secondary 4.5:1) finds four pairs to fix: the LCD's dim text, the sidebar headings, the third ink, and white on the `#3875d7` selection (4.47:1). The fixes are in 8.2: selected rows fill `#1B4C8C` with white text (8.5:1) and keep a 3 px `#3875D7` bar at the leading edge.

| Open decision | Recommendation |
|---|---|
| **Streak counter** (gate 1.1) | The grids and a growing record by default; the counter is a per-habit setting, off by default |
| **Deadline alerts** (gate 1.1) | None by default. Allow one hand-set alert per task ("Remind me at 9 PM") that fires once |
| **Gmail's restricted scope** | Stay in testing mode (sign in again every 7 days, 100 test users at most) through the small-group stage (10.5), and get verified before the whole school uses Mail |
| **Valence / LTI with URI** | Ship on iCal and Gmail. Ask URI's Brightspace admins to register the app after the small-group stage (10.5), with the group's weekly reviews as evidence, because real scores end the pending-grade guesswork |
| **Control size** (gate 2.4 fails any control under 44×44 px; 26 px rows and 13 px habit squares fail it) | Keep 44 px for buttons, tabs and orbs. Before testing, write a separate rule for dense rows and grids: 24×24 px hit areas (the WCAG 2.2 minimum) and every action on the keyboard |
| **Now-making line** | Build it, off until used. Drop it if it starts to feel like a status that has to be kept up |
| **Who Heat is for** | One account first, with nothing hard-coded to a school, so a classmate could use it next. That is also the clearest route to gate 4 |


## 4. Space

Space is the room where people and their work live. Every person is a galaxy, every project a solar system, every work a planet, and every page a sun. You move through it by zooming, and the player is not a screen you go to but a planet you come close to. The room is named after the first tab of v4's web app. Everything here is **Proposed** unless labelled **Exists today**, **Decided** or **Open**.

### 4.1 Where it comes from

| Source (all **Exists today**) | What Space takes |
|---|---|
| WWAV App v3, DISCMAN, iOS (`ios_v3/`) | the planet player, remix deck, video synth, stem player as identity, lineage Cosmos |
| v4 SOLAR SYSTEM, web (`wwav/`, at `/summer_26`) | the galaxy model, zoom tiers, suns as block pages, ↑ Push, ＋ the figure-eight, lineage with consent, browse rules |
| v5 SOLAR SYSTEM, iOS (`ios_v4/`, `server/routes/v2/`) | Kepler motion, FX moons, gallery planets, film grade forks, the universe map, constellations, the game layer |
| Portfolio Works hub (`portfolio/src/components/Hub.jsx`) | a camera that orbits a system, depth dimming |
| PRANA (`prana/SPEC.md`) | stem order, stem colours, and the DSP the deck runs on |

**Open: which v3.** The sketch calls the social box a "souped up version of WWAV app v3" and calls it intergalactic. In the archive, v3 is DISCMAN, the spring 2026 iPhone app "for music, image, text and video posts"; the galaxy is v4 (web) and v5 (iOS), SOLAR SYSTEM. The `ios_v3/` folder is a music-only build that already carries the summer planet. Space takes from both. Recommendation: read "v3" as "the iPhone app", leave the archive's numbering alone, and give Wi_WWAV its own archive entry.

### 4.2 The model

| Tier | Is | Stored in (**Exists today**) |
|---|---|---|
| Universe ("Everyone") | every galaxy, on a spiral | `Galaxy.universeX/Y` |
| Galaxy | one person: a bio sun and their systems | `Galaxy`, 1:1 with `User` |
| Solar system | one project (an album, a film set, a book, a collection): a sun and up to 21 worlds | `SolarSystem` |
| Planet | one work | `Planet` (song, film, gallery; page is new) |
| Sun | a page | `Sun`, blocks JSONB |

The desktop app uses the iPhone's `/api/v2` routes, adding only the page planet and the letter (4.7, 4.8). **21 worlds to a system**, "the length of the first record". A 22nd is refused: "A solar system holds 21 worlds. Start another one." (**Exists today:** `systems.js`.) Desktop shows all 21, seven to a ring, at radii 640, 1120 and 1580. Worlds shrink outward (200, 170, 145), and each ring is offset half a seat so the rings never form spokes (**Exists today:** `Orbits.swift`). A legacy system of more than 21 gathers the rest into one ringed overflow world: "nothing is hidden, it is gathered."

### 4.3 The room

The sky fills the room, 1280 × 726 pt in the default window. The breadcrumb sits top left ("Everyone › LMY › World Ending", every crumb clickable). The selection's info card sits at the right edge: a 64 pt miniature world, an italic title, "LMY · gen 0", key and BPM chips, and **Play**. **Newest** (4.10) is bottom left, and − · + · fit are bottom right. "Since you last looked" opens at the top when there is anything new (see 2).

Space wears the night: `#070A18`, ink `#F4EFE6`, amber suns, and royal blue `#2946FF` with exactly three uses: armed or active, the current lineage branch, and committing actions (**Exists today:** `tokens.css`; see 8). The secondary act (⇧Return) is **Add** in the sky, **↑ Push** in the player, **Quote and reply** in a reading room.

### 4.4 Moving through it

| Input | Does |
|---|---|
| Two-finger scroll, or drag on empty sky | pan; at the system tier, turn the system about its sun (as on iOS) |
| Pinch, ⌘-scroll, ⌘+ / ⌘− | zoom about the pointer; buttons step ×1.55 |
| ⌥-drag | orbit the camera: pitch 0.35–1.15 rad (default 0.62), 0.006 rad per pt across, 0.004 down (the hub's numbers) |
| Click · Return or double-click | select and show the info card · dive in (fit to 80%) or, on a work, open its player |
| Hold 400 ms on a galaxy | peek: its bio sun inline, without leaving the sky |
| Space | play or pause the selected work in the Now strip, without opening it |
| Tab or arrows · Esc · ⌘↑ | next body in orbit order · up one tier · home to your galaxy |

A drag captures after 8 pt of travel, so it can start on a planet without opening it. A tier changes only when the gesture ends (the last finger lifts, or 180 ms after the last wheel event), which "keeps a pinch from teleporting mid-gesture" (**Exists today:** `useSpaceCamera.js`). In 3D, k is a reference distance over the camera's distance.

| Scene | Enter at k | Leave at k |
|---|---|---|
| System | 1.15 | 0.075 |
| Galaxy | 0.62 | 0.07 |
| Universe | 0.8 | — ("the end of the road") |

Labels follow k at every tier (**Exists today:** v3 `CosmosRenderer.swift`): dots with haloed suns under 0.35, orbs and system names to 0.9, every title above that, and an artist line ("LMY · gen 2") from 1.6.

**The same sky on every machine.** "Nothing about an orbit is stored." Radius, angle, size and tilt derive from a world's seat and id through a stable FNV-1a hash, "never any system RNG" (**Exists today:** `orbits.js`, `StableHash`). Galaxies keep their place forever on a phyllotaxis spiral indexed by galaxy id, so "newcomers land on the rim". Owners may arrange their own systems (nullable overrides). The test: render one catalogue on Mac and Windows builds, hash every position at t = 0, and require a match, as `CosmosLayoutTests.swift` does on iOS.

**Kepler motion** (**Exists today:** `KeplerMotion.swift`). Worlds move on real ellipses (eccentricity 0.04–0.15, tilt 0.05–0.12 rad), and the base ring takes about 70 s.

- **The sky's clock is the audio engine's clock** (**Decided:** the engine is the master clock). Orbits advance only while something plays, so a silent sky holds still and passes gate 2.4 by construction. While a song plays, its planet breathes at the tempo (period 2 · 60 / BPM, scale 1.015).
- **Touching freezes it.** Pointer down stops the clock, and resting on a body eases it to 0 over 240 ms, so every click hits a frozen layout.

**Open: idle motion.** v4's starfield breathes and throws a meteor every 16–38 s with nothing playing, which gate 2.4 fails. Recommendation: still by default, with "Let the sky turn when it's quiet" in Appearance, off.

### 4.5 Real depth

v4 used no 3D library because the DOM gave "free hit-testing, labels and accessibility", and iOS fakes tilt with "a slight vertical squash". The desktop app does depth properly and keeps those three things.

**Rendering.** three.js in the web UI layer, on WebGPU where the system web view supports it and WebGL 2 otherwise. PRISMON already runs three.js ^0.182; Unquantized shares this renderer (see 7).

**What depth adds:** parallax between three star layers and the systems in front; a camera that swings around a system as the hub's does; one light from the upper left (`WWAVLight.sun` ≈ (0.30, 0.25)); opacity 0.45–1.0 by depth (**Exists today:** `Hub.jsx`); a real metaball figure-eight; and gallery photos that foreshorten.

**What stays flat is text.** Every name, chip and card is a DOM element placed at its body's projected point each frame, so text stays sharp, selectable and readable by VoiceOver. Hit-testing runs in world space against the frozen layout. Every body is also a node in an accessibility tree, in orbit order: "Low Tide. Song, A minor, 86 BPM. World 3 of 12 in World Ending." Under Reduce Motion, dives are 140 ms cross-fades and the camera has no inertia.

**Budget.** 60 fps at 2560 × 1600 on an M1 MacBook Air, with up to 2,000 galaxies as instanced points. The frame loop stops completely when nothing moves.

### 4.6 The player is the planet

In v4, "every gesture the product has lives on this screen" (**Exists today:** `PlanetScreen.jsx`); here it is the listening player. Return on a song flies the camera in over 900 ms (`dive`) and the player opens over the scene in 420 ms (`morph`); Esc flies back to the same camera. Elsewhere the same player lives in the Now strip (see 2).

**Layout, on a 560 pt stage.** With `dim = min(w, h) · 0.92` = 515 pt (**Exists today:** the shared spec in `MoonField.jsx` and `PlanetStageView.swift`):

- **The planet** is centred and is the play button. Its radius is 0.17 · dim = 88 pt.
- **Four moons**, 23 pt in radius, sit on a plus sign: vocals up, drums down, bass left, other right, each travelling its arm from 139 pt (level 0) to 242 pt (level 1) from centre. A moon is the pastel sibling of the key colour with a 3 pt rim in its PRANA stem colour: vocals `#D23C2A`, drums `#F0B90B`, other `#2E9A55`, bass `#1F4E9E`.
- **Around them:** the header (84 pt cover, title, chips "remix", "source", "tree", "team"); the 56 pt waveform between the tempo and pitch dials; **↑ Push** bottom left, **＋ add a song** bottom right; the deck in a 200 pt drawer.

"A moon's distance from the planet is its volume. There is no other fader."

**Gestures** are the canonical ones for every stem in the app (**Exists today:** `gestures.js`, `MoonGestureMachine.swift`, `FXMoons.swift`).

| On | Input | Result |
|---|---|---|
| planet | click · hold 400 ms | play / pause · bloom the master bus's FX moons |
| moon | click (under 250 ms and 8 pt) | mute at once. The moon goes "eclipsed", drawn as 8.3's hollow muted ring (v4 dimmed it to 0.45), but keeps its distance, so it remembers its level |
| moon | second click within 250 ms | revert that mute and solo instead, with no double-click delay. The solo wears the blue ring |
| moon | hold 400 ms | four FX moons bloom on the diagonals: reverb upper left, delay upper right, distortion lower left, tremolo lower right. Distance is dry/wet |
| moon | drag along its arm (8 pt slop) · across a bloomed arm | level = `clamp(level0 + (d · armDir) / span, 0, 1)` · pan |
| focused moon | arrows · M · S · Tab | level ±5% · mute · solo · next moon in file order (vocals, drums, other, bass) |

**Dials** turn with a vertical drag (150 pt for the full range, double-click resets). Tempo runs 0.50–2.00× with live BPM and pitch ±12 semitones, both keylocked. R reverses; beat-repeat runs from half a beat to eight bars (v5).

**Waveform.** 400 buckets from only the audible stems, "mute the drums and the drum shape vanishes", against a normaliser fixed to the full mix. Click or drag to seek; hold ⌥ to split it into stem colours.

**↑ Push** "forks your mix into the song's lineage without stopping playback" (**Exists today:** `PushButton.jsx`). It turns blue once the mix differs from its source. A press (or ⇧Return) sends a comet spark off the planet, shows "in the lineage" for 1.6 s, and adds " (fork)" to the title. "A fork owns no audio": it is a few kilobytes of mix state, shaped like a `.wwav`'s `wrmx` chunk, pointing at the parent's stems. It joins the lineage at once and takes a seat in your galaxy only when you drop it on a system.

**＋ add a song: the figure-eight** (**Exists today:** `FigureEight.jsx`, `audio/analysis.js`). The sheet "add a song" lists the galaxy's other songs with BPM and key badges, plus "let the second song go".

- **Shape.** Two metaball lobes on a 45° axis, each in its own key colour, blended at the waist, shimmering "tuning in…" while matching runs. The child's stems take the diagonals, and the FX moons rotate half a step to clear them.
- **Matching.** Tempo from the drum stem's onsets (60–180 BPM, octave-folded), key from the harmonic stems (shifted within ±6 semitones), the downbeat from the first drum onset. "The child bends entirely to the parent," silent until the match lands.
- **On desktop.** Both layers play at full rate (v4's web engine downsampled the child to 22 kHz mono on every device, `wwav/src/audio/OrbitEngine.js`; that is gone), and a saved figure-eight reopens with both layers, fixing v5's "reopens flattened" debt. Two songs at most; pushing one makes a fork with two parents.

**The remix deck** (**Exists today:** `ios_v3/WWAV/Views/PlayView.swift`). Clicking the active pill folds the deck away without rolling back the audio.

| Pill | Deck title | Controls |
|---|---|---|
| fx | "vox." / "master fx." | per-stem dry/wet for four effects, plus four global knobs each; delay 80–540 ms, feedback 0–75%, low-pass 800 Hz–18 kHz |
| eq | "eq · bass" | three draggable nodes at ±18 dB, and "flatten" |
| comp | "dynamics · master" | thresh −40…0 dB, ratio 1–20, attack 0.1–200 ms, release 10–2000 ms, gain 0–24 dB |
| time | "time." | speed, pitch, reverse, beat loops of 1/8 to 16 beats ("looping 4 beats · 2.00s region") |
| rec | "record mode" | arm a stem, then replace or overdub; a 12-segment meter that goes red in the top quarter; "arm a stem · hit record" |

The deck's effects are PRANA's DSP (`prana/core`) in the JUCE engine (**Decided:** one audio core). PRANA is already bit-identical on native, wasm and the Teensy, so a remix sounds the same in Space, the Console and on the device. Holding the planet also turns the deck to the master bus ("master fx.", "dynamics · master"), where **Export** writes a remix `.wwav` (see 6). Arranging and plugins live in the Console (see 5).

**The video synth** (**Exists today:** `VisualSynthSheet.swift`, `VisualSynth.metal`). V turns the sky behind the planet into the synth; ⌘⇧F makes it full screen.

- **Eight styles:** rings, particles, prism, ribbon, kaleidoscope, storm, terrain, orbit.
- **Twelve knobs** on a hardware front panel: character (hue, density, bloom, rotation), motion (speed, glow, contrast, shimmer) and stem drive (vocals, drums, other, bass). Motion comes only from the stems' levels, so a paused song holds its frame.
- **Export visual** writes a `.swav` at 1920 × 1080 or 1080 × 1080, 30 fps, through VideoToolbox; the shaders port from Metal to WGSL once and run both live and on the Rust video side. Its `wlin.parent_id` is the song's `song_id`, so the film is the song's child (**Exists today:** `formats/swav/SPEC.md`). v3's 720 × 720 exporter never had a button; this is that button.

### 4.7 Four media, four kinds of world

| Medium | Family | World | Colour | A reply is | File |
|---|---|---|---|---|---|
| Music | Mi_WWAV | a planet with four stem moons | key colour | a fork | `.wwav` |
| Film | Si_WWAV | a planet that wears a ring, ×1.09 | body from the poster; ring in the soundtrack's key colour | a grade fork or a new soundtrack | `.swav` |
| Writing | Ri_WWAV | a page planet | paper | a quoted fork | **Open** |
| Fashion | Gi_WWAV | a gallery planet, ×1.35, "a place you walk around" | from the cover photo | a "styled from" link | photos and a listing |

**Colour.** A work's colour is its key colour: hue = ((pc · 7) mod 12) · 30 with A = 0, major hsl(h, 72%, 58%), minor hsl(h, 58%, 42%) (**Exists today:** `keyColor.js`). Works with no key take a resonant colour from their image (saturation at least 0.55, lightness 0.42–0.62; **Exists today:** `posterColor.js`). Night indigo, hue 232, "still condensing", stays reserved for music whose key isn't known yet; the Console analyses key and BPM on export (see 5), so new songs arrive coloured.

#### Film

**Exists today:** in v4 a film is a planet that "wears a ring", and opening it pauses the song: "The planet is paused while this plays." In v5 it opens in "the screening room", with "the grade" beneath: saturation, contrast, brightness, RGB balance, and a varispeed that bends sound with picture like tape. On the server a film fork shares its parent's video and differs only by `colorGrade` (0–200, speed 0.25–2×) and an optional new soundtrack, `audioTrackId`.

**On desktop:**

- **The screening room.** The film fills the stage at its aspect ratio, with the grade in a 160 pt deck beneath; F gives "nothing but the film". It plays on the Rust video side, FFmpeg decode presented by wgpu against the engine's clock (**Decided**), so the grade is a real shader whose values open unchanged on the Console's colour page.
- **Forks.** The grade gains RGB curves, 5 points per channel (**Exists today:** `filmScreening.jsx`), and ↑ Push forks it. Dropping a song planet onto a film's ring forks the film with that soundtrack, giving it two parents.

#### Writing: the page planet

Writing has no world today. Suns are already block documents, so a written work is a sun's sibling: a sun is a page about someone, and a page planet is a work.

- **Look.** A pale world in paper (`#F4EFE6`) with fine ink latitude lines at 0.14 opacity, like ruled paper. A recorded reading gives it one moon, the voice, which plays on a press.
- **The reading room.** Return brings the lit face up to fill the stage, and the text rises into one 640 pt column: the title in Cormorant Garamond italic at 46 pt, the body in Inter at 17 pt with 1.6 leading (17.2:1 on night). It ends with the author's name and "That's the end."
- **The document** uses the suns' codec, which keeps keys it doesn't know (**Exists today:** `wwav/src/sun/blocks.js`):

```json
{ "v": 1, "kind": "work", "medium": "writing",
  "blocks": [
    { "type": "text", "style": "heading", "text": "Glass Hours" },
    { "type": "text", "style": "quote", "text": "the drums leave and the room gets bigger",
      "source": { "workId": "9f3c41d2…", "from": 412, "to": 451 } } ] }
```

- **The reply is a quoted fork.** Select a passage and press ⇧Return (**Quote and reply**). A new page opens with the quote as its first block and a fork edge back to the source. In the source, quoted passages carry a thin underline; clicking one lists the replies, newest first, with no count.
- **Where writing is made.** In the suns' editor (4.8); the Console is for sound and picture.

**Open: Ri's file.** Writing has no WWAV format yet. Recommendation: until Ri_WWAV defines one, export a page as Markdown, with `.wwav`'s `wmet` and `wlin` keys in its front matter (`ri: "0.1"`, `work_id`, `parent_id`, `root_id`, `generation`, `creator`). It then opens in any editor (gate 2.3) and joins the id space songs and films already share.

#### Fashion: the gallery planet

**Exists today:** v5's gallery planet is "a big lit sphere the artist has pinned photographs onto", up to 40 of them (`server/routes/v2/galleries.js`), placed at `{lon, lat}` in radians. Photos foreshorten toward the limb and hide on the far side, and the owner's pin mode keeps everyone seeing the planet "dressed the same way".

**On desktop:**

- **Looking.** Drag to spin (0.006 rad per pt) with momentum. ← and → turn the next photo to face you, and Return blows it up.
- **Lookbooks.** A lookbook is a system of gallery planets, one per look, with a sun for the season, materials and credits.
- **Listings.** A gallery planet can carry a listing (**Exists today:** `FashionListing`): size, condition, measurements, "ships from Providence, RI", and a tag reading "$120 · one of one · on the rack". You buy at the rack in Unquantized, under the existing 30-minute hold (see 7).

**Open: what a fashion reply is.** Gi's "pattern file" doesn't exist. Recommendation: for now, a reply is your own gallery planet with a "styled from" link, which waits for consent like any link. Once a pattern file exists, a pattern fork becomes a real fork edge.

### 4.8 Suns and letters

**Exists today (v4):** a sun is a blocks v1 document of text (heading, body, quote), photos (an R2 key) and details (label/value pairs). It opens as a sheet over the scene, so the camera keeps its place; an empty one reads "Nothing written here yet." The editor offers "Add words", "Add a picture", "Add details" and "Add a line", with the placeholder "Say it plainly", and saves the whole document at once, because "its ORDER is most of its meaning."

**On desktop:**

- **Editing.** Your own sun opens straight into the editor; other people's open to read.
- **Freeform layout.** The planned v2 layout is built here: drag a block to place it, a corner to size it, ⌥-drag to rotate. Placements `{x, y, w, h, z, rot}` sit beside the blocks, so older readers still see a stacked flow.
- **Heat lines.** Heat's "Now making" line and milestone beads reach a sun only when you drop them there (see 3).

**Letters.** Every devlog post is a letter, opening "Dear Wi-WWAV," since the second post, where the founder coined the name ("Wi-WWAV (we wave)… we are the wwav"), and signed "LMY". A letter is a page with a greeting and a sign-off:

```json
{ "v": 1, "kind": "letter", "greeting": "Dear Wi-WWAV,",
  "blocks": [ { "type": "text", "style": "body", "text": "Hardware is hard…" },
              { "type": "photo", "imageKey": "devlog/01JA….jpg", "width": 60 } ],
  "signoff": "Sincerely LMY", "sentAt": "2026-10-03" }
```

- **Where letters live.** A bio sun lists them newest first, ten at a time, then "Show older", then "That's everything." Letters from galaxies you've added appear in "Since you last looked"; a letter to one person goes through the message door (4.11).
- **The founder's devlog.** LMY's letters are read from `DevlogPosts`, so the archive, the `/mcp` connector and the app show the same posts. The devlog's image width (a % of the column) becomes the photo's `width`, and writes still go through `readPost` (**Exists today:** `server/routes/devlog.js`).

### 4.9 Lineage is the social graph

"Lineage is the only social graph." (**Exists today:** `wi/README.md`.) There are two kinds of edge (**Exists today:** `server/routes/v2/lineage.js`):

- **Fork edges are facts.** The machine declares them (server `parentTrackId`, `secondaryParentTrackId`, `remixDepth`; the file's `wlin`) and nobody edits them. Push, figure-eights, grades, soundtracks, quotes and visual exports make them.
- **Links are claims.** A link is one of `influence | sample | collab | cover | custom`, with a note of up to 280 characters. One touching someone else's work waits until they accept, "the only thing stopping the graph from becoming a place to attach yourself to strangers". Links inside your own galaxy accept themselves.

**Declaring a link**, which the web never had a UI for: ⌥-drag one planet onto another, pick the kind, add a note, and press **Ask**. The other owner sees "LMY says their planet is an influence of your planet", with **Agree** and **Refuse** (**Exists today:** `PendingLinks.jsx`).

**Seeing it at three scales:**

1. **One family.** The "tree" chip lays the family out as its own system: the root at the centre, forks on generation rings at 90 + (g − 1) · 70, each subtree's arc proportional to its leaves, "so families stay together" (**Exists today:** v3 `CosmosLayout.swift`). Merges are dashed, your branch wears the accent, and holding a node offers "fork from here".
2. **Two galaxies.** A constellation is "a strand of light per tie, thicker and brighter the more work has passed between them" (`ConstellationOverlayLayer.swift`).
3. **Everyone.** Gravity drift pulls remix-tied galaxies together, at most 15 units a day, into a binary orbit 200 apart (`universeDrift.js`). It shows only as a new position; a pair circles only while something plays.

**Across media.** Songs and films already share one id space ("a film made from a song is its child"); page and gallery planets join it, so a book can descend from an album. Whether a sold fork pays its ancestors is a business decision (see 10).

### 4.10 Finding work: the sky and Newest

v5 removed browse: "the sky is the only way through everyone else's work". On desktop the sky stays the main way, plus one list, **Newest**, for the keyboard, VoiceOver and quick looking. It keeps v4's rules (**Exists today:** `Feed.jsx`): one card per screen, and "Nothing plays on its own. A card shows you what a thing IS and waits to be chosen."

- **The card** carries a fact line ("Song · A minor · 128 BPM", "Writing · 1,200 words", "Fashion · 14 photos · $120") and one button: **Play**, **Watch**, **Read** or **Look**. No "N seen".
- **Order and end.** Newest first is the only order, filtered by medium (Mi · Si · Ri · Gi), "Galaxies I've added", key and BPM. 30 cards, then "Show older", then "That's everything."

**Open: near you.** The server already has a local feed: opt-in on both sides, positions snapped to 0.01° (about 1.1 km), creators' coordinates never returned (**Exists today:** `server/routes/local.js`). Recommendation: add it after v1 as a Newest filter.

### 4.11 Social features and the gates

WWAV's earlier apps had likes, follower counts, comments, push notifications and a popularity dial. The founder's gates (`wi/GATES.md`) fail each one; the table gives the default that passes.

| Feature | Exists today | Proposed default | Reason |
|---|---|---|---|
| Likes | `Like`; fuel paid +10 per like | **Add**: a private save to your Saved shelf (`v2/saved.js`), never counted, shown or announced | gate 1.3 names likes |
| Follows | `Follows`, with a push on every follow | **Add galaxy**: a private list that feeds "Since you last looked" and Newest; the person isn't told | gate 1.3 names followers |
| Counts | "N seen", rollups, leaderboards | none, for anyone, owners included | they "invite checking and comparing" |
| Comments | `Comment` | the reply is a work: a fork, a quote, a link or a letter | "The reply to a song is a remix." |
| Messages | mutual-follow DMs with snapshot attachments | the message door opens once two people have added each other. No read receipts, no unread badge | gate 1.1 |
| Notifications | APNs on likes, comments, follows | none; "Since you last looked" (see 2) | gate 1.1 names notifications |
| Popularity | New / Popular, the feed ranker | newest only | "Nothing decides for you what's worth seeing" |
| Stories | 24-hour stories | left out | they exist to be checked before they vanish |

**Open: which philosophy governs Space.** The sketch implies the familiar features; the gates are the founder's newest written word. Recommendation: ship the defaults above. If one returns, let it be Add galaxy made visible to the person added, as one "LMY added your galaxy" line, never as a number.

### 4.12 The game layer

On August 17, 2026, v5 gave the universe weather and an economy (commit `b74d899`). The middle column **Exists today** on iOS and the server.

| Mechanic | Today | Recommendation |
|---|---|---|
| Fog of war | your discovered galaxies, plus 4 unknown glows | drop: it hides work behind effort (gate 2.1) |
| Fuel | 200 to start; +10 per like received, +20 per comment received, +2 per minute listened | drop: it pays for attention (gate 1.3); the ledger stays dormant |
| Travel | 1 fuel per 132 units; wells sized by 30-day views capture flights | keep the flight as a free camera move under 1.2 s, your rocket in front; drop the cost and the wells |
| Wormholes | free, 24 h, about one per 10 galaxies | keep: a door you choose to step through |
| Comets | random: one world in your sky for 6 h; aimed: 25 fuel | keep random; drop aimed: "Free aim would make every galaxy everyone's billboard" |
| Supernovas | 1 h when 30-day views cross 100, 500, 2,000, 8,000 | drop; instead a sun flares once when its system fills its 21st seat |
| Black holes | a day-long tombstone for a deleted account | keep: "the app says so once, plainly" |

Terraforming (six archetypes, three colours, "Let it be natural"), constellations and the astronaut stay as they are.

**Open: whether Space inherits the game layer.** Recommendation: keep the map and the weather that comes from chance; drop everything paid for with attention.

### 4.13 Arriving and leaving

**Connected uploads** (flows in 2). Drag an export from the library drawer (⌘L) onto an empty seat. The planet "condenses in" from 0.82 to full scale while a ring traces the upload (**Exists today:** `Planet.jsx`, where the ring traces the stems' download; here it traces the upload); the server keeps the exact bytes and reads lineage from `wlin`. A `.swav` also gets a streaming copy (H.264 up to 1080p, CRF 23) for phones and the sky; download returns the original (see 6). Going back, drag any planet onto "Console" or press ⌘E (see 5).

**Open: unpublishing a work others have forked.** Recommendation: the planet leaves the sky, forks made before then keep playing its stems, and their trees show "withdrawn by its maker".

**Connected distribution.** A work for sale wears a tag in the sky, "$4 · on the shelf". Clicking it opens Unquantized at that shelf with the record pulled forward; after you buy, the planet reads "yours" (see 7).

**Open: what plays free in Space.** The server gates no streams today. Recommendation: the whole work plays in Space, and what's sold is the file: stems you own, offline, openable in the Console. Hearing a song apart is the first promise. A seller can choose "30 seconds in Space" instead, starting on a press.

### 4.14 School

Gate 4, "Did someone who doesn't consider themselves an artist make something with it?", "fails until it happens".

- **A class is a solar system.** The teacher's sun is the brief, and 21 seats fit most seminars. It is unlisted: members and anyone with its link see it, and it never appears in Newest.
- **Assignments.** A prompt might be "Take one stem out of this song and write four lines about what's left." Responses are forks and quoted forks, so the class's conversation is the assignment's lineage tree.
- **What stays out.** No grade from Heat ever reaches Space; grades are FERPA records (see 3). Accounts keep the 13+ birthdate gate.

**Open: younger students.** Recommendation: no accounts under 13; a teacher plays the class system to the room from their own account.

### 4.15 Left out, and why

| Left out | Why |
|---|---|
| Autoplay and hover previews | "nothing makes a sound until a press"; v1's three-second hover preview fails gate 2.4 |
| Showing who else is looking at a planet | a visible crowd is an attention count |
| Avatars walking in Space | here you travel as your rocket; bodies walk in Unquantized, where shelves give walking a purpose |


## 5. Console

The Console is the room where you make things. It opens with ⌘3. It is one timeline for audio and video, "the best parts of Premiere and Ableton smashed together", with third-party VST3 and AU plugins and MIDI from the first release (**Decided**). It is the software sibling of Mi_pro_WWAV, which the archive describes as a "Desktop studio … instruments and sequencing. Makes discs."

The Console has two verbs, written the way gate 2.1 asks: *making a song that comes apart*, and *cutting a film and its four-stem score on one clock*. Everything in this chapter is **Proposed** unless it is marked otherwise. File layouts are in 6. Files; the engine and video processes are in 9. Under the hood.

### 5.1 What it is built from

Most of the Console already exists in the repo, spread across five products. The Console puts it on one clock.

| Exists today | Where | What the Console takes |
|---|---|---|
| PRANA's engine | `prana/core/dsp`, `prana/SPEC.md` §4 | the stem chain, reverb and delay send buses, soft clip, tremolo, filters, the WSOLA pitch/time shifter, bit-identical output |
| v3's remix deck | `ios_v3/WWAV/Views/PlayView.swift` | four-knob FX, the 3-node EQ graph, compressor ranges, beat loops, "record over" and overdub |
| v4/v5's planet | `wwav/src/planet/*`, `ios_v4/…/FXMoons.swift` | moons, distance as volume, the 250 ms / 400 ms / 8 pt gestures, a waveform built from the audible stems |
| v4's 8-track editor | `wwav/src/data/projectSnapshot.js`, `server/routes/forks.js` | clips that point at stems instead of holding audio |
| MI-WWAV-OS | `MI-WWAV-OS/engine/src/model.rs`, `app/lib/screens/timeline.dart` | Clip / Tag / Sequence, cut at playhead, the OUTPUT row, the undo journal, "what you hear is what you render" |
| v3's video synth | `ios_v3/WWAV/Visuals/Shaders/VisualSynth.metal` | 8 styles and 12 knobs with per-stem drive |
| The grade | `portfolio/src/components/rooms/filmScreening.jsx`, v5 `FilmPlayerView` | saturation, contrast, brightness, RGB curves, varispeed |
| WWAV Push | `vst_plugin/` | a permanent link that later exports update |
| Local Demucs | `ios_v3/demucs_server/` | htdemucs, about 80 MB, 30–60 s per 3-minute song on Apple Silicon |

### 5.2 The window

The Console fills the room area of the shell (see 2): 1280 × 726 pt at the default window size. It has one main view, the arrangement.

```
┌ transport 56 pt ──────────────────────────────────────────────────────────────┐
│ ⏮ ▶ ● ⟲  [ BAR 042.3.120  01:21:07:14 / 92.00 BPM  A MIN  4/4 ]  GRID ○  CPU ▮▮▯ │
├ browser 220 ┬ arrangement ─────────────────────────────┬ viewer 320 × 180 ──────┤
│ Library     │ ruler: bars above, timecode below        │                        │
│ Plugins     │ ▾ Low Tide  ● ● ● ●     (stem group)     ├ inspector 320 ─────────┤
│ Samples     │   vocals ▬▬▬▬▬   drums ▬▬▬ ▬▬▬            │ the selected clip,     │
│ Purchases   │ Keys (instrument)  ▭ ▭▭ ▭                │ track or note          │
│ Takes       │ Film (video)  ▣▣▣▣▣▣▣                    │                        │
│             │ OUTPUT                                   │                        │
├─────────────┴ bottom panel 240: Editor · Chain · Mixer · Planet ───────────────┤
```

| Region | Size | Toggle | Contents |
|---|---|---|---|
| Transport | 56 pt tall | always | return to zero, play, record, loop, the screen, tempo, key, meter, grid switch, engine load |
| Browser | 220 pt | ⌥⌘B | filtered views of the library (see 2) plus your plugins; drag anything onto a lane |
| Arrangement | the rest | always | 196 pt track headers, then lanes 56 pt tall (24–240). The last row is OUTPUT |
| Viewer | 320 × 180 pt | appears with the first video track | the picture at the playhead; ⌥V widens it to half the window; tears off onto a second display |
| Inspector | 320 pt | ⌘I | every field of the selection, in Heat's Get Info drawer style |
| Bottom panel | 240 pt (120–480) | ⌥1–⌥4; the open one again closes it | Editor (piano roll, audio clip, grade, title), Chain (the track's devices), Mixer, Planet |

**Metal outside, Game Boy inside.** The chrome is MI-WWAV-OS's metal (`#FBFCFD`, `#E6EAEF`, `#C9D0D8`, edge `#98A2AD`) with a 1 px pinstripe every 4 px that "reads as texture, never as stripes". Every readout sits behind glass as a DMG-green screen: the transport screen, tempo and key, gain-reduction meters, a track's latency tag. The screens use the four DMG tones `#0B1F0B` / `#0F380F` / `#306230` / `#8BAC0F` with `#C6E24A` glow, in uppercase Menlo dot-matrix. **Exists today:** these tokens and the rule "Aqua is the case; the Game Boy is inside it" (`MI-WWAV-OS/app/lib/style/theme.dart`).

**Colour.** One accent per screen: the Aqua highlight `#3875D7` marks the selection and armed record. Meters are segmented and "DMG green until it is about to be too loud": `#8BAC0F` to −6 dBFS, amber `#F0A32E` to −1, rec `#E0453A` above. Stem lanes wear PRANA's stem colours, and every other track takes one of the six library labels. Drums yellow `#F0B90B` is never used for type, so names on drums lanes are set in ink.

**The status bar is the deck.** As MI-WWAV-OS prints its verbs on the device, the Console prints them in the shell's status bar: "Return Select · ⇧Return Cut at playhead · Esc Deselect · ⌘Z Undo move clip", with "48 kHz · 128 smp · 7.3 ms · saved" on the right.

**One view, no clip-launch grid.** v1 has no second, Ableton-style session view. A launch grid has no meaning for picture, and a second view would double the grammar ("exceptions are how a fourth verb gets in"). Performing happens on the Planet instead (5.8): loops, mutes and FX moons played live with record armed are captured onto the timeline as clips and automation. **Open:** a clip-launch view for live sets. Recommendation: leave it out until one song has been finished in the Console from start to end, then decide on that evidence.

### 5.3 Tracks

| Track | Holds | Header | Notes |
|---|---|---|---|
| Audio | audio clips and takes | arm, M, S, input, monitor, role | mono or stereo |
| Instrument | MIDI clips and one instrument | arm, M, S, MIDI input, role | any VST3/AU instrument, or the built-in Sampler: drop any clip on its keyboard to play it across the keys, or as 16 pads |
| Stem group | four lanes in file order: vocals, drums, other, bass | a 20 pt key-coloured planet, four stem lights | a dropped `.wwav`, or any audio split in place |
| Video | `.swav`, `.mp4`, `.mov` clips and their linked audio | hide, opacity, blend | thumbnails along the lane |
| Titles | text clips | hide | a zero-duration text clip given a length |
| Generator | the video synth, driven by the stems | hide, style | see 5.11 |
| Bus | the reverb and delay returns, and groups | M, S | the two returns exist in every session, as PRANA's shared sends do |

**Every audio and instrument track has a stem role**: vocals, drums, other or bass, chosen from four 10 pt dots in the header that wear the stem colours. A microphone track starts as vocals, a drum-kit instrument as drums, everything else as other. The role decides which moon the track joins in the Planet, and which stem it folds into on export.

**The fold rule** (how N lanes become four stems, as chapter 2 promised):

1. Each stem is the sum of every track with that role, post-fader and post-insert.
2. The reverb and delay returns are rendered once per role, from that role's sends only, so each stem carries its own tail and the four stems sum to the master before the master chain.
3. The master chain shapes the master only, and the export sheet says so.
4. Video, titles and generators never enter a `.wwav`.

### 5.4 Unquantized: timing as you played it

The store's name is a stance in the Console too. **Nothing you play is moved unless you ask.**

- **Grid.** The grid is always drawn. Snapping applies when you move or draw something, because arranging wants bar lines; it never touches what you recorded. G toggles snapping; holding ⌘ while dragging bypasses it once.
- **Click.** The metronome is off until you turn it on, per session. **Open:** whether new sessions start with the click on. Recommendation: off, because a free-time recording can find its tempo afterwards.
- **Tempo after the fact.** Record with no click and the Console estimates the tempo from the onsets of what you played, 60–180 BPM with a prior around 120 (**Exists today:** `wwav/src/audio/analysis.js`). It shows "≈ 86 BPM (estimate)" and offers **Follow what I played**, which bends the bar lines to your performance instead of bending your performance to the bar lines.
- **Quantize is an act.** Q quantizes the selection. Every note keeps the time you played it, so ⌥Q **Return to played** restores it at any point later.

```
note {
  pitch: 0..127, vel: 1..127, release_vel: 0..127,
  at_beats: 12.4871,          // where it sits now
  len_beats: 0.2310,
  played_at_beats: 12.4871,   // where you played it; never rewritten
  played_len_beats: 0.2310
}
```

Today v4 estimates tempo and key only when a song is played, so most works have neither, and v3 asks you to type the BPM. The Console estimates both on import and after recording, and labels each estimate as one ("never invent metrics").

### 5.5 The piano roll

The piano roll opens in the bottom panel (⌥1) when an instrument clip is selected, and drags up to full height.

- **Keyboard and rows.** A 48 pt keyboard on the left auditions on click. Rows are 12 pt (8–24). Rows outside the session key are one tone darker; **Fold to scale** hides them.
- **Notes.** Notes take the track's colour, and velocity is a bar inside each note, never colour alone.
- **Velocity lane.** 64 pt, underneath. Drag a stalk to set one note, or drag across stalks to draw a line through them.
- **Drawing.** D toggles the pencil; ⌘-click draws once. A new note takes the length of the last note you touched. Double-click deletes.
- **Moving.** ↑ ↓ a semitone, ⇧↑ ⇧↓ an octave, ← → one grid step, or 1 ms with ⌥.
- **Quantize sheet** (⇧Q): grid 1/4 to 1/64 with triplets, strength 0–100% (default 100), starts only or starts and ends (default starts only), swing 0–75%. It previews live. Undo reads "Undo quantize 24 notes".
- **Controller lanes.** Pitch bend, mod wheel, sustain and any plugin parameter, added with **+ Lane**.

### 5.6 Recording, takes and controllers

- **Arming.** Click a track's arm dot, or press R with its header focused. Armed tracks monitor through their own effects, as PRANA monitors through the target track's FX. Count-in is 0, 1 or 2 bars (default 0).
- **Takes and comping.** Recording over a loop range makes one take per pass, stacked in a disclosure under the track. Drag across a take to choose that stretch; comp joins get 10 ms crossfades. Takes stay until you empty the trash.
- **Over or with.** On a stem lane, v3's two modes return: "record over vocals" replaces the stem inside the punched range, "overdub" plays with it. Both are new clips on top; the stem itself is never written.
- **Capture.** The Console keeps the last 10 minutes of MIDI you played, armed or not. ⌘⇧R, **Capture what I just played**, makes it a clip at the times you played it.
- **Alignment.** Recorded audio is placed by CoreAudio's reported round-trip latency plus an offset you measure under Settings → Audio & MIDI → **Measure latency**. v5's commit b74d899 calls take alignment "untested on hardware", so the test is written first: a looped-back click recorded 100 times must land within ±1 sample.
- **Camera takes.** A video track can be armed with a camera. Its frames are stamped against the engine's playhead, so a performance filmed to the song plays back in sync. This is the lip-sync in `TODO_2.md`: "it will actually sync audio with the video".

**Controllers.** CoreMIDI devices appear when plugged in: "KeyStep connected · channel 1 → selected track". ⌃M starts MIDI learn: click a control, move a knob. Mappings are global unless pinned to the session. **Open:** PRANA as a USB controller. Nothing in `prana/SPEC.md` says its firmware presents as a USB MIDI device. Recommendation: specify it after Beta 1, since its four faders map onto the four roles one-to-one.

### 5.7 Plugins

**Decided:** plugins run in a separate JUCE engine process, which owns their windows; a crash takes down only the engine.

- **Checking.** On the Console's first open, a short-lived scanner process checks your plugin folders one plugin at a time, with a 30 s limit each: "Checking plugins · 37 of 212". Anything that crashes or hangs is kept out and named once: "Kept out: 2 plugins crashed while being checked. Settings → Audio & MIDI → Plugins lists them, with the reason." Formats: VST3 and AU on the Mac, VST3 on Windows.
- **Using one.** Drag a plugin onto a track, or double-click it in the browser. The Chain tab (⌥2) shows the track's devices as metal cards. Built-ins draw their controls with a DMG screen. A third-party card shows the plugin's first eight automatable parameters (or ones you pick) and **Open window**.
- **Windows.** Plugin windows float above the Console and reopen where you left them. Keys the plugin doesn't use pass back to the app, so Space still plays.

**When the engine crashes.** The session lives in the app; the engine only renders. So the app restarts the engine, reloads the session, and drops a sheet:

> "The audio engine stopped. 'Tape Echo' on the track 'Keys' was running when it did. Restarting…"
> then: "Back. 'Tape Echo' is off until you turn it on. Changes made inside its own window in the last 41 s may be lost."

The buttons are **Keep it off** and **Try it again**. The engine reports plugin state on every stop, every 60 s and before every save, which is where "41 s" comes from. **Open:** one process per plugin. Recommendation: not in v1; log a month of engine crashes per plugin and decide on that.

**Delay compensation** is automatic from each plugin's reported latency, shown as a DMG tag in the header: "+2,048 smp". While a track is armed, **Low-latency monitoring** bypasses plugins over 256 samples on that track's monitor path only ("bypassed while monitoring"); playback is untouched.

**Freeze** renders a track through its plugins and switches them off: "Frozen. 3 plugins off. Unfreeze to edit." **Flatten** makes that audio the track's clip, undoably. A session missing a plugin shows "Missing: 'Grand Piano' (AU)" and plays the frozen audio if there is some, else nothing. Sending a session to someone offers **Freeze plugins they may not have**.

### 5.8 The mixer: strips and a planet

Two views of one mix. Both write the same parameters, so one undo label covers either ("Undo vocals level").

**Strips** (⌥3). Each strip is 76 pt wide: input and output, four visible inserts, sends A (Reverb) and B (Delay), a pan knob, a −∞ to +6 dB fader beside a segmented meter, M, S and arm, and a name plate in the track's colour. Stem lanes and role groups wear their stem colour on the plate and meter cap. The master strip adds the master chain.

**Planet** (⌥4). The session is a planet in its key colour and its four roles are moons, in v4's geometry: vocals up, drums down, bass left, other right. Each moon is that role's sum, and "distance from the planet is its volume".

| Gesture | Does |
|---|---|
| Click a moon | mutes it at once; eclipsed, it keeps its distance |
| Second click within 250 ms | reverts that mute and solos instead |
| Hold 400 ms | blooms the four FX moons on the diagonals (reverb, delay, distortion, tremolo); distance is dry/wet |
| Drag along the arm (8 pt slop) | sets the level |
| Drag across the arm while bloomed | pans (v5) |
| ⌘-click a moon, or Return on a focused moon | enters it: the role becomes the planet and its tracks orbit as moons; Esc steps back out |
| ↑ ↓ · M · S | level ±5% · mute · solo the focused moon |

Inside a role, up to 12 tracks show as moons; more gather into one ringed moon, as v5 gathers worlds ("nothing is hidden, it is gathered"). The planet breathes at the session tempo only while the transport runs. Nothing moves while nothing plays (gate 2.4).

### 5.9 Built-in effects, time and pitch

The built-ins run PRANA's DSP from `prana/core`, wrapped as engine processors. Their controls follow `TODO_2.md`: every effect gets four knobs.

| Device | Knobs (plus dry/wet) | Ranges | Source |
|---|---|---|---|
| Reverb | decay, room size, stereo width, type | room / hall / plate / ambient | `TODO_2.md`; v3; PRANA Freeverb |
| Delay | time, feedback, tone, width | 80–540 ms or synced (dotted 1/8 default), 0–75%, LPF 800 Hz–18 kHz | v3; PRANA ping-pong |
| Distortion | drive, character, tone, output | tone follows drive from 12 to 5 kHz until you turn it | PRANA soft clip |
| Tremolo | rate, depth, shape, phase | 1/8 note default, sine → square | PRANA; v3 |
| EQ | three draggable nodes on a graph | ±18 dB, bezier curve, **Flatten** | v3 `EQGraph` |
| Compressor | thresh, ratio, gain, attack, release | −40…0 dB, 1–20, 0–24 dB, 0.1–200 ms, 10–2,000 ms | v3 `CompressorState.swift` |
| Filter | LPF, HPF | 20 kHz → 200 Hz, 20 Hz → 2 kHz, Q 1.0 | PRANA |
| Limiter | ceiling, release | master only | PRANA's master chain |

**Time and pitch on a clip.** The clip inspector offers **As played** (no stretch, the default), **Follow tempo** (keylocked, following the tempo map) and **Tape** (pitch moves with speed). Every clip also has transpose (±24 st, ±50 cents), **Reverse** (a flag, not a new file) and loop. Loop lengths snap to 1/8–16 beats, the range `TODO_2.md` asked for, and the looped section is shaded yellow. The keylocked shifter is PRANA's: about 30 ms of latency while active, bit-exact at ratio 1, and grainy at the extremes, "as time-domain shifting does".

**Second songs.** Drop a second `.wwav` onto a session and **Match to session** uses v4's figure-eight maths: an octave-folded tempo ratio, a key shift within ±6 semitones through the relative major, and first downbeats aligned. "The child bends entirely to the parent."

**Device parity.** A session at 44.1 kHz whose stems use only built-in devices renders in PRANA's 128-frame blocks and is held to PRANA's golden-hash tests (`prana/tests/golden`), so a disc remix sounds the same on the device and in the Console. The session inspector says which case you are in: "Matches PRANA" or "Doesn't match PRANA: 2 plugins, 48 kHz".

### 5.10 Split anything into four

Select any audio clip and choose Clip → **Split into stems** (⌃⌘S). Demucs (htdemucs, the 4-stem model) runs locally in a background worker, never in the audio engine, and reports the model's real segments: "Splitting 'break.wav' · segment 4 of 12". Intel Macs and Windows machines without a supported GPU run on the CPU, more slowly, and the sheet says so before you start. Only the 4-stem model ships, because a `.wwav` holds four stems.

The clip becomes a stem group in place, roles set, with the original kept muted underneath as a collapsed "source" lane. Undo reads "Undo split into stems", and the export records `splitter: "demucs"`.

**Open:** what a local split costs. v3 meters "splits" because the server pays Replicate; a local split costs the company nothing. Recommendation: local splits are free and unmetered, and the paid server split stays for phones.

### 5.11 Video

**The frame** comes from the first video clip: 1080p or 4K at 24, 25, 30 or 60 fps. The session inspector can change it to 1:1 (the video synth's square) or 9:16 at 1080 × 1920 (v3's unused Composer canvas). FFmpeg's libraries decode, using VideoToolbox's hardware decoder where the codec allows (**Decided**; see 9.5). Footage over 1080p gets half-resolution proxies, built in the background and marked "proxy" in the viewer.

| Act | Mouse | Keys |
|---|---|---|
| Cut at playhead | — | ⇧Return (the timeline's one secondary act) |
| Trim, leaving a gap | drag an edge | ⌥[ · ⌥] trims start or end to the playhead |
| Ripple trim | ⌥-drag an edge | later clips on the track follow |
| Roll | ⌃-drag a shared edge | moves a cut between two clips |
| Slip | ⌥⌘-drag a clip | changes which part of the source shows |
| Ripple delete | — | ⌥⌫ closes the gap |
| Shuttle · in/out | — | J K L (L twice for 2×) · I O |

A video clip's sound lands on a linked audio track with the role "other"; Clip → **Unlink** separates them.

**The grade** (⌥1 with a video clip selected) is a metal panel with a DMG luma waveform scope:

- **Saturation, contrast, brightness**, each 0–200 with 100 as shot, the numbers a film fork's `colorGrade` already stores (**Exists today:** `server/routes/films.js`).
- **RGB curves**, 5 points per channel (**Exists today:** the portfolio's screening room).
- **Varispeed** 0.25–2×. In Tape, sound bends with picture "like tape" (v5); Keylocked holds pitch.

Grades run as wgpu shaders at full frame rate, with **Copy grade** and **Paste grade**.

**Titles** come in three styles: "Name" (Cormorant Garamond italic over an Inter line), "Caption" (Inter, bottom centre) and "Credit" (Menlo capitals, wide tracking). Each has a 0–2 s fade and sits on a 3 × 3 grid or anywhere you drag it. **Captions from lyrics** sends the vocals stem to the existing Ripple analysis, which returns lyrics in timestamped segments (**Exists today:** `server/services/geminiService.js`), and they come back as caption clips for you to correct.

**The video synth as a generator.** A generator track renders the video synth, with its eight styles and twelve knobs (see 4.6). Each drive listens to its role's bus split into low, mid and high bands by one-pole filters, as v3's `StemBandAnalyzer` does, with no FFT. Every knob can be automated, so a chorus can open the bloom. The shaders are ported from `VisualSynth.metal` to wgpu. **Exists today:** v3's MP4 exporter (720 × 720, 30 fps, H.264, 4 Mbps), which nothing in v3's UI calls. The Console renders at the session frame, computing the bands from the mix it exports frame by frame, so the export moves exactly as the preview did.

### 5.12 One timeline model

A session is a Sequence: an edit list, never media. **Exists today:** MI-WWAV-OS, where "No pixels or samples are touched until export." Its `kind` was `mix | edit`; the Console's one kind holds both, which is the Premiere and Ableton join.

```
Sequence {
  id: ULID, kind: "session", title, key: "A minor",
  tempo_map: [{at_beats, bpm}], frame: {w, h, fps} | null, sample_rate: 48000,
  tracks: [{
    id, kind: audio|instrument|stem|video|titles|generator|bus,
    role: vocals|drums|other|bass|null, gain, pan, mute, solo,
    devices: [{format: builtin|vst3|au, uid, state_ref}],   // plugin state is a file in sessions/
    automation: [{param, points: [{at_beats, value}]}],
    events: [{clip_id, clip_in_ms, clip_out_ms, at_ms, at_beats?,
              params: {gain, pan, time: as_played|follow|tape, transpose, reverse, loop, grade}}]
  }]
}
```

- **Cuts are cheap.** "A cut rewrites two numbers on an edit list, which is why it is instant and why undo can take it back for free" (`timeline.dart`).
- **Undo is journalled.** Every change is a transaction that snapshots its rows; deleting a clip removes rows, never media.
- **Exports know their session.** An export is a new clip with `from_sequence` set, and opening it offers **Open the session it came from**: "the edit stays editable forever."
- **What you hear is what you render.** Export runs the engine's own graph faster than real time. A plugin that asks to render in real time gets it, and the export sheet names it.
- **Old projects come in.** A v2 8-track fork imports as two groups of four lanes, A and B, with clips, pitch, rate and effects kept.

### 5.13 Export

Export is not a button. The last row of the arrangement is OUTPUT, as on MI-WWAV-OS: select it and press Return, or press ⌘R, and the export sheet drops.

| Output | Contains | Written as |
|---|---|---|
| `.wwav` | master and four stems by role, metadata, lineage | 44.1 kHz, 16-bit, PRANA's layout (see 6) |
| `.swav` | the picture with the mix as soundtrack | H.264 or HEVC through VideoToolbox, then `wmet` and `wlin` appended |
| Plain | WAV 24-bit at session rate, MP3 320, MP4 | for tools that know nothing of WWAV |

The sheet says out loud everything that changes on the way out:

- "vocals ← Lead vox, Double, Ad-libs (3 tracks)", one line per role
- "This session is 48 kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered."
- "Stems are pre-master. Your master chain (1 plugin) shapes the master only."
- "3:58 → about 210 MB." A `.wwav` holds about 81 minutes, and a longer session is refused with that sentence.

**Lineage is set for you.** `type` is `original` for a new session, `remix` for one that came from someone's planet, `split` for a split song; `parent_id`, `root_id` and `generation` follow PRANA's rules. When one export makes both files, the film's `parent_id` is the song's `song_id`: "A film made from a song is its child" (`formats/swav/SPEC.md`). Progress shows real stages only: "Rendering master · 1:12 of 3:58", "Rendering stems", "Encoding · frame 2,410 of 5,712", "Packing".

**Open:** sessions run at 48 kHz for video while `.wwav` 0.1 is fixed at 44.1 kHz, 16-bit. Recommendation: keep 0.1 fixed and convert on export, saying so, and weigh a 48 kHz `.wwav` in 6, since PRANA reads 44.1 kHz only.

### 5.14 Playback: every file comes apart

The sketch says the Console is "also good for downloaded playback". Double-click a `.wwav` in Finder (the app registers `.wwav` and `.swav`) and it opens in the listening player, the planet (see 2). ⌘E opens it in the Console as a stem group; a purchase opens the same way. The file is never written to: your changes live in a session that points at it.

The **Listen** layout (⌥0) folds the browser, inspector and lanes away, leaving one large planet over a waveform built only from the audible stems ("mute the drums and the drum shape vanishes") and four thin lanes. It is the same session, laid out for listening.

| File | Opens as |
|---|---|
| A remix (`wrmx`) | four lanes with its levels, mutes, FX, pitch, speed, time and filters applied |
| A plain WAV | one lane, "master only", with **Split into stems** beside it |
| A newer major version | the master: "Made by a newer WWAV. Playing the master." |

### 5.15 PRANA discs and the device

**Making a disc.** File → **Make a disc…** writes chosen songs to a PRANA disc (a USB-C flash stick in an acrylic case, `prana/SPEC.md` §2) in PRANA's layout, as loose `NN Song.wwav` files at the root, after checking each against the read rules: "All 9 songs pass. 1.9 GB of 3.8 GB." This is what "Makes discs" means.

**Bringing remixes home.** Plug in a disc that has been in a PRANA: "4 remixes and 6 takes from your PRANA. **Bring them in**". Each remix arrives with its lineage (parent is the song's `song_id`, generation + 1) and its `wrmx` settings on four lanes; takes arrive as take lanes on their stem's track. One thing is said plainly: "The device saves its master without FX or speed. Wi_WWAV plays this remix from its settings." `creator` and `device_id` stay empty until device linking (PRANA's M10, deferred) exists.

**Open:** a PRANA view in the app. The simulator (`mi-wwav.com/prana`, three.js, core in an AudioWorklet) could open as a tear-off window to play a disc before you write it, with PRANA's faders, knobs and round screen. Recommendation: after v1, built natively on the same `prana/core` the engine already hosts, so the preview is the device and not a likeness of it.

### 5.16 The grammar on a keyboard and a gamepad

MI-WWAV-OS's three verbs map one-to-one, so a gamepad can arrange a song.

| Verb | Keyboard | Gamepad | In the timeline |
|---|---|---|---|
| Move | arrows | d-pad | move the highlight between clips and tracks |
| SELECT | Return | A | select the clip at the playhead |
| Grab | hold Return 210 ms | hold A | the clip sticks to the highlight; arrows drag; release drops |
| SHIFT + SELECT | ⇧Return | LB + A | cut at playhead |
| REVERSE | Esc | B | deselect, then out to the track view; never deletes |
| Undo | ⌘Z | LB + B | labelled with what it undoes |
| F1 / F2 | ⌥3 Mixer / ⌥1 Editor | View / Menu | navigate only, never change content |

The 210 ms hold is the OS's "one magic number in the input path". The planet keeps its own 400 ms hold, because blooming FX moons is a different act on a different object.

### 5.17 Push to Space

**Exists today:** WWAV Push gives a song a permanent link on its first export, and "Every subsequent export replaces the audio under the same link". Its v0 left out version history: old stems stayed in storage, unseen.

In the Console, publishing is a drop. Drag the OUTPUT row, or a finished export, onto the "Space" segment or onto one of your solar systems.

- **The first push** makes a planet and its permanent link.
- **Each later push** adds a version under the same link. The session inspector lists them, "v3 · Oct 6 · new bridge", "v2 · Oct 4", "v1 · Oct 2". Listeners hear the newest, and every version stays playable on the work's page.
- **Push every export** carries over from WWAV Push as a switch on the session.

The status bar walks real stages, "Uploading Low Tide · part 3 of 27", then "Up. Low Tide is in your galaxy." There is no Demucs wait: the stems travel inside the `.wwav`, where WWAV Push waited 2–4 minutes for the server to split a bounce. **Ask for feedback** sends a selected range to Ripple, which answers in [MM:SS] and never in Hz, dB or LUFS (see 2).

**Open:** versions and identity. A version keeps the work's `song_id`; a remix gets a new id with a parent; `.wwav` 0.1 has no version field. Recommendation: keep versions on the server and add an optional `version` to `wmet` in 0.2 (see 6).

### 5.18 Walkthrough: from a hum to a planet

1. **Record an idea.** ⌘N makes a session, ⌘T an audio track, which starts as vocals because its input is a microphone. R, then Space: hum over a guitar for 52 s, no click. The screen reads "BAR — · FREE · REC". On stop: "≈ 86 BPM (estimate) · **Follow what I played**". Press it and the bar lines bend to the take.
2. **Arrange with an instrument.** ⌘⇧T adds an instrument track; drag an AU piano onto it. Play chords on the USB keyboard and they land where you played them. Select only the low notes, ⇧Q: 1/8, strength 50%, starts only. Undo reads "Undo quantize 11 notes".
3. **Split a sample.** Drag `break.wav` from Purchases onto a lane and press ⌃⌘S. About 20 s later four lanes sit where the clip was. Mute vocals, other and bass; drag one drum hit onto the Sampler's keyboard and play it as a pad.
4. **Cut a video to it.** Drop a 4K phone clip on a video track; proxies build while you work. Step to bar lines with ← → and press ⇧Return at each. Ripple-delete the dead takes with ⌥⌫. Grade: saturation 140, contrast 115, blue lifted in the shadows. A "Name" title, "Low Tide", over the first 4 s. A prism generator at 30% opacity over the last chorus, drums drive 80.
5. **Export both.** Select OUTPUT, press Return, tick `.wwav` and `.swav`. The sheet reads "vocals ← Hum · drums ← break (drums) · other ← Piano, Guitar · bass ← (silent)" and "3:58 → about 210 MB". Rendering takes about a minute.
6. **Push to your galaxy.** Drag the export onto "Space" and drop it on the system "World Ending". "Up. Low Tide is in your galaxy." The song is a planet; the film is a ringed world that is its child. A week later you push a new bridge, and the same link holds v2.

### 5.19 What the Console leaves out, and why

| Left out | Why |
|---|---|
| A clip-launch grid | one timeline was asked for; **Open**, see 5.2 |
| AAX | it serves Pro Tools only; VST3 and AU cover the rest |
| Surround and Atmos | a `.wwav` is stereo |
| Video stems | "Si_WWAV will define film stems"; a `.swav` holds one picture |
| 6-stem splitting | a `.wwav` holds four stems |
| Auto-mastering | Claude estimates and drafts, never decides; feedback is offered instead |
| Play counts in the Console | the room is for making, and gate 1.3 holds everywhere |
| A paywall on export | "Never gate leaving with your own work" (see 10) |


## 6. Files: .wwav, .swav and the session

Three files carry everything Wi_WWAV makes. A song is a `.wwav`, a film is a `.swav`, and work in progress is a `.wwavsession`. The first two exist today, and the app writes them exactly as PRANA and Wi already do. The third is new. **Decided:** the Console exports both, and the new repo brings PRANA's core and the format code in as a git submodule. Sections 6.1 to 6.4 describe what **Exists today**; from 6.5 on, everything is **Proposed** unless marked otherwise.

Files win over database rows for Wi's reason, "A post is a file", and for gate 2.3: a person can leave with every output, and the outputs work without the company. RIFF WAVE dates from 1991 and ISO BMFF from 2001.

### 6.1 .wwav 0.1: a song as one file

**Exists today:** `prana/SPEC.md` §6; `prana/tools/wwav_pack.py`, the reference; `prana/core/disc/wwav.cpp`, the device; `wi/src/formats/wwav.js`, the browser, which mirrors the reference's verdicts line for line.

A `.wwav` is an ordinary RIFF WAVE whose `data` chunk is the master, so it plays in any WAV player. Everything WWAV adds sits in chunks after the audio, which WAV readers skip.

```
RIFF <size> WAVE                 a 44-byte head: RIFF, fmt, data header
  fmt   44.1 kHz, 16-bit, stereo PCM
  data  the master, 4 bytes a frame
  wmet  metadata JSON
  wstm  16-byte header, zeros to the next 512-byte boundary,
        then frames of VOC_L VOC_R DRM_L DRM_R OTH_L OTH_R BAS_L BAS_R
  wlin  lineage JSON
  wrmx  remix settings JSON (remixes only)
```

Chunks are padded to an even length, as RIFF requires. The pad inside `wstm` puts the first stem sample at a file offset divisible by 512 (`ALIGN = 512` in `wwav_pack.py`), one storage block, so PRANA streams the stems with one read per 1024 frames straight into its 8-channel buffer.

**The `wstm` header** (16 bytes, little-endian, `<HBBBBHII`):

| Offset | Type | Field | Value |
|---|---|---|---|
| 0 | u16 | version | 1 |
| 2 | u8 | stems | 4 |
| 3 | u8 | channels per stem | 2 |
| 4 | u8 | bits | 16 |
| 5 | u8 | reserved | 0 |
| 6 | u16 | pad | zero bytes after the header |
| 8 | u32 | rate | 44100 |
| 12 | u32 | frames | the master's frame count |

**Stem order is fixed:** vocals, drums, other, bass. It is PRANA's fader order, Wi's fader order, and the order of the Now strip's lights and every stem lane and meter in the app (colours in 8).

**`wmet`** keys are written in this order, with `": "` and `", "` between them, so the same song packed twice gives the same bytes:

| Key | Type | Rule |
|---|---|---|
| `wwav` | string | `"0.1"`; its leading digits are the major version |
| `song_id` | string | 32 lowercase hex, 128 bits |
| `title`, `artist` | string | UTF-8 |
| `bpm` | number | optional; 20–400, whole or to 2 decimals |
| `key` | string | optional, e.g. "A minor" |
| `frames` | integer | the master's length |
| `type` | string | `original`, `split` (Demucs stems, from the WWAV disc) or `remix` |
| `splitter` | string | optional, e.g. `demucs` |
| `created` | string | `YYYY-MM-DD`, or "" while the clock is unknown |

**`wlin`** is `{"parent_id": null, "root_id": "<32 hex>", "generation": 0, "creator": "", "device_id": ""}`. An original has no parent, its own id as root, and generation 0. A remix's parent is its song's `song_id`, its root is the song's root, and its generation is the song's + 1. On PRANA, `creator` and `device_id` stay empty until device linking (M10, deferred).

**`wrmx`** holds "the song as you hear it". Levels and effects are fader positions from 0 to 1:

```
{"tracks": [{"stem": "vocals", "vol": 0.800, "mute": false,
             "fx": {"reverb": 0.250, "delay": 0.000, "distortion": 0.000, "tremolo": 0.000}},
            … drums, other, bass …],
 "master": {"vol": 1.0, "pitch": 0, "speed": 1.000, "time": 1.000, "lpf": 1.000, "hpf": 0.000},
 "mode": "stems", "start": 0, "length": 10495800}
```

`pitch` is in semitones; `speed` (varispeed) and `time` are ratios; `lpf` is 1.0 when open; `master.vol` is always 1.0. On PRANA, SHIFT + REC writes the four stems as they play (with takes mixed in), these settings, and a `data` master summed at your levels and mutes **without** FX, speed, pitch, time or filters.

#### Ids

- `song_id` is 128 random bits: from the Teensy's TRNG, the browser's `crypto`, or a fixed seed in the parity harness.
- A song that came from the platform takes its id from its track, "so a song has the same id everywhere and its remixes share a parent" (`prana/SPEC.md` §12; computed in `prana/web/src/sim/wwavdisc.js`):

```
h = 0x6c62272e07bb014262b821756295c58d                 // FNV-1a 128 offset basis
for each UTF-8 byte b of "wwav-track:" + trackId:
  h = ((h XOR b) * 0x0000000001000000000000000000013b) mod 2^128
song_id = h as 32 lowercase hex
```

#### How a reader degrades

Readers never guess. The reference tool and Wi's reader give these verdicts word for word, and the device acts on the same rules:

| Condition | Verdict |
|---|---|
| the master isn't 44.1 kHz 16-bit stereo PCM | "not listed: the master isn't 44.1 kHz 16-bit stereo PCM" |
| no `wmet` and `wlin` | "the master only: no wmet and wlin, so a plain WAV" |
| a major version above 0 | "the master only: version 1.0 is newer than this reader (0.x)" |
| `wstm` missing, not v1 with 4 stereo 16-bit stems at 44.1 kHz, cut off, or its frames differ from `data`'s or `wmet`'s | "the master only: …" and the reason |
| all of it holds | "4 stems, and the master" |

The app's file inspector (⌘I) shows the same verdict for every `.wwav` in the library.

#### Size

| Part | Bytes a frame | Per minute |
|---|---|---|
| master | 4 | 10.6 MB |
| stems | 16 | 42.3 MB |
| a `.wwav` | 20 | 52.9 MB |

A 3:58 song is about 210 MB. The packer refuses anything over 4 GB, the RIFF limit, which caps a `.wwav` at about 81 minutes. **(verify)** Some older WAV readers treat sizes as signed and stop at 2 GB, about 40 minutes of `.wwav`.

**Round trip.** `wwav_pack.py unpack` writes the song folder back with the `song_id` in `song.txt`, and `pack` gives back the same bytes.

### 6.2 .swav 0.1: a film as one file

**Exists today:** `formats/swav/SPEC.md`, `formats/swav/swav_pack.py`, `wi/src/formats/swav.js`.

```
[ftyp] [moov] [mdat] …   any ISO BMFF film (.mp4, .m4v, .mov), untouched
[wmet]                   metadata JSON
[wlin]                   lineage JSON, exactly a .wwav's
```

- The two boxes are appended, `wmet` first, and nothing may follow them. MP4 chunk offsets (`stco`, `co64`) are absolute, so anything inserted before `mdat` would break them. Appending moves nothing. The one change: a last box of size 0 gets its real size written first.
- `wmet` is `{"swav": "0.1", "film_id": "<32 hex>", "title", "artist", "type": "original", "created"}`, in fixed order and spacing.
- **Songs and films share one id space.** A film's `parent_id` can be a song's `song_id`: "A film made from a song is its child."
- **Reading** takes the first `wmet` and `wlin`. Both must be JSON objects with `swav` at major version 0, or the file is a plain MP4 with no identity. ffprobe reports the same streams, ffmpeg 6.1 decodes it without a warning, and Chromium plays VP9 and Opus. **Unpacking** returns the MP4 byte for byte.
- **Limits:** remuxers drop unknown boxes, so "the film survives, its identity doesn't"; there are no film stems ("Si_WWAV will define film stems"); and pack writes originals only.

### 6.3 Every chunk and box

| Name | In | Holds | Required |
|---|---|---|---|
| `fmt ` | `.wwav` | 44.1 kHz, 16-bit, stereo PCM | yes |
| `data` | `.wwav` | the master | yes |
| `wmet` | both | identity JSON | for identity |
| `wstm` | `.wwav` | four interleaved stereo stems | for stems |
| `wlin` | both | lineage JSON | for identity |
| `wrmx` | `.wwav` | remix settings | remixes only |
| `ftyp`, `moov`, `mdat`, … | `.swav` | the film, untouched | yes |
| `wgrd` | `.swav` | the grade and soundtrack as a recipe | **Proposed** for 0.2 (6.10) |

### 6.4 What plays where

The column marked **(verify)** is untested. ffmpeg, the pack tools and wi-wwav.com are tested in `wi/GATES.md` (2.3) and the format specs; PRANA's column is built in its simulator; the app's column and the session row are **Proposed**.

| File | QuickTime, a DAW **(verify)** | ffmpeg, ffprobe | pack tools | PRANA | wi-wwav.com | Wi_WWAV app |
|---|---|---|---|---|---|---|
| `.wwav`, original or split | the master | the master, no errors | "4 stems, and the master" | stems | stems | stems |
| `.wwav`, remix | its master | its master | stems and settings | stems with settings | stems; settings not applied | stems with settings |
| `.wwav`, master only | the master | the master | "the master only" | the master | the master | the master, with **Split into stems** |
| `.swav` | the film | the film | identity and lineage | — | the film | the film and its family |
| `.wwavsession` | `media/` files play | — | — | — | — | the session |
| a 48 kHz `.wwav` (not proposed) | plays | plays | "not listed" | not listed | not listed | not listed |

The last row is why the format stays at 44.1 kHz (6.7).

### 6.5 The session: `.wwavsession` (Proposed)

A session is the Console's edit list (see 5.12) plus everything it needs to open on another machine: a folder with an extension. On the Mac it is a package, one icon in Finder. In the library it lives at `sessions/<ULID>.wwavsession`; **Save a copy…** and **Send session…** write it under its title.

```
Low Tide.wwavsession/
  session.json      the session
  media/            every file a clip points at, ULID-named
  plugin-state/     plugin states over 256 KB, ULID-named
  renders/          exports and freezes made from this session
  journal/          the undo journal and autosave history
  cache/            peaks, video proxies, analysis; safe to delete, rebuilt
```

**`session.json`** is UTF-8 with LF line endings, a two-space indent and keys in a fixed order, so saving the same session twice gives the same bytes and a session kept in git diffs line by line.

```
{
  "wwavsession": "0.1",
  "id": "01JC5Q8V3M2T7R9X4K6W0YHZNB",
  "title": "Low Tide", "key": "A minor",
  "sample_rate": 48000, "frame": {"w": 1920, "h": 1080, "fps": 24},
  "tempo_map": [{"at_beats": 0, "bpm": 86.0}],
  "buses": ["vocals", "drums", "other", "bass"],
  "tracks": [ …role, gain, pan, devices, automation, events, as in 5.12… ],
  "midi": { "<clip ULID>": [ …notes, with played_at kept, as in 5.4… ] },
  "video": { "edits": [ … ], "grade": { … }, "titles": [ … ] },
  "lineage": {
    "work": {"song_id": "9f2c…", "film_id": "41ab…", "version": 3},
    "from": {"song_id": "e07d…", "version": 1, "title": "glass hours", "sha256": "…"},
    "exports": [{"file": "renders/01JC….wwav", "version": 3, "sha256": "…",
                 "at": "2026-10-06T21:14:03Z"}]
  }
}
```

- **Clips are edit lists.** An event names a media file by ULID, with in, out and position: "No pixels or samples are touched until export" (**Exists today:** MI-WWAV-OS). A dropped `.wwav` stays one file, and its four stem lanes read it in place.
- **Media costs nothing twice.** In the library, `media/` holds APFS clones of the library's files, so a 210 MB song used in three sessions takes 210 MB. Media is never written after import, so Windows uses hard links. On exFAT the files are copied, and the sheet says so: "Copying 1.4 GB of media into the session."
- **Plugin state** records the plugin and the version that saved it:

```
{"format": "vst3", "uid": "<32 hex class id>", "name": "Tape Echo", "version": "2.1.4",
 "state": {"inline": "<base64>", "bytes": 18432, "sha256": "…"}}
{"format": "au", "uid": "aumf:TpEc:Vndr", "name": "Grand Piano", "version": "1.3.0",
 "state": {"file": "plugin-state/01JC….bin", "bytes": 41943040, "sha256": "…"}}
```

States up to 256 KB sit inline as base64; larger ones, such as a sampler's 40 MB of mappings, are files, so an autosave never rewrites them unchanged. An older installed version is named: "Saved with 'Tape Echo' 2.1.4. You have 2.0.0, so its settings may not load." A missing plugin plays its frozen audio (see 5.7).

- **The lineage block** says what the session will become. A session reserves its `song_id` and `film_id` when it is made, so every export of it is a version of one work (6.8). `from` records a forked parent's version and sha256, so the exact parent can be found after its maker pushes a new one.

**Autosave and the undo journal.**

- Every change is a transaction recording the affected parts of the session before and after, as MI-WWAV-OS's `Txn` does, appended to `journal/undo.ndjson` at once. ⌘Z says what it will undo: "Undo move clip". The journal survives quitting, so the label is the same after a relaunch.
- `session.json` is rewritten 2 s after the last change and at every transport stop, to a temporary file that is then renamed, so a crash leaves the old file or the new one, never half of one. On open, newer journal entries are replayed: "Recovered 14 changes from 21:12."
- Deleting a clip removes it from the edit list, never from `media/`. **Clean up media…** lists unused files with their sizes and moves them to the trash on a press.
- `journal/` keeps a copy of `session.json` every 10 minutes of work for 30 days, under **File → Revert to**. ⌘S writes at once and says "Saved", though nothing ever needs it.

**On Windows** a `.wwavsession` is a plain folder. File → Open, dragging it onto the app, and a right-click **Open in Wi_WWAV** all open it. Names inside are 26-character ULIDs, so paths stay under Windows' 260-character default. Titles become folder names with `\ / : * ? " < > |` replaced by spaces, as PRANA's disc writer does.

**Sending.** **Send session…** writes one `Low Tide.wwavsession.zip`, stored rather than compressed (audio doesn't compress), without `cache/`, and offers to freeze plugins the other person may not have. Without the app, a session is still a folder of every recording you made and one JSON file any text editor reads.

### 6.6 Four stem buses: how N tracks become four stems (Proposed)

A `.wwav` holds four stems; a session holds any number of tracks. The rule that joins them lives in the mixer, not the export, so what you hear while working is what the file holds.

- **Every track routes to one of four stem buses**, vocals, drums, other or bass, by its role (see 5.3). A track routed into a group takes the group's role, and its role dot says so. Nothing is unassigned: a microphone track starts as vocals, a drum instrument as drums, lanes from a `.wwav` keep their stems, and everything else starts as other, including a video clip's sound.
- **The buses are the stems.** Each stem bus has its own copy of the reverb and delay returns, fed only by its own tracks' sends, so each stem carries its own tail: "Returns: 2 × 4 stems".
- **The master is their sum**, then the master chain. Stems are written before the master chain and the master after it (see 5.13).
- **In the mixer** (⌥3) the four stem buses stand between the tracks and the master, 96 pt wide against a track strip's 76, capped and metered in their stem colours, with a member count: "vocals · 3 tracks". Clicking a bus lights its tracks. The master strip reads "Σ 4 stems → master chain". A bus with no tracks reads "silent" and is written as silence.
- **The fold check.** On export the app subtracts the summed stems from the master with the master chain bypassed. Under −80 dBFS the sheet says "Stems sum to the master." Above it, it names the cause, usually a non-linear plugin on a return.
- **Clipping is said out loud.** A stem bus over 0 dBFS can't be written at 16 bits: "drums peaks at +1.8 dBFS. **Lower all four stems by 2 dB**." The master is unchanged, and the sheet says a player summing the stems will play 2 dB quieter.

### 6.7 Sample rate, and conversions said out loud (Proposed)

- **New sessions run at 48 kHz,** the rate of video. A session opened from a `.wwav` runs at 44.1 kHz, so a remix makes the round trip without conversion and can match PRANA (see 5.9). Clips at another rate are resampled as they play, and their header says "48 kHz → 44.1 kHz".
- **A `.wwav` is always 44.1 kHz, 16-bit.** Export resamples with a band-limited polyphase filter, then adds TPDF dither once, at the last step, without noise shaping, so the four stems' dither sums as plain noise.
- **Wi's rule, kept:** "Converting would quietly change the audio." Wi refuses; the app converts only on a press, and says what will change first.

| When | What changes | What the app says |
|---|---|---|
| exporting a `.wwav` from 48 kHz | rate and bit depth | "This session is 48 kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered." |
| exporting a `.wwav` from 44.1 kHz | bit depth | "The .wwav will be 16-bit, dithered." |
| exporting a `.swav` | the sound is encoded | "The film's sound is AAC at 320 kbps. The .wwav beside it holds the lossless stems." |
| posting a 48 kHz, 24-bit WAV | rate and bit depth | "This is 48 kHz, 24-bit. Posting needs 44.1 kHz, 16-bit." **Convert a copy** (the original stays) |
| a session over 81 minutes | refused | "A .wwav holds about 81 minutes. This session is 94." |

**Open: a 48 kHz `.wwav`.** It would save film work one resample, but PRANA, the pack tool and Wi would not list it at all, because the master's format is their first check. Recommendation: keep 44.1 kHz, 16-bit for all of 0.x, and decide 48 kHz and 24-bit together for 1.0, once PRANA's hardware has been measured.

### 6.8 Versions, remixes and forks (Proposed)

| Word | What it is | Id | File |
|---|---|---|---|
| **Version** | a new export of your own work | the same `song_id`, `version` + 1 | a `.wwav` with the same identity |
| **Fork** | a work taken somewhere new, by anyone | a new `song_id`, `parent_id` = the source | a remix `.wwav` |
| **A fork in Space** | made with ↑ Push in the player; owns no audio | a new id, on the server | none until exported (see 4.6) |

**Versions.** WWAV Push overwrote the audio under a permanent link and hid the old stems (**Exists today:** `vst_plugin/README.md`). Here each version is kept unchanged, with its own sha256, and the link plays the newest. Version 0.1 has no field for this, so **`wmet` 0.2** adds one optional key after `created`, `"version": 3` (absent means 1), and **`wlin` 0.2** adds `"parent_version"`. It is a minor bump: every 0.x reader still lists the file and plays its stems, because readers accept any JSON object and check only the major version.

**A remix from the Console** is any export from a session with a `from`. It is written with `type` `remix`, a new `song_id`, `parent_id` = the source's `song_id`, `root_id` = the source's root, generation + 1, and `creator` = your account name at export. `device_id` stays "": the desktop app isn't a device, and a per-install id would be a tracking number with no use. The export sheet's one secondary action switches between two ways of writing it:

- **As settings**, when the four stem lanes are the parent's stems untouched (plus takes) and every change is one PRANA has: level, mute, the four FX, pitch, speed, time, filters. `wstm` holds the stems and `wrmx` the settings, as PRANA writes them, so the remix reopens with every control where you left it, on the device too.
- **Baked**, for anything else (new tracks, plugins, edits, automation). `wstm` holds the four rendered stem buses, with no `wrmx`: "This remix has 2 new tracks and a plugin, so its stems are rendered. It sounds the same everywhere."

**The master keeps the stems' length.** Readers use the stems only when `data`, `wstm` and `wmet` agree on the frame count, and speed and time change the length. So a settings remix's master carries levels, mutes, FX, pitch and filters, with speed and time left at 1.00: "The master for other players is at 1.00×. Wi_WWAV and PRANA play it at 1.25× from its settings." PRANA's master leaves speed out too, which keeps its frame count. A baked remix renders speed into everything.

**Ids on the server.** The first upload of a `song_id` claims it for that account. The same id from the same account with a higher `version` is a new version; the same sha256 again does nothing ("Already up"); the same id from another account is refused: "This file's id belongs to another work, 'Low Tide' by LMY." Platform tracks keep their FNV-1a ids, so a WWAV Push track's versions and remixes still meet. **Exists today:** Wi's server keys each post by `workId`, `parentId` and `rootId` read from the file (`server/routes/wi.js`).

### 6.9 Thin remixes: PRANA's "Step 5" (Open)

A remix carries full stems at 53 MB a minute, even when it only moved four faders. PRANA's draft had a Step 5, never built: a remix placed as a song loads its parent's stems. Space's forks already work like this on the server: "A fork owns no audio" (**Exists today:** `server/routes/forks.js`).

Two facts narrow the choice:

- A settings remix with no takes has a `wstm` whose audio is byte for byte its parent's. The library and R2 can store that range once, keyed by its sha256, and assemble the whole file when it leaves, checked against the whole file's sha256.
- A thin file (master and settings, no `wstm`, about 10.6 MB a minute) plays its stems only where its parent already is. Leaving with it means leaving with something that needs the company, which gate 2.3 fails.

**Open:** thin remixes as files. Recommendation: thick files wherever a file leaves (exports, downloads, purchases, Export everything, PRANA discs) and thin storage inside the library and on the server. The format doesn't change, and each set of stems is stored once.

### 6.10 .swav 0.2 (Proposed)

- **Remixes.** `type` gains `remix`, and pack and the app write `parent_id`, `root_id` and generation for films as `.wwav` does. A film cut in the Console to a song is born that song's child.
- **The grade as a recipe.** A film fork on the server shares its parent's video and differs by `colorGrade` (saturation, contrast, brightness, red, green, blue at 0–200, speed 0.25–2×) and an optional new soundtrack (**Exists today:** `server/routes/films.js`). A `wgrd` box after `wlin` carries that recipe and the soundtrack's `song_id`; "nothing comes after them" becomes "only WWAV boxes come after them". Generic players show the original picture and Wi_WWAV applies the grade. **Bake** renders a new film instead.
- **Open: film stems for Si_WWAV.** The app encodes its own exports, so it can write a layout it must never impose on a received file: one composite video track, enabled, which every player shows, plus up to four layer tracks marked disabled in their `tkhd`, which players skip **(verify)**. The layers follow the Console's track kinds (plate, subject, titles, generator) and carry alpha, as HEVC with alpha through VideoToolbox. The composite is the master; the layers are the stems. Recommendation: build it after one film has been cut in the Console and someone has asked to take a film apart.

### 6.11 Words and garments (Open)

| Family | Placeholder | Shape | Opens without WWAV in | A fork is |
|---|---|---|---|---|
| Ri_WWAV, writing | `.rwav` | Markdown with `wmet` and `wlin` keys in its front matter (`ri: "0.1"`, `work_id`, `parent_id`, `root_id`, `generation`, `creator`), as 4.7 recommends | any text editor | a quoted fork |
| Gi_WWAV, garments | `.gwav`, the "pattern file" | SVG pattern pieces, with the same JSON in SVG's own metadata element; DXF-AAMA for cutting tables as an export | any browser | a pattern fork |

Both join the shared id space, so a book can descend from an album. Until they exist, writing and fashion travel as Markdown, photos and listings with identity in the database, the one place where "a post is a file" doesn't hold yet. Recommendation: keep `.md` for writing until a page must carry its images in one file, and name Gi's file when a Gi_cro_WWAV prototype cuts its first piece ("One pattern file works on all three tiers").

### 6.12 Files in the store (Proposed)

- **Unquantized sells exact bytes.** The buyer downloads the file the creator dropped on the shelf, byte for byte. The server never remuxes, transcodes or re-tags it, so a `.swav` keeps its identity and a `.wwav` its stems.
- **No DRM and no per-buyer watermark.** Either would change the bytes. Every buyer gets the same file, and its sha256 is on the receipt.
- **Downloads check themselves.** The app hashes the file as it arrives: "Verified. sha256 matches the receipt." A mismatch discards the download, tries once more, then says what happened.
- **Each thing sold is a file the creator made.** ".wwav, master only" (see 2) is written on the creator's machine, with the same `song_id`, never stripped by the server. A `.swav`'s streaming copy (see 4.13) and the booth's previews are never sold or downloaded.
- **Ownership is checked on the server.** Today streams answer without a purchase check, and a pending purchase counts as owned (**Exists today**, a gap: `server/routes/films.js`, `purchase.js`). The download route checks for a completed purchase before it signs a URL.
- **A purchase covers every version.** Buy "Low Tide" at version 3, and version 4 arrives when its maker pushes it; version 3 stays. **Open:** selling a version separately. Recommendation: no; a version is a fix, and a new work gets a new id.
- The $1 stem download, a ZIP of four stems and the original (**Exists today:** `server/routes/stems.js`), retires: the `.wwav` is the stems.

### 6.13 Tests written before the code

Rule 2 says to write down what a fail looks like before testing. Each of these fails as stated:

1. **Byte parity.** An original exported by the app, unpacked by `wwav_pack.py` and packed again, has a different sha256 (pack writes originals only). The same for `.swav` and `swav_pack.py`.
2. **One verdict.** Any of the four readers (Python, PRANA's C++, Wi's JavaScript, the app's Rust) gives a different sentence for any file in one shared test corpus.
3. **Plays without us.** ffprobe or ffmpeg reports an error on any export.
4. **The session.** Save, quit and reopen changes `session.json` by a byte or changes ⌘Z's label; killing the app mid-edit loses a journalled change.

### 6.14 What the files leave out, and why

| Left out | Why |
|---|---|
| Compression inside `wstm` | PRANA streams raw frames, one read per 1024, and every WAV tool reads PCM |
| More than four stems | PRANA has four faders, and the mixer has four stem buses |
| A 24-bit or 48 kHz `.wwav` | no reader today would list it (6.7) |
| DRM and watermarks | they change the bytes (6.12) |


## 7. Unquantized

Unquantized is the store. The sketch calls it "a 3D walkable video/record store for direct-to-consumer creative digital media", with coffee-shop ambiance and Mii-like characters, "where you can walk around infinitely and purchase digital goods". In the family model it is the third word of "a device, a format, and a marketplace": the Console makes the work, `.wwav` and `.swav` hold it, and Unquantized is where its maker sells it. It is also the only route to closing gate 1.4, "the business works if 1.1–1.3 are true", which stays open until a business exists.

Everything in this chapter is **Proposed** unless labelled. Most of the money machinery **Exists today** on the server, and it needs the fixes in 7.20 before it can carry a launch.

### 7.1 What it is built from

| Piece | Exists today | Becomes |
|---|---|---|
| PRISMON first-person engine | `portfolio/src/components/rooms/prismon/game/Engine.js`: capsule player, pointer lock, doors, E to interact, bloom and film grain | the walk, the doors, the look-and-press loop |
| PRISMON artifacts | `game/Artifact.js`: a stack of typed pages, a lit audio sphere, a garment on a floating rail, a framed board | writing, records, clothes, posters |
| Kepler orbits, soft repulsion | `portfolio/src/components/Hub.jsx` | where booths stand, and how they move between visits |
| Astronaut | `Users.astronaut`; `ios_v4/WWAV/Models/AstronautModels.swift` | your body, and each maker's figure |
| Checkout, payouts, listings | `server/routes/purchase.js`, `connect.js`, `fashion.js`, `shipping.js` | the counter, your booth, the rails |
| Made Archive | `server/models/MadeArchiveItem.js`, typed film, music, writing or clothes | the four halls |

### 7.2 The door

⌘4 opens Unquantized on the pavement outside the shop at dusk. The frame is still. The front is oak and glass, 9 m wide, lit from inside in lamp glow `#FFE9C8`. On the left pane, in gold leaf, "Unquantized" is set in Cormorant Garamond italic over Jost capitals tracked 0.34 em: "MUSIC · FILM · WRITING · FASHION". Through the glass you see the counter and the first shelves, and nothing moves. Your rocket is parked at the kerb. The door says "Press E to open." (A on a gamepad.)

The press swings the door open over 600 ms (PRISMON's `Door`, which today eases at a damping rate of 2.2; the 600 ms is new), brings the room sound up over 1.2 s, and walks you in. The sound has three placed layers: the air of the room, the espresso machine at the counter (loudest within 2 m), and cups and the grinder every 20–70 s. There are no voices, because a murmur would imply a crowd that isn't there (7.5). Steam, lamp flicker and the makers' breathing start with the sound.

**Why only after a press.** Gate 2.4 fails "anything moves on screen while nothing is playing, or anything plays without a press", and Wi's rule is "nothing makes a sound until a press". The door is that press. Sound and motion are one switch: the speaker glyph in the lower-right corner turns both off, the room returns to its still frame, and the store stays fully usable. Room sound runs 0–100% in Settings, default 40%. Leaving, switching rooms, or five minutes without input stops it.

**Open: ambiance.** The sketch asks for it; the gates fail sound or motion that starts on its own. Recommendation: the door press starts it, as above.

### 7.3 The floor

In the Console, "unquantized" means nothing you play is moved unless you ask (see 5.4). In the store it means nothing stands on a grid.

- **The plaza.** A round room 14 m across. The counter stands in its middle under an oculus, a 4 m opening onto the evening sky, after the portfolio's Turrell crater. The New table stands just inside the door, with the corkboard of event flyers beside it.
- **Four halls** open off the plaza through 4 m arches: music (Mi) ahead right, writing (Ri) ahead left along the window wall, film (Si) to the right, fashion (Gi) to the left. The window is on your left as you enter, so light falls from the upper left, where `WWAVLight.sun` (0.30, 0.25) puts it on every orb.
- **A hall is a solar system.** Its centrepiece is its sun: the listening booths in Mi, the reading table in Ri, the screening room in Si and the fitting mirror in Gi. Booths stand around it on elliptical tracks.

| Quantity | Value |
|---|---|
| Track n, semi-major axis | 5 m + 3.2 m · n |
| Track eccentricity | 0.04–0.15, hashed per track (v5's range) |
| Booth | 2.4 × 1.2 m footprint, 2.2 m tall |
| Small shop (4 items or fewer) | one shelf, 1.2 m wide |
| Spacing along a track | 3.6 m, centre to centre, with a 2 m aisle in front |
| Clearance kept by soft repulsion | 1.2 m between footprints |

A hall with 100 shops is about 42 m across, 10 s on foot at PRISMON's 4.4 m/s. A hall with 1,000 shops needs 18 tracks and is about 125 m across: 28 s walking, 18 s with Shift (×1.55). Halls push outward from the plaza as they grow, so they never overlap; the passage behind each arch lengthens instead.

**The same store for everyone.** A shop's seat comes from its id. Its track is the first with room when it opens, so newcomers land on the rim, as galaxies do in v5, and its starting phase is the golden angle times its index plus an FNV-1a jitter. The server computes each day's layout on the first read of the day ("no scheduler", like universe weather) and every app caches it, because WebKit and WebView2 don't promise the same `Math.sin`.

**Drift.** Each day every booth advances along its track by Kepler's third law (**Exists today:** `Hub.jsx`): the inner track turns once in 28 days, track 4 in about six months, track 17 in about three years. Neighbouring tracks run at different speeds, so booths "pass like trains on neighbouring tracks" and a shop has new neighbours next week. Where eccentric tracks come within 1.2 m, the Hub's soft repulsion nudges booths apart. Shops tied by a fork or an accepted link drift together, the younger moving one track a day toward the elder's: v5's gravity drift (`server/utils/universeDrift.js`), laid on a floor. Nothing moves while you're inside; the day's step applies the next time you come through the door.

### 7.4 "Walk around infinitely"

Gate 1.1 fails "infinite scroll", and Wi's answer is "It ends." The Proposed store is finite and continuous: one floor with no loading screens, growing only when a shop opens, and every hall ends at a curved back wall 3 m beyond its outermost track. On that wall, in gold leaf, is "That's everything." Under it, smaller: "Music, as of Oct 6. New shops open on the outer track." A door in the wall opens onto the night and takes you to Space (⌘2), at the galaxy of the last booth you looked at.

**Open: infinite or finite.** Recommendation: finite. An infinite floor would have to be filled with repeats or generated filler, and the wall is where a person decides they are done. The drift supplies the feeling of endlessness instead: the store is never laid out the same way twice, and it still ends.

### 7.5 Bodies

**You.** Your astronaut walks in. The record stores IDs, never art, so the store draws the same person in its own style: Mii-like and out of the suit, the head a quarter of your height, eyes and mouth as simple marks, clothes in your rocket's colours. The camera sits 3.5 m behind and 1.2 m up, pitched 12° down; V switches to first person at PRISMON's eye height of 1.65 m. The space bar never jumps here; it plays.

The figure is drawn from the same stored fields (see 2), restyled:

| Astronaut field | In the shop |
|---|---|
| race, complexion | skin tone and face shape, flat-shaded with one highlight from the upper left |
| physique (slight, average, round) | build and height; the head stays a quarter of your height |
| hair {style, color} | the same style id, drawn as one rounded mass |
| cosmetics (up to 8; tattoos and visors today) | kept as simple marks and solids |
| rocket {hull, accent} | your clothes: hull colour on the jacket, accent on the trim |

You change it in Settings → Account → "your astronaut & rocket", and the shop redraws you the next time you come through the door.

**The makers.** Each booth has its maker standing behind it, drawn from their astronaut and holding their stem player in their own skin (wrap or paint, **Exists today:** v3). A maker is a portrait, not a presence: the figure stands there whether or not they are online and never says which. Looking at one shows "Ana · who is Ana?", as the portfolio's sun reads "who is LMY?"; E opens Ana's bio sun over the store.

**Everyone else.** By default you see no other shoppers, except people you came with once parties ship (7.19). There is no "3 people here" and no crowd at a shelf, because a visible crowd around a record works as an attention count (gate 1.3).

**Open: strangers in the store.** The sketch's Mii-like characters suggest a full shop. Recommendation: makers at their booths and your own party; never strangers, and never any count.

**Open: selling cosmetics.** Recommendation: don't; a cosmetics market invites checking and comparing people (gate 1.3).

### 7.6 Records

**A booth** in the Mi hall has a pale oak frame with the maker's name hand-lettered across the top in Cormorant Garamond italic, and a back panel washed in their sun's tint or the key colour of their newest work. Up to four sleeves stand face-out on top, with crates below. **One crate is one solar system and holds 21 records**, the system's own cap. Sleeves are 31.4 cm square, and spines wear the work's key colour, or night indigo when the key is unknown.

**Looking.** Point at a record within 3.2 m, PRISMON's reach, and its label rises:

```
glass hours
Ana · Song · A minor · 128 BPM
gen 2 · remix of World Ending by LMY
$4 · .wwav, master and four stems
```

**Picking up.** E (or a click, or A) lifts the record. The camera eases in over 420 ms and the pointer is released. The sleeve slides off and the record becomes a planet in your hands, 22 cm across, lit from the upper left, its four moons on a plus: vocals up, drums down, bass left, other right. The nearest wall's light shifts toward the record's key colour over 760 ms, the portfolio's light-wash time. Nothing sounds yet. While you hold something you stand still, and the keys go to the planet.

**Listening.** Space, or a click on the planet, plays it. The moons work as on every planet (see 4.6): a click mutes at once, a second click within 250 ms reverts the mute and solos, a 400 ms hold blooms the FX moons, and a drag along the arm sets the level: a moon's distance from the planet is its volume. The room sound drops 18 dB while a record plays. Esc puts the record back in its crate, and ⌘Z reads "Undo put back".

**Listening booths.** Six glass booths with benches stand at the centre of the music hall. Carry a record into one and the door closes: the room sound goes to zero, the planet fills the glass, and the deck from Space appears, with keylocked tempo and pitch dials, a waveform drawn from only the audible stems, and **＋ add a song**, which pairs the record with a song from your library as a figure-eight matched by tempo, key and first downbeat (**Exists today:** `wwav/src/audio/analysis.js`). Walking out stops it, and as PRISMON says, "Leave, and your place is kept."

**Open: how much plays before buying.** Recommendation: exactly what Space plays (4.13), the whole work by default, so a song never sounds different by where you found it; what's sold is the file. A seller can choose "30 seconds" from a start point they set, and the label then reads "30-second listen".

### 7.7 The screening room

Films stand on the Si hall's shelves as film cans, 38 cm across, each lid ringed in the film's poster colour pushed to a resonant shade (**Exists today:** `portfolio/src/utils/posterColor.js`), as a film planet "wears a ring" in Space. Pick up a can and its poster turns to face you over "Film · 12:40 · 1080p · $6 · .swav".

The screening room is the hall's centrepiece: 24 seats in four curved rows facing a 6 m screen. Sit with a can (E on a seat) and press Space. The house lights fall over 900 ms, the Hub's dive time, and the film plays on the native video surface Space and the Console use (FFmpeg decode, wgpu compositing, **Decided**), set into the room's screen. F gives "nothing but the film", G opens the grade (see 4.7), and anything you were hearing pauses: "The planet is paused while this plays." Sellers choose the whole film or the first N minutes as the preview.

### 7.8 The reading table

Writing stands in the Ri hall as stacks of typed pages, PRISMON's page artifact, and as numbered zines (the Made Archive's Issues). The reading table runs 4.8 m along the window, with eight chairs and a lamp at each. Sit with a stack and its first page rises into Space's 640 pt reading column (see 4.7), printed here on warm paper `#FFE9C8` in ink `#3D2E22` (11.0:1). The lamp warms as you read; that is the only motion. Where the seller's free reading ends, the page reads "The rest is in the book. $8." What's sold is the file: Markdown with the `ri` front matter Space proposes (4.7, Open), or a zine's existing PDF.

### 7.9 The rails

The Gi hall is rails. Each listing hangs as PRISMON's merch artifact, a garment extruded from a silhouette on a floating rail, with its cover photo projected on its face. There is one silhouette for each of `FashionListing`'s seven categories, and a booth here is a 2.4 m rail of up to 12 hangers. Pick a garment up and drag to spin it (0.006 rad per point, as gallery planets do); ← and → step through its up to 8 photos. Its tag reads:

```
Waxed chore jacket
M · like new · $120 · one of one
ships from Providence, RI
```

I opens the whole tag: MEASUREMENTS (up to 12 free-form fields, such as "chest 21 in · length 28 in"), material, brand and colour. Variants show as size chips; a sold-out size reads "none left in L". `User.preferredSizes` **Exists today** and nothing uses it, so "My sizes" dims everything not in your sizes to 45%, never hiding it.

**The fitting mirror** at the hall's centre shows your astronaut holding the garment up, and says what it is: "This shows colour, not fit. Check the measurements." One-of-one holds are in 7.11.

**Open: Arbor Vitae.** The Shopify store (`client/src/components/ArborVitaeV2.jsx`) lives apart. Recommendation: a Gi booth that links out to its checkout until its stock moves onto `FashionListing`.

### 7.10 What is sold

| Item | On the floor | The buyer gets | Today |
|---|---|---|---|
| Song | a record | the exact `.wwav` bytes, master and four stems, DRM-free, sha256 on the receipt | track purchases sell access, not a file |
| Song, master only | a record | a `.wwav` with no `wstm` chunk | new |
| Album | a crate | every song as `.wwav`, plus a disc edition in PRANA's layout (`NN Song.wwav` at the root), zipped for a PRANA disc (a USB flash stick) | album purchases; `prana/SPEC.md` |
| Film | a can | the exact `.swav` bytes | film purchases |
| Remix license | a card in the crate | the right to sell a remix of this work (7.14) | new |
| One-of-one exclusive | a record under glass | the file; the work leaves sale for everyone else | Made Archive beats, locked by `isSold` |
| Writing | a stack or zine | Markdown or PDF | zines with `pdfUrl` exist |
| Clothing | a hanger | the garment, shipped | `FashionListing` and Shippo |
| Event ticket | a flyer on the corkboard | a ticket; a free event is one press, "Going" | `Event`, RSVP |
| Preorder | an object on the plinth | a pledge, charged only if the drop funds | new |

Prices run up to $2,000 for digital work (today's cap) and $1–$10,000 for clothes. A $0 item reads "Free · take one" and downloads with no checkout.

**Drops.** The founder's roadmap ends in a Kickstarter "to hopefully get early adopters to pre order there own Mi-WWAV." The plinth by the counter holds the drop as an object, such as PRANA's simulator model, a 92 × 180 × 26 mm slab in `#FAF9F6`. A pledge saves your card with a Stripe SetupIntent; on the end date every pledge is charged if the drop funded, and none if it didn't: "Not funded yet · ends Feb 1. Nothing is charged unless it funds."

**Open: drop progress.** All-or-nothing preorders usually show a progress bar, which is a public sales count (gate 1.3). Recommendation: show one word, "Funded", once it is true, and "Not funded yet" with the date before that.

### 7.11 The counter

The counter holds the espresso machine, the clerk (7.16) and your bag. **Buying never needs it.** With something in hand, B (or Buy on its label) opens Stripe Checkout in a sheet for that one item, and nothing is charged until you pay there (**Decided:** direct distribution, no Apple cut). ⌥B adds it to the bag instead: "2 in your bag · $13". The line item says "(10% platform fee included)".

| Rule | Exists today |
|---|---|
| The fee is 10% of the total, included in the price | `purchase.js` |
| Digital: separate charges and transfers; the seller receives 90% after the webhook, and Stripe's processing comes out of WWAV's 10% | `connect.js` |
| Clothes and tickets: destination charges `on_behalf_of` the seller, because "we don't want the platform absorbing physical-goods disputes" | `purchase.js` |
| Your own items, and digital items you already own, are refused; 20 checkouts an hour | `purchase.js`, `rateLimit.js` |

**The bag** is new server work, since `create-checkout` takes one item. Digital items share one charge, transferred to each seller under a `transfer_group`; each seller's physical goods take a charge of their own, because a destination charge has one destination. The sheet says so first: "1 payment for 3 files, 1 for Ana's jacket."

**One-of-one holds.** **Exists today:** Buy on a one-of-one runs one atomic `UPDATE … WHERE status='active'` that holds it for 30 minutes, and a second buyer gets 409 "Just sold or being purchased". The holder's tag reads "Held for you · 29:41", the only countdown in the store. Everyone else sees the garment greyed on its rail: "On hold. If it isn't bought, it's back by 4:12 PM." **Fix:** the Stripe session lives 24 hours today, so a stale hold lasts that long; create it with `expires_at` 30 minutes out, Stripe's minimum, so hold and session end together.

**The receipt** reads "World Ending.wwav · 212,448,316 bytes · sha256 9f3c…e1". The file lands in Library → Purchases, the planet in Space reads "yours", and ⌘E opens its stems in the Console.

**Open: minimum price.** At $1, Stripe's fixed fee on a US card (30¢) is larger than WWAV's 10¢, and today's stem download sends the seller 100%. Recommendation: a $2 minimum for paid digital items, with stems included in the `.wwav` price.

### 7.12 Your booth

**Opening.** Your booth opens when payouts are ready. **Exists today:** a listing can't go live until Connect onboarding reports `details_submitted`, `charges_enabled` and `payouts_enabled`, and clothes also need a ship-from address. Until then your booth's shutter is down, visible only to you, with "Your shelf opens when payouts are set up." and **Set up payouts**; Heat holds the task "Finish payout setup so your shelf can open" (see 3).

**Stocking.** Publishing is a drop: drag an export from the library drawer (⌘L) onto a crate, and the sheet asks for price, what the buyer gets and which crate ("connected selling", see 2).

**The back room** is a door behind your booth that only you can open.

| Shelf | Shows | Exists today |
|---|---|---|
| Sales | a ledger, newest first (date, item, gross, fee, net, payout), and the 30-day graph at 90% | `/purchases/sales-graph` |
| Orders to ship | Shippo rates, "Buy label", or "Or enter tracking manually" | `shipping.js` |
| Payouts | balance, "Request payout", the Stripe Express dashboard | `connect.js` |
| Notes for the clerk | what the clerk may say about each item | new |

Sales stay in the back room: no "N sold" appears on the floor, and nothing is sorted by how well it sells, even here. A sale reaches you in "Since you last looked" (see 2), never as a notification.

### 7.13 Shelf order

There are four orders: newest (the default), artist (in List view, 7.17), lineage family and alphabet. O cycles the order inside a booth, and the records re-file over 420 ms. Lineage family groups works by root, oldest ancestor first, generations in order.

Nothing is ever ordered by sales, plays or saves, and none of those is counted in public: "Nothing decides for you what's worth seeing."

**The New table** just inside the door holds the 12 newest records from the whole store, face-out and newest first, then a card reading "That's everything new this week." It is where a shop on an outer track starts out next to the door.

### 7.14 Provenance

Every item shows its family. The line "gen 2 · remix of World Ending by LMY" comes from the file's own `wlin` (**Exists today:** `.wwav` and `.swav` 0.1) and is checked against the server's lineage.

Press T, or click the line, and the tree opens above the item in your hands: small lit planets in their key colours, fork edges in ink at 0.14 opacity, merge edges dashed in the accent, and human links (influence, sample, collab, cover) labelled and shown only once accepted. A film made from a song hangs from it as its child. Click an ancestor for "Go to shelf" if it's for sale, or "Open in Space" if it isn't.

**Open: royalties to ancestors.** Today `collaboratorUserIds` and lineage parents get nothing from a sale. A concrete proposal:

1. Each listing says whether remixes of it may be sold: "No", "With a license" (the default) or "Yes".
2. A remix license is an item the parent's maker sells, carrying a **lineage share** of 0–30%, default 10%.
3. When a remix sells, the parent's maker receives that share of the remixer's 90%, as a second transfer from the same charge. On a $10 remix: WWAV $1, the parent's maker $0.90, the remixer $8.10.
4. One level up only, until v1's ledger shows how the split behaves.
5. Shares under $1 accumulate in the ledger instead of being skipped.
6. Collaborators on a listing set fixed percentages, totalling 100% before it goes live.

Recommendation: ship 1–6 for songs and films. Writing and clothes have no remix of their own yet (see 4.7).

### 7.15 Finding things without billboards

- **The window** is v5's free random comet, which carries one world into someone else's sky for 6 hours. The front window holds one record, chosen by seeded chance with equal odds per shop (not per item, so a large catalogue can't crowd it), the same for everyone, changed every 6 hours: "In the window until 6 PM, by chance: glass hours, by Ana."
- **The wormhole door.** Once a day a door appears in one hall's wall for 24 hours and opens onto a booth in another hall, chosen by chance. It is free, like v5's wormholes: "charging for weather would only teach people to ignore it."
- **Lineage and added galaxies.** Every family tree is a path to walk, and booths of people you've added in Space show a small lit lamp that only you see.
- **No paid placement.** No endcap, banner or window is for sale.

**Open: paid reach.** v5 charges 25 fuel to aim a comet, because "free aim would make every galaxy everyone's billboard", and Space drops aimed comets (4.12). Creator stores usually sell promotion. Recommendation: sell none, so money decides only what someone pays for a file.

### 7.16 The clerk

The clerk is Claude, behind the counter in an apron; anywhere else, press ? while holding something. Like Ripple, it answers about one record from what the record and its seller say: metadata, lineage, the seller's notes, and the listening analysis if there is one (Ripple's Gemini pass). It keeps Ripple's rules: [MM:SS] timestamps; "describe what things SOUND like", never Hz, dB or LUFS; and "NEVER invent metrics".

> **Does the bass come back?**
> Yes. It drops out at [2:14] and comes back at [2:46], lower and rounder. Ana's note says it was recorded on a borrowed bass at her sister's house.

> **Who plays guitar?**
> I don't know. Neither the file nor Ana's notes say.

The clerk never compares records by sales or popularity, since it has neither, never says what's "best", and never adds to your bag or buys. It can offer "Go to shelf", which still needs your press. It is off until its first use, and when it fails it says "The clerk can't answer right now." (see 2).

### 7.17 Skipping the walk

Walking is for browsing and is never required (gate 2.1).

- **⌘K.** "Go to shelf: Ana" puts you facing Ana's booth with a 140 ms cut. A record result puts the record in your hands.
- **Links.** A record's link on www.wi-wwav.com opens the app at the shelf, record in hand; on a phone without the app it opens the flat web face with **Play** and **Buy** (see 2). Space's "$4 · on the shelf" tag does the same.
- **Step count.** A known record takes ⌘K, typing, Return, B, then Stripe. Gate 2.1 needs this counted by doing it.
- **List view (⌘⇧L).** The whole store as a 2D list, not a lesser one: halls as sections, shops as rows, items with the same fact lines, the same four orders, Listen, Buy, Save for later and the family tree. VoiceOver reads "glass hours. Song by Ana, A minor, 128 BPM, 4 dollars. Remix of World Ending." It is the default under Reduce Motion.

On a record, Return listens and ⇧Return is the one secondary act, **Save for later** (see 2): the record goes to your Saved shelf, never counted and never shown to the seller.

### 7.18 The engine

- **Renderer.** three.js in the web layer, shared with Space (4.5). From PRISMON it takes collision, `Door`, the raycast look, the room lifecycle `{build, update, dispose, onInteract}`, ACES tone mapping, bloom, grain and vignette.
- **Budget.** Sleeves are instanced, one draw per hall; covers stream by distance, 128 px beyond 12 m up to 1024 px in hand; booths beyond 30 m draw as lit blocks. The target is 60 fps at 2560 × 1600 on an M1 MacBook Air in a hall of 1,000 shops, and the frame loop stops when you stand still with the room sound off.
- **Sound.** Every sound goes through the audio engine, the master clock (**Decided**), so booth, room sound and Now strip share one output. Records stream their stems by range reads, as Wi does.
- **Input.** WASD and mouse look under pointer lock, E to interact, Esc to step back. The gamepad uses the Console's mapping (5.16): sticks walk and look, A interacts, B steps back, LB + A saves, LB + B undoes. PRISMON has no gamepad support today.
- **Accessibility.** List view (7.17). HUD labels are ink `#3D2E22` on sand `#E8DCC8` (9.6:1), secondary text `#6B5643` (5.1:1, not the mock's `#8A7560` at 3.2:1), and the accent `#C89668` is for rings only. The palette is from v3's never-shipped design mock (`ios_v3/DESIGN/wwav-screens.jsx`; see 8).

### 7.19 Together, later

Parties come after v1. **Come with me** gives a 4-character code, after REPUBLIC's WebSocket relay (`games/republic/network/relay_server.py`), the repo's only presence precedent, rebuilt in Node on the existing server. Up to four people walk in together. Hand a friend a record (E on them) and both engines start it at the same server time. No voice chat, no strangers, no "N here".

### 7.20 Before launch

These gaps were found by reading the code, not by running it. Each must close before the store takes money from strangers.

- [ ] **Entitlements.** Downloads of `.wwav` and `.swav` must check for a *completed* `Purchase` on the server. Today the film stream has no purchase check (`server/routes/films.js`), `streaming.js` never reads `Purchase`, and `GET /api/purchase/check` counts a pending purchase as owned.
- [ ] **Double payout.** `/connect/balance` and `/request-payout` don't filter by `purchaseType`, so destination-charge sales (clothes, tickets) look pending and could be transferred twice. `/request-payout` also pays 90% on `stem_download`, where the automatic path pays 100%.
- [ ] **Fashion inventory.** `variants` are never checked or decremented at purchase. Choose the size before checkout and decrement it in the hold's transaction.
- [ ] **Exclusive files.** `/api/ma/audio/:filename` serves a one-of-one beat to anyone who knows its name.
- [ ] **Seller email.** `sendFashionSaleEmail` is looked up but never exported, so clothing sellers get no sale email.
- [ ] **Payouts.** Amounts under $1 are skipped; carry them forward (7.14 needs this). `autoPayoutSchedule` has no job.
- [ ] **Also:** expire hold and session together, build the bag (7.11), and print "ships in the US" on every tag while shipping is US-only.

### 7.21 Left out, and why

| Left out | Why |
|---|---|
| Star ratings and reviews | a score of other people's opinions is an attention count; the reply to a record is a remix |
| "Bestsellers", "trending", "N sold", "because you bought" | "Nothing decides for you what's worth seeing" |
| Sound on approach, hover previews | nothing plays without a press |
| "Only 2 left", sale countdowns | they pressure you to buy now; the only countdown is your own 30-minute hold |
| DRM | the file is the product, byte for byte (gates 1.2 and 2.3) |
| Walking on a phone | the floor needs a keyboard, mouse or gamepad, as PRISMON does; the web face is flat |
| Furniture (Zi) | out of scope for this app |


## 8. Look, sound and feel

The app speaks one design language in three registers. The case is the same in every room: a brushed-metal title bar, one light from the upper left, the same stem colours and the same words. What sits inside the case changes with the room. Heat and the Console's chrome are the **desk**, Space is the **night**, and Unquantized is the **interior**. Everything in this chapter is **Proposed** unless it carries another label. Where a value here differs from a raw token quoted in an earlier chapter, build this chapter's value.

### 8.1 Where it comes from

| System | Where it lives | The app takes | The app leaves |
|---|---|---|---|
| Heat's Aqua skin | Heat | the case: brushed metal, gel buttons, Aqua stripes, source list, sheets, the olive LCD, full dark tokens | the slate desk behind the window |
| MI-WWAV-OS style | `MI-WWAV-OS/app/lib/style/theme.dart` | "Aqua is the case; the Game Boy is inside it"; DMG screens; deck metal with pinstripes; the 140 ms beat | Helvetica Neue (8.4) |
| v3 iOS | `ios_v3/WWAV/Theme/`, `Visuals/` | the night palette, `WWAVLight.sun`, key colour, the starfield port, titles that end in a period | four pastel palettes, Nunito, the breathing tab pill |
| v4 web | `wwav/src/styles/tokens.css`, `planet.css` | seven ink opacities, one accent with three uses, Cormorant over Inter, 120/240/420 ms | the limousine-black ground, except after hours in the shop |
| PRANA | `prana/SPEC.md` | the stem colours, state by shape, the Turrell field, Jost | blinking LEDs (8.6) |
| Crater | `portfolio/src/crater.css`, the Sculptor of Time handoff | amber lamps, the sunset field, the 760 ms light-wash | the hourly supernova |
| v3's design mock | `ios_v3/DESIGN/wwav-screens.jsx` | sand, clay and the warm ink, as the shop's materials (see 7) | Fraunces |
| Wi, the wall | `wi/src/wi.css` | paper and ink for the web face; a ring that carries a muted light's contrast | its adjusted stem hues (8.12) |
| Legacy icons | `assets/` | all five pictures | their old meanings (8.5) |

v1's scanlines and RGB fringe stay out; the founder called the old web look "so ugly and has such bad contrast that it makes things hard to see and use".

### 8.2 Three registers

| | Desk | Night | Interior |
|---|---|---|---|
| Rooms | Heat; the Console's chrome; Settings; every sheet | Space; the expanded player | Unquantized |
| Ground | brushed metal, white wells, `#edf3fe` stripes | `#070A18` under a key-tinted starfield | oak and felt in lamp light, walls washed in PRANA's field |
| Ink | `#1b1b1b` | `#F4EFE6` | `#3D2E22` |
| One accent | Aqua highlight `#3875D7` | royal blue `#2946FF` | crema `#C89668` |
| Type | Lucida Grande, Menlo | Cormorant Garamond italic, Inter | Cormorant Garamond italic, Jost, Inter |
| Light appearance | Heat's light tokens | night | dusk |
| Dark appearance | Heat's dark tokens | night | after hours |

The web face at www.wi-wwav.com keeps Wi's own look (**Exists today:** `wi/src/wi.css`): paper `#f7f6f2`, ink `#161616`, the system's type at 17 px, light and dark from the phone. A share page is read by someone who never installed anything, so it borrows none of the app's costume.

#### The desk

**Exists today (Heat):** Lucida Grande 13 px at 1.35; brushed metal (1 px light and dark lines over a radial highlight on `#e4e4e4→#c9c9c9`); gel pills with a hard break at 45/55%; white and `#edf3fe` rows; a `#e7ecf2` source list; sheets that drop from the title bar over a 25% black backdrop; Stickies yellow; glass beads; a 3 px `rgba(56,117,215,.6)` focus ring; and a full set of dark tokens (8.11).

The desk has two metals with two jobs. **Case metal** is Heat's: title bar, toolbars, sheets, inspector. **Deck metal** is MI-WWAV-OS's paler `#FBFCFD`/`#E6EAEF`/`#C9D0D8`, with a 1 px pinstripe every 4 px that "reads as texture, never as stripes". It covers the Console's mixer strips and device cards (see 5).

**The Game Boy inside.** Every Console readout sits behind glass as a DMG screen: `#0B1F0B` and `#0F380F` grounds, `#306230` for unlit segments, `#8BAC0F` for secondary text and `#C6E24A` glow for primary, in uppercase Menlo inside the bezel's scanlines and vignette (**Exists today:** MI-WWAV-OS `ScreenBezel`). Meters stay "DMG green until it is about to be too loud" (see 5). Heat keeps the iTunes olive LCD, so the screen's colour names the room: olive is planning, green is making. DMG screens look the same in light and dark, because a lit screen doesn't change with the room's lamp.

Five fixes bring the desk's pairs up to gate 2.4's bars (ratios in 8.11):

- Selected rows fill `#1B4C8C`, MI-WWAV-OS's deep Aqua, with white text at 8.5:1 in both appearances. White on `#3875d7` is 4.47:1, so `#3875D7` stays the focus ring, the gel and a 3 px bar at the row's leading edge.
- LCD dim text goes from `#6b7050` (3.9:1) to `#4b5034` (6.4:1).
- Source-list headings go from `#6e7781` (3.8:1) to `#4f5761` (6.2:1).
- Heat's third ink goes from `#7a7a7a` (4.3:1) to `#5f5f5f` (6.4:1).
- Blue gel buttons take an ink label, as Mac OS X's default button did; the darkest stop under it is 4.6:1. Selected segments deepen to `#336dcc→#1B4C8C` so a white label holds 5.0:1. The dark gel's top stop goes from `#6a6d71` to `#5e6165`.

#### The night

**Exists today (v3, v4):** ground `#070A18`, deep `#030510`, clay `#1A2142`, ink `#F4EFE6` ("spanish lace"), starlight `#DDE4FB`, royal blue `#2946FF`. "The palette is nearly binary": hierarchy comes from weight, size and space, and **seven ink opacities are the only hierarchy scale**: 100, 70, 50, 35, 20, 12 and 7%. On the night, 100% is 17.2:1 and 70% is 8.5:1, so both can carry body text. 50% is 4.8:1 and carries secondary text only. 35% and below are fills, never text. Lines are 1 px at 100% or 70%, "never a soft hairline".

Royal blue has exactly three uses: armed or active (a solo, a dirty ↑ Push), the current lineage branch, and committing actions. It is 3.2:1 on the night, so it is a ring or a fill, never text.

The starfield is v4's: three parallax layers of 190 stars in `#dde4fb`, a field banded by the current work's key hue and capped at luminance 52 "so text always survives", a breathing core at 42% of the height, and a rare meteor. v3's port seeds it from `0x57574156`, "WWAV" in ASCII, so it is the same sky on every machine. Suns run `#fff8e6 → #ffd89a → #f0a23c → #b25f12` with an amber glow. Every orb is a three-stop lit sphere whose rim takes its key's glow tone. That tone holds at least 4:1 against the night in all 24 keys and in indigo, so no planet disappears in F minor.

Space is night in both appearances (**Exists today:** v3 forces Cosmos and Play onto the night palette), because the key-tinted sky is the work's colour and lit orbs need a dark ground to read.

#### The interior

The shop is lit, not painted. Its walls take PRANA's Turrell field, `#F7F4EE→#E6DFD0`, which drifts toward rose `#FBEDE8` or cool `#DDE2E6` on 53–131 s cycles, but only while the room sound is on (8.6). Its materials come from v3's never-shipped mock: sand `#E8DCC8` and sand deep `#D4C4A8` for felt and plaster, clay `#B89878` and clay deep `#7A5E45` for oak, glow `#FFE9C8` for lamp light and paper. The shopfront and the listening booths are clear glass, which takes the room's light and has no colour of its own. The ink is the mock's `#3D2E22`, "never pure black". Crater's amber `#e8915b` and `#f4b483` colour the lamps, and Crater's sunset field `#f8d0a4→#d4763f` is the sky in the oculus over the counter (see 7).

**Light is never a ground for text.** Words in the shop are printed on something: a shelf tag, a paper strip on a spine, the menu board, a receipt. Ink on paper `#FFE9C8` is 11.0:1, and on sand it is 9.6:1. Secondary text is `#6B5643` (5.1:1). Crema is the shop's one accent, and it is a light. On sand it is 1.9:1, so wherever it marks a control it sits inside a 1 pt ink hairline.

**After hours** is the shop's dark appearance. The windows go dark, the walls fall to v4's limousine `#0F0C09`, the lamps stay amber, and paper stays paper, so a tag reads 11.0:1 at any hour. PRISMON's film grain holds as a still frame whenever the frame loop stops (see 7).

### 8.3 What every register shares

- **One light.** `WWAVLight.sun` ≈ (0.30, 0.25), the upper left. It places the highlight on every orb, avatar and slider thumb, the gloss on every gel, the shop's window, and the brightest corner of a Turrell wash. In 3D the room's lamps stay where the room puts them, and anything held up to you (a record, a garment, a manuscript) is lit from the camera's upper left.
- **A work's colour is its key colour:** hue = ((pc · 7) mod 12) · 30 with A = 0, major hsl(h, 72%, 58%), minor hsl(h, 58%, 42%), and night indigo, hue 232, for "still condensing" (**Exists today:** `keyColor.js`). A key colour is a fill, never a ground for text: in E minor and A major neither ink reaches 4.5:1. A spine carries its title on a paper strip.
- **Stem colours are PRANA's, "the same everywhere":** vocals `#D23C2A`, drums `#F0B90B`, other `#2E9A55`, bass `#1F4E9E`. The fill is exactly that hex in every room. No single hue clears 3:1 on every ground (bass is 2.5:1 on the night, drums 1.5:1 on deck metal), so every stem mark has a 1 pt ring in its register's ink, and the ring carries the contrast. Drums yellow is never type. State is shape:

```
audible               filled disc, ink ring
muted                 2 pt stem-colour ring, open centre, ink ring
soloed                filled, plus a 2 pt outer ring (royal blue in Space, ink elsewhere)
silent under a solo   filled at 45%, dashed ink ring
selected              filled, plus a 2 pt notch beneath (PRANA blinks; the app doesn't)
```

- **Heat's colours are fills.** Cool `#4f9be6`, warm `#efa431`, hot `#e0402c` and overdue `#8f1d16` fill tubes, pill borders and dots. The level word is ink, because hot on white is 4.25:1.
- **One accent per screen,** named in 8.2. v4's test holds: "If blue appears more than once or twice on a screen, something is wrong."
- **Spacing and corners.** A 4 px grid: 4, 8, 12, 16, 24, 32, 48, 64. Radii: chip 6, button 10 (gels are full pills), card 14, a sheet's lower corners 8.

### 8.4 Type

| Face | Where | Sizes | Why |
|---|---|---|---|
| Lucida Grande (Mac), Lucida Sans Unicode (Windows) | the desk: rows, labels, sheets, menus | 11, 13, 15, 20 pt; body 13 at 1.35 | Heat's face and Aqua's, until Yosemite (2014) moved to Helvetica Neue. Both are Lucida Sans and ship with their systems, so nothing is bundled. |
| Menlo (Mac), Consolas (Windows) | DMG screens, timecode | 11, 13, 20 pt, capitals | MI-WWAV-OS's dot-matrix face. Fixed width keeps "128.00 BPM" from jittering, PRANA's rule for "1.00x". |
| Cormorant Garamond italic, 300 and 400 | the name of a work, a person or a place, in the night and the shop | 20 pt minimum; 30 for headers; 46 for suns | v4's name face, from Crater. Line height 1.18 or more; weight 400 below 28 pt, where 300's hairlines thin. |
| Inter | running text and labels in the night and the shop | 11 (capitals, +0.08 em), 13, 15, 17 for reading | v4's one family, for anything read twice. |
| Jost | signs, tags and price cards in the shop | 11–15 pt capitals tracked 0.34 em; tabular figures | PRANA's face, in Futura's lineage (v1 was Futura until March). The shop's tags match the device's screen. |

Inter, Cormorant Garamond and Jost are OFL fonts bundled with the app. **Nunito retires:** its round terminals read soft against metal and crowd Cormorant in the night. What made v3's type its own, lowercase italic titles that end in a period ("lineage.", "time.", "vox."), moves to Cormorant. DM Mono (PRISMON) and Fraunces (the v3 mock) aren't used, because Menlo and Cormorant already do their jobs.

**Open: one sans or two.** Lucida in the desk and Inter elsewhere puts two sans faces in one window. Bundling Inter alone would be simpler and identical on Windows, but it would turn the Aqua case into a costume. Recommendation: keep Lucida for the desk, and recheck it on the Windows build, where Lucida Sans Unicode looks oldest.

### 8.5 Icons

**Exists today:** five 2000 × 2000 PNGs drawn in thick black lines (`assets/`), wired to a song card in `client/src/components/popups/SongInfoPopup.jsx`. The gates retired two of their old meanings, so each picture gets a job it can still honestly show:

| Picture | Was | Becomes |
|---|---|---|
| A red rose (`heart.png`) | like | **Add**, a private save that is never counted (see 4) |
| A two-masted schooner (`message.png`) | comments | the message door and letters: a ship carries post |
| A looped paperclip (`copy.png`) | share | **Copy link** to the web face |
| A plus (`plus.png`) | remix | **＋ add a song** |
| An up arrow (`up_arrow.png`) | unknown | **↑ Push** |

They are redrawn as vectors on 16, 20 and 28 pt grids, one stroke weight per size, in the register's ink; the rose keeps its red inside an ink line. Play, stop, record and loop are drawn in the same hand. SF Symbols aren't used, because their licence covers Apple platforms and the app goes to Windows.

### 8.6 Motion

Motion comes from sound. Nothing on screen moves while nothing plays, and nothing plays without a press (gate 2.4). The scale is built on MI-WWAV-OS's 140 ms beat with v4's curves: `cubic-bezier(0.22, 1, 0.36, 1)` for anything coming to rest, `cubic-bezier(0.32, 0.72, 0, 1)` for sheets. Nothing bounces.

| Token | ms | Use | From |
|---|---|---|---|
| `press` | 70 | a gel darkens to 88% | half a beat |
| `beat` | 140 | mute, solo, select, a room change; every move under Reduce Motion | MI-WWAV-OS |
| `fade` | 280 | fades, reveals, a sheet dropping | v4's 240 and Heat's 220, put on the beat |
| `morph` | 420 | the player opening; a record lifted to your hands | v4 |
| `wash` | 760 | a wall or sky taking a key colour | the portfolio's light-wash |
| `dive` | 900 | entering a planet; house lights falling | the Works hub |

Toasts hold for 2600 ms. Every idle motion the app inherits gets a decision:

| Motion | Today | In the app |
|---|---|---|
| A planet breathing at its tempo (period 2 · 60 / BPM, scale 1.015) | v4, always | only while it plays |
| Moons floating ±3 pt over 5.4 s | v3 | gone; a moon's place is its level |
| Twinkle, breathing core, meteors, a sun's 9 s pulse | v3, v4, v5 | on the engine's clock, so they stop with the sound (Open in 4) |
| The tab pill breathing over 5.4 s | v3 | gone; the switcher is a gel control |
| Field drift, steam, lamp flicker | PRANA; 7 | only while the room sound is on |
| The hourly supernova | portfolio | left out: an event on a timer is one nobody pressed |
| A planet pulsing for new work | portfolio | a still dot in "Since you last looked" |
| An LED blinking while selected | PRANA | a still notch |

Meters, playheads and waveforms move whenever audio moves, because they are information. Under Reduce Motion (Mac) or with animation effects off (Windows), every move becomes a 140 ms cross-fade, cameras lose their inertia, and breathing stops. A ceremony after a press is allowed if it ends within 900 ms: v2's Game Boy cartridge returns as a `.wwav` dropped on the Console's transport screen, sliding into its slot in 420 ms.

### 8.7 Sound

The app is an instrument, so its own sounds must never be mistaken for the work or land in a take. **It makes no interface sounds**: no clicks, swooshes or alerts. There are three exceptions. Each starts only after a press and has its own switch.

- **The Console's click.** Off by default in each session (see 5). An accent at 1,568 Hz and a beat at 1,047 Hz, 12 ms sine bursts at −12 dBFS on the cue bus, which no recording or export taps.
- **Heat's chime.** One struck bell with a 1.2 s decay when a focus session ends, off by default (see 2). Nothing sounds to call you back from a break.
- **Unquantized's room sound.** Air, the espresso machine and cups, after the door press (see 7).

**Exists today, unused:** `portfolio/public/media/site-music/` holds `base.m4a`, `hover.m4a` and `click.m4a`. They are three files of exactly 21.8 s, which is eight bars at 88 BPM by arithmetic. Their names suggest layers meant to answer a pointer, and nothing references them. **Proposed:** they become the shop's house music on the counter speaker, under the room-sound switch. `base` loops. `hover` joins over 280 ms while you face a shelf, and `click` joins while something is in your hands. All three stop, rather than duck, when a record plays, so two pieces of music never overlap.

**Open: house music in the shop.** Recommendation: use these three files, because they are the founder's own and already built to layer. Drop them if they compete with the records more than they warm the room.

Space makes no sound but the works. Rockets, wormholes and meteors are silent, because in a sky full of songs a sound effect would be one more song.

### 8.8 Haptics

On a Mac with a Force Touch trackpad, the Rust side calls `NSHapticFeedbackManager`. Haptics never fire on a timer, never carry information alone, and follow the system's trackpad setting. Windows gets none at first.

| Moment | Feel |
|---|---|
| A clip edge snaps to a grid line, the playhead or another clip | alignment |
| A dragged work enters a drop target: your galaxy, a system, a shelf | alignment |
| A moon reaches either end of its arm; a dial passes 1.00× or 0 st | level change |
| Force click on a moon | blooms the FX moons at once, without the 400 ms hold |
| Force click on a planet or record | opens its info card, as Quick Look does for a file |

### 8.9 Accessibility bars

The bars are gate 2.4's, set out in 2: body text 7:1 and secondary 4.5:1 in light and dark, in every register; 44 × 44 pt hit areas; every gesture on a key; nothing moving while nothing plays; no state shown by colour alone. Two rules are added here, written down before testing as the gates require:

- **What counts as body text.** Running text and list rows are body text (7:1). Labels on controls, column headings and DMG secondary lines are secondary (4.5:1).
- **Desktop posture.** The gate's rule covers phones only: "body text under 16 px on a phone". That is about 0.46° of view at 33 cm. From 55 cm in front of a 13-inch MacBook Air (112 pt per inch), the same angle is about 19 pt. The rule here: text you read for more than a line (the reading room, letters, Mail, liner notes, the clerk's answers) is at least 17 pt. Rows and labels, which are scanned, may be 13 pt, the Mac's own system size. Nothing is under 11 pt. Measure at 1024 × 680 and 1280 × 800, at default size and at ⌘+'s 20 pt, in light and dark, in all four rooms.

**Open: reading size.** 17 pt is 0.40°, under the phone rule's angle. Recommendation: test 17 and 19 pt with the founder at their own desk, and fix the number before any gate run.

Focus is the 3 px Aqua ring in the desk and a 2 pt ink ring in the night and the shop, since royal blue is spent on its three uses. Under Increase Contrast, deck metal drops its pinstripes, night text uses 100% ink only, and the shop's walls hold at the field's lightest tone. Every stem mark speaks: "Vocals, 70 percent, audible", "Drums, muted", "Bass, soloed".

### 8.10 Words

The voice is the repo's: plain, second person, present tense, short. Real numbers replace adjectives. Every omission and every disabled control says why in one sentence. Endings are said out loud. No exclamation marks, no emoji. An ellipsis means a sheet follows ("Export everything…") or work is under way ("Syncing…").

- **Case.** Buttons and menus are sentence case, as Heat writes them. Headings in the night and the shop are lowercase italic and end in a period. Labels are tracked capitals, and so are DMG screens. Product and room names keep their case ("Heat", "Mi_WWAV"), and a work's name is set exactly as its maker typed it.
- **Claude.** The word "AI" never appears and nothing gets a sparkle; Counsel's spec already rules out "Any 'AI' branding in the UI". Claude is named wherever Claude acts, because you should know who was sent what. Its output is labelled as an estimate or a draft, and it never gives a number it can't trace.
- **Undo instead of questions.** ⌘Z replaces "Are you sure?". Confirmation remains only where undo can't reach: money moving, and deleting an account (type DELETE).

| Moment | Copy | Source |
|---|---|---|
| Empty | "Nothing here right now." | Heat |
| The end | "That's everything." | Wi |
| A limit and its reason | "A solar system holds 21 worlds. Start another one." | v4 |
| A file that is less than it could be | "Plain audio comes in as master only." | 2 |
| A pause you didn't ask for | "The planet is paused while this plays." | v4 |
| Undo | menu "Undo move clip"; toast "Undone — move clip"; "Nothing to undo." | MI-WWAV-OS |
| Waiting, honestly | "Splitting 'break.wav' · segment 4 of 12" | 5.10; v4's rule: "anything finer than these stages would be invented" |
| Failing | "Couldn't save that change. Check your connection and try again." | Heat |
| Disabled | "Habit limit reached" | Heat |
| Claude | "Claude's estimate: 45m. It read the title, the notes and your past averages." | Proposed |

Never: "Oops", "Awesome", "AI-powered", "Trending", "Don't break your streak", "3 people are looking at this", or an unread count.

### 8.11 Tokens

Every token lives in one file, `design/tokens.json`. The build compiles it into CSS custom properties for the web UI, a Rust module for the wgpu compositor (title cards, scopes), and a C++ header for the engine's 22 pt strip above third-party plugin windows, so no token can drift between processes.

```json
{ "night": { "ground": "#070A18", "ink": "#F4EFE6",
             "inkSteps": [1.0, 0.70, 0.50, 0.35, 0.20, 0.12, 0.07],
             "textSteps": { "body": [1.0, 0.70], "secondary": [0.50] },
             "accent": "#2946FF", "accentUses": ["armed", "branch", "commit"] } }
```

| Token | Light | Dark | Contrast | From |
|---|---|---|---|---|
| `desk.ink` · `ink2` · `ink3` | `#1b1b1b` · `#4a4a4a` · `#5f5f5f` | `#ececec` · `#c2c2c2` · `#9a9a9a` | 17.2 · 8.9 · 6.4 on white; 12.5 · 8.3 · 5.3 on `#26282b` | Heat; `ink3` darkened |
| `desk.well` · `stripe` | `#ffffff` · `#edf3fe` | `#26282b` · `#2c3038` | ink 15.5 on stripe | Heat |
| `desk.caseMetal` | `#e4e4e4→#c9c9c9` | `#5a5d61→#3f4246` | ink 10.4 | Heat |
| `desk.deckMetal` | `#FBFCFD` `#E6EAEF` `#C9D0D8`, edge `#98A2AD` | case metal | `#161D26` 14.0; `#5D6975` 4.6 | MI-WWAV-OS |
| `desk.select` | `#1B4C8C`, white text | same | 8.5 | MI-WWAV-OS |
| `desk.highlight` | `#3875D7` | `#3a6fc4` | rings and gels only | Heat |
| `desk.lcd` | `#f2f4e4→#dfe3c6`; ink `#262a17`; dim `#4b5034` | `#2c3121→#20241a`; `#d7e0a8`; `#8e9670` | 11.2 · 6.4; 9.6 · 5.1 | Heat; dim darkened |
| `desk.sourceList` | `#e7ecf2`; heading `#4f5761` | `#2e3136`; `#9aa3ad` | 6.2; 5.1 | Heat; heading darkened |
| `screen.dmg` | `#0B1F0B` `#0F380F` `#306230` `#8BAC0F` | same | `#8BAC0F` 5.0 on `#0F380F` | MI-WWAV-OS |
| `screen.glow` · `amber` · `rec` | `#C6E24A` · `#F0A32E` · `#E0453A` | same | 9.0 · 6.3 · 3.2 (rec is never text) | MI-WWAV-OS |
| `night.ground` · `deep` · `clay` | `#070A18` · `#030510` · `#1A2142` | same | | v3, v4 |
| `night.ink` | `#F4EFE6` at 100 · 70 · 50% | same | 17.2 · 8.5 · 4.8 | v4 |
| `night.star` · `accent` | `#DDE4FB` · `#2946FF` | same | 15.5 · 3.2 (rings only) | v3, v4 |
| `night.sun` | `#fff8e6→#ffd89a→#f0a23c→#b25f12` | same | | v4 |
| `room.field` | `#F7F4EE→#E6DFD0`, toward `#FBEDE8` or `#DDE2E6` | `#0F0C09` | | PRANA; v4 |
| `room.material` | sand `#E8DCC8`, sand deep `#D4C4A8`, clay `#B89878`, clay deep `#7A5E45` | same, lamplit | | v3 mock |
| `room.ink` · `ink2` · `paper` | `#3D2E22` · `#6B5643` · `#FFE9C8` | same, on paper | 9.6 · 5.1 on sand; 11.0 on paper | v3 mock |
| `room.accent` · `lamp` · `sky` | `#C89668` · `#e8915b`, `#f4b483` · `#f8d0a4→#d4763f` | same | accent 1.9 on sand, inside an ink hairline | v3 mock; Crater |
| `stem.*` | `#D23C2A` `#F0B90B` `#2E9A55` `#1F4E9E` | same | carried by an ink ring | PRANA |
| `heat.*` | `#4f9be6` `#efa431` `#e0402c` `#8f1d16` | same | fills only | Heat |
| `light.sun` | (0.30, 0.25) | same | | v3 |

### 8.12 Left out, and open decisions

| Left out | Why |
|---|---|
| Palettes you pick (v2's four, v3's four pastels) | Each palette is another set of contrast checks, and every work already brings its own colour |
| A night mode that warms the screen | The system already does this (Wi's reason) |
| Heat's slate desk `#7d8a99` | It was the web page behind the window; a native window sits on your own desktop |

| Open | Recommendation |
|---|---|
| One sans or two | Lucida in the desk, Inter elsewhere; recheck on Windows |
| Reading size, 17 or 19 pt | Test both at the founder's desk before the gate run |
| House music from `site-music/` | Use it under the room-sound switch; drop it if it competes with the records |
| Wi's adjusted stem hues (`#d93b30`, `#c99300`, `#2e9150`, `#2f6fd0`) | When the wall becomes the web face, move it to PRANA's hexes with ink rings, so the colours match on the device, in the app and on a phone |


## 9. Under the hood

This is the engineering plan. The two-process split, Tauri, JUCE hosting third-party VST3 and AU, FFmpeg with wgpu and VideoToolbox, the engine as master clock, direct distribution and the existing mi-wwav.com server are **Decided**. Everything else is **Proposed** unless marked **Exists today** or **Open**. Numbers are targets to test against, not measurements.

### 9.1 Two processes

| Process | Built with | Owns | Its main thread runs |
|---|---|---|---|
| `Wi_WWAV.app` | Tauri 2: a Rust core, and the web UI in WKWebView (WebView2 on Windows) | windows, menus, the four rooms, the library, undo, sync, the account, video | Tauri's event loop |
| `wwav-engine` (`Contents/Helpers/wwav-engine.app`) | JUCE 8 (C++) with PRANA's `prana/core` | the audio device, the graph, MIDI, third-party plugins and their windows | JUCE's message loop |
| `wwav-scan` | JUCE, command line | one pass over the plugin folders, then exits | none |

Why two:

1. **A plugin is someone else's code inside the audio callback.** A crashing plugin takes its process with it: in one process, the whole app, mid-sentence in Heat; split, only the engine, which the app restarts (9.3).
2. **Both frameworks want the main thread.** On macOS one framework drives `NSApplication` from it. In the app that is Tauri; wherever plugin editors live it must be JUCE, because AU and VST3 editors build their views on the message thread and many assume it is the main one.
3. **Real-time hygiene.** The audio thread shares its process with nothing that compiles JavaScript, collects garbage or decodes video.

The engine is a faceless helper app (`LSUIElement`, no Dock icon), so plugin windows belong to a real application. Keys a plugin window doesn't take go back to the app's single input router (MI-WWAV-OS's one raw-key reader, `router.dart`), so ⌘Z, ⌘K, ⌘1–⌘4 and Space still work.

```
+---------------------------- Wi_WWAV.app (Tauri 2) ------------------------------+
| main thread: event loop, windows, menus, the one input router                   |
|                                                                                 |
| +------------------- web UI (WKWebView) ------------------+ +-- viewer -------+ |
| | Heat | Space | Console | Unquantized                    | | CAMetalLayer    | |
| | timeline, mixer, three.js rooms                         | | wgpu composite  | |
| +-----------------------------^---------------------------+ +-------^---------+ |
|          commands (JSON) / channels (raw bytes)                     |           |
| +-----------------------------+------- Rust core -------------------+---------+ |
| | library.sqlite + ULID media | undo journal | upload queue | Keychain auth   | |
| | video: FFmpeg decode -> wgpu -> VideoToolbox encode | take writer           | |
| | engine supervisor | mi-wwav.com, Google and Brightspace clients             | |
| +----------+------------------------------------------+-----------------------+ |
+------------|------------------------------------------|-------------------------+
             | command socket                           | shared memory
             | JSON ops -> results, events              | clock, meters, peaks,
             |                                          | input ring, crumb
+------------v------------------------------------------v-------------------------+
| wwav-engine (JUCE 8 + prana/core, no Dock icon)                                 |
| main thread:   JUCE message loop, plugin editor windows                         |
| audio thread:  graph (tracks, stem buses, VST3/AU, delay comp.) -> CoreAudio    |
| workers:       disk streaming, graph rebuilds, watchdog                         |
+---------------------------------------------------------------------------------+
  wwav-scan: one plugin at a time -> scan results       HTTPS: mi-wwav.com, R2
```

### 9.2 The wire and the clock

Traffic splits by rate.

**Commands** travel over a Unix domain socket (a named pipe on Windows), owner-only, as length-prefixed JSON in the envelope MI-WWAV-OS already uses (**Exists today:** `MI-WWAV-OS/engine/src/api.rs`):

```
-> {"id": 412, "op": "param.set", "args": {"node": "01JC5R…", "param": "gain_db", "value": -3.0}}
<- {"id": 412, "ok": true}
<- {"ev": "plugin.latency", "node": "01JC5T…", "samples": 2048}
```

They run at tens a second at most. `wwav-engine-cli` speaks the same socket: "Anything provable in a terminal is true in the app."

Commands reach the audio thread through a lock-free queue. The audio thread never locks, allocates or logs; PRANA's core already runs with no heap and no libc (`prana/SPEC.md`). A new track or plugin is built on a worker thread and swapped in with one atomic pointer write. "A live tweak is a parameter write, never a graph rebuild" (**Exists today:** `MI-WWAV-OS/engine/src/mixer.rs`).

**Everything fast** sits in one shared-memory region, created by the app so it outlives the engine:

```
header   magic "WWAV", layout version, sample rate, block size
clock    seqlock: seq, sample_pos (i64), host_time_ns (u64), rate (f64), state
meters   ring, per block: peak and RMS per track and bus, DSP load, dropouts
peaks    ring: waveform peaks of takes being recorded
input    ring: raw input audio while recording, 2 s deep
crumb    u64: the graph node now processing, written before each call into it
```

**The engine is the master clock** (**Decided**). Every callback publishes where the playhead will be when this block reaches the speaker, output latency and plugin delay already subtracted. The web UI gets clock anchors ten times a second and extrapolates the cursor at the display rate; the video presenter reads the clock every refresh (9.5); Space's orbits run on it, so a silent sky holds still (4). Meters reach the web UI once per frame through a Tauri channel as raw bytes, never JSON.

### 9.3 When the engine falls over

The session lives in the app; the engine only renders (5.7).

1. The engine dies. The app sees the socket close and the child exit.
2. It reads `crumb` and names the plugin instance that was running.
3. It starts a new engine, checks the protocol version, and sends `session.load` built from the session as it stands, with each plugin's last reported state. The instance that crashed loads switched off.
4. The transport comes back stopped at the same playhead. Nothing resumes on its own.
5. The sheet in 5.7 explains, with **Keep it off** and **Try it again**.

The target is 2 s plus the plugins' own load time.

- **Plugin state** is collected on every stop, every 60 s and before every save (5.7): at worst a minute of knob turns in one plugin's window is lost.
- **Takes survive.** Input audio crosses the `input` ring and the app writes the file, so a crash keeps the take to its last block: "Take kept up to the crash · 3:12.4".
- **Hangs.** If the clock stalls for 500 ms while playing, or a ping goes unanswered for 1 s, the app kills the engine and runs the same steps.
- **Repeat crashes.** Three in ten minutes from one plugin and it stays off: "crashed 3 times".
- **If the app dies,** the engine sees its parent's pipe close, stops the audio and exits.

One process per plugin stays **Open** in 5.7, with its recommendation: not in v1; decide after a month of crash logs.

### 9.4 Audio

**The graph** is 5.12's tracks, then the four stem buses, then the master (6.6).

- Clips stream PCM from the library: WAV, AIFF, CAF and `.wwav`. The app decodes compressed imports (MP3, AAC, FLAC) once into a WAV beside the untouched original, so the engine reads one kind of thing.
- Reader threads keep 2 s of every playing clip in memory ahead of the playhead and hold files under 32 MB whole. A `.wwav` is one file feeding four stem lanes, read in 1024-frame runs as PRANA reads `wstm`.
- Built-in devices are PRANA's DSP wrapped as JUCE processors (5.9). Clips at another rate pass through the same polyphase resampler export uses (6.7).

**Settings.** Sessions run at 48 kHz, or 44.1 kHz when opened from a `.wwav` (6.7). The block is 128 frames (2.67 ms at 48 kHz), 64 while monitoring an armed input, 256 or 512 for heavy mixes. The audio thread joins the device's audio workgroup; v1 renders on that one thread, and tracks go parallel only if the reference session (9.13) misses its budget.

**Delay compensation.** When a node's reported latency changes, the graph recomputes each path off the audio thread and inserts delay so all paths meet aligned at every sum. Recordings are placed by CoreAudio's round trip plus the offset **Measure latency** finds (5.6).

**Plugins.** VST3 everywhere, AU (v2 and v3) on the Mac (**Decided**). CLAP is left out of v1: JUCE doesn't host it, and a third hosting layer would be ours to write and keep. `wwav-scan` checks one plugin at a time with a 30 s limit (5.7); a plugin that crashes or hangs is not checked again until its version or file date changes. Launch never waits for a scan.

**MIDI** comes through CoreMIDI via JUCE. Timestamps become sample offsets inside the block, so a note lands where it was played, not at the block's edge (5.4). MIDI 2.0 is left out of v1: the controllers in reach speak MIDI 1.0, the path JUCE has tested longest.

**Rendering.** Export runs the same graph objects on a non-real-time thread as fast as the CPU allows, at the same block size, with plugins told `setNonRealtime(true)`, the hook WWAV Push uses to see an export (**Exists today:** `vst_plugin/README.md`). Automation is sample-accurate and commands carry sample times, so a session renders as it played: "what you hear is what you render, by construction" (`mixer.rs`). The export sheet names the exceptions: plugins that ask to render in real time (5.12), and plugins that change their own processing offline.

**Determinism, from PRANA** (**Exists today:** `prana/SPEC.md`, `prana/tests`). Built-in DSP compiles with no fast-math and `-ffp-contract=off`, uses its own sin, cos, exp and tanh, flushes denormals explicitly, and never calls Accelerate on those paths. Renders are then bit-identical run to run and across arm64 and x86_64, which the goldens check, and a 44.1 kHz session of built-ins is held to PRANA's golden hashes (5.9). Third-party plugins can't promise this, so their tests null against the last render instead (9.11).

### 9.5 Video

Video lives in the Rust core (**Decided**).

- **Decode.** FFmpeg's libraries, with VideoToolbox decoding H.264, HEVC and ProRes in hardware and software as the fallback. Frames stay on the GPU as IOSurface-backed Metal textures that wgpu wraps without a copy. One decoder thread per visible track keeps about 250 ms of frames ahead of the clock.
- **Proxies.** Footage over 1080p gets half-resolution proxies in the background (5.11): ProRes 422 Proxy, every frame a keyframe, so scrubbing never waits on a long GOP. That is about 20 GB per hour of 4K at 30 fps, in the session's `cache/`, safe to delete. Export always uses originals. MI-WWAV-OS's rule holds: "Slow is acceptable, dropped frames during editing are not."
- **Composite.** wgpu on Metal (DX12 on Windows). Each layer is a texture, transform, opacity, blend and grade in one WGSL pass. Grades use the fields a film fork already stores, so a grade made here is a valid fork (**Exists today:** `sanitizeGrade` in `server/routes/films.js`).
- **Present.** The viewer is a native `CAMetalLayer` beside the web view, following the rectangle the web layout reports. In the 3D rooms, three.js sends a screen's four projected corners and the layer takes the perspective transform that maps the film onto that quad, so a seated film sits in its screen (7.7). Each display refresh, the presenter asks the clock where the sound will be when this frame is on glass, shows the newest frame at or before it, and drops (and counts) frames more than one late. Bluetooth delay comes from the device's own report.
- **Encode.** Export doesn't follow the clock. Frame n sits at n ÷ fps and its sound starts at sample ⌊n × rate ÷ fps⌋; the engine renders the audio first and the encoder pulls frames by number, so picture and sound line up exactly. H.264 and HEVC go through VideoToolbox, AAC through AudioToolbox, into an MP4 with `+faststart`. `.swav`'s `wmet` and `wlin` are appended last, because nothing may follow them (`formats/swav/SPEC.md`).

The 3D rooms draw with three.js in the web view, on WebGPU where the view has it and WebGL 2 otherwise (4.5, 7.18).

### 9.6 Storage

The library is the folder `~/Music/Wi_WWAV/` (2).

- **`library.sqlite`** holds clips, tags, sequences, Heat, plugin scan results and store receipts, in WAL mode with FTS5 for search. Every committed change is its own transaction, so nothing needs saving; the status bar says "Saved on this Mac". A nightly `VACUUM INTO` keeps seven dated copies.
- **`media/`** holds every imported, recorded and rendered file under a ULID, written once and never edited (**Exists today:** `MI-WWAV-OS/engine/src/ids.rs`).
- **`sessions/`** holds `.wwavsession` packages (6.5).

**The undo journal** follows MI-WWAV-OS's design (**Exists today:** `MI-WWAV-OS/engine/src/store.rs`), reimplemented in the app's Rust core. MI-WWAV-OS contributes the idea, not the code (**Decided**). Every mutation runs in a transaction that snapshots the affected rows before and after.

```sql
CREATE TABLE txn     (id    TEXT PRIMARY KEY,   -- ULID, so order is time
                      label TEXT NOT NULL,      -- "move clip" -> "Undo move clip"
                      room  TEXT NOT NULL,      -- heat | space | console | unquantized | library
                      state TEXT NOT NULL);     -- done | undone
CREATE TABLE txn_row (txn_id TEXT NOT NULL REFERENCES txn(id),
                      seq INTEGER NOT NULL, tbl TEXT NOT NULL, row_id TEXT NOT NULL,
                      before TEXT, after TEXT,  -- JSON; NULL means the row didn't exist
                      PRIMARY KEY (txn_id, seq));
```

- Undo writes `before` back in reverse; redo writes `after` forward. ⌘Z walks the current room's entries, labelled through the `history` op. The journal survives relaunch.
- No `ON DELETE CASCADE` anywhere: "an unjournaled row is an unrestorable one."
- A session keeps the same shape in its own `journal/undo.ndjson`, so its history travels with it (6.5).
- Work that has left the Mac, such as a payment or a sent letter, is not a journal entry, and the Edit menu says so: "Can't undo a purchase." Publishing is undoable only while the upload is queued: ⌘Z clears `published_at` and the queue loses it. Once the server has it, the Edit menu reads "Can't undo a publish. Unpublish 'World Ending'…" (see 2).

**Media is reclaimed only on a press.** Deleting removes rows, never files. **Clean up media…** lists files that no row and no journal entry names ("1.8 GB in 214 files") and moves them to `trash/`; emptying the trash deletes them. So undo can always bring a deleted clip back with its sound.

### 9.7 Sync and the server

The server stays (**Decided**): Express, Postgres and R2 on Heroku. What is local is the truth (2).

**The queue is a query** (**Exists today:** `MI-WWAV-OS/proto/README.md`): `published_at IS NOT NULL AND remote_id IS NULL`. Each 8 MiB part is signed just before it goes up, because presigned URLs last 300 s, and recorded in `upload_part(clip_id, n, etag)`, so a resumed upload skips it. Retries back off from 2 s to 5 min with jitter. The publish body carries `settings: {origin: "wi_wwav", clipId}` (2), so a retry never posts twice.

**Heat sync** goes to new per-user tables, because the PKM's tables have no `userId` (3.15). Each field carries a sequence number from a per-device counter, and the server keeps the higher one field by field, so a slow older write never overwrites a newer one (the PKM's `useAutosave.js` pattern).

**Reused as they are** (**Exists today**):

| Endpoint | Used for |
|---|---|
| `/api/upload/sign`, `/process` | not used by the desktop, which splits locally (5.10); kept for cloud splits from phones and the web |
| `/api/upload/sign-stem`, `/process-stems` | the four stems unpacked from a `.wwav` for the web and iPhone players, uploaded beside the exact file (`/api/upload/parts`), so nothing is split twice |
| `/api/upload/sign-video` (2 GB cap) | films |
| `/api/upload/sign-replace`, `/:trackId/replace-audio` | a new version under a permanent link, as WWAV Push does |
| `/api/publish`, `/api/unpublish` | a work into a system or onto a shelf |
| `/api/lineage/global`, `/api/tracks/:id/lineage`, `/api/tracks/:id/fork` | family trees; ↑ Push (v1 mix states, v2 8-track projects) |
| `/api/v2/*` (galaxies, systems, planets, suns, lineage-links, saved, catalog, universe, travel) | Space, as the iPhone uses it (4) |
| `/api/purchase/create-checkout`, `/api/purchase/check`, `/api/purchases/*` | buying, receipts, sales |
| `/api/connect/*`, `/api/fashion-listings`, `/api/orders`, `/api/events` | payouts, clothes, shipping, tickets (7) |
| `/api/auth/refresh`, `/api/auth/me` | the 7-day JWT, refreshed after a 401 (2) |

**New** (**Proposed**):

| Endpoint | Why |
|---|---|
| desktop sign-in, authorization code with PKCE | below |
| `/api/entitlements`, `/api/entitlements/:kind/:id/file` | a download checks for a *completed* purchase and gets a URL good for 60 s; today nothing checks (7.20) |
| `/api/upload/parts` | multipart straight to R2: `/sign` is one PUT capped at 250 MB, and a 6-minute `.wwav` is about 318 MB |
| `/api/store/*` | halls, shelves and the bag (7.11), grown from `/api/v2/catalog` |
| `/api/heat/changes` | Heat sync, pulled with a cursor, pushed in batches |
| `/api/assist/:task` | Claude, with prompts held on the server (9.8) |
| `/desktop/latest.json` | the update manifest (9.9) |

Production doesn't run `sync({ alter: true })` for new tables, so each new model gets an explicit `Model.sync()` at boot, as `DevlogPost` and `WiPost` do (`server/index.js`).

**Sign-in** opens the system browser on mi-wwav.com with a PKCE challenge and listens on a loopback address for the answer (RFC 8252). The password, Sign in with Apple and password managers stay in the browser; the app receives the account's usual JWT and a refresh token. The server already runs an OAuth 2.1 server with PKCE for the devlog's Claude connector (**Exists today:** `server/mcp/auth.js`); desktop sign-in adds a public client that checks real accounts. A device-code variant (RFC 8628) serves PRANA's deferred account link, planned as "a code on the device that you enter on mi-wwav.com" (`prana/SPEC.md`).

**Checkout** loads Stripe's hosted page in a separate web view with no access to the app's commands, then confirms with `/api/purchase/check` before downloading.

**The web face.** Share links open on www.wi-wwav.com, so a friend on a phone installs nothing. Wi's player reads PCM a range at a time, "so nothing is decoded up front", under four faders (**Exists today:** `wi/`); a `.swav` plays as the MP4 it is; a work for sale has **Buy**. Wi serves one account today, so `WiPosts` needs an owner and public read routes.

### 9.8 Security and privacy

- **Secrets live in the Keychain** (Credential Manager on Windows): account and Google tokens, and the Brightspace iCal link, which carries a private token. Never in SQLite, logs or crash reports. They are bound to the app's signing identity, so the engine, which runs strangers' code, can't read them; it holds no network credentials at all.
- **Library validation is relaxed only where plugins load,** in the engine and the scanner.
- **The web view is fenced.** Tauri 2 capabilities give each window only the commands it uses, no remote scripts load, and words other people wrote render as sanitized Markdown, never HTML.
- **Claude.** The founder's Anthropic key never ships inside the app, where anyone could extract it. Calls go to `/api/assist/:task`, one endpoint per job (score a task, read an announcement, the clerk), with the prompt on the server and a limit per account. Claude estimates and drafts, never decides; the prompts keep "never invent metrics" (`rippleCreatorService.js`). An email's text passes through as a request body and is not stored.
- **Location** is off by default. When on, it is snapped to 0.01° (about 1.1 km) on the Mac before it leaves, the server's own snap (**Exists today:** `server/routes/local.js`).
- **Heat is local first** and syncs privately; Space never shows a grade (4.14). Grades are FERPA education records. **Open:** whether grade rows sync at all. Recommendation: off by default; when on, encrypted on the Mac with a Keychain key, so the server holds only ciphertext.
- **Google.** The app has its own OAuth client, signing in through the browser with PKCE. `gmail.readonly` is a restricted scope: testing mode asks again every 7 days and allows 100 test users, and going public means Google's security assessment. **Open:** when to verify. Recommendation: stay in testing mode through the small-group stage (10.5), and get verified before the whole school.
- **No analytics.** Crash reports are opt-in, carry no media, titles or tokens, and name the plugin, because that's what fixes the bug.

### 9.9 Shipping on the Mac, and Windows later

A direct download, not the Mac App Store, whose sandbox blocks plugin hosting (**Decided**).

| Item | Plan |
|---|---|
| Identity | Developer ID Application certificate (Apple Developer Program, $99 a year) |
| Runtime | hardened runtime on all three executables |
| `wwav-engine` | `com.apple.security.cs.disable-library-validation` so others' plugins load; `com.apple.security.device.audio-input` |
| `wwav-scan` | `disable-library-validation` |
| `Wi_WWAV.app` | `com.apple.security.device.camera` for camera takes (5.6); library validation on |
| Notarization | `xcrun notarytool submit --wait`, then `xcrun stapler staple` on the DMG |
| Architectures | universal: arm64 and x86_64 for Rust, JUCE and FFmpeg |
| Updates | Tauri's updater: an Ed25519-signed manifest at `mi-wwav.com/desktop/latest.json`, files on R2 |

- **Updates never interrupt.** They download in the background and install when you quit, never mid-playback: "Update ready · installs when you quit". Sparkle would add delta updates; Tauri's updater serves Windows from the same manifest and keys.
- **The iCloud trap.** iCloud Desktop and Documents stamp `com.apple.FinderInfo` onto build output and codesign refuses it, as MI-WWAV-OS and WWAV Push found (`MI-WWAV-OS/README.md`, `vst_plugin/README.md`). Builds go to `~/Library/Developer/wi-wwav-build`; the release script runs `xattr -cr` before signing.
- **Protected plugins** sometimes need `allow-unsigned-executable-memory`. **Open.** Recommendation: add it only if a plugin in the test set fails without it.
- **Intel-only plugins** can't load into an arm64 engine. **Open.** Recommendation: offer "Open the engine under Rosetta" per session, as a bridge; Apple has said Rosetta's general use ends after macOS 27.
- **Minimum macOS. Open.** Recommendation: macOS 13, which runs on Macs back to 2017. "Wwav is about everybody" (devlog), and an old Intel laptop is what many people have.

**Windows** follows 3–6 weeks after the Mac (**Decided** estimate): WebView2; ASIO under Steinberg's free SDK agreement, WASAPI as fallback; VST3 only; wgpu on DX12; hardware decode and encode through FFmpeg (D3D11VA, Media Foundation, NVENC, Quick Sync, AMF); Authenticode through Microsoft's Trusted Signing or an OV certificate. The first build is x64, because an ARM64 engine can't load x64 plugins.

### 9.10 Licences

| Part | Licence | What it means here |
|---|---|---|
| JUCE 8 | AGPLv3 or commercial | Wi_WWAV is closed and sells things, so commercial; a free Starter tier applies under a revenue cap (check the cap at the version pinned). WWAV Push already uses JUCE 8.0.4 (**Exists today**) |
| VST3 SDK | MIT from 3.8 (October 2025); confirm at the version pinned | the logo needs Steinberg's separate agreement, so the UI writes "VST3" in plain text |
| Audio Units | Apple's SDK | no fee |
| FFmpeg | LGPL 2.1 or later | built `--disable-gpl --disable-nonfree --enable-shared`, dylibs in `Contents/Frameworks`, no libx264, libx265 or fdk-aac, source and build script published. A user may swap the libraries if they re-sign them; the notes say how. LAME (MP3 export) is LGPL too |
| Tauri, wgpu, Rust crates | MIT or Apache-2.0 | credited in About |
| Surge XT | GPLv3 | tests only, never shipped |

Codec patents (H.264, HEVC, AAC) are a question for a lawyer, logged in `docs/DECISIONS.md`. Encoding through the operating system's encoders means the app ships none of its own.

### 9.11 Tests

The gates' rule applies: write down what a fail looks like before testing, and "a check with no evidence is marked open. It is never marked passed" (`wi/GATES.md`). That is what makes long stretches of autonomous work checkable: each milestone in `docs/PLAN.md` names its fail criteria before any code.

| Suite | Proves | Runs on |
|---|---|---|
| Golden renders | sessions in `tests/sessions/` render to the same SHA-256 every time; 44.1 kHz built-in sessions match PRANA's goldens, which change only through `tools/update_golden.sh` in a commit that says why | Linux; macOS nightly |
| Plugin hosting | Surge XT loads, takes parameters, restores its state, renders, and nulls against its last render below −96 dBFS; its editor opens and closes | VST3 on Linux, AU on macOS |
| Delay compensation | test plugin `delay-n` adds a known latency; its sum with a dry copy nulls | Linux |
| Kill and restart | test plugins `crasher` (dies on a note) and `hanger` (never returns), plus 200 kills at random moments in playback, recording and render. Pass: back in 2 s, session hash unchanged, takes intact to the last block, transport stopped | Linux, macOS |
| Formats | the Rust `.wwav` and `.swav` writers match `wwav_pack.py` and `swav_pack.py` byte for byte; unpack then pack is identity; `ffprobe` accepts every export | Linux |
| Journal | random edit runs: undo all returns the first state byte for byte; redo all the last | Linux |
| Picture and sound | a test film with a flash and a click each second exports with zero offset | macOS |
| UI | `mock-engine` speaks the socket and writes a fake clock and meters; the web UI runs in Playwright (WebKit, Chromium) with a fake Tauri bridge against stored screenshots of every room | Linux |
| Soak | 8 hours of the reference session: no dropouts, flat memory | a Mac, each release |

`tauri-driver` has no WebDriver for WKWebView, so Playwright's WebKit stands in, and a pass by hand on a real Mac covers the gap.

**CI** is GitHub Actions. Everything that can run on Linux does; macOS runners take AU, VideoToolbox, signing and a nightly universal build. On a private repository a macOS minute bills at about ten Linux minutes, so macOS jobs on pull requests stay under 10 minutes. **Open:** a self-hosted Mac mini runner. Recommendation: once the macOS bill passes about $50 a month.

### 9.12 The repo

A new repository (**Decided**); `wi-wwav-desktop` is a working name.

```
wi-wwav-desktop/
  app/src-tauri/      Rust core: store, journal, sync, auth, video, engine supervisor
  app/ui/             web UI: shell/, heat/, space/, console/, unquantized/
  engine/             wwav-engine and wwav-scan (JUCE, CMake); test-plugins/
  formats/            submodule: Mi-WWAV at a pinned commit (prana/core, prana/tools,
                      formats/swav, wi/src/formats)
  web/                shared UI parts and the share page for www.wi-wwav.com
  design/tokens.json  compiled to CSS, a Rust module and a C++ header (8)
  tests/  tools/  .github/workflows/
  docs/PLAN.md        milestones in build order, fail criteria written first
  docs/DECISIONS.md   date, choice, reason, what was turned down
  docs/QUESTIONS.md   Open decisions for the founder, each with a recommendation
```

An agent working alone writes its question into `QUESTIONS.md` and moves to other work rather than guessing.

**Open:** the submodule pulls the whole Mi-WWAV repo, iOS apps and portfolio included, for four folders. Recommendation: use a sparse checkout now, and split those folders into their own repository only if clone times start to hurt.

### 9.13 Budgets

Reference machine: a 2020 M1 MacBook Air with 8 GB. Reference session: 24 audio tracks, two `.wwav`s as stem lanes, 8 Surge XT instrument tracks, 40 built-in devices and two 4K video tracks.

| What | Budget |
|---|---|
| Audio callback | 2.67 ms at 128 frames, 48 kHz; DSP load at most 70% at the 99.9th percentile |
| Dropouts | none in an hour of the reference session |
| Click to sound | a mute is heard within one block and one socket hop, 10 ms at most |
| UI | 60 fps everywhere; 120 fps on ProMotion for timeline scrolling and the Space camera |
| 3D rooms | 60 fps at 2560 × 1600 in a hall of 1,000 shops (7.18) |
| Video | two graded 4K streams, no dropped frames; more on proxies |
| Cold launch | Heat usable in 1.5 s; the engine opens its device in 0.8 s alongside, and Heat never waits for it or a scan |
| Engine restart | 2 s plus plugin load |
| Offline render | 10× real time or faster for sessions of built-ins |
| Library search | 50 ms at 50,000 clips |
| Memory | app 300 MB idle, not counting WebKit's processes; engine 150 MB empty |

CI measures what it can. The rest is measured by hand at each release and written into `docs/PLAN.md` beside its budget.


## 10. Business, community and the gates

Wi_WWAV is a business. This chapter covers how it makes money, who it is for, and how each money-making part holds up against the founder's own written design philosophy, the four gates in `wi/GATES.md`. Everything here is **Proposed** unless it carries another label. Prices and fees that already run on the server are marked **Exists today**.

### 10.1 Why it is a business

The founder set the terms, in letters to Wi-WWAV and in the platform's first line:

| Words | Source |
|---|---|
| "I am creating technology for artists and selling it." | devlog, Oct 1 |
| "The intersection of technology, the humanities, and commerce." | devlog, Oct 1 |
| "an artist first technology company" | devlog, Oct 3 |
| "Wwav is about everybody" | devlog, Sep 30 |
| "I need to build something irreplaceable by the iphone." | devlog, Oct 1 |
| "a rebellion against the record label system" | `intro.md` |

Those six lines become five commitments, each kept in code rather than copy:

1. **The artist keeps the money and the masters.** A digital sale pays the seller 90% (**Exists today:** `server/routes/purchase.js`, `connect.js`). WWAV takes no rights in a work beyond hosting and delivering it. Distribution without a label in the middle is the rebellion.
2. **Leaving costs nothing.** Export, downloading your own files, and Export everything are free and work while you are signed out (see 2).
3. **The price is a number everybody can reach.** The free app is a whole product: Heat, the Console with plugins, a galaxy, and a shelf.
4. **It does what a phone can't.** Hosting VST3 and AU plugins, a song and its film on one clock, and a store you walk through need a computer. That is "irreplaceable by the iphone" for software; the device line in 10.2 is the hardware answer. Direct distribution with no Apple cut (**Decided**) is the founder's "taking another bite out of apple" (devlog, Sep 29).
5. **Money comes from selling things, never from attention.** Every revenue line in 10.4 sells a file, a garment, a ticket, a device, a seat, storage or an hour of someone's time. None of them sells a view. This is the app's hypothesis for gate 1.4 (10.6).

### 10.2 The family model, applied

"Every WWAV family follows one model: a device, a format, and a marketplace. You make the work on the device, it lives in a WWAV format, and creators can sell it." (`archive/src/content.js:369`). Wi_WWAV is the first place where all four media share one marketplace.

| Family | Field | Hardware | Device in this app | Format | Marketplace |
|---|---|---|---|---|---|
| Mi_WWAV | sound | PRANA (Mi-WWAV Beta 1), in development; Mi_cro and Mi_pro tiers described | the Console (5) | `.wwav` (**Exists today**) | Unquantized's music hall |
| Si_WWAV | sight | concept: "a camera and mobile editing terminal in one device" | the Console's video side (5.11) | `.swav` (**Exists today**) | the screening room |
| Ri_WWAV | words | concept: e-reader and writer | the page-planet editor (4.7) | none yet; Markdown or PDF on the shelf (**Open**, see 6.11) | the reading table |
| Gi_WWAV | garments | concept: a design-to-garment machine | photos and measurements only | the unnamed "pattern file" (**Open**) | the rails |

Zi_WWAV (furniture) is left out, because the sketch names four media and nothing in the app makes a chair.

**The Console is Mi_pro_WWAV's software.** The archive describes Mi_pro_WWAV as "Desktop studio: a stronger processor, many inputs and outputs, instruments and sequencing. Makes discs." Each clause maps to something in the app:

- **A stronger processor:** the Mac.
- **Many inputs and outputs:** Core Audio devices.
- **Instruments and sequencing:** instrument plugins, MIDI and the piano roll (**Decided**).
- **Makes discs:** File → Make a disc, which writes PRANA's disc layout onto a disc, a USB-C flash stick (5.15).

Shipping the software first gives Mi_pro_WWAV users before it has a case.

**The app sells the hardware** in three ways, none of them an advertisement:

- **The disc round trip.** Make a disc in the Console, play it on a PRANA, and bring the remixes home with their lineage (5.15).
- **The plinth.** Unquantized's drop plinth holds the device as an object you can turn in your hands. Pledges are charged only if the drop funds (7.10).
- **Founding members first.** A Founding seat (10.4) carries a reserved place in line for the device. It is not a discount on it.

**Post 34's roadmap.** Each step is quoted from the post, beside the app's part in it:

| Step | What the app does |
|---|---|
| "1. build beta v1" | Heat's WWAV workspace holds the milestones (its placeholder today reads "Beta v1 working"). The Console writes test discs. |
| "2. ^ use beta v1 to find cofounders" | Letters in Space show the build. The published gates and left-out tables show the inside, which is gate 2.5's question: "Would you show the inside to someone you respect, who knows what they're looking at?" |
| "3. build beta v2 with cofounders" | Testers' disc round trips come back as remixes with lineage. A survey can't give that test data. |
| "4. build out a kickstarter campaign" | The campaign runs on Kickstarter. The plinth, a letter to Wi-WWAV and the Founding list point to it. |
| "5. start selling preorders & manufacturing beta v3" | Late pledges go through the plinth once the campaign closes. Heat holds the manufacturing timeline. |

**Open: Kickstarter or the store's own preorders.** Kickstarter adds a 5% platform fee on top of payment processing; the plinth costs only Stripe's processing. *Recommendation:* run step 4 on Kickstarter, because the point of step 4 is reaching people who already back hardware. Use the plinth for everything after the campaign closes.

### 10.3 Wi-WWAV: the community

"I think Wi-Wwav (we wave) is a perfect name for the people who are customers and fans of wwav, we are the wwav, so just shorten it and its Wi-WWAV." (devlog, Sep 30). The app takes its working name from this line: Wi_WWAV is the place and Wi-WWAV is the people in it. The letters keep the founder's hyphen. Hyphen or underscore is an **Open** naming decision (see 11).

- **Letters, not posts.** Since the second post, every devlog post opens "Dear Wi-WWAV," and is signed "LMY". In the app, a letter is a page with a greeting and a sign-off (see 4.8). The app's own release notes are letters too: "Dear Wi-WWAV, 0.4 makes discs. …". They appear on LMY's sun and in "Since you last looked", and are never pushed.
- **Building in public.** The devlog is public, and Claude can read it through the `/mcp` connector (**Exists today**). The new repo publishes its `GATES.md` with fail criteria before results, as Wi did, and every chapter's "Left out, and why" table. That is what a future cofounder reads first.
- **No audience, only people.** There are no followers to count. You add a galaxy, and its letters reach you when you look (see 4.11), the founder's included.
- **Numbered first members.** A Founding member can show their number on their sun ("Founding #12"). It is off by default, because a badge that only some people can have works as status.

### 10.4 Money

#### Revenue lines

| Line | What is sold | Price | WWAV keeps | Status |
|---|---|---|---|---|
| Store fee | songs (master and four stems), films, writing, clothes, tickets | set by the seller: up to $2,000 for digital work, $1–$10,000 for clothes | 10% of the total, included in the price | **Exists today** (`purchase.js`) |
| Pro | cloud splits and hosting room | one price list, **Open** below | all, less processing | exists in three versions |
| Founding Member | Pro for life and a number from 1 to 500 (**Exists today**); first in line for the device (**Proposed**) | $199.99 once | all, less processing | **Exists today** on iOS (`foundingMember.js`; the client says $199.99, the `iap.js` header $249.99) |
| Split packs | 50 cloud splits | $9.99 | all, less processing | **Exists today** on iOS |
| Hardware | Mi-WWAV preorders | **Open** (below) | the margin | planned (post 34) |
| Commissions | LMY's time: songs, mixes, film scores | the portfolio's price sheet | all, less processing | the quote flow exists; payment isn't wired |
| Education | cohorts | $500 a seat, 12 seats | all, less processing | **Exists today**, out of date |

If all 500 Founding seats sell, they bring in $99,995 before fees, and that money arrives before the hardware does.

#### Three price generations, one price list

| Generation | Where | Prices |
|---|---|---|
| Web PRO (v1) | Stripe, with the amount kept in an environment variable; CLAUDE.md | $11 a month with a 14-day trial. The header of `iap.js` and `APP_STORE_READINESS_PLAN.md` extend it to $11 / $99 / $249.99 / $12 |
| iOS client | `ios_v3/…/Products.swift`, `MiWwav.storekit` | $7.99 a month, $69.99 a year, $199.99 Founding, $9.99 for 50 splits |
| Ops doc | `MONETIZATION_SETUP.md` | $4.99 a month, $19.99 a year, 3-day trial |

`Users` also carries three entitlement fields at once: `isPro`/`proExpiresAt`, `tier` and `credits`. The desktop app reads only `tier`. Today only Apple verification (`server/routes/iap.js`) sets it. The Stripe webhook (`server/routes/subscription.js`) sets `isPro`/`proExpiresAt`, so it must be changed to set `tier` as well, which is new server work. A plan bought on the Mac is the same plan on the iPhone, and the iOS app still sells it through Apple.

**Open: one desktop price list.** *Recommendation:*

| Plan | Price | What it pays for |
|---|---|---|
| Wi_WWAV | free | Everything that runs on your computer: Heat; the Console with plugins and unmetered local splits; export; Make a disc. Also 10 GB of published work hosted in your galaxy (about 45 songs as `.wwav`), a shelf at the 10% fee, and 10 cloud splits for life |
| Pro | $7.99 a month or $69.99 a year | What runs on WWAV's computers: 100 cloud splits a month from the iPhone or the web, and 100 GB of published work hosted |
| Founding | $199.99 once, seats 1–500 | Pro for life, the number, and a reserved place in line for the device |
| Split pack | $9.99 for 50 | Cloud splits that keep until you use them |

The plan sheet states the rule behind this list as Pro's own description: "Pro pays for the parts of WWAV that run on our computers. What runs on yours is free."

- **Files on a shelf don't count toward hosting.** The store fee pays for them.
- **Storage cost.** At R2's list price of $0.015 per GB-month, a full Pro allowance costs $1.50 a month to store.
- **Why $7.99.** It is the newest price in shipped code, so one number can hold on every platform. A $7.99 month through Stripe nets about $7.46 after the standard US card fee (2.9% + 30¢). On iOS, Apple keeps 15–30%.

**Splits after local Demucs.** In v3 a split was the unit of currency, because each one costs a Replicate GPU run. On a Mac, Demucs runs in a background worker for free (see 5.10; v3's dev server took 30–60 s a song on Apple Silicon), so the desktop meters nothing local. Splits remain a unit only for cloud splitting, which phones and the web still need, and the quota in `server/utils/tier.js` and the reserve-and-refund transaction in `splits.js` stay as they are (**Exists today**).

**Open: expiring packs.** Packs expire 90 days after purchase today. *Recommendation:* they keep until used. An expiry date is a timer that pushes use, which is gate 1.1's concern, and the money for a pack has already been paid.

**Founding Member.** **Exists today:** the 500-seat counter is one server row behind a row lock, and a `foundingMemberNumber` never changes. The desktop sells the same seats through Stripe, against the same counter and 10-minute reservation. Checkout shows your number before you pay: "You'd be Founding #213." Nothing counts down the seats left. Once the last one goes, the plan sheet says what v3 says: "Program closed — all 500 slots claimed".

**Claude isn't sold.** Task scoring, reading school mail, feedback and the clerk are free for everyone. Each account has a daily limit set by the server (Proposed: 50 calls), and the founder's key pays (see 2). The desktop app uses no credits. Ripple Creator's credits (15 for a chat, 40 for a strategy) stay with the old iOS app.

**Commissions.** LMY's booth in Unquantized has a commission card that opens the portfolio's quote flow (**Exists today:** `portfolio/src/components/rooms/services/QuoteFlow.jsx`).

- **The flow.** "Are you a filmmaker or a musician?", then the price sheet ($1,000 a song with 2 revisions, $200 mixing, $200 mastering, $400 per 5-minute block of film score) and "Your first song is on me."
- **The deposit.** The $200 refundable deposit page still reads "Payments are being connected". In the app it goes through the store's checkout, and a paid deposit becomes a Heat project from the Song / Project Brief template (see 3).

**Open: commissions for every maker.** *Recommendation:* after v1, give every maker the same card with their own price sheet, at the 10% fee.

**Education.** The conservatory is already written (**Exists today:** `education.js`): 4 courses, 42 lessons, 126 assignments and about 157 hours, with each lesson's three assignments tagged study → make → refine.

- **Lessons.** A lesson opens as a Console session with its stems loaded, and its three assignments land in Heat. Lessons are free, because they teach the tool and they are the shortest path to gate 4.
- **Cohorts.** What is sold is a person's time: 12 seats at $500, refundable until the start (as `cohort.js` does), run as a class solar system in Space (see 4.14). `cohort.js` holds one date, July 1, 2026, which has passed, so it needs a date per cohort.

#### Anti-luxury pricing

"teenage engineering only does one thing wrong in my opinion and its seriously wrong. There products are luxury items which is codename for inaccessible to poor ppl." (devlog, Sep 30). These rules make it checkable:

- **Never gate leaving with your own work.** Export, downloads and Export everything are free and work while signed out (see 2). A bought file is the exact file, DRM-free, with its sha256 on the receipt (see 7).
- **No feature works only with expensive gear:**
  - Demucs runs on the CPU on Intel Macs. It is slower and says so (5.10).
  - The built-in instruments and effects can finish a song without any third-party plugin.
  - The Console records from the built-in microphone.
  - Unquantized's List view gives the whole store on any machine (7.17).
  - Performance targets a 2020 M1 MacBook Air, not a new Pro.
- **Free is a whole product.** Someone who never pays can plan a term, finish a song, put it in Space and sell it.
- **Every price is the whole price.** There is no "from $" and no fee added at checkout. The line item reads "(10% platform fee included)" (**Exists today**).
- **Nothing paid for expires.**
- **The fee is the same for everyone.** Pro buys storage and splits. It never buys a lower fee, a better shelf or more reach.

**Open: the device's price.** The devlog names no price. Its one signal is "TE's best products are like 2k". *Recommendation:* put a price ceiling in the campaign plan before step 4 and announce it in a letter to Wi-WWAV, so "Wwav is about everybody" is a number before it is a campaign page.

### 10.5 School

Heat started as a student's tool. Its prompts describe "a college student in computer engineering and Japanese", and its sync is hard-coded to the University of Rhode Island (see 3).

School matters to the business because of gate 4: "Did someone who doesn't consider themselves an artist make something with it?" The gate "fails until it happens". The Wi wall can't pass it, because it is locked to one account (`wi/GATES.md`). A university is full of people who don't consider themselves artists, and they already open a planner every day.

**Small groups come before the whole school.**

| Stage | Who | Needs | Limit |
|---|---|---|---|
| 1. One | the founder | the iCal feed; Gmail in testing mode | none |
| 2. A small group | 5–20 classmates: a club or one section | nothing hard-coded to one school (3.11); each person on Google's test-user list | Google allows 100 test users while an app is in testing, and makes each sign in again every 7 days |
| 3. A class | one teacher who wants it | a class solar system (4.14); assignments as forks | accounts 13+; grades never appear in Space |
| 4. The school | anyone at URI | Google verification for the restricted Gmail scope, which includes a yearly security assessment; registration in the school's Brightspace | the school's terms |

Stage 2 is the first point where gate 4 can pass. Its fail criterion is written down first:

- Before they start, each person in the group is asked once, in person: "Do you consider yourself an artist?"
- The answers go into `GATES.md`, with each person's permission.
- The gate passes the first time someone who answered no makes something and keeps it.

**The Brightspace ask.** The iCal feed works now and needs no approval: each student pastes their own private calendar link, and Heat reads the due dates. Real scores, course lists and gradebook weights need D2L's Valence REST API or an LTI 1.3 tool, and the school's Brightspace administrator has to register either one. The ask, in plain words:

> We'd like Wi_WWAV registered as a read-only Brightspace app. Each student connects their own account and can disconnect it at any time. It reads their courses, due dates and grades so they can plan their week. Grades stay private to the student: they never appear on anyone's profile, they are never sold or shared, and they are deleted when the student asks. We'll sign the school's data-privacy agreement.

Grades are education records under FERPA. Keeping them private is part of the design, not a setting a student has to find.

**Open: when to ask.** *Recommendation:* ask after stage 2, with the group's weekly reviews as evidence that Heat helps; twenty students make a different request from one.

**Open: money from the school.** Universities fund student ventures through innovation centres, pitch competitions and grants. *Recommendation:* apply to the ones that take no equity, and lead with gate 4 and the conservatory. Before taking school money or using school equipment, read the university's intellectual-property policy for student work, because some policies claim work made with significant school resources.

### 10.6 The gates, applied to the whole app

Rule 2 says to write down what a fail looks like before testing. The new repo's first commit holds a `GATES.md` with the fail criteria below and no results. A check with no evidence is marked open and is never marked passed.

| Gate | Fails if (desktop wording) | Proposed design that passes | Before any test |
|---|---|---|---|
| 1.1 Healthier | anything stretches use past what the person came for: autoplay, a list or floor without an end, notifications, badges, streak counters, "up next" | "Since you last looked", pull only (2); every list ends "That's everything."; the store ends at a wall (7.4); focus rounds never start on their own (3); nothing plays without a press | open; conflicts below |
| 1.2 Freer | any file can't leave as its exact bytes, or the whole account can't leave in one action | Export everything with ⌘⇧E, even when signed out (2) | open; testable at the first slice |
| 1.3 Not addicted | anyone, owners included, is shown a count of other people's attention, or anything is ordered by engagement or sales | private saves; Add galaxy; newest first; sales appear only as a money ledger in the seller's back room, never ranked and never on the floor (7.12) | open; read every API field |
| 1.4 Business | see the criterion proposed below | the revenue lines in 10.4 | open |
| 2.1 Complexity | for any room's verb, steps after ≥ steps before, counted by doing them | Heat: *knowing what to do next*. Space: *hearing a song apart*. Console: *finishing a song that comes apart*, and *cutting a film to its score on one clock*. Unquantized: *buying a file from the person who made it* | open; count at each room's first build |
| 2.2 Flourishing | no positive answer after a month | *a term planned and kept, a song finished that comes apart, and a shelf that pays*; Heat's own records are the month of evidence | open |
| 2.3 Freedom | Wi's (a)–(c), plus (d): a session, export or purchase needs WWAV's server to open | offline first (2); files play in `ffmpeg` and the pack tools; purchases are plain files | open |
| 2.4 Being & body | chapter 2's desktop bars: body text 7:1 and secondary 4.5:1, 44 × 44 pt targets, every gesture on a key, nothing moves while nothing plays, nothing plays without a press | the store's door press starts sound and motion together (7.2); the starfield moves only while something plays (4) | open; dense rows need their own rule (3.18) |
| 2.5 Craft | an independent review finds a bug that is left unfixed; the tests aren't green; one plugin can take the app down | the audio engine runs as a separate process and restarts with the session reloaded (**Decided**) | open |
| 2.6 Wisdom | no written list of what was left out | 10.7, plus every chapter's own table | passes once this document is committed |
| 3.1 Immortal | scored 1–10 | the value sits in files (RIFF/WAVE from 1991, ISO BMFF from 2001, JSON); the web UI dies first, then plugin formats | unscored |
| 3.2 Invisible | seconds spent thinking about the tool | proxy: presses from opening a `.wwav` in Finder to hearing it apart (target 2: double-click, Space) | unscored |
| 3.3 Paradigm | the behaviour that stops | a song sold as a sealed, flattened file; a label standing between an artist and a buyer | unscored |
| 3.4 Fresh | how far outside its category it sits | it adopts the DAW, the social network and the store, and moves outside them by sharing one file across all three | unscored |
| 3.5 Anti-entropy | what accumulates and what decays | lineage links and Heat's records accumulate; signed links and plugin compatibility decay | unscored |
| 4 Mission | fails until it happens | the school stages (10.5) | fails |

**A fail criterion for 1.4.** On the Wi wall, 1.4 stays open because no business runs on it. The proposed wording below would be committed before the store opens:

> Fails if, 12 months after the store takes its first payment, the store fee, Pro, Founding and split packs together don't cover the server's running costs (Heroku, Postgres, R2, Replicate, the Anthropic API, Apple's developer program); or if any revenue line depends on a count of attention.

Hardware and commissions are left out of the sum on purpose: the platform has to carry itself.

**Where the sketch and the gates disagree.** In each row, the Proposed default is the design that passes the gate, and each row is **Open**, with the recommendation shown:

| Sketch or earlier WWAV | Gate | Proposed default | Recommendation | See |
|---|---|---|---|---|
| Habit streaks | 1.1 names streaks | a record that only grows: "Done 41 days since August 26" | a streak counter as a per-habit setting, off by default | 3.9 |
| Deadline reminders | 1.1 | none | one alarm you set yourself, off by default | 2 |
| "walk around infinitely" | 1.1 | one continuous floor that ends at "That's everything." | finite; the drift supplies the endlessness | 7.4 |
| Coffee-shop ambiance | 2.4 | starts when you press the door; one switch turns off sound and motion together | the default | 7.2 |
| Mii-like crowds | 1.3 | makers at their booths, plus your own party | never strangers, never counts | 7.5 |
| Likes | 1.3 | private saves, never counted | the default | 4.11 |
| Follows and follower counts | 1.3 | Add galaxy, kept private | if anything, a single line such as "LMY added your galaxy", never a number | 4.11 |
| Push notifications, badges | 1.1 | "Since you last looked" | the default | 2 |
| Popularity, trending, bestsellers | 1.3 | newest, artist, family or alphabet | the default | 4.10, 7.13 |
| Fuel economy and aimed comets | 1.3 | fuel dropped; chance kept (wormholes, random comets) | drop everything that is paid for with attention | 4.12 |
| Drop progress bars | 1.3 | "Funded" or "Not funded yet" | the default | 7.10 |
| Challenges (live now: `wwav-2026`, ends Oct 23, US Eastern, which is 2026-10-24 04:00 UTC) | 1.3 | one entry and one vote each (**Exists today**); vote totals never shown; the winner is named in a letter | keep, with totals hidden | — |
| Paid promotion | the spirit of 1.3 | nothing for sale | money decides only what a file costs | 7.15 |

### 10.7 What Wi_WWAV doesn't do, and why

| Left out | Why |
|---|---|
| Ads | An ad sells a person's attention to someone else, and the business has to work without selling attention (gate 1.4). |
| Selling or sharing data | A planner, a mailbox and a gradebook are private, and grades are FERPA records. |
| Paid placement, boosts, featured slots | Money decides what a file costs, not who sees it. |
| Likes, plays, views, follower counts | "A count of other people's attention invites checking and comparing." |
| Rankings, "For You", trending | "Nothing decides for you what's worth seeing." |
| Notifications and Dock badges | Anything that reaches out keeps a person past what they came for (gate 1.1). |
| Streak counters by default | A streak that can break pulls you back to protect a number instead of to do the thing. |
| A paywall on export or on your own files | "Never gate leaving with your own work." |
| DRM | A bought file has to play in tools that know nothing of WWAV (gate 2.3). |
| The Mac App Store | Its sandbox blocks loading other developers' plugins, and Apple takes a cut of every sale. |
| Credits, fuel or any in-app currency | A currency between a person and their money hides what things cost, and it can be paid in attention. A split pack isn't one: it buys one service, cloud splits, at a dollar price, and keeps until used. |
| A lower fee or a better shelf for Pro | The fee is the same for everyone, so paying never buys reach. |
| Purchases that expire | Money already paid shouldn't run out on a timer. |
| Rights to your masters | WWAV hosts and delivers your work; taking your masters is what labels do, and WWAV exists against that. |
| A device that needs the app | Mi-WWAV plays its discs (USB flash sticks in an acrylic case) without an account; the app makes discs and is not a key to the device. |
| Accounts under 13 | Younger students take part through a teacher's account, played to the room. |


## 11. Roadmap, open decisions and glossary

This chapter covers four things: the order things get built in, what the first two weeks must prove, every decision still open, and what the words in this document mean. The build order and the estimates are **Decided**: the founder settled them in conversation. What each stage contains, and how each one is checked, is **Proposed**.

### 11.1 Build order

The order follows the file. Every room reads and writes `.wwav` and `.swav`, so the format spine comes first. Each later stage works on its own if work stops there.

| Stage | What exists at the end | Estimate |
|---|---|---|
| 0. Thin slice | the seven items in 11.3, end to end, passing on a second Mac | 1–2 weeks |
| 1. Shell and library | the window, room switcher and Now strip; `~/Music/Wi_WWAV/` with import, reader verdicts, tags and pins; ⌘K, ⌘⇧N and labelled ⌘Z; sign-in through the browser; the upload queue; Export everything with its offline `index.html` (see 2) | grows out of the slice, alongside stage 2; not estimated on its own |
| 2. Heat | Today with the time column, Plan my day and the Pomodoro timer; Tasks, Calendar, Grades, Habits and Mail; spaces; the Brightspace iCal feed and Google sign-in; the artifact's records imported with every id kept (see 3.15) | about 1 week |
| 3. Console | first, a song you can finish: audio and instrument tracks, VST3 and AU, MIDI and the piano roll, takes, the four stem buses, local splits, `.wwav` export, Make a disc. Then video: cuts, the grade, titles, generators, `.swav` export (see 5) | 2–4 months to finishing a song; video adds 1–2 months, overlapping |
| 4. Space | your galaxy over the existing `/api/v2` routes, the planet player and deck, ↑ Push, lineage at three scales, Newest, "Since you last looked", publishing by drop (see 4) | 2–4 weeks |
| 5. Unquantized | the door, plaza and four halls, booths, listening booths, screening room, reading table, rails, the counter and bag, the back room, List view; every fix in 7.20 before the store takes a stranger's money | 1–2 months |
| 6. Windows | the same app on WebView2, ASIO or WASAPI, VST3, DX12 and Authenticode (see 9.9) | 3–6 weeks after the Mac |

Worked most days, that is roughly 4–8 months to a v1 of all four rooms (**Decided** estimate).

**Why this order.**

- **Heat comes second** because it takes a week and gets used every day. Its records become the month of evidence gate 2.2 asks for.
- **The Console comes third** because it is the longest and riskiest stage, and Space and the store have nothing new to carry until it exports.
- **Space comes before the store** because a record in the shop is a planet with a price, and the server routes Space needs already exist.
- **The store comes last** because it takes money, and the gaps listed in 7.20 must close first.
- **Windows waits** for a Mac v1, so it ports a finished app rather than a moving one.

**What could move the dates.** Three things were named as uncertain, and none can be estimated from here. Each has a way to make it smaller:

| Uncertain | Why | How it is made smaller |
|---|---|---|
| The plugin long tail | every plugin breaks a host in its own way, and the failures show up in use, not in a scan | the founder's own plugin folder is the test set from stage 0; the engine logs crashes per plugin; `crasher` and `hanger` test the recovery path (9.11) |
| Audio stability | dropouts, devices changing mid-session, and take alignment that v5's own commit calls "untested on hardware" | the loopback test (100 recordings within ±1 sample), the 8-hour soak and the 200 random kills, each with its fail criterion written first |
| Feel | whether 250 ms, 400 ms, 8 pt, the 140 ms beat and a 4.4 m/s walk feel right can only be judged by playing | a session with the founder at the end of every stage, and every timing kept as one named constant |

**After v1.** These wait until all four rooms run, each for the reason its chapter gives:

- parties ("Come with me", 7.19);
- "Near you" (4.10);
- commissions for every maker (10.4);
- a PRANA view in the app (5.15);
- PRANA as a USB controller, after Beta 1 (5.6);
- the clip-launch decision (5.2);
- film stems (6.10);
- Gi's pattern file (6.11);
- an ARM64 Windows engine (9.9).

### 11.2 Who can check what

An agent working alone can build and verify much of the app on Linux. Some checks need a Mac, and some can only be judged by the founder. When an agent reaches one of those, it writes the question into `docs/QUESTIONS.md` and moves on to other work rather than guessing (9.12).

| Area | Without the founder's Mac (Linux CI, an agent alone) | Needs a Mac | Only the founder can judge |
|---|---|---|---|
| Files | the Rust `.wwav` and `.swav` writers, byte for byte against `wwav_pack.py` and `swav_pack.py`; one verdict from all four readers; a session that reopens identical | the Finder package icon; double-click opening a `.wwav` as a planet | whether 44.1 kHz holds into 1.0 (6.7) |
| Audio | golden renders against PRANA's hashes; Surge XT as VST3; `delay-n`; 200 kills with `crasher` and `hanger` | AU hosting; CoreAudio and CoreMIDI devices; the loopback latency test (an interface and a cable); the 8-hour soak on the reference M1 Air | whether a song can be finished in the Console; how the founder's own plugins behave |
| Video | frame-indexed export maths; FFmpeg software decode | VideoToolbox decode and encode; the `CAMetalLayer` presenter; the flash-and-click test; camera takes | whether cutting feels like cutting |
| Rooms | the web UI in Playwright against `mock-engine`; Heat's maths (heat, Plan my day, grades); Space's layout hashes; journal undo-all and redo-all | WKWebView behaviour; Keychain; Force Touch; Reduce Motion; 60 fps on the reference machine | gesture timings; Lucida or Inter; 17 or 19 pt at the founder's desk; the shop's room sound and house music |
| Server | `/api/entitlements`, multipart parts, Heat sync, the bag and every 7.20 fix, against test Postgres and an R2 stand-in, as Wi's gates were run | — | prices, the fee, royalties to ancestors |
| Accounts | the iCal parser against a stored raw feed | — | the founder's Brightspace link; Google sign-in in testing mode; Stripe live keys; the Apple Developer account and its Developer ID |
| Shipping | the universal build script | signing, hardened runtime, entitlements, notarization, and Gatekeeper on a second Mac | when anyone else gets a copy |
| Gates | every fail criterion, written before testing | gate 2.4's measurements at 1024 × 680 and 1280 × 800 | gate 2.2's month of use; gate 4's question asked in person; every decision in 11.4 |

### 11.3 The first thin slice

The slice proves the architecture with the least of each part: two processes, the shared clock, one plugin, one picture, both formats and a signed build. These are the parts that cost the most to change later. Each item's fail criterion goes into `docs/PLAN.md` before its code is written. The slice is done only when all seven items pass on a second Mac that has never built the app.

| # | Item | Fails if |
|---|---|---|
| 1 | The app opens | a cold launch on the reference machine takes over 1.5 s to show the window, or ⌘1–⌘4 doesn't switch between four empty rooms |
| 2 | The engine starts | `wwav-engine` shows a Dock icon, or the app waits on the engine to draw, or after `kill -9` during playback the engine isn't back within 2 s with the transport stopped at the same playhead |
| 3 | One `.wwav` plays its four stems | the app's verdict differs from `wwav_pack.py info`; a click on a stem light isn't heard within 10 ms; or a second click within 250 ms doesn't revert the mute and solo instead |
| 4 | One third-party effect, with its window | a VST3 or AU effect from the founder's plugin folder doesn't load into the engine; its window doesn't open as the engine's own; its state isn't restored after a restart; or a key it doesn't use (Space, ⌘Z) fails to reach the app |
| 5 | One synced video lane | the flash-and-click test film shows a flash more than one frame away from its click, or drops frames at 1080p |
| 6 | `.swav` export | ffprobe reports an error; `swav_pack.py info` can't read its `wmet` and `wlin`; unpacking doesn't return the encoded MP4 byte for byte; or the exported flash and click land on different frames |
| 7 | Signed and notarized, on a second Mac | downloaded through a browser onto a Mac that never built it, the app draws a Gatekeeper warning; `spctl --assess` or `codesign --verify --deep --strict` fails; or the engine refuses item 4's plugin, which is what `disable-library-validation` is for |

**What the slice needs from the founder:**

- an Apple Developer account ($99 a year) and its Developer ID certificate;
- a second Mac;
- one effect plugin the founder actually uses.

**What it leaves out:** Heat, Space, the store, the library beyond one file, instrument tracks and the piano roll (MIDI and instrument plugins still arrive at the start of stage 3, as **Decided**, and the slice's engine protocol and session format carry MIDI events from the first commit), and Windows. Each is a later stage. The slice exists to learn early whether the two processes, the clock and signing behave.

### 11.4 Open decisions

This list holds every **Open** item in chapters 1–10, with duplicates merged. Each goes into `docs/QUESTIONS.md` with its recommendation. If a stage reaches one before it has an answer, the stage builds the recommendation as written and logs it in `docs/DECISIONS.md`, so no stage waits.

#### The whole app

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

#### Heat

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

#### Space

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

#### Console and files

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

#### Unquantized

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

#### Engineering and shipping

| # | Decision | From | Recommendation |
|---|---|---|---|
| 48 | `allow-unsigned-executable-memory` for copy-protected plugins | 9.9 | Add it only if a plugin in the test set fails without it. |
| 49 | Intel-only plugins | 9.9 | Offer "Open the engine under Rosetta" per session, as a bridge. |
| 50 | Minimum macOS | 9.9 | macOS 13. |
| 51 | A self-hosted Mac mini CI runner | 9.11 | Add one once macOS runner minutes pass about $50 a month. |
| 52 | The `formats/` submodule | 9.12 | A sparse checkout of Mi-WWAV now. Split the four folders out only if clone times start to hurt. |

#### Business and school

| # | Decision | From | Recommendation |
|---|---|---|---|
| 53 | Kickstarter or the store's own preorders | 10.2 | Kickstarter for roadmap step 4, and the plinth for every pledge after the campaign closes. |
| 54 | One desktop price list | 10.4 | A free plan (all local features, 10 GB hosted, 10 cloud splits); Pro at $7.99 a month or $69.99 a year; Founding at $199.99 once, seats 1–500; a split pack at $9.99 for 50; one `tier` entitlement everywhere. |
| 55 | Expiring split packs | 10.4 | Packs keep until used. |
| 56 | Commissions for every maker | 10.4 | After v1, the same commission card with each maker's own price sheet, at the 10% fee. |
| 57 | The device's price | 10.4 | Write a price ceiling into the campaign plan before step 4, and announce it in a letter to Wi-WWAV. |
| 58 | Money from the school | 10.5 | No-equity grants and competitions, leading with gate 4. Read the university's IP policy for student work first. |
| 59 | A fail criterion for gate 1.4 | 10.6 | Adopt it: the gate fails if, 12 months after the store's first payment, the store fee, Pro, Founding and packs don't cover server running costs, or if any line depends on a count of attention. |

### 11.5 Glossary

| Term | Meaning | See |
|---|---|---|
| `.gwav` | Placeholder name for Gi_WWAV's pattern file: SVG pattern pieces with identity in their metadata (**Open**) | 6.11 |
| `.rwav` | Placeholder name for a Ri_WWAV writing file; until it exists, writing is Markdown with `wmet` and `wlin` front matter | 6.11 |
| `.swav` | A film as one file: any ISO BMFF film (MP4, MOV) with `wmet` and `wlin` boxes appended; version 0.1 exists | 6.2 |
| `.wwav` | A song as one file: a RIFF WAVE whose `data` chunk is the master, with four stems in `wstm`, plus `wmet`, `wlin` and, for remixes, `wrmx`; 44.1 kHz, 16-bit; version 0.1 exists | 6.1 |
| `.wwavsession` | A Console session as a package: `session.json`, `media/`, `plugin-state/`, `renders/`, `journal/`, `cache/` | 6.5 |
| ↑ Push | Forks the mix you hear into the song's lineage without stopping playback; the fork owns no audio | 4.6 |
| ＋ add a song | Joins a second song to the first as a figure-eight, matched by tempo, key and first downbeat | 4.6 |
| ⌘K, ⌘⇧N, ⌘L | The command palette, quick capture, and the library drawer, in every room | 2 |
| Add | The private save that replaces the like; it goes to your Saved shelf and is never counted | 4.11 |
| Add galaxy | The private replacement for following; it feeds Newest and "Since you last looked" | 4.11 |
| Aqua | The classic Mac OS X look Heat wears: brushed metal, gel buttons, striped rows; its highlight `#3875D7` is the desk's one accent | 8.2 |
| Arbor Vitae | The founder's separate Shopify clothing store | 7.9 |
| Astronaut | Your body in Space and Unquantized, stored as IDs (race, complexion, physique, hair, cosmetics, rocket), never as art | 2, 7.5 |
| AU, VST3 | The plugin formats the engine hosts: AU on the Mac, VST3 on both; CLAP and AAX are left out | 9.4 |
| Back room | The door behind your own booth: the sales ledger, orders to ship, payouts, notes for the clerk | 7.12 |
| Bag | Several shop items paid for in one checkout; new server work | 7.11 |
| Bio sun | The sun at the centre of your galaxy: your page | 4.8 |
| Block | A span of time in Today's time column, given to a task or a habit | 3.5 |
| Booth | A maker's place on the shop floor, on a track around its hall's centrepiece | 7.3, 7.6 |
| Brightspace | D2L's learning system, used at URI; Heat reads its iCal feed now and Valence or LTI later | 3.11 |
| Case metal, deck metal | Heat's brushed metal (title bar, sheets) and MI-WWAV-OS's paler pinstriped metal (mixer strips, device cards) | 8.2 |
| Claude | Anthropic's model. It scores tasks, reads school mail, gives feedback and serves as the clerk; it estimates and drafts and never decides; calls go through mi-wwav.com | 2, 9.8 |
| Class system | An unlisted solar system for a class, with the teacher's brief as its sun | 4.14 |
| Clerk, the | Claude behind Unquantized's counter, answering about one record from what the file and its seller say | 7.16 |
| Clip | Any one thing in the library: a `.wwav`, a `.swav`, plain audio or video, an image or text (from MI-WWAV-OS) | 2 |
| Come with me | Parties of up to four people walking the shop together; after v1 | 7.19 |
| Comet | v5's chance visitor: one world carried into someone else's sky for 6 hours; aimed comets are dropped | 4.12 |
| Connected distribution | Space and Unquantized: a for-sale planet links to its shelf, and purchases land in your library | 1.3, 2 |
| Connected profiles | Heat and Space: only the lines you drop on your galaxy cross | 1.3, 3.14 |
| Connected selling | Console and Unquantized: an export becomes a record on your shelf, paid out through Stripe Connect | 1.3, 7.12 |
| Connected uploads | Console and Space: exports go up as exact bytes, and planets come back as sessions | 1.3, 4.13 |
| Console | The room for making (⌘3), and the software side of Mi_pro_WWAV | 5 |
| Constellation | Strands of light between two galaxies, brighter the more work has passed between them | 4.9 |
| Cool, Warm, Hot, Overdue | Heat's four levels: under 0.34, from 0.34, from 0.70, and past due | 3.1 |
| Crate | One solar system on a booth's shelf, holding up to 21 records | 7.6 |
| Crater | The portfolio's Turrell-lit page, source of the shop's amber lamps and its sunset sky | 8.1 |
| Crema | The shop's one accent, `#C89668`, used as a light and never as text | 8.2 |
| Current task | The one task that focus time is logged to; set with C | 3.5 |
| Decided, Proposed, Open, Exists today | This document's four status labels | front matter |
| Delay compensation | Automatic alignment of signal paths through plugins that report latency | 5.7, 9.4 |
| Demucs | The stem-splitting model (htdemucs, four stems); it runs locally in the Console and on Replicate for phones | 5.10 |
| Desk, night, interior | The three visual registers: Heat and the Console's chrome, Space, and Unquantized | 8.2 |
| Device parity | A 44.1 kHz session of built-in devices renders bit for bit as PRANA would: "Matches PRANA" | 5.9 |
| DISCMAN | The codename of WWAV v2 (web) and v3 (iOS), spring 2026 | 1.5, 4.1 |
| DMG screen | A Game Boy-green readout behind glass in the Console | 5.2, 8.2 |
| Drop | How publishing works: drag a work onto a system, a shelf or a room segment | 2 |
| Engine | `wwav-engine`, the separate JUCE process that owns audio, MIDI and plugins, and is the master clock | 9.1 |
| Estimate chain | A task's own estimate, else its type's average, else difficulty × 20 minutes | 3.1 |
| Export everything | One action (⌘⇧E) that writes every file, session, purchase and record, with an offline `index.html` | 2 |
| FERPA | The US law on education records; grades stay private by default | 3.11, 10.5 |
| Figure-eight | Two songs joined, the second bent to the first's tempo and key | 4.6 |
| Fold rule, fold check | How N tracks become four stems, and the export test that the stems sum to the master | 5.3, 6.6 |
| Fork | A work taken somewhere new, by anyone: a new id with a parent. Fork edges are facts, not claims | 4.9, 6.8 |
| Founding Member | $199.99 once, seats 1–500: Pro for life, a fixed number, and a place in line for the device | 10.4 |
| FX moons | Reverb, delay, distortion and tremolo, which bloom on the diagonals after a 400 ms hold; distance is dry/wet | 4.6 |
| Galaxy | One person in Space: a bio sun and their solar systems | 4.2 |
| Gallery planet | A photo or fashion work: a lit sphere with up to 40 photos pinned on | 4.7 |
| Gates, the | The founder's written design audit (`wi/GATES.md`): 1 corruption, 2 good design, 3 great design (scored), 4 mission. Fail criteria are written before testing | 10.6 |
| Generation | How many forks a work is from its root (`wlin.generation`) | 6.1 |
| Generator track | The video synth as a Console track, driven by the stems | 5.11 |
| Get Info | Heat's inspector drawer for one task | 3.1 |
| Gi_WWAV | The garments family (fashion) | 1.2, 10.2 |
| Grade, the | A film's saturation, contrast, brightness, RGB curves and varispeed; a grade fork changes only these | 4.7, 5.11 |
| Hall | One of the shop's four wings (Mi, Ri, Si, Gi), each a solar system of booths | 7.3 |
| Heat | The room for planning time (⌘1), grown from the founder's claude.ai artifact of the same name | 3 |
| Heat score | v = 1 − days left ÷ runway, clamped to 0–1; 1.1 when overdue, 0.05 with no due date | 3.1 |
| JUCE | The C++ audio framework (version 8) the engine and the scanner are built on | 9.1 |
| Key colour | A work's colour: hue = ((pc · 7) mod 12) · 30 with A = 0; major hsl(h, 72%, 58%), minor hsl(h, 58%, 42%); night indigo, hue 232, when the key is unknown | 4.7, 8.3 |
| Kepler motion | Worlds and booths move on real ellipses; in Space only while something plays | 4.4, 7.3 |
| Letter | A page with a greeting and a sign-off; every devlog post is one ("Dear Wi-WWAV," … LMY) | 4.8, 10.3 |
| Library | The folder `~/Music/Wi_WWAV/`: `library.sqlite`, `media/`, `sessions/`, `purchases/`, `trash/` | 2, 9.6 |
| Lineage | The family tree of works, carried in each file's `wlin`: "the only social graph" | 4.9 |
| Lineage link | A claim between works (influence, sample, collab, cover, custom) that waits for the other owner's **Agree** | 4.9 |
| Lineage share | The proposed cut of a remix sale (0–30%, default 10%) paid to the parent's maker | 7.14 |
| List view | The whole shop as a 2D list (⌘⇧L) | 7.17 |
| Listening booth | A glass booth in the music hall where a record plays with the deck | 7.6 |
| LMY | Liam, the founder; how the letters are signed | 10.3 |
| mi-wwav.com | The existing server, which stays: Heroku, Express, Postgres and Cloudflare R2 | 9.7 |
| Mi_cro_WWAV, Mi_pro_WWAV | Mi's pocket tier and its desktop-studio tier ("Makes discs.") | 10.2 |
| Mi_WWAV | The sound family; PRANA is its first beta | 10.2 |
| Mi-WWAV | How the hardware line is written ("Mi-WWAV Beta 1") | 10.2 |
| MI-WWAV-OS | A separate handheld OS, simulated in Flutter and Rust; the app takes its grammar, timeline, undo journal and DMG screens | 2, 5 |
| Moon | One stem of a song, orbiting its planet; its distance from the planet is its volume | 4.6 |
| New table | The 12 newest records in the whole shop, just inside the door | 7.13 |
| Newest | Space's one list: newest first, 30 at a time, then "That's everything." | 4.10 |
| Now making | An opt-in line on your sun naming what you're working on | 3.14 |
| Now strip | The 520 × 44 pt LCD in the title bar, with the task on the left and the track on the right | 2 |
| OUTPUT row | The arrangement's last row; select it and press Return to export | 5.13 |
| Page planet | A written work as a world: paper with ruled latitude lines | 4.7 |
| PKM | The founder's personal knowledge manager at `/admin/pkm`, the source of capture, recurrence and the weekly review | 3.13 |
| Plan my day | Heat's rule-based drafting of blocks into the day's free time | 3.5 |
| Planet | One work in Space: a song, film, page or gallery | 4.2 |
| Plinth | The shop's place for drops: all-or-nothing preorders, charged only if they fund | 7.10 |
| Pomodoro | Heat's focus timer: 25-minute rounds and 5-minute breaks, with 15 minutes every fourth break | 3.5 |
| PRANA | Mi-WWAV Beta 1: a Teensy 4.1 stem player with four faders and a disc bay. Its C++ core runs the device, its simulator and the app's built-in effects | 5.9, 9.4 |
| PRISMON | The portfolio's first-person three.js engine, which the shop is built on | 7.1 |
| Pro | The paid plan, which pays for what runs on WWAV's computers: cloud splits and hosting | 10.4 |
| Proxy | A half-resolution ProRes copy of footage over 1080p, used for editing | 5.11, 9.5 |
| Remix deck | The player's drawer of fx, eq, comp, time and rec pills, from v3, running PRANA's DSP | 4.6 |
| Ri_WWAV | The words family (writing) | 10.2 |
| Ripple | WWAV's feedback on a work, given in [MM:SS] and never in Hz, dB or LUFS; Ripple Creator writes release plans | 2, 3.14 |
| Room sound | The shop's air, espresso machine and cups, started by the door press | 7.2 |
| Rule 2 | "Write down what a fail looks like before testing" | 10.6 |
| Runway | How many days before a due date heat starts rising: 2 × difficulty + 1 | 3.1 |
| Sampler | The Console's built-in instrument: any clip across the keys, or as 16 pads | 5.3 |
| Screening room | Where films play, in Space and in the shop's film hall | 4.7, 7.7 |
| Secondary act | The one extra action each screen has, on ⇧Return | 2 |
| Sequence | An edit list, never media; a Console session is one | 2, 5.12 |
| Shelf | What you sell, as seen from outside the shop: your booth's crates, "Your shelf" in the astronaut menu, and the "$4 · on the shelf" tag in Space | 7.6, 7.12 |
| Si_WWAV | The sight family (film) | 10.2 |
| Since you last looked | The pull-only list at the top of Space: forks, links waiting on you, sales, new work from galaxies you've added | 2 |
| SOLAR SYSTEM | The codename of WWAV v4 (web, summer 2026, `/summer_26`) and v5 (iOS): the galaxy model | 4.1 |
| Solar system | One project: a sun and up to 21 worlds, seven to a ring | 4.2 |
| `song_id`, `film_id` | 128-bit ids written as 32 hex characters; songs and films share one id space | 6.1 |
| Space | The social universe (⌘2), named after v4's first tab | 4 |
| Spaces | User-defined filters that replace Heat's three fixed workspaces | 3.4 |
| Split, split pack | A Demucs run. Local splits are free, cloud splits are metered, and a pack of 50 costs $9.99 | 5.10, 10.4 |
| Stem | One of a song's four parts, always in the order vocals, drums, other, bass | 6.1 |
| Stem bus | One of the mixer's four buses; its output is that stem in the export | 6.6 |
| Stem colours | PRANA's vocals `#D23C2A`, drums `#F0B90B`, other `#2E9A55`, bass `#1F4E9E`, the same everywhere | 8.3 |
| Stem group | A Console track of four stem lanes, from a `.wwav` or a split | 5.3 |
| Stem player | WWAV's instrument; in Wi_WWAV, the thing your figure holds, wearing your skin | 2 |
| Stem role | The stem a Console track folds into | 5.3 |
| Stripe | Payments: Checkout for buying and Connect Express for paying sellers, with no Apple cut on direct distribution | 7.11 |
| Sun | A page: a bio sun for a person, a project sun for a system | 4.8 |
| Tauri | The app framework (version 2): a Rust core, with the UI in the system web view | 9.1 |
| "That's everything." | How every list, hall and shelf ends | 2, 7.4 |
| Thin remix | A remix stored without its own copy of the stems: "thick files, thin storage" | 6.9 |
| Tier | The one entitlement field the desktop app reads | 10.4 |
| Track (shop) | A booth's elliptical path around its hall's centrepiece | 7.3 |
| Turrell field | PRANA's wall wash, `#F7F4EE→#E6DFD0`, on the shop's walls; it drifts toward rose or cool only while the room sound is on | 8.2 |
| ULID | The time-sortable id that names every media file and session | 2 |
| Undo journal | Every change as a labelled transaction, behind ⌘Z in every room | 2, 9.6 |
| Universe | Every galaxy on one spiral ("Everyone") | 4.2 |
| Unquantized | The shop (⌘4). In the Console, the rule that nothing you play is moved unless you ask | 5.4, 7 |
| URI | The University of Rhode Island, the founder's school | 3, 10.5 |
| Valence, LTI 1.3 | D2L's REST API and the LTI standard: the route to real grades, registered by the school | 3.11 |
| Version | A new export of your own work: the same id, with the version number one higher | 6.8 |
| Video synth | Eight styles and twelve knobs driven by the stems; it exports a `.swav` | 4.6, 5.11 |
| Web face | www.wi-wwav.com as share pages, downloads and public shelves | 9.7 |
| wgpu | The Rust GPU layer that composites video (Metal on Mac, DX12 on Windows) | 9.5 |
| `wgrd` | A proposed `.swav` 0.2 box that carries a grade and a soundtrack as a recipe | 6.10 |
| Wi | The existing private wall at www.wi-wwav.com (`wi/`): one account, every post a `.wwav` or `.swav`; Proposed to become the app's web face | 1.5, 9.7 |
| Wi_WWAV | This app, said "we wave" | 1.5 |
| Wi-WWAV | The community of customers and fans: "we are the wwav" | 1.5, 10.3 |
| Window, the | The shop window, holding one record chosen by chance every 6 hours | 7.15 |
| `wlin` | Lineage JSON: `parent_id`, `root_id`, `generation`, `creator`, `device_id` | 6.1 |
| `wmet` | Identity JSON: format version, `song_id` or `film_id`, title, artist, type, created; songs add bpm, key, frames and splitter | 6.1, 6.2 |
| Wormhole | A free door to a place chosen by chance, open for 24 hours; in the shop, the wormhole door | 4.12, 7.15 |
| `wrmx` | Remix settings JSON in a remix `.wwav`: levels, mutes, effects, pitch, speed, time and filters | 6.1 |
| `wstm` | The four interleaved stereo stems, aligned to 512 bytes | 6.1 |
| WWAV | World-Wide Audio-Visual, built on one idea: "every song comes apart" | 1.1 |
| WWAV Push | The JUCE plugin that gives a song a permanent link on its first export | 5.17 |
| `wwav-scan` | The short-lived process that checks plugins one at a time | 5.7, 9.1 |
| Zi_WWAV | The furniture family, left out of scope | 1.2 |

**Example names.** World Ending and More Love are labels of the founder's albums ("Listen to This When the World is Ending", "More Love Kills the Beast", `portfolio/src/albums.js`). Low Tide, glass hours, Ana, Tape Echo and Grand Piano are examples made up for this document. KeyStep stands in for any USB keyboard controller.

### 11.6 Sources

Every **Exists today** claim in this document comes from these sources, read in October 2026:

- **Heat:** the founder's claude.ai artifact, one HTML file of 1,481 lines. It is not in this repo.
- **The sketch:** four boxes and their connections, as described in conversation.
- **Devlog:** posts 1, 2, 34 and 67 in `DevlogPosts`, read through the `/mcp` connector (`server/routes/devlog.js`, `server/mcp/devlog.js`).
- **Philosophy and names:** `wi/GATES.md`, `wi/README.md`, `archive/src/content.js`, `intro.md`.
- **Formats:**
  - `prana/SPEC.md`, `prana/core/`, `prana/tests/`, `prana/tools/wwav_pack.py`
  - `formats/swav/SPEC.md`, `formats/swav/swav_pack.py`
  - `wi/src/formats/`
- **v3:** `ios_v3/` (`WWAV/Views/PlayView.swift`, `WWAV/Visuals/`, `demucs_server/`, `DESIGN/wwav-screens.jsx`).
- **v4 and v5:**
  - `wwav/src/` (`planet/`, `sun/blocks.js`, `styles/tokens.css`)
  - `ios_v4/`
  - `server/routes/v2/`, `server/utils/universeDrift.js`
- **MI-WWAV-OS:**
  - `MI-WWAV-OS/ARCHITECTURE.md`, `MI-WWAV-OS/proto/README.md`
  - `MI-WWAV-OS/engine/src/` (`model.rs`, `store.rs`, `ids.rs`, `api.rs`, `mixer.rs`)
  - `MI-WWAV-OS/app/lib/style/theme.dart`
- **WWAV Push:** `vst_plugin/`.
- **Commerce, social and Claude:**
  - routes in `server/routes/`: `purchase.js`, `connect.js`, `fashion.js`, `shipping.js`, `films.js`, `stems.js`, `foundingMember.js`, `splits.js`, `local.js`, `forks.js`, `cohort.js`, `rippleCreator.js`, `wi.js`
  - services in `server/services/`: `claudeService.js`, `geminiService.js`, `rippleCreatorService.js`
  - also `server/mcp/auth.js` and `server/utils/tier.js`
- **Portfolio:**
  - `portfolio/src/pkm/`, `portfolio/src/education.js`, `portfolio/src/albums.js`, `portfolio/src/crater.css`
  - `portfolio/src/components/Hub.jsx`, `portfolio/src/components/rooms/prismon/`, `portfolio/src/components/rooms/services/QuoteFlow.jsx`
  - `portfolio/public/media/site-music/`
- **Notes and the rest:** `TODO_2.md`, `todo.md`, `assets/`, `client/src/components/ArborVitaeV2.jsx`, `games/republic/network/relay_server.py`.


