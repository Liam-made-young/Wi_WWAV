# Wi_WWAV

*One desktop app with three views: Heat, where you plan your time and keep a private profile with a simple public face; Space, where you live among other people's work as a galaxy; and the Console, where you make songs and films on one clock.*

- **Working name:** Wi_WWAV, said "we wave".
- **Date:** 6 October 2026. **Scope narrowed:** 7 October 2026, in conversation with the founder (`docs/SCOPE_CUT.md`): three views instead of four rooms, the walkable shop cut (commerce comes later, inside Space), Space and the Console trimmed to their core, and Claude reached through an MCP server.
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
| 1 | [The idea](#1-the-idea) | what the app is, the sketch translated, the two connections, the name, one day with it |
| 2 | [One app, three views](#2-one-app-three-views) | the window, the Now strip, the library, one account, the shared grammar, Claude through MCP |
| 3 | [Heat: the profile view](#3-heat-the-profile-view) | planning time: Today, tasks by heat, focus sessions, grades, habits, school mail; private by default, with a public view; the MCP server |
| 4 | [Space: the social view](#4-space-the-social-view) | the universe of galaxies in real 3D, the planet player, the four media as worlds, lineage, commerce later |
| 5 | [Console: the creation view](#5-console-the-creation-view) | one timeline for audio, MIDI and basic video, with plugins, takes, splits and export |
| 6 | [Files: .wwav, .swav and the session](#6-files-wwav-swav-and-the-session) | the formats byte by byte, the session package, versions and forks |
| 7 | [Look, sound and feel](#7-look-sound-and-feel) | two visual registers, type, motion, sound, words, tokens |
| 8 | [Under the hood](#8-under-the-hood) | the processes, the clock, audio, video, storage, sync, the MCP server, shipping, tests |
| 9 | [Business, community and the gates](#9-business-community-and-the-gates) | money, Wi-WWAV, school, and the whole app against the gates |
| 10 | [Roadmap, open decisions and glossary](#10-roadmap-open-decisions-and-glossary) | build order, the first slice, every open decision, every term |

## 1. The idea

### 1.1 What Wi_WWAV is

Wi_WWAV is one native desktop app with three views (**Decided**).

| View | Key | Is | What you do there |
|---|---|---|---|
| **Heat** | ⌘1 | the profile view | plan your time: today's plan, tasks ranked by heat, a focus timer, grades, habits, school mail. All of it is private by default, and a simple public version shows behind your sun in Space |
| **Space** | ⌘2 | the social view | live among people and their work: every person is a galaxy, every project a solar system, every work a planet |
| **Console** | ⌘3 | the creation view | make: one timeline for audio and video, with plugins, exporting `.wwav` songs and `.swav` films |

The window around the views never changes. Its brushed-metal title bar holds the view switcher and a Now strip that shows the task you're on beside the track you're hearing (see 2).

WWAV stands for "World-Wide Audio-Visual", and it rests on one idea: "every song comes apart". A song is a `.wwav`, an ordinary WAV whose master plays anywhere, with four stems inside in PRANA's order: vocals, drums, other, bass. The same file comes apart in every view. It shows as four lights in the title bar, four moons in Space and four lanes in the Console. A film is a `.swav`, an ordinary MP4 that carries its identity and its parent. Both formats **exist today** (see 6).

The archive gives every WWAV family one model: "a device, a format, and a marketplace. You make the work on the device, it lives in a WWAV format, and creators can sell it." Wi_WWAV is that model in software, for four media at once:

- **The device** is the Console.
- **The format** is the pair `.wwav` and `.swav`.
- **Space** carries work between people. It will hold the marketplace too, later (**Decided**; see 4).
- **Heat** holds the hours all of it takes, and is the profile the work comes from.

Some things are left out on purpose: a feed ranked by popularity, likes and follower counts, notifications, autoplay, and anything paid for with attention. The founder's own gates (`wi/GATES.md`) fail each of them. Where the sketch asks for one, this document makes the passing design the default, names the conflict, and marks it **Open** (see 9.6).

### 1.2 The sketch, translated

The founder's sketch draws four boxes and the lines between them. Three of the boxes become views.

| Sketch box | View | What it is | Reuses (**Exists today**) | Family |
|---|---|---|---|---|
| The organizer, in a window labelled "Wi-WWAV" | Heat, the profile view | Today with a time column, Plan my day and a Pomodoro timer; tasks, calendar, grades, habits, mail; a private PKM with a simple public version | Heat, a claude.ai artifact of 1,481 lines; the PKM (`portfolio/src/pkm/`) | none; it holds the time behind all of them |
| "souped up version of WWAV app v3", an "intergalactic 3D social media for music, film, writing & fashion" | Space, the social view | a universe you move through in real 3D; the player is the planet; the reply to a song is a fork | v3's planet player (`ios_v3/`); v4's galaxy (`wwav/`); v5's orbits and gallery planets (`ios_v4/`, `server/routes/v2/`) | Mi, Si, Ri, Gi as four kinds of world |
| The "Mi-WWAV console": one timeline for audio and video, "the best parts of Premiere and Ableton smashed together" | Console, the creation view | a JUCE engine hosting VST3 and AU, MIDI through third-party instruments, local splits, and plain video cutting on the engine's clock | `prana/core`; MI-WWAV-OS's timeline and undo-journal designs (ideas, not code); WWAV Push (`vst_plugin/`) | Mi (Mi_pro_WWAV's software) and Si |

The fourth box, "unquantized", a walkable store, is not a view (**Decided**). In the founder's words: "Eventually the space will hold commerce instead of having that be a 4th place." Selling comes later, inside Space, and nothing of it is designed yet (see 4).

Zi_WWAV (furniture) stays out, because the sketch names four media and nothing in the app makes a chair.

### 1.3 The two connections

```
Heat ── connected profiles ── Space ── connected uploads ── Console
```

**Connected profiles (Heat and Space).** Heat is a private PKM by default. A simple public version of it, your **public Heat view**, is what anyone sees when they open the sun in the middle of your galaxy (**Decided**). By default it has room for two things, each shown once you press Show: a "Now making" line and the milestone timelines of projects you link to a solar system. Anything else, a task, a habit, a note or a grade, stays private until you switch that one item public. Counts and comparisons between people never cross (3.15).

**Connected uploads (Console and Space).** Dropping an export on a solar system publishes it. The server keeps the exact bytes and reads the work's family from the file's `wlin` chunk. Going back, **Open in Console** (⌘E) on any song or film in Space opens it as a session with four stem lanes, ready to tear apart and fork (4.12, 5.14).

### 1.4 Why one app, and why a desktop

**One app**, because the views share everything that matters: one file format, one account, one library, one undo journal, and one audio engine. That engine's playhead is the clock for the Console, the Now strip and Space's orbits. As three apps, every line in the sketch would be a trip through Finder.

**A desktop**, because the founder's test for hardware holds for software too: "I need to build something irreplaceable by the iphone." Several of the app's jobs need a computer:

- hosting other developers' plugins;
- cutting 4K film against four stems on one clock;
- drawing a universe of galaxies in real 3D at 60 fps. Real 3D is a necessity (**Decided**).

It is a direct download, because the Mac App Store's sandbox blocks plugin hosting (**Decided**).

**A business**, in the founder's words: "I am creating technology for artists and selling it", "an artist first technology company". For now the money comes from hosting and cloud splits, never attention; selling work comes later, inside Space (see 9). "Wwav is about everybody" sets the floor: the free app is a whole product, and the performance target is a 2020 M1 MacBook Air.

**Not yet public.** In the founder's words, it "is not going out for a while" (**Decided**). Privacy defaults stay conservative, and nothing here has to be ready for a public launch now.

The devlog pulls the other way twice. Post 1 wonders about "One medium: music; one screen?", and post 34's roadmap is all hardware. Wi_WWAV doesn't replace the devices; it is the software layer above them. Writing PRANA discs waits for the disc: "PRANA disc not ready yet, so definitely not writing software yet." The app does compete for one founder's hours, so it is built in stages that each stand alone (see 10).

### 1.5 The name

**Wi_WWAV** is said "we wave". The sketch labels the Heat window "Wi-WWAV", and the second devlog letter coins the word: "I think Wi-Wwav (we wave) is a perfect name for the people who are customers and fans of wwav, we are the wwav, so just shorten it and its Wi-WWAV." The app takes the name of the people it is for. The underscore follows the archive's rule for family names, "never a hyphen" (`archive/src/content.js:281`).

**Exists today:** www.wi-wwav.com is already Wi_WWAV, a private wall of `.wwav` and `.swav` files that one account can sign in to (`wi/`). The rest of this document calls that site **Wi**, or the wall, so "Wi_WWAV" always means the app. **Proposed:** it becomes the app's web face, serving share pages and downloads (8.7).

**Open: hyphen or underscore.** *Recommendation:* use the underscore for the app and the hyphen for the people, as letters already do ("Dear Wi-WWAV,"). Domains keep the hyphen either way, because a hostname can't contain an underscore.

**On version numbers.** The sketch's "WWAV app v3" is, in the archive, DISCMAN, the spring 2026 iPhone app "for music, image, text and video posts", while the galaxy is v4 (web) and v5 (iOS), SOLAR SYSTEM. Space is built from both, and which one "v3" means is **Open** (4.1).

### 1.6 A day with Wi_WWAV

Tuesday, October 6. All of this is **Proposed**, and every name, key and number is the one the later chapters specify.

**8:40 AM, Heat.** The app opens on Today and syncs, because the last sync was over 15 minutes ago: "Synced 8:41 AM: 2 new tasks, 1 date change". One task came from the Brightspace calendar feed: "Grammar quiz 4", JPN 201, due Wednesday 11:59 PM.

Claude Desktop is open beside it. You ask it to score what came in. It calls `list_tasks`, then asks before its first `update_task`, as it does for any tool you haven't allowed. The quiz now reads "Claude's estimate: 45m, difficulty 2. It read the title, the notes and your past averages." The Edit menu reads "Undo Claude's estimate". Claude has also read last night's school mail through its own Gmail connector and recorded two threads in Mail: "Grade posted" for MTH 142, which waits as a pending grade, and "Nothing to do" for a classroom change.

The quiz's tube is amber, which means Warm. At difficulty 2 the runway is 5 days, so it will turn Hot with 1.5 days left, at 11:59 this morning.

**Plan my day** drafts dashed blocks into the time column in heat order, each with its reason: "Due tomorrow 11:59 PM, Warm." Return accepts them: "4 blocks · 3h 10m planned · 2 due today".

At 9:00 you press C on the quiz to make it current, then F. The LCD reads "24:59 · Focus 1 of 4 · Grammar quiz 4". The space bar starts World Ending, the album's title song. You click its vocals light, the voice drops out within one audio block, and the band plays on to study to. When the round ends: "Focus done. 25m logged to Grammar quiz 4." Then it waits: "Break 5:00. Press F to start it."

At 11:59, while you're in class, the tube fills past 0.70 and turns red. The strip reads "Hot: Grammar quiz 4". Nothing pings.

**2:00 PM, Console.** The next block, "Mix the second verse", is linked to the session Low Tide, A minor, 86 BPM. You press ⌘3, and the Now strip still reads "Mix the second verse · focus 24:12 left".

- **Keys.** You add an instrument track (⌘⇧T) with an AU piano. "KeyStep connected · channel 1 → selected track." The chords land where you played them. You quantize only the low notes, 1/8 at 50%: "Undo quantize 11 notes".
- **A crash.** A third-party delay, "Tape Echo", crashes, and only the engine falls: "The audio engine stopped. 'Tape Echo' on the track 'Keys' was running when it did. Restarting…" Two seconds later you press **Try it again**. The focus countdown never stopped.
- **A split.** You drag `break.wav` from your library onto a lane and press ⌃⌘S: "Splitting 'break.wav' · segment 4 of 12". About 20 s later there are four lanes. You keep the drums and mute the other three.
- **Picture.** You drop in a 4K phone clip of the shore, cut on bar lines with ⇧Return, and ripple-delete the dead seconds with ⌥⌫.

You select OUTPUT and press Return with `.wwav` and `.swav` ticked. The sheet reads "This session is 48 kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered.", "Stems sum to the master." and "3:58 → about 210 MB." A minute later both files exist, and the film's parent is the song.

**4:30 PM, Space.** You drag the export onto "Space" and drop it on the solar system World Ending. The planet condenses in while a ring traces the upload: "Up. Low Tide is in your galaxy." It is brick red, hsl(0, 58%, 42%), because A minor's hue is 0, and the ringed film hangs from it. Back in Heat you tick **Reached** on the milestone "Low Tide mixed", and its bead fills on the EP timeline behind your sun. Nothing else is published.

**6:10 PM, Ana.** Ana has added your galaxy, so Low Tide tops her Newest. She mutes the vocals moon, pulls the drums moon in to 40%, and presses ↑ Push. A spark leaves the planet, and "Low Tide (fork)" joins your song's family at generation 1, owning no audio of its own. Nobody tells you.

**8:15 PM, Space.** You open Ana's sun. Under her bio is her public Heat view: "Now making: glass hours, the last verse", and one timeline with three beads. Her grades aren't there, because she never switched them on.

**10:30 PM, Since you last looked.** At the top of Space you find Ana's fork, two new works from galaxies you've added, and a letter that opens "Dear Wi-WWAV,". The list ends "That's everything since 8:15 PM." You go back to Ana's planet "glass hours" and press **Open in Console** (⌘E). A session opens with four stem lanes under the header "fork of glass hours · gen 3". You solo the bass, listen once, and quit. Nothing reaches out before morning, when the app opens on Today with the quiz at the top, Hot.

### 1.7 Left out, and why

| Left out | Why |
|---|---|
| A fourth view for selling (Unquantized) | "Eventually the space will hold commerce instead of having that be a 4th place." |
| Connected selling and connected distribution | Commerce comes later, inside Space, and nothing of it is designed now. |
| Writing PRANA discs | "PRANA disc not ready yet, so definitely not writing software yet." |
| Zi_WWAV (furniture) | The sketch names four media, and nothing in the app makes a chair. |


## 2. One app, three views

This chapter covers the shell, which is everything that stays put when you change views. Everything here is **Proposed** unless it carries another label. The views themselves are chapters 3 (Heat), 4 (Space) and 5 (Console).

### 2.1 The window

There is one main window. It opens at 1280 × 800 pt and can shrink to 1024 × 680.

| Band | Height | Contents, left to right |
|---|---|---|
| Title bar | 52 pt | traffic lights · view switcher · Now strip · search pill · galaxy chip |
| View | the rest | the current view |
| Status bar | 22 pt | the view's own count · sync state · save state |

The title bar is brushed metal in every view, in light and dark. As MI-WWAV-OS puts it, "Aqua is the case; the Game Boy is inside it." The case never changes, so the controls never move. What is inside the case does change: striped Aqua lists in Heat, the night in Space, and metal with DMG-green screens in the Console (see 7).

The title bar holds four controls:

- **View switcher.** A three-segment gel control, 76 pt per segment, reading "Heat", "Space", "Console", on ⌘1–⌘3. The selected segment wears the deepened blue gel (`#336dcc→#1B4C8C`), so its white label holds 5.0:1 (7.2). Changing views is a 140 ms cross-fade, the MI-WWAV-OS "beat", and a cut under Reduce Motion.
- **Traffic lights.** Real window controls. Heat's are only decorative today.
- **Search pill.** Opens ⌘K.
- **At narrow widths.** Below 1180 pt the search pill becomes a 28 pt magnifier and the Now strip narrows to 440 pt, so the title bar fits at 1024 pt.
- **Galaxy chip.** A 28 pt miniature of your own galaxy. It opens "Your galaxy", "Your public Heat view", "Settings…", "Export everything…" and "Sign out".

### 2.2 The Now strip

**Exists today:** Heat's toolbar LCD. It is pale olive `#f2f4e4→#dfe3c6` with ink `#262a17`, shows two lines over a 6 px meter, and becomes phosphor `#d7e0a8` on `#20241a` in dark mode.

**Proposed:** the LCD grows to 520 × 44 pt and is split by an etched 1 px divider. The task sits on the left and the track on the right, so one glance answers "what am I doing" and "what am I hearing".

| Half | Line 1 | Line 2 | Meter | Click |
|---|---|---|---|---|
| Task | "Hot: Grammar quiz 4" | "Today 4:00 PM, JPN 201 · focus 18:40 left" | that task's heat, cool→warm→hot | opens Heat with the task selected |
| Track | 18 pt key-coloured planet, "World Ending" | four stem lights, "1:42 / 3:58" | the playhead | expands the player |

The task half shows the current task (set with C or by starting focus; see 3.5), or else the hottest open task, as Heat's LCD does now. The track half shows whatever the audio engine is playing; in the Console it shows the session: "BAR 42.3 · 128.00 BPM · REC ARMED". The engine is the master clock and publishes its playhead in shared memory (see 8), so the strip and the video viewer read the same value and cannot disagree.

**Stem lights.** There are four 8 pt lights, in PRANA's order and colours: vocals `#D23C2A`, drums `#F0B90B`, other `#2E9A55`, bass `#1F4E9E`.

- **State is shown by shape, not colour.** Filled means audible. A hollow ring means muted. Filled with a 2 pt outer ring means soloed.
- **A click works like clicking a moon.** It mutes at once. A second click within 250 ms reverts the mute and solos instead.
- **Hit area.** Each light has a 44 × 44 pt hit area.

**When a half is empty**, the strip reads "All clear / Nothing open right now." and "Nothing playing / Select anything and press Space."

### 2.3 Views keep their place

Each view is built once and lives until you quit, so switching back finds its camera, scroll, selection, open sheet and half-typed text where you left them. **Exists today:** v4's player "mounts once, outside the routes, and never unmounts" (`wwav/src/components/PlayerShell.jsx`).

**The player is a state, not a screen.** There is one listening player, and the audio engine owns it.

- **Expanded,** it is the planet player: over Space, or as a sheet dropped from the title bar in any other view.
- **Collapsed,** it is the planet in the strip. In v4's words, "the bar is just the planet seen from far away" (`wwav/src/planet/MiniBar.jsx`).
- **To collapse it,** press Esc or pinch out with two fingers.

| You are | You do | What happens |
|---|---|---|
| Listening | Switch views | It keeps playing. |
| Listening | Press play in the Console | Listening pauses and the strip reads "Paused for the Console". Stopping the Console doesn't resume it. |
| Listening | Open a film | "The planet is paused while this plays." (v4) |
| In a Heat focus session | Switch views | The countdown continues in the strip. It is silent unless you turned the chime on. |

Nothing resumes on its own. The Console can tear its video viewer or mixer off onto a second display. Plugin windows belong to the audio engine (see 5).

### 2.4 One account, one you

You have one WWAV account: the existing `Users` row at mi-wwav.com. Its 7-day JWT is kept in the macOS Keychain and refreshed after a 401. WWAV Push already refreshes on a 401, though it keeps its tokens in a properties file.

| Part | What it is | Stored as | Seen by | Status |
|---|---|---|---|---|
| Galaxy | your place in Space; the title-bar chip | `Galaxy`, one per user (slug, `skySeed`) | everyone | Exists today (v4) |
| Bio sun | your page, in the middle of your galaxy | blocks v1 `{v:1, blocks:[…]}` | everyone | Exists today (`wwav/src/sun/blocks.js`) |
| Public Heat view | the simple public version of Heat, shown under your bio when someone opens your sun | the items you've made public, synced to your account | everyone | Decided |
| Stem player | your instrument's skin, wrapped from a photo or painted | `/api/user/stem-player-customization` | you | Exists today (v3) |
| Heat | your time, as a private PKM | local SQLite, synced privately, except private grades and courses, which stay on the Mac | only you, except what you switch public | Proposed |

**Two profile views** (**Decided**). Your private Heat (⌘1) is the whole profile, and only you see it. Your public Heat view is what anyone sees when they open your sun in Space. By default it has room for two things, each shown once you press Show: a "Now making" line and the milestone timelines of linked projects. Every other record (task, project, milestone, habit, note, course, grade, focus record) is private until you switch that item public, grades included (3.15).

**The stem player.** v3's tutorial says "the user's stem player IS their account". In Wi_WWAV your galaxy is who you are and the stem player is what you hold: your skin wraps the strip's planet when nothing is loaded. A loaded song always wears its own key colour.

### 2.5 The library: files first

Wi's rule carries over: "A post is a file." The library is a folder you can open in Finder:

```
~/Music/Wi_WWAV/
  library.sqlite   clips, tags, sequences, Heat records, the undo journal
  media/           01JA2B7X9Q4M8K3T5V6W0YHZRC.wwav   (ULID-named)
  sessions/        Console sessions (see 6)
  trash/           deleted media, kept until you empty it
```

**File names are ULIDs.** They sort by creation time as plain strings (**Exists today:** `MI-WWAV-OS/engine/src/ids.rs`). A song's title lives inside its file, in `wmet`. Renaming a song never renames anything on disk, and two songs called "untitled" never collide.

**Three models, from MI-WWAV-OS** (**Exists today:** `MI-WWAV-OS/engine/src/model.rs`):

- **Clip** is any one thing: a `.wwav`, a `.swav`, plain audio or video, an image, or text. A note is a zero-duration text clip.
- **Tag** is `user`, `card` or `system`. System tags are places: your solar systems. Dropping a clip on one publishes it there.
- **Sequence** is an edit list, never media. A Console session is a sequence. Rendering it makes a new clip that points back through `from_sequence`, so "the edit stays editable forever."

**Organisation, from v3's library** (**Exists today:** `ios_v3/WWAV/Models/Track.swift`, `LibraryFolder.swift`):

- **Tags** are lowercase, with at most 12 per clip.
- **Colour labels** come in six, for sorting only: red `#E0383E`, orange `#F08A2C`, yellow `#E8C73A`, green `#3CAA68`, blue `#3D7DD5`, purple `#9359C9`.
- **Pins** fill a row of 4 slots.
- **Smart folders** are AND rules over tags, colour, a BPM range, key and kind, such as "128–132 BPM · minor · tag:live-drums".
- **Bulk edits** cover tag, colour, move and delete.

**Where the library appears.** ⌘L slides a 280 pt source-list drawer over any view. The Console's browser and Space's publish drawer are filtered views of this same library.

A plain WAV comes in as master only and says so. Nothing is converted without a press (see 6).

**Open: copy or reference.** Should imports be copied into `media/` or left where they are? Recommendation: copy songs, films and anything under 2 GB, so the library and its export are complete. Offer "Leave in place" for camera folders and long raw video.

### 2.6 The two connections

| Connection | Views | What crosses | What never crosses |
|---|---|---|---|
| Connected profiles | Heat ↔ Space | your public Heat view: room for a "Now making" line and the milestone timelines of linked projects, each shown once you press **Show**; any other item you switch public, one at a time (3.15) | anything still private; counts and totals worked out across your records; any comparison between people |
| Connected uploads | Console ↔ Space | exported `.wwav`/`.swav` as exact bytes, with lineage read from `wlin`; any song or film back into the Console with **Open in Console** | unexported takes, unpublished sessions |

Heat also links tasks and projects to Console sessions, so focus follows you into the Console (3.15). That link is private and never reaches Space.

**Connected profiles: a timeline behind your sun.**

1. In Heat's sidebar, drag the project "EP" onto the solar system "World Ending" under "Your galaxy", or set it in Get Info → Project. The sheet reads "This links EP to World Ending. Its timeline will show on your public Heat view." **Show** adds the milestone beads (titles, dates, reached or not) to your public view and to the system's sun. **Keep private** links it and shows nothing. ⌘Z reads "Undo show timeline".
2. Ticking **Reached** on "EP v1 mixed" later fills its bead and publishes nothing else. In the other direction, your project sun's secondary act, **Plan in Heat**, creates a linked milestone.
3. Any other item goes public the same way, one at a time, with the **Public** switch in Get Info. A grade's switch says plainly what it does: "Grades are private by default. Your school keeps them as education records. Turning this on shows this grade to anyone who opens your sun." ⌘Z reads "Undo make public".

**Connected uploads: from a friend's planet to your fork.**

1. In Space, select a friend's planet "glass hours" and press **Open in Console** (⌘E), or drag it onto the "Console" segment. A session opens with four stem lanes and the parent's remix snapshot applied, under the header "fork of glass hours · gen 3". The session points at the parent's file and never writes to it (5.14).
2. Tear it apart: record a vocal take, mute the original vocals and export a `.wwav`. It is written with `wmet.type` `remix`, `wlin.parent_id` set to the parent's `song_id`, `generation` 3 (the parent's + 1) and `creator` you. How N lanes fold into four stems is in 5 and 6.
3. Drag the export onto "Space" and choose the system "Covers". Once it lands, the fork edge shows in the parent's lineage. A human link such as "sample" waits until the other owner presses **Agree** (**Exists today:** v4 lineage consent).

**Commerce, later** (**Decided**). Works in Space will one day carry a price and be bought where they are. Until then nothing crosses for money, and nothing in this chapter depends on it (see 4).

### 2.7 One grammar in every view

These rules come from MI-WWAV-OS. When a feature breaks one, the feature changes.

**Undo is ⌘Z everywhere, always free, and always labelled.** Every change is a transaction that snapshots the affected rows before and after (**Exists today:** `MI-WWAV-OS/engine/src/store.rs`).

- **Labels.** The Edit menu reads "Undo move clip", and a toast says "Undone — move clip" for 2.6 s. ⌘⇧Z redoes.
- **Scope.** There is one journal, but ⌘Z acts on the view you are in. Claude's tool calls go through the same journal (2.11).
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
| Library | open | add to selection |

**Publishing is a drop.** You publish by dragging onto a system or a view segment, wherever a target can be shown.

**⌘K is one palette** over tasks, clips, sessions, galaxies, settings and actions.

- **Results.** Local fuzzy matches come first and server results follow after 200 ms (**Exists today:** `portfolio/src/pkm/palette/CommandPalette.jsx`). Filters: `tag:`, `key:`, `bpm:`, `is:hot`, `is:remix`, `@name`.
- **Opening.** ⌘Return opens a result in its other view: a song in the Console, a person at their sun in Space.

**⌘⇧N is quick capture.** It opens a 420 × 160 pt panel from any view, or from any app if you turn that on.

- **Capturing.** Type, paste a link, drop a file, or hold R to record a voice memo. Enter saves and keeps the panel open. The footer reads "3 in inbox · captured ✓" (**Exists today:** `portfolio/src/pkm/capture/CaptureModal.jsx`).
- **Triage.** The inbox sits at the top of Heat → Today, with **→ task**, **→ note**, **→ project** and **→ upload** on each item.

| Keys | Does | Where |
|---|---|---|
| ⌘1 · ⌘2 · ⌘3 | Heat · Space · Console | everywhere |
| Space | play / pause | wherever media is, outside text |
| ⌘K · ⌘⇧N · ⌘L | palette · quick capture · library drawer | everywhere |
| ⌘Z · ⌘⇧Z | undo · redo, labelled | everywhere |
| Esc | close, collapse, deselect; never deletes | everywhere |
| ⇧Return | the screen's one secondary act | everywhere |
| ← → ↑ ↓ | move selection; level ±5% on a focused stem | lists; planet, Console lanes |
| M · S | mute · solo the focused stem | planet, Console lanes, strip |
| ⌘E · ⌘⇧E | Open in Console · Export everything… | a selected song or film · everywhere |

### 2.8 Offline first

What is local is the truth. Heat, the library and the Console work with no network for as long as you like. Space shows what you have already visited, marked "Seen Oct 4". Publishing, visiting somewhere new, and opening a work you've never opened in the Console each need a connection, and each says so on its button. The app's own MCP server is local and needs none; Claude needs its own (2.11).

**The upload queue is a query, not a list** (**Exists today:** `MI-WWAV-OS/proto/README.md`). A drop sets `published_at` at once, and `remote_id` fills in when the server acknowledges.

- **What counts as waiting.** Anything with `published_at IS NOT NULL AND remote_id IS NULL`. The queue survives a crash with no queue file, and unpublishing early cancels the upload with no extra logic.
- **Uploading.** Each upload signs just before it starts, because presigned URLs last 300 s. It goes up in 8 MiB parts, as Wi's uploads do, and resumes from the last finished part.
- **No double posts.** The publish body carries `settings: {origin: "wi_wwav", clipId: <ULID>}`, so a retry never posts twice.

The status bar says only real stages:

1. "Offline. 2 works wait to go up; they leave when you're back."
2. "Uploading World Ending · part 14 of 27"
3. "Up. World Ending is in your galaxy."

**Heat records sync privately** to your account, except private grades and courses, which stay on the Mac unless encrypted sync is on (8.7, **Open**). Each field has a monotonic sequence number, so a slow older write never overwrites a newer one (**Exists today:** the PKM's `useAutosave.js`; see 8). Items you switch public sync the same way and show on your public Heat view once the server has them.

### 2.9 You can leave with everything

File → **Export everything…** (⌘⇧E) is one action. It writes a folder or a zip, is never behind a paywall, and works while you are signed out.

| In the export | What it is |
|---|---|
| `media/` + `manifest.json` | every library file byte for byte, with its sha256 |
| `sessions/` | every Console session, with the plugin state it saved |
| `heat.json` | tasks, milestones, habits, courses, grades, focus sessions, each with its public or private setting |
| `notes/` | Markdown with frontmatter and `[[wikilinks]]`; opens in Obsidian |
| `galaxy.json` | your galaxy, systems, suns as blocks, lineage links |
| `index.html` | plays every song (stems apart) and every film from disk |

**Exists today:** Wi's one-press export with an offline `index.html` (`wi/README.md`), and the PKM's Obsidian zip.

The test is written before it is run: export, turn Wi-Fi off, and open `index.html`. Every song must come apart, and every film must play.

### 2.10 Notifications: pull only

**Proposed default:** nothing reaches out. There are no OS notifications, no Dock badge, no sounds, and no unread dots on the view switcher.

**"Since you last looked"** is a list at the top of Space, built when you open it. It holds forks of your works, links waiting on you ("*name* says their planet is an influence of your planet", with **Agree** and **Refuse**), and new works and letters from galaxies you have added. It ends with "That's everything since Oct 4." Jobs you started yourself, such as splits, exports and uploads, report in the status bar and finish quietly.

**Open: reminders and badges.** The sketch's planner implies "due at 4 PM" reminders, and gate 1.1 fails anything that reaches out. Recommendation: never notify or badge about what other people do. Allow one kind of OS notification: an alarm you set yourself on one task or focus session, off by default.

### 2.11 Claude, through MCP

**Decided:** Wi_WWAV is an MCP server. Claude connects to it from claude.ai, Claude Desktop or Claude Code and calls the app's tools. The app holds no Anthropic key, calls no model, and has no Claude limit of its own.

Claude estimates and drafts, and never decides. Each change it makes carries a one-sentence reason. Ripple Creator's rule holds throughout: "never invent metrics." Both rules are written into the tool descriptions, which are the only instructions the app gives Claude.

**Two ways in.**

- **Local, first.** A stdio server that Claude Desktop and Claude Code start on your Mac. It acts on the local library, so it needs no account and no network of its own.
- **Remote, later.** A server through mi-wwav.com for claude.ai, which needs Heat sync first (8.7). The devlog already reaches claude.ai this way (**Exists today:** `/mcp` with OAuth sign-in, `server/mcp/devlog.js`, `server/mcp/auth.js`).

| Tool | Does | Shows in the app as |
|---|---|---|
| `list_tasks` | reads open tasks with heat, due date and estimate | nothing; reading changes nothing |
| `add_task` | makes a task, such as one found in a school email | a new task, labelled as Claude's, with its reason |
| `update_task` | sets difficulty, estimate and a one-line reason | "Claude's estimate: 45m. It read the title, the notes and your past averages." |
| `plan_day` | runs Plan my day, Heat's own written rule | dashed blocks in the time column, waiting for Return |
| `get_grades` | reads courses and grades | nothing |
| `add_pending_grade` | records a grade notice with no score, linked to Brightspace | a pending grade |
| `log_focus` | logs a focus session on a task | a focus record |
| `record_mail_thread` | records a school thread Claude read, with its state | a row in Mail: **Grade posted**, **Task made** or **Nothing to do** |

**School mail.** Claude reads it through its own Gmail connector, then calls `add_task`, `add_pending_grade` and `record_mail_thread`. The app never asks Google for Gmail, so it needs no restricted scope, no Google verification and no sign-in that lapses every 7 days. Mail lists the threads Claude recorded, with their state, and **Open in Gmail** (3.10).

**Consent.** Claude's own tool-permission prompts replace the app's consent sheets: Claude asks before it calls a tool you haven't allowed. Settings → Claude lists the tools the app offers, and switching one off removes it from what Claude can see.

**Undo.** Every Claude action is a visible, labelled, undoable tool call. **Proposed:** the MCP tools act on the local library through the same journal as the UI (2.7), so ⌘Z undoes them, and the label names who made the change: "Undo Claude's estimate".

**Without Claude**, nothing stops working:

- **Estimates** fall back to your average for the type, else difficulty × 20 min.
- **Due dates** still arrive from the Brightspace calendar feed (3.11).
- **Plan my day** is Heat's own rule, not Claude.
- **Mail** shows only what Claude has recorded, and Gmail itself is one click away.

### 2.12 Accessibility bars

These are gate 2.4's bars, written down before testing. A desktop rule is added because the gate's posture rule covers phones only.

- **Contrast and text.** Body text 7:1 and secondary 4.5:1, in light and dark, in both registers. Running text (the reading room, letters, Mail) is at least 17 pt (7.9, **Open**); rows and labels are at least 13 pt; ⌘+ scales both, up to 20.
- **Targets and keys.** Hit areas are at least 44 × 44 pt, and every gesture has a key. Newest and ⌘K mean Space never requires flying.
- **Motion and state.** Nothing moves while nothing plays, and state is never colour alone. VoiceOver reads a moon as "Vocals, 70 percent, audible".

The existing tokens measured against these bars, and the fixes they need (LCD dim text to `#4b5034`, selected rows filled `#1B4C8C` with white text), are in 7.2 and 7.11.

### 2.13 Settings

Settings is a classic Mac preferences window with an Aqua icon toolbar.

| Pane | Contents |
|---|---|
| Account | galaxy address, stem player skin, sign out, delete account (type DELETE) |
| Library | location, copy or leave in place, Empty trash |
| Heat | spaces, school, Brightspace calendar link, other calendars' iCal links |
| Audio & MIDI · Video | devices, buffer 64–1024 samples, plugin folders; hardware encode, proxy media |
| Claude | the MCP connection: how to add Wi_WWAV to Claude Desktop and Claude Code, the tools offered with a switch on each, and Claude's recent changes, each with Undo |
| Privacy | one table of everything public, with a switch on each; your public Heat view as others see it |
| Appearance · Keyboard | Light, Dark or Match system (Space is always night), text size (⌘+ / ⌘−); every shortcut |

### 2.14 First launch

There are five steps. Each has exactly one secondary action, "Skip for now", and without an import the whole flow takes under three minutes.

1. **Sign in.** The button **Sign in or create an account** opens mi-wwav.com in your browser (8.7). The form there asks for email, username, password, a birthdate for the 13+ gate, and an invite key when one is required (**Exists today:** v4 registration). The browser hands the sign-in back to the app.

   **Open:** should "Skip for now" let you in without an account? Recommendation: yes. Heat, the library and the Console are fully local. Space can be looked at but not published to or pushed from, which is how the signed-out universe already works.
2. **Claim your galaxy.** The copy is v4's: "You have no galaxy yet. A galaxy is yours. Projects orbit it as solar systems, and each song or film is a world inside one. The sun at the centre is where you say who you are." The button reads **Make my galaxy**. An optional first line on your sun has the placeholder "Say it plainly". Below it: "Behind your sun, people can see a simple version of Heat, once you choose what goes there. Everything in Heat stays private until then."
3. **Import your folder.** Before anything is copied, you see what the folder holds: "214 files: 38 .wwav, 12 .swav, 160 plain audio, 4 other. Plain audio comes in as master only." The button reads **Bring them in**. It runs in the background, and pressing again picks up after an interruption.
4. **Add your calendars.** Each comes in as a private iCal address, so the app needs no Google sign-in:
   - "Paste your Brightspace calendar link": the per-student iCal feed, which works now with no approval.
   - "Add another calendar": any calendar's private iCal address. Google Calendar gives one in each calendar's settings, as its secret address in iCal format.

   The school-approved route (Valence and LTI 1.3) is in chapter 3.
5. **Connect Claude.** The sheet shows how to add Wi_WWAV to Claude Desktop or Claude Code, and lists the tools Claude will see (2.11). Skipping leaves Heat whole: estimates use your averages, and Plan my day is Heat's own rule.

The app then opens on Heat → Today. The strip reads "All clear" on the left and "Nothing playing" on the right.

### 2.15 Left out, and why

| Left out | Why |
|---|---|
| The astronaut and the rocket (the title-bar portrait, the first-launch maker, the Account setting) | Cut on 7 Oct 2026 to narrow v1. Your galaxy is who you are; `Users.astronaut` is not used. |
| A fourth segment, Selling settings, the purchases folder and receipts | Commerce comes later, inside Space. |
| Claude calls made by the app (`/api/assist/:task`, a daily limit per account, in-app consent sheets) | Wi_WWAV is an MCP server instead. Claude's own permission prompts ask before each tool call. |
| Google sign-in for Gmail and Calendar | Claude reads mail through its own Gmail connector, and calendars come in as iCal addresses, so the app needs no Google account. |
| Feedback on your work through Ripple | Cut on 7 Oct 2026 to narrow v1. |
| OS notifications and Dock badges about other people | Anything that reaches out fails gate 1.1; "Since you last looked" is pulled, never pushed. |

## 3. Heat: the profile view

Heat is the profile view. It opens with ⌘1 and comes first in the view switcher, because the day starts there (see 2). It is where you plan your time and keep what you'd keep in a private notebook: tasks, grades, habits, notes and school mail. By default all of it is private, and a simple public version of it shows behind your sun in Space (**Decided**; see 3.15). Claude reaches Heat through the app's MCP server, and the app itself calls no model (**Decided**; see 3.12 and 3.13).

The founder already uses Heat: today it is a single-file artifact inside claude.ai. Section 3.1 describes that file exactly, and all of it is **Exists today** (Heat, one HTML file of 1,481 lines). From 3.2 on, everything is **Proposed** unless it is marked **Decided** or **Open**.

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

### 3.2 From one file to a view

| Today | In Wi_WWAV |
|---|---|
| Three fixed workspaces as top tabs | User-defined **spaces**, which filter every tab |
| List / Calendar / Habits / Grades | **Today / Tasks / Calendar / Grades / Habits / Mail** |
| Get Info only while a task is selected | A right column of widgets: **Now, Habits, Hot tasks, Mail, Grades** |
| Minutes typed in after finishing | Focus sessions write `actualMin` as you work |
| claude.ai connectors, hard-coded to URI | Calendars as private iCal addresses, set per school. School mail is read by Claude through Claude's own Gmail connector |
| Claude called from inside the page (`sample`) | Claude calls the app's MCP tools. The app holds no key and calls no model |
| Habits in Personal, Grades in Classes | Both global |
| One private artifact | Everything private by default, a **Public** switch on every record, and a simple public version behind your sun |

The heat algorithm, the estimate chain, the grade maths, the LCD, the sheets and the copy ("Heat will rank it.") carry over unchanged.

### 3.3 The view's window

Heat fills the view area of the one main window (see 2; **Decided**: one desktop app, Mac first): 1280 × 726 pt at the default window, 1024 × 606 at the smallest. Heat's LCD moves up into the title bar as the Now strip's task half (see 2.2), so from left to right Heat's toolbar holds "+", the six tabs as one segmented control and Sync. Search is the title bar's ⌘K pill (see 2.1). Below sit the 190 px sidebar, the main view, and a 290 px right column. Below 1240 pt of window width, the right column folds into a 44 px strip of widget icons that open as popovers.

Each tab has one primary action ("+" or N) and exactly one secondary action:

| Tab | "+" adds | Secondary action |
|---|---|---|
| Today | A task straight into the plan | Plan my day |
| Tasks | A task | Triage inbox (only while the inbox has items) |
| Calendar | A task due 11:59 PM on the selected day | Today |
| Grades | A grade | Add course |
| Habits | A habit (at 6: "Habit limit reached") | Show the year |
| Mail | (hidden) | Open in Gmail |

Sync, in the toolbar, reads your calendars (3.11). ⌘Z undoes anything, and the menu names what it will undo: "Undo mark done", "Undo move block", "Undo Claude's estimate". Esc closes a sheet or drawer, or clears the selection. It never discards saved work.

### 3.4 Spaces

Workspaces become **spaces**: named filters that apply to Today, Tasks, Calendar and Mail. The sidebar opens with All, then each space with its hue dot and open count, then "New space…". Grades, Habits and calendar events ignore the filter, because they belong to the person rather than to a project.

A space keeps the settings the old workspaces had built in, now editable in a sheet: name, hue, group kind (course, milestone or free text), group label, types, placeholders and Claude persona. `list_tasks` hands the persona to Claude with that space's tasks, so its estimates fit the kind of work (3.13). Classes, WWAV and Personal are created with today's values. In Classes, the group kind is "course", so a task's course points at a Grades course record and each course is defined once. The WWAV persona is rewritten to cover the app: "a solo founder building WWAV: the PRANA handheld (Teensy 4.1, C++ firmware, PCBs), the Wi_WWAV desktop app, and an album."

### 3.5 Today

Today is the sketch's "today's plan", and Heat opens on it. The main view puts a 300 px time column on the left and the plan list on the right, with a 112 px Pomodoro panel across the bottom. The header reads "Today, Tuesday, October 6", with the subtitle "4 blocks · 3h 10m planned · 2 due today".

#### The time column

The time column reuses the PKM calendar's scale (`portfolio/src/pkm/calendarUtils.js`): 44 px per hour, a 15-minute snap and 30-minute default blocks. It runs from 7 AM to midnight and scrolls so that now sits a third of the way down. A 1 px red line marks the current minute.

- **Blocks** have a 3 px left border in the task's heat colour, the title and the length ("45m"). The current block gets the `#3875d7` ring, and finished blocks dim to 50% with a check.
- **Events** from your iCal calendars (3.11) are grey and hatched, and read-only. They sit behind blocks, so a clash shows as an overlap.
- **Drag** a task onto the column to make a block as long as its estimate, rounded up to 15 minutes. Drag a block's bottom edge to resize it. Resizing changes the block, never the estimate. A task can have several blocks.
- **P** puts the selected task into the next free gap after now.

#### The plan list

The plan list has four sections, and any empty one is hidden: **Planned** (in time order, with start times), **Due today, not planned**, **Recurring today ↻**, and **Hot, not planned** (up to 5 suggestions). Below them, the **daily note** sits collapsed to one line until clicked: "How's the day going? Markdown + [[wikilinks]] welcome."

**Plan my day** fills the time between now and "Day ends at" (11 PM by default) with a written rule, not Claude. It takes unplanned open tasks in heat order and gives each a block the length of its estimate, rounded up to 15 minutes and capped at 90 ("45m left to plan"), in the first gap that fits. The drafts appear dashed, each with a reason: "Due tomorrow 11:59 PM, Hot." Return accepts all of them, a click accepts one, and Esc clears them. The empty state says "Nothing planned yet. Drag a task onto the time column, or press Plan my day." Claude's `plan_day` runs this same rule and leaves the drafts waiting for you (3.13).

#### The Pomodoro timer

The Pomodoro timer is an olive LCD panel. It shows 32 px tabular digits ("24:59"), a line such as "Focus 1 of 4 · Mix the second verse", and a meter that drains.

- **Lengths.** Focus defaults to 25 minutes, with 50 minutes or a custom 10–90 as options. Breaks are 5 minutes, and every fourth break is 15.
- **Keys.** F starts or pauses. ⇧F stops and logs. I marks "Pulled away", which pauses the timer and records an interruption. The space bar stays play/pause for media, because music under a focus session is the usual case.
- **Ending.** The LCD reads "Focus done. 25m logged to Mix the second verse." It is silent unless you turned on Heat's chime, which is off by default (7.7). The break waits for you: "Break 5:00. Press F to start it." Nothing starts without a press.
- **Other views.** The timer keeps running when you switch views. The Now strip's task half shows "focus 18:42 left" (see 2.2); the view switcher stays plain.

Every focus session belongs to the **current task**, and its minutes add to that task's `actualMin`. If a task has logged time, checking it completes it at once, and the status bar says "Done. Took 1h 15m across 3 focus sessions." with an Undo. Get Info's "Took" field adjusts the time by hand. Only a task with no logged time still asks "Time it took". The per-type averages now come mostly from measured time.

#### The right column

These are the sketch's five widgets, in its order. Each is a brushed-metal panel with a small grey heading, and each can be hidden from the View menu.

| Widget | Shows | Actions | Empty |
|---|---|---|---|
| **Now** | The current task, its space dot and heat tube, "Block ends 3:30 PM", and the timer state | Start focus (F), Done (⌘↩), Open link | "Nothing is current. Pick a task and press C, or drag one here." |
| **Habits** | Today's orbs; "3 of 5 done" | Click an orb to toggle it | "No habits yet." |
| **Hot tasks** | Up to 5 Hot or Overdue tasks from all spaces | Select; drag onto the time column | "Nothing is hot." |
| **Mail** | The 3 newest school threads Claude recorded | Open in Mail | "No school mail recorded." |
| **Grades** | The lowest course and its letter; "2 new grades to enter" | Open Grades | Hidden until a course exists |

One task is current at a time. You set it with C, by dragging a task onto Now, or by starting focus on a selection. While a task is selected, Get Info slides over the right column in 280 ms (`fade`, 7.6), and Esc slides it back.

### 3.6 Tasks

Tasks is Heat's List view with the gaps filled.

- **When.** A new **When** column (the scheduled date) brings in the PKM's rule: "scheduled (when I'll work on it) is distinct from due (deadline)". Heat still comes only from the due date.
- **Sidebar.** Inbox, All open, Hot, Due this week, Scheduled, Someday, Done. Below them, Projects with their milestones, or Courses, or Areas, then "Your average time".
- **Subtasks** indent under a disclosure triangle. A parent's estimate is the sum of its open children.
- **Recurrence** is stored as an RRULE, as in the PKM ("Every weekday", "Custom…"). Each finished occurrence is a row of its own, and the series never flips to done. A recurring task's heat comes from its next occurrence. These rows show ↻.
- **Timeline.** With a milestone space selected, the bead timeline sits above the list as it does today.
- **Keys.** ↑ and ↓ move the selection. Return edits, ⌘I opens Get Info, ⌘↩ marks done, and ⌫ deletes with Undo.
- **Dragging a row:** onto a Calendar day to schedule it, onto Today's column to make a block, onto a milestone to link it, or onto "Your galaxy" to make it your Now making line (3.15).

A task Claude added shows its source in Get Info ("Claude, Oct 6 8:41 AM") and the reason it gave. The empty state keeps Heat's line: "Add your first WWAV task and Heat will rank it."

### 3.7 Calendar

The month grid stays as built. Week and Day views are added on the PKM's grid. M, W and D switch views, ← and → page, and T jumps to today. In Week view, an all-day strip holds due pills and milestone beads. A deadline shows as a small heat-coloured flag on the right edge of its column ("due 11:59 PM"), so a block and its deadline read on one line. An unscheduled tray on the left lists this week's open tasks that have no block, in heat order, ready to drag in. Brightspace items are tasks, so they never draw as grey events. Events from other calendars' iCal addresses do draw as grey, read-only events, and never make tasks.

### 3.8 Grades

Grades is global. The header reads the current term from a Term record ("Fall 2026 grades") instead of hard-coded text.

- **Course editor.** Code, name, categories with weights, scale and notes. If the weights don't add up, it says so: "Weights add to 95%. The other 5% is unassigned."
- **Category keywords.** Today the rules that guess a grade's category (exit ticket, Edfinity, kanji, Lab 5a…) are hard-coded. They move onto each category as editable keywords, starting from the current rules.
- **What it would take.** This is plain arithmetic: "To finish with a B (83%), you need 78.4% on the remaining 35%." When the target can't be reached, it says so: "A B is out of reach; the highest possible is 81.2% (B-)."

Pending grades keep their yellow "Enter score" banners. A pending grade comes from Claude (`add_pending_grade`) or from you, and it has no score until you type one. Claude can read your grades with `get_grades`, but it never writes a score.

Grades are private by default. Each grade, and each course, has its own **Public** switch (3.15). Nothing else about grades reaches Space, and no course percentage, letter or comparison ever does.

### 3.9 Habits

Habits is global. It keeps the limit of 6, the orbs, the 14-day grid, the grace until midnight, and the advice: "Keep them small enough that you never skip." The log stops being pruned at 400 days, since a year of daily keys is about 6 KB. "Show the year" opens a 53 × 7 grid. A habit can have a length ("Practise kanji, 20m"). A habit with a length appears under Recurring in Today, can be blocked like a task, and ticks itself when a focus session on it reaches that length.

**Open: the streak counter.** The sketch keeps Heat's "12-day streak". Gate 1.1 names them as a fail: "Anything stretches use past what the person came for: autoplay, infinite scroll, notifications, streaks, 'up next'". The gate-passing default replaces the counter with a record that only grows ("Done 41 days since August 26"), with the grids as the picture. Nothing breaks, so nothing pulls you back to protect a number. *Recommendation:* make the growing record the default, and keep the counter as a per-habit setting that is off by default.

### 3.10 Mail

Mail lists the school threads Claude has recorded. Heat does not read Gmail. Claude reads your mail through its own Gmail connector, then calls `record_mail_thread` for each thread (3.13), so a thread is on this tab only after Claude has been through it.

The main view splits into a 320 px thread list and a pane for the selected thread. The sidebar holds **All** and the three states with counts. Each row shows the sender and time, then the subject, with chips for the course code and the thread's state: **Grade posted**, **Task made**, or **Nothing to do** (Claude found no deadline).

The pane shows what Claude recorded: the subject, sender and time, the course, the state, Claude's one-line reason, and a link to what it made, such as the task "Grammar quiz 4" or the pending grade. Heat keeps no message body and no attachments, so there is nothing to render and no remote images to block.

Each thread has one action bar. **Open in Gmail** is the secondary action. It opens the thread at `mail.google.com` in your browser. **Make a task** (T) is for doing it by hand: it opens the task sheet with the subject as the title and notes that start "From mail:" with a link back to the thread. Mail has no reply, send or delete. Gmail already does those, and Wi_WWAV never touches your mailbox.

The empty state says "No school mail recorded yet. Ask Claude to read it."

The app has no Google sign-in. It asks Google for nothing, so there is no restricted scope, no Google verification and no sign-in that lapses every 7 days.

### 3.11 Calendars and Brightspace in the app

Nothing is hard-coded to one school. A School sheet holds the name, the Brightspace host (`brightspace.uri.edu`), the Brightspace iCal link, the course-code pattern (default `/^([A-Z]{3})\s?(\d{3})/`) and the term dates. The time zone comes from the system, which gives America/New_York for URI without naming it. Settings → Heat holds the same sheet, with a list of other calendars below it (see 2.13).

Calendars come in only as **private iCal addresses**. Mail comes in only through Claude.

| Route | Gives | Needs | When |
|---|---|---|---|
| Brightspace calendar **iCal feed** | Due dates for every course | The student's private link, kept in the Keychain; no approval | Now |
| **Other calendars' iCal addresses** (Google Calendar gives one in each calendar's settings, as its secret address in iCal format) | Other events for the time column | That calendar's private address, kept in the Keychain | Now |
| **Claude's Gmail connector** | Grade notices and announcements, as tasks, pending grades and Mail rows | Claude, with Wi_WWAV's MCP tools (3.13) | Now, when you ask Claude |
| D2L **Valence REST API** or an **LTI 1.3** tool | Real scores, exact due dates, course lists, gradebook weights | Registration by the school's Brightspace admin | Later, with the school |

Anyone who holds an iCal address can read that calendar, so treat each like a password. Heat keeps them in the Keychain on this Mac and leaves them out of `heat.json`.

The iCal path keeps Heat's filters (titles ending " - Due"; skip `/non-graded/i` and cancelled items). They were written against the feed as Google Calendar shows it, so they are checked against a raw feed before shipping. The VEVENT `UID` becomes the task id, and tasks that arrived through Google Calendar in the artifact are matched by Heat's duplicate test (the due day and course match, and the titles share at least 60% of their words) and take it on. An item missing from two syncs in a row gets a grey "No longer in Brightspace" tag and is never deleted on its own.

**Rhythm.** Heat syncs calendars on open if the last sync was over 15 minutes ago, then hourly, and reports "Synced 3:41 PM: 2 new tasks, 1 date change". Claude's changes arrive when it makes them, each labelled as Claude's. When a feed can't be read, the line says so and names the calendar: "Couldn't read the Brightspace calendar. Heat will try again in an hour."

Grades are education records under FERPA. Whichever route brings them in, they stay private by default, and only you can switch one public (3.15).

### 3.12 Claude in Heat

**Decided:** Claude works on Heat through the app's MCP tools (3.13), from Claude Desktop or Claude Code. The app holds no Anthropic key, has no Claude button and no daily limit, and makes no model call itself. Claude estimates and drafts. It never decides. Each change it makes shows its one-line reason, carries Claude's name, and can be undone.

| Job | Claude calls | Reads | Writes | Shows in Heat as | Without Claude |
|---|---|---|---|---|---|
| Score a task | `list_tasks`, `update_task` | Title, notes, due date, your average minutes by type, the space's persona | Difficulty, minutes, a one-line reason | "Claude's estimate: 45m, difficulty 2. It read the title, the notes and your past averages." as the field's hint | The estimate chain |
| Score a batch of new tasks | `list_tasks`, then `update_task` for each | The same | Difficulty and minutes for each, clamped 5–600 | One estimate per task, each its own undo | Per-type defaults |
| Make tasks from school mail | Claude's Gmail connector, `add_task`, `record_mail_thread` | The message (Claude reads it, never Heat), today's date and the existing tasks | A task with notes that start "From mail:", plus a Mail row in state **Task made** | A new task labelled as Claude's, with its reason | **Make a task** in Mail (T), or type it |
| Record a grade notice | Claude's Gmail connector, `add_pending_grade`, `record_mail_thread` | The notice | A pending grade with no score and a link to Brightspace, plus a Mail row in state **Grade posted** | A yellow "Enter score" banner | Type the grade in |
| Note a thread with nothing to do | `record_mail_thread` | The thread | A Mail row in state **Nothing to do** | A row in Mail | Nothing |
| Plan the day | `plan_day` | Open tasks, free time, the day's end | Dashed drafts, not committed | Dashed blocks in the time column, waiting for Return | Plan my day, which is the same rule |
| Look at your grades | `get_grades` | Courses, items, and the percentages Heat worked out | Nothing | Nothing | The Grades tab |
| Log time you spent | `log_focus` | The task | A focus record | Minutes added to the task's time | The timer, or Get Info's "Took" |

**Undo.** Each write goes through the same journal as an edit you make (2.7), so ⌘Z undoes it, and the Edit menu says whose change it is: "Undo Claude's estimate", "Undo Claude's task". Settings → Claude lists Claude's recent changes, each with Undo, for when you've moved to another view since.

**Never invent metrics.** Ripple Creator's rule, "NEVER invent metrics", is written into the tool descriptions: Claude may only restate numbers Heat returned. Heat's own words stay plain. It does not use Ripple Creator's hype-coach voice.

**Without Claude**, everything in Heat works. Estimates fall back to your average for the type, else difficulty × 20 minutes. Due dates still arrive from the calendar feeds. Plan my day is a written rule. Mail stays empty until Claude records something, and Gmail itself is one click away.

### 3.13 The MCP server

**Decided:** Wi_WWAV is an MCP server. The first one is local. It is a small stdio helper shipped inside the app, `wi-mcp`, that Claude Desktop and Claude Code start on your Mac. Settings → Claude shows the lines to paste into Claude's config. It needs no account and no network of its own.

**Proposed:** the helper writes through the app's own store code, so a tool call is one journal transaction like any edit. If the app is open, the change shows on screen at once. If it is closed, the change is there when it opens.

**Common rules.**

- **Reasons.** Every tool that changes something takes a required `reason`, one sentence of at most 200 characters. Heat shows it beside the change.
- **Dates and minutes.** Dates are ISO 8601 with the local offset, such as `2026-10-07T23:59:00-04:00`. Minutes are whole numbers.
- **Results.** A tool returns plain JSON, and every write returns the record it changed and its `undo_label`. A failure returns one plain sentence, with the MCP error flag set: "No open task has that id."
- **Labels.** The journal row carries the actor, `claude`, and the tool. The labels are fixed (below).
- **Tool descriptions.** They are the only instructions the app gives Claude. Every write tool's description opens with these two lines:

```
Estimates and drafts. Never decides: the person accepts, edits or undoes every change.
Never invent metrics: only restate numbers this app returned.
```

- **What no tool can do.** No tool marks a task done, deletes anything, changes a due date, writes a score, sends mail, or reads or sets a **Public** switch. Privacy is yours alone (3.15).
- **Consent.** Claude's own tool-permission prompts ask before a tool you haven't allowed. Settings → Claude has a switch for each tool, and a tool switched off is missing from the list the server offers.

**The tools.** There are eight. The list agrees with 2.11.

| Tool | Arguments | Result | Undo label |
|---|---|---|---|
| `list_tasks` | `status?` `open` (default), `done` or `all`; `space?`; `due_before?`; `limit?` (default 50, at most 200) | `tasks[]`: `id`, `title`, `type`, `space`, `course?`, `due`, `scheduled?`, `difficulty`, `estimate_min`, `estimate_by` (`you`, `claude` or `default`), `estimate_reason?`, `heat` (`v`, `level`), `notes`, `source`, `done`. Also `averages[]` (your average minutes and count by type) and `spaces[]` (name and persona) | none; reading changes nothing |
| `add_task` | `title`; `space?`; `type?`; `course?` (a code such as `JPN 201`); `due?`; `notes?`; `source_id?` (such as a Gmail message id); `mail_thread_id?`; `reason` | `{ task, created }`. If `source_id` matches a task already made, `created` is false and nothing is added | "Undo Claude's task" |
| `update_task` | `id`; `difficulty?` (1–5); `estimate_min?` (clamped to 5–600); `reason`. At least one of the first two | `{ task, clamped }`, with the new heat | "Undo Claude's estimate" |
| `plan_day` | `date?` (default today); `day_ends?` (default 23:00) | `{ drafts[], unplanned[], minutes_left }`; each draft has `task_id`, `start`, `minutes` and Heat's `reason`. The drafts also appear dashed in the time column. Claude cannot accept them | none; a draft is not a change until you press Return, which is your edit |
| `get_grades` | `course?` (a code) | `{ term, courses[] }`: each course has `code`, `name`, `scale`, categories with weights, `items[]` (`name`, `category`, `score?`, `out_of`, `dropped`, `pending`), and Heat's `current_pct`, `decided_pct` and `letter` | none |
| `add_pending_grade` | `course`; `item`; `posted_at?`; `link?` (the Brightspace address); `mail_thread_id?`; `reason`. There is no score argument | `{ grade, created }`. A grade for the same course and item is not made twice | "Undo Claude's pending grade" |
| `log_focus` | `task_id`; `minutes` (1–600); `started_at?` (default: now minus `minutes`); `reason` | `{ focus_record, actual_min }` | "Undo Claude's focus log" |
| `record_mail_thread` | `thread_id` (Gmail's); `subject`; `from`; `received_at`; `course?`; `state` (`grade`, `task` or `nothing`); `task_id?`; `reason` | `{ thread, created }`. The same `thread_id` again updates the state and reason and leaves one row | "Undo Claude's mail note" |

`record_mail_thread` stores the subject, sender, time and Claude's reason. It never takes the message body.

Because `add_task`, `add_pending_grade` and `record_mail_thread` don't repeat themselves on the same source, Claude can read the same mail twice and leave nothing doubled. That replaces Heat's old list of the last 400 processed message ids.

**Remote, later.** A server through mi-wwav.com serves claude.ai with the same eight tools, arguments and labels. It needs Heat sync first (8.7). Sign-in works as the devlog's `/mcp/write` does (**Exists today:** `server/mcp/auth.js`). Its `get_grades` returns only the grades you made public, because private grades are not on the server (8.8).

### 3.14 Capture, notes and the weekly review

These come from the PKM (`portfolio/src/pkm/`), which already runs them for one person.

- **Quick capture (⌘⇧N, in every view).** A sheet that stays open for rapid entry and shows "4 in inbox · captured ✓". From the Console, a capture also stores the session and the playhead, so "fix the snare at 1:32" opens at 1:32. Captures wait in the Inbox until they are triaged "→ task", "→ note", "→ project" or "→ upload".
- **Song / Project Brief.** A project can start from the PKM's template: "Vibe / references · Tempo / key · Status: sketch · Sections · Stems / instrumentation · To finish". On Save, Heat offers "Make 4 tasks from To finish?". When the project is linked to a Console session, the tempo and key fill in from it, and the project's dot takes the work's key colour.
- **Weekly review.** A five-step wizard adapted from the PKM's (Inbox → Active projects → Someday → Schedule next week → Done), with Ripple's weekly-report questions folded into the last step:

| Step | What happens |
|---|---|
| 1 Inbox | Triage every capture, ending at "Inbox zero ✓" |
| 2 Last week | Facts only: tasks done and focus time per space, milestones reached, and estimate accuracy ("Homework: estimated 1h 15m, took 1h 32m across 4") |
| 3 Projects | Each active project's next milestone; set it to active, on hold, someday or archived |
| 4 Next week | Drag tasks onto the next 7 days |
| 5 Note | Heat sets out "What moved / What slipped / Next week's one thing" with the facts from step 2 beneath each; you write the note and press "Review complete ✓" |

- **Export.** Heat's share of the whole-app export (see 2.9) is Obsidian-ready markdown with `[[wikilinks]]`, plus `heat.json` with every record and its public or private setting.

### 3.15 Connections

#### Private by default

**Decided.** In the founder's words: "Heat is like a facebook typa thing. By default everything will be a private PKM, with a simple version appearing public, but I can toggle anything to be public, including grades."

- **Private.** Every record is private: task, project, milestone, habit, note, course, grade and focus record. A record that belongs to another follows it: a task's blocks and occurrences, and a course's pending grades.
- **A simple public version.** Two items are the default public version: your **Now making** line, and the **project timelines** you link to a solar system. Each shows once you set it, with one press on **Show**.
- **Anything else.** Every record has a **Public** switch in Get Info, off by default. It works one item at a time, and grades have it too.

Heat has two profile views (see 2.4). Your private Heat (⌘1) is the whole profile, and only you see it. Your **public Heat view** is what anyone sees when they open the sun in the middle of your galaxy in Space. It sits under your bio blocks, in this order: the Now making line, the project timelines, then any other public items under a heading for each kind. The galaxy chip's "Your public Heat view" opens it, and Settings → Privacy lists everything public with a switch on each (see 2.13).

The public version, as it ships by default:

| Item | Where | Viewers see | Never shown |
|---|---|---|---|
| **Now making** | One line under your bio on your sun | The text you approved: "Now making: the second verse of More Love" | When it was set, time spent, the task, the space |
| **Project timeline** | The project's sun, and your public Heat view | Milestone beads with titles, dates, and reached or not | Open-task counts, tasks, estimates |

Sharing is a drop. Drag a task onto "Your galaxy" in the sidebar, edit the line in the sheet ("This line will show on your public Heat view."), and press **Show**. Drag a project onto its solar system, listed under "Your galaxy", to show its timeline. Get Info has the same switch for keyboard use. The Now making line clears quietly when its task is done, or after 7 days.

#### The Public switch

**Decided.** The **Public** switch sits in Get Info on every record, and it is off by default. ⌘Z reads "Undo make public". Switching an item back to private removes its public copy from the server.

**What a public record shows** (**Proposed**). Only the fields of that one record:

| Record | Viewers see | Stays with you |
|---|---|---|
| Task | Title, due date, done or not | Notes, difficulty, estimate, heat, time spent |
| Project | Title, status, target date | Its tasks |
| Milestone | Title, date, reached or not | Its tasks |
| Habit | Title and its day-by-day grid | Any streak or running count |
| Note | Its text | |
| Course | Code and name | Its grades, percentage and letter |
| Grade | The course, the item, the score and what it was out of | The course percentage and letter, which are worked out across grades |
| Focus record | The task's title, the date and that session's minutes | Totals |

A grade's switch says plainly what it does: "Grades are private by default. Your school keeps them as education records. Turning this on shows this grade to anyone who opens your sun." Grades are FERPA education records while a school holds them. Here the student chooses to show their own, and each one stays private until they do.

#### What never crosses

Heat never sends Space a number worked out across records, and never compares people. That means no weekly load, no focus total, no done count, no habit streak, no course percentage or letter, and no ranking or side-by-side between two people, whatever is switched public. Wi_WWAV left counts out because a count "invites checking and comparing" (gate 1). A public record describes one piece of work, not the person.

The Public switch decides what other people see. It does not limit what Claude reads on your Mac. That is Claude's own permission prompt (3.13).

#### Console and Space

- **Links.** A task, milestone or project can point at a Console session, a solar system or a single work (`link: {kind, id}`). Double-clicking the link opens it in its view. A link is private and never reaches Space.
- **Focus follows you.** Start focus on a task linked to a session, and Now offers "Open session". Over the Console, the Now strip keeps reading "Mix the second verse · focus 18:42 left" (see 2.2), and that time counts toward the same session.
- **Milestones in Space.** A WWAV milestone linked to a solar system draws its bead with a small planet glyph in the system's colour. Reaching the milestone fills that bead if the timeline is shown, and publishes nothing else. Publishing stays a drop (see 4).
- **Release plans become projects.** Ripple Creator (`server/routes/rippleCreator.js`, **Exists today**) generates plans with tasks in `pre`, `launch` and `post` phases. In Heat, "New release plan" makes three milestones (Pre-release at −28 days, Release day, Post-release at +28 days) with an empty three-phase template. If you ask Claude to fill a phase, it adds the tasks with `add_task`. Existing plans import as projects (draft → someday, active → active, completed and archived → archived).

### 3.16 Data

Heat's records live in the app's local database and sync to the person's account on mi-wwav.com (see 8). Grade rows are the exception: private grades stay on the Mac unless you turn on encrypted sync (Open, 8.9). A grade you switch public is copied to the server in the clear, because other people have to read it, and the copy is removed when you switch it back. The server tables are new, because the PKM's tables have no `userId`. Writes stay optimistic, and the status bar says where things stand: "Saved on this Mac · Synced 3:41 PM".

Every record below has a `public` flag, `false` by default. `ProfileShare` rows are the two default public items themselves, and exist only once you press **Show**. The app stores no Google token of any kind, because it has no Google sign-in.

```ts
Space        { id, name, hue, groupKind: "course"|"milestone"|"free", groupLabel, types[], persona }
Task         { id, spaceId, title, type, courseId?, projectId?, milestoneId?, parentTaskId?,
               due, scheduledDate?, rrule?, difficulty, estMin, estBy: "you"|"claude"|"default", estReason?,
               adjustMin, notes, link?, done, doneAt,
               source: "you"|"ical"|"claude", sourceId?, public }   // actualMin = Σ FocusSession.focusMin + adjustMin
TaskOccurrence { id, taskId, date, doneAt }
TimeBlock    { id, taskId?|habitId?, date, start, minutes, origin: "you"|"plan" }
FocusSession { id, taskId?|habitId?, startedAt, endedAt, focusMin, interruptions, view,
               source: "timer"|"claude", public }
HeatState    { currentTaskId?, timer: { phase: "focus"|"break"|"idle", round, endsAt }, planDrafts[] }
Project      { id, spaceId, title, status: "active"|"on_hold"|"someday"|"archived", targetDate?, link?, public }
Milestone    { id, spaceId, projectId?, title, date, done, order, link?, public }
Habit        { id, title, minutes?, log: { "YYYY-MM-DD": true }, showCounter: false, public }
Term / Course / Grade   // as today, plus termId, category keywords, source "you"|"claude"|"valence", public on Course and on each Grade
MailThread   { id, gmailThreadId, subject, from, receivedAt, course?,
               state: "grade"|"task"|"nothing", reason, taskId?, recordedBy: "claude" }   // no body; never public
Calendar     { id, name, kind: "brightspace"|"ical", keychainRef, lastSyncedAt }          // the address itself is in the Keychain
Capture      { id, text, link?, triagedAt?, resultType?, resultId? }
DailyNote    { date, markdown, public }
Note         { id, title?, markdown, projectId?, link?, public }   // Proposed: the note that can go public (3.15's table); a daily note stays a DailyNote
ProfileShare { id, kind: "now"|"timeline", sourceId, text?, targetId, clearsAt? }   // the two default public items
```

The undo journal, in `library.sqlite`, records who made each change (`you` or `claude`) and which tool, which is what the labels in 3.12 are made from.

**Moving in.** The artifact has no export today, so step 0 is a "Download JSON" button on it. The import maps each workspace to a space and keeps every id (event ids, the `em-` and `gp-` hashes, the 400 processed Gmail ids), so the first sync in the app finds nothing new instead of everything twice. The Gmail ids and hashes become `sourceId`s, so when Claude reads the same mail it finds those tasks already made. Everything imports as private.

### 3.17 Keyboard

| Key | Action |
|---|---|
| ⌘1 / 1–6 | Heat; then Today, Tasks, Calendar, Grades, Habits, Mail (no field focused) |
| N, ⌘⇧N, ⌘K | New item; quick capture; command palette |
| ↑ ↓, Return, ⌘I | Move; edit; Get Info |
| ⌘↩, ⌫ | Mark done; delete (with Undo) |
| C, P | Make current; plan into the next free gap |
| ⇧Return | The tab's secondary action (3.3): Plan my day, Triage inbox, Today, Add course, Show the year, Open in Gmail |
| F, ⇧F, I | Focus start or pause; stop and log; pulled away |
| M W D T ← → | Calendar views, today, page |
| T (in Mail), ⌥⌘R, ⌘F | Make a task; sync calendars; filter the current list |
| ⌘Z, ⇧⌘Z, Esc | Labelled undo and redo; close without losing saved work |

### 3.18 Left out, and why

| Left out | Why |
|---|---|
| Notifications, dock badges, reminders that fire on their own | Anything that reaches out fails gate 1.1; Heat shows what is due when it is open |
| Auto-starting the next round | An "up next" keeps a person past what they came for |
| Ripple's momentum score and daily missions | A productivity score invites checking, and the weekly facts cover the same ground |
| Replying or sending in Mail | Gmail already does it, and Wi_WWAV never touches your mailbox |
| Submitting to Brightspace | Writing back needs the school's approval, and planning doesn't need it |
| The PKM's kanban and graph views | The sketch names six tabs; the Obsidian export gives a graph to anyone who wants one |
| Google sign-in, the Gmail restricted scope, Google verification and 7-day testing sign-ins | Claude reads mail through its own Gmail connector, and calendars come in as iCal addresses, so the app needs no Google account. |
| Heat reading Gmail itself: its Brightspace query, School label and saved searches, message bodies, the reading pane, "Load images" | Mail shows only the threads Claude recorded, with their state. |
| Heat's own calls to Claude: `/api/assist/:task`, a daily limit per account, consent sheets, the "Ask Claude to score" button | Wi_WWAV is an MCP server instead. Claude's own permission prompts ask before each tool call. |
| Syllabus import by Claude | The app calls no model, and none of the eight tools writes a course. Type the course in. Cut on 7 Oct 2026 to narrow v1. |
| A Claude-drafted weekly note | The same: no tool writes a note. Heat lays out the facts and you write it. |
| A Claude tool that writes a score, marks a task done, or sets the Public switch | Claude estimates and drafts, never decides. Scores and privacy are yours. |
| The "Finish payout setup so your shelf can open" task | Commerce comes later, inside Space. Cut on 7 Oct 2026 to narrow v1. |

### 3.19 The look, and open decisions

Heat keeps its Aqua and brushed-metal skin (see 7). Its LCD stays iTunes olive while the Console's screens are DMG green, so the screen's colour says which view you're in. A check against gate 2.4 (body text 7:1, secondary 4.5:1) finds five pairs to fix: the LCD's dim text, the sidebar headings, the third ink, white on the `#3875d7` selection (4.47:1), and the label on the blue gel buttons. The fixes are in 7.2: selected rows fill `#1B4C8C` with white text (8.5:1) and keep a 3 px `#3875D7` bar at the leading edge.

| Open decision | Recommendation |
|---|---|
| **Streak counter** (gate 1.1) | The grids and a growing record by default; the counter is a per-habit setting, off by default |
| **Deadline alerts** (gate 1.1) | None by default. Allow one hand-set alert per task ("Remind me at 9 PM") that fires once |
| **Valence / LTI with URI** | Ship on iCal and Claude's Gmail connector. Ask URI's Brightspace admins to register the app after the small-group stage (9.5), with the group's weekly reviews as evidence, because real scores end the pending-grade guesswork |
| **Control size** (gate 2.4 fails any control under 44×44 pt; 26 px rows and 13 px habit squares fail it) | Keep 44 pt for buttons, tabs and orbs. Before testing, write a separate rule for dense rows and grids: 24×24 pt hit areas (the WCAG 2.2 minimum) and every action on the keyboard |
| **Now making line** | Build it, shown only once you've written one. Drop it if it starts to feel like a status that has to be kept up |
| **Remote MCP server** | Build it after Heat sync. Same eight tools and labels. A remote call writes to the synced copy, and the Mac journals it when it arrives, with the same "Undo Claude's…" label. Its `get_grades` returns only the grades you made public, because private grades are not on the server (8.8). |
| **Who Heat is for** | One account first, with nothing hard-coded to a school, so a classmate could use it next. That is also the clearest route to gate 4 |

## 4. Space: the social view

Space is the social view. It opens with ⌘2, and it is where people and their work live. Every person is a galaxy, every project a solar system, every work a planet, and every page a sun. You move through it by zooming, and the player is not a screen you go to but a planet you come close to. The view is named after the first tab of v4's web app. Everything here is **Proposed** unless labelled **Exists today**, **Decided** or **Open**.

Space is for looking, listening and reading (**Decided**). Its only controls on sound are the level, mute and solo of the four moons, and ↑ Push, which forks that mix. Anything more is made in the Console, and **Open in Console** (⌘E) is one press from any song or film. In the founder's words: "remixing is Mi-WWAV and Console, not Space."

### 4.1 Where it comes from

| Source (all **Exists today**) | What Space takes |
|---|---|
| WWAV App v3, DISCMAN, iOS (`ios_v3/`) | the planet player, stem player as identity, lineage Cosmos |
| v4 SOLAR SYSTEM, web (`wwav/`, at `/summer_26`) | the galaxy model, zoom tiers, suns as block pages, ↑ Push, lineage with consent, browse rules |
| v5 SOLAR SYSTEM, iOS (`ios_v4/`, `server/routes/v2/`) | Kepler motion, gallery planets, the universe map |
| Portfolio Works hub (`portfolio/src/components/Hub.jsx`) | a camera that orbits a system, depth dimming |
| PRANA (`prana/SPEC.md`) | stem order and stem colours |

**Open: which v3.** The sketch calls the social box a "souped up version of WWAV app v3" and calls it intergalactic. In the archive, v3 is DISCMAN, the spring 2026 iPhone app "for music, image, text and video posts"; the galaxy is v4 (web) and v5 (iOS), SOLAR SYSTEM. The `ios_v3/` folder is a music-only build that already carries the summer planet. Space takes from both. Recommendation: read "v3" as "the iPhone app", leave the archive's numbering alone, and give Wi_WWAV its own archive entry.

### 4.2 The model

| Tier | Is | Stored in (**Exists today**) |
|---|---|---|
| Universe ("Everyone") | every galaxy, on a spiral | `Galaxy.universeX/Y` |
| Galaxy | one person: a bio sun and their systems | `Galaxy`, 1:1 with `User` |
| Solar system | one project (an album, a film set, a book, a collection): a sun and up to 21 worlds | `SolarSystem` |
| Planet | one work | `Planet` (song, film, gallery; page is new) |
| Sun | a page | `Sun`, blocks JSONB |

The desktop app uses the iPhone's `/api/v2` routes, adding only the page planet and the letter's shape (4.7, 4.8). **21 worlds to a system**, "the length of the first record". A 22nd is refused: "A solar system holds 21 worlds. Start another one." (**Exists today:** `systems.js`.) Desktop shows all 21, seven to a ring, at radii 640, 1120 and 1580. Worlds shrink outward (200, 170, 145), and each ring is offset half a seat so the rings never form spokes (**Exists today:** `Orbits.swift`). A legacy system of more than 21 gathers the rest into one ringed overflow world: "nothing is hidden, it is gathered."

### 4.3 The room

The sky fills the view, 1280 × 726 pt in the default window. The breadcrumb sits top left ("Everyone › LMY › World Ending", every crumb clickable). The selection's info card sits at the right edge: a 64 pt miniature world, an italic title, "LMY · gen 0", key and BPM chips, and **Play** (**Watch**, **Read** or **Look** for the other media). **Newest** (4.10) is bottom left, and − · + · fit are bottom right. "Since you last looked" opens at the top when there is anything new (see 2.10). The title bar's galaxy chip, your own galaxy in miniature, is the way home (see 2.1).

Space wears the night: `#070A18`, ink `#F4EFE6`, amber suns, and royal blue `#2946FF` with exactly three uses: armed or active, the current lineage branch, and committing actions (**Exists today:** `tokens.css`; see 7). The secondary act (⇧Return) is **Add** in the sky and **↑ Push** in the player (see 2.7).

### 4.4 Moving through it

| Input | Does |
|---|---|
| Two-finger scroll, or drag on empty sky | pan; at the system tier, turn the system about its sun (as on iOS) |
| Pinch, ⌘-scroll, ⌥⌘+ / ⌥⌘− | zoom about the pointer; buttons step ×1.55. ⌘+ and ⌘− stay text size (2.13) |
| ⌥-drag from empty sky | orbit the camera: pitch 0.35–1.15 rad (default 0.62), 0.006 rad per pt across, 0.004 down (the hub's numbers) |
| Click · Return or double-click | select and show the info card · dive in (fit to 80%) or, on a work, open its player |
| Hold 400 ms on a galaxy | peek: its bio sun inline, without leaving the sky |
| Space | play or pause the selected work in the Now strip, without opening it |
| Tab or arrows · Esc · ⌘↑ | next body in orbit order · up one tier · home to your galaxy |
| ⌘E · drag a planet onto "Console" | Open in Console (4.6) |

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

**Decided:** Space is drawn in real 3D. v4 used no 3D library because the DOM gave "free hit-testing, labels and accessibility", and iOS fakes tilt with "a slight vertical squash". The desktop app does depth properly and keeps those three things.

**Rendering.** three.js in the web UI layer, on WebGPU where the system web view supports it and WebGL 2 otherwise. PRISMON already runs three.js ^0.182.

**What depth adds:** parallax between three star layers and the systems in front; a camera that swings around a system as the hub's does; one light from the upper left (`WWAVLight.sun` ≈ (0.30, 0.25)); opacity 0.45–1.0 by depth (**Exists today:** `Hub.jsx`); and gallery photos that foreshorten.

**What stays flat is text.** Every name, chip and card is a DOM element placed at its body's projected point each frame, so text stays sharp, selectable and readable by VoiceOver. Hit-testing runs in world space against the frozen layout. Every body is also a node in an accessibility tree, in orbit order: "Low Tide. Song, A minor, 86 BPM. World 3 of 12 in World Ending." Under Reduce Motion, dives are 140 ms cross-fades and the camera has no inertia.

**Budget.** 60 fps at 2560 × 1600 on an M1 MacBook Air, with up to 2,000 galaxies as instanced points. The frame loop stops completely when nothing moves.

### 4.6 The player is the planet

In v4, "every gesture the product has lives on this screen" (**Exists today:** `PlanetScreen.jsx`); here it is the listening player. Return on a song flies the camera in over 900 ms (`dive`) and the player opens over the scene in 420 ms (`morph`); Esc flies back to the same camera. Elsewhere the same player lives in the Now strip (see 2.2 and 2.3).

**Layout, on a 560 pt stage.** With `dim = min(w, h) · 0.92` = 515 pt (**Exists today:** the shared spec in `MoonField.jsx` and `PlanetStageView.swift`):

- **The planet** is centred and is the play button. Its radius is 0.17 · dim = 88 pt.
- **Four moons**, 23 pt in radius, sit on a plus sign: vocals up, drums down, bass left, other right, each travelling its arm from 139 pt (level 0) to 242 pt (level 1) from centre. A moon is the pastel sibling of the key colour with a 3 pt rim in its PRANA stem colour: vocals `#D23C2A`, drums `#F0B90B`, other `#2E9A55`, bass `#1F4E9E`.
- **Around them:** the header (84 pt cover, title, chips "remix", "source", "tree", "team"); the 56 pt waveform; **↑ Push** bottom left and **Open in Console** bottom right.

"A moon's distance from the planet is its volume. There is no other fader."

**Gestures** are the canonical ones for every stem in the app (**Exists today:** `gestures.js`, `MoonGestureMachine.swift`).

| On | Input | Result |
|---|---|---|
| planet | click | play / pause |
| moon | click (under 250 ms and 8 pt) | mute at once. The moon goes "eclipsed", drawn as 7.3's hollow muted ring (v4 dimmed it to 0.45), but keeps its distance, so it remembers its level |
| moon | second click within 250 ms | revert that mute and solo instead, with no double-click delay. The solo wears the blue ring |
| moon | drag along its arm (8 pt slop) | level = `clamp(level0 + (d · armDir) / span, 0, 1)` |
| focused moon | ← → ↑ ↓ · M · S · Tab | level ±5% · mute · solo · next moon in file order (vocals, drums, other, bass) |

These four controls change what you hear and nothing else. The level, mute and solo run in the one audio engine (**Decided**; see 8).

**Waveform.** 400 buckets from only the audible stems, "mute the drums and the drum shape vanishes", against a normaliser fixed to the full mix. Click or drag to seek; hold ⌥ to split it into stem colours.

**↑ Push** "forks your mix into the song's lineage without stopping playback" (**Exists today:** `PushButton.jsx`). It turns blue once the mix differs from its source. A press (or ⇧Return) sends a comet spark off the planet, shows "in the lineage" for 1.6 s, and adds " (fork)" to the title. "A fork owns no audio": it is a few kilobytes of mix state, shaped like a `.wwav`'s `wrmx` chunk, pointing at the parent's stems. It joins the lineage at once and takes a seat in your galaxy only when you drop it on a system.

**Open in Console** (⌘E, **Decided**). On any song or film, in the player or with the planet selected in the sky, it opens the work in the Console as a session to "tear it apart completely". A song opens with four stem lanes and the parent's remix snapshot applied, under a header such as "fork of glass hours · gen 3". A film opens with its picture on a video track. The session points at the parent's file and never writes to it (5.14). Dragging the planet onto the "Console" segment does the same. A work you have never opened in the Console needs a connection first, and the button says so (see 2.8). What you can do once it is open is in 5 (5.14).

### 4.7 Four media, four kinds of world

All four are worlds you look at, listen to or read. Space makes none of them (**Decided**).

| Medium | Family | World | Colour | A reply is | File |
|---|---|---|---|---|---|
| Music | Mi_WWAV | a planet with four stem moons | key colour | a fork, made in the Console or with ↑ Push | `.wwav` |
| Film | Si_WWAV | a planet that wears a ring, ×1.09 | body from the poster; ring in the soundtrack's key colour | a fork, made in the Console | `.swav` |
| Writing | Ri_WWAV | a page planet | paper | a link, which waits for consent (4.9) | **Open** |
| Fashion | Gi_WWAV | a gallery planet, ×1.35 | from the cover photo | a link, which waits for consent (4.9) | photos |

**Colour.** A work's colour is its key colour: hue = ((pc · 7) mod 12) · 30 with A = 0, major hsl(h, 72%, 58%), minor hsl(h, 58%, 42%) (**Exists today:** `keyColor.js`). Works with no key take a resonant colour from their image (saturation at least 0.55, lightness 0.42–0.62; **Exists today:** `posterColor.js`). Night indigo, hue 232, "still condensing", stays reserved for music whose key isn't known yet; the Console estimates key and BPM on import and after recording, and a song exported with a key arrives coloured (5.4, 5.13).

#### Film

**Exists today:** in v4 a film is a planet that "wears a ring", and opening it pauses the song: "The planet is paused while this plays." In v5 it opens in "the screening room".

**On desktop:**

- **The screening room.** The film fills the stage at its aspect ratio, and F gives "nothing but the film". Esc flies back to the same camera. It plays on the Rust video side, FFmpeg decode presented by wgpu against the engine's clock (**Decided**), and anything you were hearing pauses.
- **Viewing only.** The room has no grade and no fork. To change a film, press **Open in Console** (4.6). A film made from a song hangs from that song's planet and is its child in the lineage (4.9).

#### Writing: the page planet

Writing has no world today. Suns are already block documents, so a written work is a sun's sibling: a sun is a page about someone, and a page planet is a work.

- **Look.** A pale world in paper (`#F4EFE6`) with fine ink latitude lines at 0.14 opacity, like ruled paper. A recorded reading gives it one moon, the voice, which plays on a press.
- **The reading room.** Return brings the lit face up to fill the stage, and the text rises into one 640 pt column: the title in Cormorant Garamond italic at 46 pt, the body in Inter at 17 pt with 1.6 leading (17.2:1 on night). It ends with the author's name and "That's the end."
- **The document** uses the suns' codec, which keeps keys it doesn't know (**Exists today:** `wwav/src/sun/blocks.js`):

```json
{ "v": 1, "kind": "work", "medium": "writing",
  "blocks": [
    { "type": "text", "style": "heading", "text": "Glass Hours" },
    { "type": "text", "style": "body", "text": "the drums leave and the room gets bigger" } ] }
```

- **Where writing is made.** In the suns' block editor (4.8); the Console is for sound and picture.

**Open: Ri's file.** Writing has no WWAV format yet. Recommendation: until Ri_WWAV defines one, export a page as Markdown, with `.wwav`'s `wmet` and `wlin` keys in its front matter (`ri: "0.1"`, `work_id`, `parent_id`, `root_id`, `generation`, `creator`). It then opens in any editor (gate 2.3) and joins the id space songs and films already share.

#### Fashion: the gallery planet

**Exists today:** v5's gallery planet is "a big lit sphere the artist has pinned photographs onto", up to 40 of them (`server/routes/v2/galleries.js`), placed at `{lon, lat}` in radians. Photos foreshorten toward the limb and hide on the far side, and the owner's pin mode keeps everyone seeing the planet "dressed the same way".

**On desktop:**

- **Looking.** Drag to spin (0.006 rad per pt) with momentum. ← and → turn the next photo to face you, and Return blows it up. Esc puts it back.
- **Lookbooks.** A lookbook is a system of gallery planets, one per look, with a sun for the season, materials and credits.

**Open: where pages and galleries are made.** Space only shows them. Pages can be written in the suns' block editor (4.8), but nothing in the app makes a gallery. Recommendation: v1 shows galleries that already exist and makes none, and a gallery maker waits until fashion has a format of its own (see 6).

Writing and gallery works have no Open in Console; the Console is for sound and picture.

### 4.8 Suns and letters

**Exists today (v4):** a sun is a blocks v1 document of text (heading, body, quote), photos (an R2 key) and details (label/value pairs). It opens as a sheet over the scene, so the camera keeps its place; an empty one reads "Nothing written here yet." The editor offers "Add words", "Add a picture", "Add details" and "Add a line", with the placeholder "Say it plainly", and saves the whole document at once, because "its ORDER is most of its meaning." The blocks are plain and stacked, one after another.

**On desktop:**

- **Editing.** Your own sun opens straight into the editor; other people's open to read. There is no layout to drag, size or rotate. The blocks flow in order.
- **The bio sun** is in the middle of your galaxy. It opens your **public Heat view** under your bio blocks (**Decided**; see 2.4 and 3.15). The view holds what you have shown: a "Now making" line, then the milestone timelines of the projects you linked, then any other record you switched public, each kind under its own heading. Anyone who opens your sun sees it, read-only, and you see the same page through the galaxy chip's "Your public Heat view". It never shows a count or a comparison (see 3.15).
- **A project sun** shows the milestone timelines linked to its solar system, as beads with titles, dates, and reached or not. On your own project sun, the secondary act (⇧Return) is **Plan in Heat**, which makes a linked milestone (see 2.6).
- **Heat reaches a sun only when you show it.** Making a Now making line or a timeline public is one press on **Show** in Heat (see 3.15). Nothing else in Heat is on a sun.

**Letters.** Every devlog post is a letter, opening "Dear Wi-WWAV," since the second post, where the founder coined the name ("Wi-WWAV (we wave)… we are the wwav"), and signed "LMY". A letter is a page with a greeting and a sign-off:

```json
{ "v": 1, "kind": "letter", "greeting": "Dear Wi-WWAV,",
  "blocks": [ { "type": "text", "style": "body", "text": "Hardware is hard…" },
              { "type": "photo", "imageKey": "devlog/01JA….jpg", "width": 60 } ],
  "signoff": "Sincerely LMY", "sentAt": "2026-10-03" }
```

- **Read-only on a sun** (**Decided**). Space has no letter editor. The founder's devlog is written on the page at mi-wwav.com, and the app reads it.
- **Where letters live.** A bio sun lists them newest first, ten at a time, then "Show older", then "That's everything." Letters from galaxies you've added appear in "Since you last looked" (see 2.10); a message to one person goes through the message door (4.11).
- **The founder's devlog.** LMY's letters are read from `DevlogPosts`, so the archive, the `/mcp` connector and the app show the same posts. The devlog's image width (a % of the column) becomes the photo's `width`, and writes still go through `readPost` (**Exists today:** `server/routes/devlog.js`).

### 4.9 Lineage is the social graph

"Lineage is the only social graph." (**Exists today:** `wi/README.md`.) There are two kinds of edge (**Exists today:** `server/routes/v2/lineage.js`):

- **Fork edges are facts.** The machine declares them (server `parentTrackId`, `remixDepth`; the file's `wlin`) and nobody edits them. A fork comes from ↑ Push, or from a `.wwav` or `.swav` exported by a Console session that was opened from its parent.
- **Links are claims.** A link is one of `influence | sample | collab | cover | custom`, with a note of up to 280 characters. One touching someone else's work waits until they accept, "the only thing stopping the graph from becoming a place to attach yourself to strangers". Links inside your own galaxy accept themselves.

**Declaring a link**, which the web never had a UI for: ⌥-drag from one planet onto another, pick the kind, add a note, and press **Ask**. (A ⌥-drag that starts on empty sky orbits the camera instead; see 4.4.) The other owner sees "LMY says their planet is an influence of your planet", with **Agree** and **Refuse** (**Exists today:** `PendingLinks.jsx`; the same list sits in "Since you last looked", see 2.10).

**Seeing a family.** The "tree" chip lays the family out as its own system: the root at the centre, forks on generation rings at 90 + (g − 1) · 70, each subtree's arc proportional to its leaves, "so families stay together" (**Exists today:** v3 `CosmosLayout.swift`). Your branch wears the accent. Return on a node opens its player, and ⌘E opens it in the Console, which is how you fork from there. There is no view of two galaxies or of everyone; the only graph is the family.

**Across media.** Songs and films already share one id space ("a film made from a song is its child"); page and gallery planets join it, so a book can descend from an album. Whether a sold fork pays its ancestors waits for commerce (4.13).

### 4.10 Finding work: the sky and Newest

v5 removed browse: "the sky is the only way through everyone else's work". On desktop the sky stays the main way, plus one list, **Newest**, for the keyboard, VoiceOver and quick looking. It keeps v4's rules (**Exists today:** `Feed.jsx`): one card per screen, and "Nothing plays on its own. A card shows you what a thing IS and waits to be chosen."

- **The card** carries a fact line ("Song · A minor · 128 BPM", "Writing · 1,200 words", "Fashion · 14 photos") and one button: **Play**, **Watch**, **Read** or **Look**. No "N seen".
- **Order and end.** Newest first is the only order, filtered by medium (Mi · Si · Ri · Gi), "Galaxies I've added", key and BPM. 30 cards, then "Show older", then "That's everything."

### 4.11 Social features and the gates

WWAV's earlier apps had likes, follower counts, comments, push notifications and a popularity dial. The founder's gates (`wi/GATES.md`) fail each one; the table gives the default that passes.

| Feature | Exists today | Proposed default | Reason |
|---|---|---|---|
| Likes | `Like` | **Add**: a private save to your Saved shelf (`v2/saved.js`), never counted, shown or announced | gate 1.3 names likes |
| Follows | `Follows`, with a push on every follow | **Add galaxy**: a private list that feeds "Since you last looked" and Newest; the person isn't told | gate 1.3 names followers |
| Counts | "N seen", rollups, leaderboards | none, for anyone, owners included | they "invite checking and comparing" |
| Comments | `Comment` | the reply is a work: a fork, made in the Console or with ↑ Push | "The reply to a song is a remix." |
| Messages | mutual-follow DMs with snapshot attachments | the message door opens once two people have added each other. No read receipts, no unread badge | gate 1.1 |
| Notifications | APNs on likes, comments, follows | none; "Since you last looked" (see 2.10) | gate 1.1 names notifications |
| Popularity | New / Popular, the feed ranker | newest only | "Nothing decides for you what's worth seeing" |
| Stories | 24-hour stories | left out | they exist to be checked before they vanish |

**Open: which philosophy governs Space.** The sketch implies the familiar features; the gates are the founder's newest written word. Recommendation: ship the defaults above. If one returns, let it be Add galaxy made visible to the person added, as one "LMY added your galaxy" line, never as a number.

### 4.12 Arriving and leaving

**Connected uploads** (flows in 2.6). Drag an export from the library drawer (⌘L) onto a solar system, or onto an empty seat. The planet "condenses in" from 0.82 to full scale while a ring traces the upload (**Exists today:** `Planet.jsx`, where the ring traces the stems' download; here it traces the upload); the server keeps the exact bytes and reads lineage from `wlin`. A `.swav` also gets a streaming copy (H.264 up to 1080p, CRF 23) for phones and the sky; download returns the original (6.2, 8.7). Uploads wait for a connection and finish on their own (see 2.8). Going back, press ⌘E or drag any planet onto "Console" (4.6). A work's **Copy link** (**Proposed**) opens its share page on www.wi-wwav.com with **Play**, so the paperclip row in 7.5 stays true (8.7).

**Open: unpublishing a work others have forked.** Recommendation: the planet leaves the sky, forks made before then keep playing its stems, and their trees show "withdrawn by its maker".

### 4.13 Commerce, later

**Decided.** In the founder's words: "Eventually the space will hold commerce instead of having that be a 4th place." Works in Space will one day carry a price and be bought where they are. Nothing of it is designed or built now: no price, no tag on a planet, no checkout and no payout. The walkable shop is gone. Until then the whole work plays in Space, and nothing in this chapter depends on it. What commerce would mean for the business is in 9.

### 4.14 Left out, and why

| Left out | Why |
|---|---|
| The game layer: fog of war, fuel, travel cost, wormholes, comets, supernovas, black holes, terraforming | Cut on 7 Oct 2026 to narrow v1. |
| Constellations, gravity drift and binary pairs | Cut on 7 Oct 2026 to narrow v1. Lineage is the only graph, and a family tree is its one view. |
| The astronaut and the rocket, and avatars of any kind | Cut on 7 Oct 2026 to narrow v1. Your galaxy chip is who you are; nothing walks in Space. |
| FX moons and the 400 ms hold that blooms them, the tempo and pitch dials, reverse, beat repeat, the remix deck | "Remixing is Mi-WWAV and Console, not Space." |
| ＋ add a song (the figure-eight) | "Remixing is Mi-WWAV and Console, not Space." |
| The video synth | "Remixing is Mi-WWAV and Console, not Space." |
| Export from the player | Exporting is the Console's job (see 5). |
| Film grade forks and new-soundtrack forks in Space | "Remixing is Mi-WWAV and Console, not Space." A film fork is made in the Console. |
| Quote-and-reply forks of writing, and "styled from" fashion replies | Cut on 7 Oct 2026 to narrow v1. Space makes no work; a reply is a fork from the Console or ↑ Push. |
| A freeform layout for suns | Cut on 7 Oct 2026 to narrow v1. A sun is plain stacked blocks. |
| Writing letters in the app | Letters are read-only on a sun. The devlog is written on the page. |
| "Near you" | Cut on 7 Oct 2026 to narrow v1. Newest is the one list. |
| Prices, listings and the shop tag on a planet; buying from Space | Commerce is later (4.13). |
| School: classes as solar systems, assignments, younger students | Later. Cut on 7 Oct 2026 to narrow v1, and not designed now. |
| Autoplay and hover previews | "nothing makes a sound until a press"; v1's three-second hover preview fails gate 2.4 |
| Showing who else is looking at a planet | a visible crowd is an attention count |

## 5. Console: the creation view

The Console is the creation view. It opens with ⌘3, and it is where you make things. It is one timeline for audio and video, "the best parts of Premiere and Ableton smashed together", with third-party VST3 and AU plugins and MIDI from the first release (**Decided**). It is the software sibling of Mi_pro_WWAV, which the archive describes as a "Desktop studio … instruments and sequencing."

The Console has two verbs, written the way gate 2.1 asks: *making a song that comes apart*, and *cutting a film and its four-stem score on one clock*. It is also where a song or film from Space is torn apart: **Open in Console** (⌘E) lands here (see 4.6 and 5.14). Everything in this chapter is **Proposed** unless it is marked otherwise. File layouts are in 6. Files; the engine and video processes are in 8. Under the hood.

### 5.1 What it is built from

Most of the Console already exists in the repo, spread across four products and one tool. The Console puts the kept parts on one clock.

| Exists today | Where | What the Console takes |
|---|---|---|
| PRANA's engine | `prana/core/dsp`, `prana/SPEC.md` §4 | the few built-in effects: reverb, delay, distortion, tremolo, the filters and the master limiter |
| v4's 8-track editor | `wwav/src/data/projectSnapshot.js`, `server/routes/forks.js` | the idea: clips that point at stems instead of holding audio |
| MI-WWAV-OS | `MI-WWAV-OS/engine/src/model.rs`, `app/lib/screens/timeline.dart` | Clip / Tag / Sequence, cut at playhead, the OUTPUT row, the undo journal, the three verbs, "what you hear is what you render" |
| WWAV Push | `vst_plugin/` | a permanent link that later exports update |
| Local Demucs | `ios_v3/demucs_server/` | htdemucs, about 80 MB, 30–60 s per 3-minute song on Apple Silicon |

The rest of v3's remix deck, v3's video synth and the grade are left out (5.18).

### 5.2 The window

The Console fills the view area of the shell (see 2): 1280 × 726 pt at the default window size. It has one main view, the arrangement.

```
┌ transport 56 pt ──────────────────────────────────────────────────────────────┐
│ ⏮ ▶ ● ⟲  [ BAR 042.3.120  01:21:07:14 / 92.00 BPM  A MIN  4/4 ]  GRID ○  CPU ▮▮▯ │
├ browser 220 ┬ arrangement ─────────────────────────────┬ viewer 320 × 180 ──────┤
│ Library     │ ruler: bars above, timecode below        │                        │
│ Plugins     │ ▾ Low Tide  ● ● ● ●     (stem group)     ├ inspector 320 ─────────┤
│ Takes       │   vocals ▬▬▬▬▬   drums ▬▬▬ ▬▬▬            │ the selected clip,     │
│             │ Keys (instrument)  ▭ ▭▭ ▭                │ track or note          │
│             │ Film (video)  ▣▣▣▣▣▣▣                    │                        │
│             │ OUTPUT                                   │                        │
├─────────────┴ bottom panel 240: Editor · Chain · Mixer ────────────────────────┤
```

| Region | Size | Toggle | Contents |
|---|---|---|---|
| Transport | 56 pt tall | always | return to zero, play, record, loop, the screen, tempo, key, meter, grid switch, engine load |
| Browser | 220 pt | ⌥⌘B | filtered views of the library (see 2) plus your plugins; drag anything onto a lane |
| Arrangement | the rest | always | 196 pt track headers, then lanes 56 pt tall (24–240). The last row is OUTPUT |
| Viewer | 320 × 180 pt | appears with the first video track | the picture at the playhead; ⌥V widens it to half the window; tears off onto a second display |
| Inspector | 320 pt | ⌘I | every field of the selection, in Heat's Get Info drawer style |
| Bottom panel | 240 pt (120–480) | ⌥1–⌥3; the open one again closes it | Editor (the piano roll for an instrument clip, the waveform, gain and loop for an audio clip), Chain (the track's devices), Mixer |

**Metal outside, Game Boy inside.** The chrome is MI-WWAV-OS's metal (`#FBFCFD`, `#E6EAEF`, `#C9D0D8`, edge `#98A2AD`) with a 1 px pinstripe every 4 px that "reads as texture, never as stripes". Every readout sits behind glass as a DMG-green screen: the transport screen, tempo and key, the limiter's gain-reduction meter, a track's latency tag. The screens use the four DMG tones `#0B1F0B` / `#0F380F` / `#306230` / `#8BAC0F` with `#C6E24A` glow, in uppercase Menlo dot-matrix. **Exists today:** these tokens and the rule "Aqua is the case; the Game Boy is inside it" (`MI-WWAV-OS/app/lib/style/theme.dart`). The register is in 7. Look.

**Colour.** One accent per screen: the Aqua highlight `#3875D7` marks the selection and armed record. Meters are segmented and "DMG green until it is about to be too loud": `#8BAC0F` to −6 dBFS, amber `#F0A32E` to −1, rec `#E0453A` above. Stem lanes wear PRANA's stem colours, and every other track takes one of the six library labels. Drums yellow `#F0B90B` is never used for type, so names on drums lanes are set in ink.

**The status bar is the deck.** As MI-WWAV-OS prints its verbs on the device, the Console prints them in the shell's status bar: "Return Select · ⇧Return Cut at playhead · Esc Deselect · ⌘Z Undo move clip", with "48 kHz · 128 smp · 7.3 ms · saved" on the right.

**Decided: no clip-launch view in v1.** There is one view, and no second, Ableton-style session view with a grid of clips. A launch grid has no meaning for picture, and a second view would double the grammar ("exceptions are how a fourth verb gets in"). Playing live is recording: arm a track, play, and what you played is a take (5.6).

**Heat in the Console.** A session can be linked to a task or project in Heat, and focus follows you in. The Now strip keeps the task on its left and shows the session on its right, "BAR 42.3 · 128.00 BPM · REC ARMED", and the focus countdown runs on (see 2.2 and 2.6). Pressing play here pauses a song you were listening to in the player, and nothing resumes on its own (see 2.3).

### 5.3 Tracks

| Track | Holds | Header | Notes |
|---|---|---|---|
| Audio | audio clips and takes | arm, M, S, input, monitor, role | mono or stereo |
| Instrument | MIDI clips and one instrument | arm, M, S, MIDI input, role | a third-party VST3 or AU instrument only (**Decided**). The app has no built-in instrument and no sampler |
| Stem group | four lanes in file order: vocals, drums, other, bass | a 20 pt key-coloured planet, four stem lights | a dropped `.wwav`, or any audio split in place |
| Video | `.swav`, `.mp4`, `.mov` clips and their linked audio | hide | thumbnails along the lane |
| Bus | the reverb and delay returns, and groups | M, S | the two returns exist in every session, as PRANA's shared sends do |

The four stem lights on a stem group's header are the title bar's: a click mutes at once, and a second click within 250 ms reverts the mute and solos instead (see 2.2). There is no titles track and no generator track (5.18).

**Every audio and instrument track has a stem role**: vocals, drums, other or bass, chosen from four 10 pt dots in the header that wear the stem colours. A microphone track starts as vocals, a drum-kit instrument as drums, everything else as other. The role decides which stem bus the track feeds, and so which stem it folds into on export.

**The fold rule** (how N lanes become four stems, as chapter 2 promised):

1. Each stem is the sum of every track with that role, post-fader and post-insert.
2. The reverb and delay returns are rendered once per role, from that role's sends only, so each stem carries its own tail and the four stems sum to the master before the master chain.
3. The master chain shapes the master only, and the export sheet says so.
4. A video track never enters a `.wwav`. Its sound does, because it lives on a linked audio track with a role.

### 5.4 Timing: as you played it

**Nothing you play is moved unless you ask.**

- **Grid.** The grid is always drawn. Snapping applies when you move or draw something, because arranging wants bar lines; it never touches what you recorded. G toggles snapping; holding ⌘ while dragging bypasses it once.
- **Click.** The metronome is off until you turn it on, per session. **Open:** whether new sessions start with the click on. *Recommendation:* off, so a recording starts as free as you played it.
- **Quantize is an act, and it is for MIDI only** (**Decided**). Q quantizes the selected notes. Every note keeps the time you played it, so ⌥Q **Return to played** restores it at any point later. Audio is never quantized and never stretched to a grid: it plays at the speed it was recorded. To line a recording up, move the clip or cut it.

```
note {
  pitch: 0..127, vel: 1..127, release_vel: 0..127,
  at_beats: 12.4871,          // where it sits now
  len_beats: 0.2310,
  played_at_beats: 12.4871,   // where you played it; never rewritten
  played_len_beats: 0.2310
}
```

- **Looping.** A clip can loop, as a flag on the clip and not a new file. The loop length snaps to 1/8–16 beats, the range `TODO_2.md` asked for, and the looped section is shaded yellow.
- **Estimates.** Today v4 estimates tempo and key only when a song is played, so most works have neither, and v3 asks you to type the BPM. The Console estimates both on import and after recording (tempo 60–180 BPM with a prior around 120; **Exists today:** `wwav/src/audio/analysis.js`) and labels each as one: "≈ 86 BPM (estimate)" ("never invent metrics"). The estimate sits beside the session's tempo and key, and changes nothing until you type it in. It never moves audio.

### 5.5 The piano roll

The piano roll opens in the bottom panel (⌥1) when an instrument clip is selected, and drags up to full height.

- **Keyboard and rows.** A 48 pt keyboard on the left auditions on click, through the track's instrument. Rows are 12 pt (8–24). Rows outside the session key are one tone darker; **Fold to scale** hides them.
- **Notes.** Notes take the track's colour, and velocity is a bar inside each note, never colour alone.
- **Velocity lane.** 64 pt, underneath. Drag a stalk to set one note, or drag across stalks to draw a line through them.
- **Drawing.** D toggles the pencil; ⌘-click draws once. A new note takes the length of the last note you touched. Double-click deletes.
- **Moving.** ↑ ↓ a semitone, ⇧↑ ⇧↓ an octave, ← → one grid step, or 1 ms with ⌥.
- **Quantize sheet** (⇧Q): grid 1/4 to 1/64 with triplets, strength 0–100% (default 100), starts only or starts and ends (default starts only), swing 0–75%. It previews live. Undo reads "Undo quantize 24 notes".
- **Controller lanes.** Pitch bend, mod wheel, sustain and any plugin parameter, added with **+ Lane**.

### 5.6 Recording, takes and MIDI devices

- **Arming.** Click a track's arm dot, or press R with its header focused. Armed tracks monitor through their own effects. Count-in is 0, 1 or 2 bars (default 0).
- **Takes, with no comping** (**Decided**). Recording over a loop range makes one take per pass, stacked in a disclosure under the track, for audio and for MIDI. Click a take to choose it: one take plays at a time, and the track never joins pieces of two. Takes stay until you empty the trash.
- **Over or with.** On a stem lane, two modes: "record over vocals" replaces the stem inside the punched range, "overdub" plays with it. Both are new clips on top; the stem itself is never written.
- **MIDI.** CoreMIDI devices appear when plugged in: "KeyStep connected · channel 1 → selected track". Armed, the track records what you play as a MIDI take, and a note lands where it was played, not at the block's edge. Pitch bend, mod wheel and sustain are recorded onto their controller lanes (5.5).
- **Alignment.** Recorded audio is placed by CoreAudio's reported round-trip latency plus an offset you measure under Settings → Audio & MIDI → **Measure latency**. v5's commit b74d899 calls take alignment "untested on hardware", so the test is written first: a looped-back click recorded 100 times must land within ±1 sample.

### 5.7 Plugins

**Decided:** plugins run in a separate JUCE engine process, which owns their windows; a crash takes down only the engine.

- **Checking.** On the Console's first open, a short-lived scanner process, `wwav-scan`, checks your plugin folders one plugin at a time, with a 30 s limit each: "Checking plugins · 37 of 212". Anything that crashes or hangs is kept out and named once: "Kept out: 2 plugins crashed while being checked. Settings → Audio & MIDI → Plugins lists them, with the reason." Formats: VST3 and AU on the Mac, VST3 on Windows.
- **Using one.** Drag a plugin onto a track, or double-click it in the browser. The Chain tab (⌥2) shows the track's devices as metal cards. Built-ins draw their one amount on a DMG screen. A third-party card shows the plugin's first eight automatable parameters (or ones you pick) and **Open window**.
- **Windows.** Plugin windows float above the Console and reopen where you left them. Keys the plugin doesn't use pass back to the app, so Space still plays.

**When the engine crashes.** The session lives in the app; the engine only renders. So the app restarts the engine, reloads the session, and drops a sheet:

> "The audio engine stopped. 'Tape Echo' on the track 'Keys' was running when it did. Restarting…"
> then: "Back. 'Tape Echo' is off until you turn it on. Changes made inside its own window in the last 41 s may be lost."

The buttons are **Keep it off** and **Try it again**. The engine reports plugin state on every stop, every 60 s and before every save, which is where "41 s" comes from. Heat lives in the app, so a focus countdown never stops. **Open:** one process per plugin. *Recommendation:* not in v1; log a month of engine crashes per plugin and decide on that.

**Delay compensation** is automatic from each plugin's reported latency, shown as a DMG tag in the header: "+2,048 smp". While a track is armed, **Low-latency monitoring** bypasses plugins over 256 samples on that track's monitor path only ("bypassed while monitoring"); playback is untouched.

**A missing plugin plays nothing, and is named.** A session opened without "Grand Piano" shows "Missing: 'Grand Piano' (AU)" on that track, and the track is silent until the plugin is installed. An older installed version is named too (6.5).

### 5.8 The mixer: strips

**Strips** (⌥3). Each strip is 76 pt wide: input and output, four visible inserts, sends A (Reverb) and B (Delay), a pan knob, a −∞ to +6 dB fader beside a segmented meter, M, S and arm, and a name plate in the track's colour. Stem lanes and role groups wear their stem colour on the plate and meter cap. The master strip adds the master chain. Undo reads "Undo vocals level". On a focused strip, M and S mute and solo, and ↑ ↓ move the level 5% (see 2.7).

**The four stem buses stay in the mixer.** They stand between the tracks and the master, 96 pt wide against a track strip's 76, capped and metered in their stem colours, with a member count: "vocals · 3 tracks". Clicking a bus lights its tracks. The master strip reads "Σ 4 stems → master chain". A bus with no tracks reads "silent" and is written as silence. The fold check and the clipping sentences are in 6.6.

### 5.9 Built-in effects

**Decided:** the Console has a few built-in effects, and third-party VST and AU effects are how you get more. The built-ins are PRANA's, as PRANA has them, run on `prana/core`'s DSP wrapped as engine processors. Each has one amount.

| Device | Amount | What it is | Where it sits |
|---|---|---|---|
| Reverb | one | a fixed medium room (Freeverb) | the reverb return, fed by sends |
| Delay | one | a dotted eighth at the session tempo, ping-pong and damped, with fixed feedback | the delay return, fed by sends |
| Distortion | one | a soft clip; its tone darkens from 12 kHz to 5 kHz as the amount rises | an insert |
| Tremolo | one | depth, with one swell per eighth note, locked to the grid | an insert |
| Filter | low-pass and high-pass, one amount each | the low-pass sweeps 20 kHz → 200 Hz, the high-pass 20 Hz → 2 kHz, Q 1.0 | an insert on a track or on the master chain |
| Limiter | none | PRANA's master limiter, fixed | master only |

Amounts glide over about 30 ms, so a move never zippers. There is no built-in EQ, no built-in compressor and no extra knobs on these six. If a stem needs an EQ or a compressor, put a plugin on it. The Chain tab shows each built-in as a card with its amount on a DMG screen.

Only a Filter on the master fits `wrmx`'s `lpf` and `hpf`. A Filter on any track, or Distortion or Tremolo on any track other than a lone stem lane, makes the remix Baked (5.13, 6.8).

### 5.10 Split anything into four

**Decided:** splitting runs locally. Select any audio clip and choose Clip → **Split into stems** (⌃⌘S). Demucs (htdemucs, the 4-stem model) runs in a background worker, never in the audio engine, and reports the model's real segments: "Splitting 'break.wav' · segment 4 of 12". Intel Macs and Windows machines without a supported GPU run on the CPU, more slowly, and the sheet says so before you start. Only the 4-stem model ships, because a `.wwav` holds four stems.

The clip becomes a stem group in place, roles set, with the original kept muted underneath as a collapsed "source" lane. Undo reads "Undo split into stems", and the export records `splitter: "demucs"`.

**Open:** what a local split costs. v3 meters "splits" because the server pays Replicate; a local split costs the company nothing. *Recommendation:* local splits are free and unmetered, and the paid server split stays for phones.

### 5.11 Video

**Decided: basic.** Video is clips on a track and the cuts between them, and nothing is done to the picture itself.

**The frame** comes from the first video clip: 1080p or 4K at 24, 25, 30 or 60 fps. FFmpeg's libraries decode, using VideoToolbox's hardware decoder where the codec allows (**Decided**; see 8.5). The viewer shows the frame that the engine's clock says is on glass (8.2). Where video tracks overlap, the top track's clip is the picture. Footage over 1080p gets half-resolution proxies, built in the background and marked "proxy" in the viewer.

| Act | Mouse | Keys |
|---|---|---|
| Move a clip | drag | ← → with a clip grabbed (5.15) |
| Cut at playhead | — | ⇧Return (the timeline's one secondary act) |
| Trim, leaving a gap | drag an edge | ⌥[ · ⌥] trims start or end to the playhead |
| Ripple trim | ⌥-drag an edge | later clips on the track follow |
| Roll | ⌃-drag a shared edge | moves a cut between two clips |
| Slip | ⌥⌘-drag a clip | changes which part of the source shows |
| Ripple delete | — | ⌥⌫ closes the gap |
| Shuttle · in/out | — | J K L (L twice for 2×) · I O |

A video clip's sound lands on a linked audio track with the role "other"; Clip → **Unlink** separates them. The sound is cut with the picture while linked.

**What is not there.** No grade, no RGB curves, no varispeed, no titles, no captions, no transitions, and no generator. No automation and no effects on a video track. A cut is a cut. The reasons are in 5.18.

### 5.12 One timeline model

A session is a Sequence: an edit list, never media. **Exists today:** MI-WWAV-OS, where "No pixels or samples are touched until export." Its `kind` was `mix | edit`; the Console's one kind holds both, which is the Premiere and Ableton join.

```
Sequence {
  id: ULID, kind: "session", title, key: "A minor",
  tempo_map: [{at_beats, bpm}], frame: {w, h, fps} | null, sample_rate: 48000,
  tracks: [{
    id, kind: audio|instrument|stem|video|bus,
    role: vocals|drums|other|bass|null, gain, pan, mute, solo,
    devices: [{format: builtin|vst3|au, uid, state_ref}],   // plugin state is a file in sessions/
    automation: [{param, points: [{at_beats, value}]}],     // none on a video track
    events: [{clip_id, clip_in_ms, clip_out_ms, at_ms, at_beats?,
              params: {gain, pan, loop}}]
  }]
}
```

- **Cuts are cheap.** "A cut rewrites two numbers on an edit list, which is why it is instant and why undo can take it back for free" (`timeline.dart`).
- **Undo is journalled.** Every change is a transaction that snapshots its rows; deleting a clip removes rows, never media.
- **Exports know their session.** An export is a new clip with `from_sequence` set, and opening it offers **Open the session it came from**: "the edit stays editable forever."
- **What you hear is what you render.** Export runs the engine's own graph faster than real time. A plugin that asks to render in real time gets it, and the export sheet names it.
- **Old projects come in** (**Proposed**). A v4 8-track fork imports as two groups of four lanes, A and B, with its clips and levels kept. Pitch and rate have no place in the Console, so the sheet lists them and leaves them out: "Not brought in: pitch on 3 clips."

### 5.13 Export

Export is not a button. The last row of the arrangement is OUTPUT, as on MI-WWAV-OS: select it and press Return, or press ⌘R, and the export sheet drops.

| Output | Contains | Written as |
|---|---|---|
| `.wwav` | master and four stems by role, metadata, lineage | 44.1 kHz, 16-bit, PRANA's layout (see 6) |
| `.swav` | the picture with the mix as soundtrack | H.264 or HEVC through VideoToolbox, then `wmet` and `wlin` appended |
| Plain | WAV 24-bit at session rate | for tools that know nothing of WWAV |

**A remix from a session** is written one of two ways, and the sheet's one secondary action switches between them (6.8):

- **As settings**, when the four stem lanes are the parent's stems untouched (plus takes) and every change is one PRANA's `wrmx` can hold: a level, a mute, or a reverb, delay, distortion or tremolo amount on a stem, or a Filter on the master. `wstm` holds the stems and `wrmx` the settings.
- **Baked**, for anything else (new tracks, plugins, edits, automation). `wstm` holds the four rendered stem buses, with no `wrmx`: "This remix has 2 new tracks and a plugin, so its stems are rendered. It sounds the same everywhere."

The sheet says out loud everything that changes on the way out:

- "vocals ← Lead vox, Double, Ad-libs (3 tracks)", one line per role
- "This session is 48 kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered."
- "Stems are pre-master. Your master chain (1 plugin) shapes the master only."
- "3:58 → about 210 MB." A `.wwav` holds about 81 minutes, and a longer session is refused with that sentence.

**Lineage is set for you.** `type` is `original` for a new session, `remix` for one that came from someone's planet, `split` for a split song; `parent_id`, `root_id` and `generation` follow PRANA's rules. When one export makes both files, the film's `parent_id` is the song's `song_id`: "A film made from a song is its child" (`formats/swav/SPEC.md`). Progress shows real stages only: "Rendering master · 1:12 of 3:58", "Rendering stems", "Encoding · frame 2,410 of 5,712", "Packing".

**Key and tempo.** Export writes the session's key and tempo into `wmet` when the session has them (typed by you, or an estimate you confirmed, 5.4).

**Open:** sessions run at 48 kHz for video while `.wwav` 0.1 is fixed at 44.1 kHz, 16-bit. *Recommendation:* keep 0.1 fixed and convert on export, saying so, and weigh a 48 kHz `.wwav` in 6, since PRANA reads 44.1 kHz only (6.7).

**Open:** MP3 320 and a plain MP4. *Recommendation:* not in v1; a `.swav` already plays as an MP4.

### 5.14 Playback: every file comes apart

The sketch says the Console is "also good for downloaded playback". Double-click a `.wwav` in Finder (the app registers `.wwav` and `.swav`) and it opens in the listening player, the planet (see 2). Any file can then be torn apart: ⌘E opens it in the Console, and so does **Open in Console** on any work in Space (see 4.6). The file is never written to: your changes live in a session that points at it.

| File | Opens as |
|---|---|
| A song from Space | four stem lanes with the parent's remix snapshot applied, under the header "fork of glass hours · gen 3" |
| A film (`.swav`) | its picture on a video track, with its sound on the linked audio track |
| A remix (`wrmx`) | four lanes with its levels, mutes, effects and filters applied. Pitch, speed and time are listed and not applied: "Not applied: speed 1.25×. The Console doesn't stretch audio." |
| A plain WAV | one lane, "master only", with **Split into stems** beside it |
| A newer major version | the master: "Made by a newer WWAV. Playing the master." |

A work you have never opened in the Console needs a connection first, and the button says so (see 2.8).

The **Listen** layout (⌥0) folds the browser, inspector and lanes away, leaving one large planet over a waveform built only from the audible stems ("mute the drums and the drum shape vanishes"; see 4.6) and four thin lanes. It is the same session, laid out for listening.

### 5.15 The grammar on a keyboard

MI-WWAV-OS's three verbs map to keys, and everything else follows them (see 2.7).

| Verb | Key | In the timeline |
|---|---|---|
| Move | arrows | move the highlight between clips and tracks |
| SELECT | Return | select the clip at the playhead |
| Grab | hold Return 210 ms | the clip sticks to the highlight; arrows drag; release drops |
| SHIFT + SELECT | ⇧Return | cut at playhead |
| REVERSE | Esc | deselect, then out to the track view; never deletes |
| Undo | ⌘Z | labelled with what it undoes |
| Panels | ⌥1 Editor · ⌥2 Chain · ⌥3 Mixer | navigate only, never change content |

The 210 ms hold is the OS's "one magic number in the input path".

### 5.16 Push to Space

**Exists today:** WWAV Push gives a song a permanent link on its first export, and "Every subsequent export replaces the audio under the same link". Its v0 left out version history: old stems stayed in storage, unseen.

In the Console, publishing is a drop. Drag the OUTPUT row, or a finished export, onto the "Space" segment or onto one of your solar systems.

- **The first push** makes a planet and its permanent link.
- **Each later push** adds a version under the same link. The session inspector lists them, "v3 · Oct 6 · new bridge", "v2 · Oct 4", "v1 · Oct 2". Listeners hear the newest, and every version stays playable on the work's page.
- **Push every export** carries over from WWAV Push as a switch on the session.

The status bar walks real stages, "Uploading Low Tide · part 3 of 27", then "Up. Low Tide is in your galaxy." (see 2.8). There is no Demucs wait: the stems travel inside the `.wwav`, where WWAV Push waited 2–4 minutes for the server to split a bounce.

**Open:** versions and identity. A version keeps the work's `song_id`; a remix gets a new id with a parent; `.wwav` 0.1 has no version field. *Recommendation:* keep versions on the server and add an optional `version` to `wmet` in 0.2 (6.8).

### 5.17 Walkthrough: from a hum to a planet

1. **Record an idea.** ⌘N makes a session, ⌘T an audio track, which starts as vocals because its input is a microphone. R, then Space: hum for 52 s, no click. The screen reads "BAR — · FREE · REC". The inspector shows "≈ 86 BPM (estimate)". You type 86 and the bar lines now sit where your hum does, with not one sample moved. Record a second pass over the same range: two takes stack under the track, and a click on the second makes it the one that plays.
2. **Arrange with an instrument.** ⌘⇧T adds an instrument track; drag an AU piano onto it. Play chords on the USB keyboard and they land where you played them. Select only the low notes, ⇧Q: 1/8, strength 50%, starts only. Undo reads "Undo quantize 11 notes".
3. **Split a sample.** Drag `break.wav` from the Library onto a lane and press ⌃⌘S. About 20 s later four lanes sit where the clip was. Mute vocals, other and bass, and keep the drums. Send the piano to the delay return: one amount.
4. **Cut a video to it.** Drop a 4K phone clip on a video track; proxies build while you work. Step to bar lines with ← → and press ⇧Return at each. Ripple-delete the dead takes with ⌥⌫. The clip's sound sits on its linked audio track.
5. **Export both.** Select OUTPUT, press Return, tick `.wwav` and `.swav`. The sheet reads "vocals ← Hum · drums ← break (drums) · other ← Piano, Shore · bass ← (silent)" and "3:58 → about 210 MB". Rendering takes about a minute.
6. **Push to your galaxy.** Drag the export onto "Space" and drop it on the system "World Ending". "Up. Low Tide is in your galaxy." The song is a planet; the film is a ringed world that is its child. A week later you push a new bridge, and the same link holds v2.

### 5.18 Left out, and why

| Left out | Why |
|---|---|
| A clip-launch grid and a second view | **Decided:** no clip-launch view in v1 (5.2). |
| Performing on the Planet; the Planet mixer view | Cut on 7 Oct 2026 to narrow v1. The mixer is strips only. |
| v3's remix deck: four-knob effects, the EQ graph, compressor ranges, beat loops | Cut on 7 Oct 2026 to narrow v1. Third-party plugins are how you get more. |
| A built-in EQ and compressor; four knobs on every effect | Decided on 7 Oct 2026: the built-ins are PRANA's six, one amount each. |
| A built-in Sampler, pads and custom instruments | Decided on 7 Oct 2026: instruments are third-party VST3 and AU only. |
| Clip time and pitch modes (As played, Follow tempo, Tape), audio transpose, reverse | Cut on 7 Oct 2026 to narrow v1. Audio plays at the speed it was recorded. |
| "Follow what I played", tempo-following and audio quantize | Audio is never stretched to a grid (5.4). Quantize is for MIDI only. |
| Second songs and **Match to session** | Cut on 7 Oct 2026 to narrow v1. |
| Device parity and "Matches PRANA" | Cut on 7 Oct 2026 to narrow v1. |
| Comping and 10 ms crossfade joins | Decided: one take plays at a time (5.6). |
| Camera takes | Cut on 7 Oct 2026 to narrow v1. |
| **Capture what I just played** | Cut on 7 Oct 2026 to narrow v1. Record it like any take. |
| MIDI learn | Cut on 7 Oct 2026 to narrow v1. |
| PRANA as a USB controller | Cut on 7 Oct 2026 to narrow v1. |
| Freeze, flatten and **Freeze plugins they may not have** | Cut on 7 Oct 2026 to narrow v1. A missing plugin plays nothing and is named (5.7). |
| The grade: saturation, contrast, brightness, RGB curves, copy and paste grade | Decided: video is basic, with no effects (5.11). |
| Varispeed and any speed change on video | Decided: video is basic. |
| Titles, captions from lyrics and the titles track | Cut on 7 Oct 2026 to narrow v1. |
| The video synth and the generator track | Cut on 7 Oct 2026 to narrow v1. |
| Transitions, and opacity or blend on a video track | Decided: no automation or effects on video. |
| A 1:1 or 9:16 frame | Their sources were the video synth and an unused canvas. The frame is the first clip's. |
| Make a disc, and bringing a PRANA's remixes home | "PRANA disc not ready yet, so definitely not writing software yet." |
| The PRANA view | Cut on 7 Oct 2026 to narrow v1; it waits for the disc. |
| The gamepad column of the grammar | Cut on 7 Oct 2026 to narrow v1. The three verbs stay on keys. |
| **Ask for feedback** | Cut on 7 Oct 2026 to narrow v1. Claude works only through the MCP server (see 2.11). |
| The Purchases source in the browser | Commerce is later, inside Space (see 4.13). |
| AAX | it serves Pro Tools only; VST3 and AU cover the rest |
| Surround and Atmos | a `.wwav` is stereo |
| Video stems | "Si_WWAV will define film stems"; a `.swav` holds one picture |
| 6-stem splitting | a `.wwav` holds four stems |
| Auto-mastering | Claude estimates and drafts, never decides, and the app calls no model |
| Play counts in the Console | the room is for making, and gate 1.3 holds everywhere |
| A paywall on export | "Never gate leaving with your own work" (see 9) |

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

**Stem order is fixed:** vocals, drums, other, bass. It is PRANA's fader order, Wi's fader order, and the order of the Now strip's lights and every stem lane and meter in the app (colours in 7).

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

### 6.4 What plays where

The column marked **(verify)** is untested. ffmpeg, the pack tools and wi-wwav.com are tested in `wi/GATES.md` (2.3) and the format specs; PRANA's column is built in its simulator; the app's column and the session row are **Proposed**.

| File | QuickTime, a DAW **(verify)** | ffmpeg, ffprobe | pack tools | PRANA | wi-wwav.com | Wi_WWAV app |
|---|---|---|---|---|---|---|
| `.wwav`, original or split | the master | the master, no errors | "4 stems, and the master" | stems | stems | stems |
| `.wwav`, remix | its master | its master | stems and settings | stems with settings | stems; settings not applied | stems with levels, mutes and effects; pitch, speed and time not applied (5.14) |
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
  renders/          exports made from this session
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
  "video": { "edits": [ … ] },
  "lineage": {
    "work": {"song_id": "9f2c…", "film_id": "41ab…", "version": 3},
    "from": {"song_id": "e07d…", "version": 1, "title": "glass hours", "sha256": "…"},
    "exports": [{"file": "renders/01JC….wwav", "version": 3, "sha256": "…",
                 "at": "2026-10-06T21:14:03Z"}]
  }
}
```

The `video` block holds cuts only: which part of which file sits where. A video track has no grade, titles or generator to record (5.11).

- **Clips are edit lists.** An event names a media file by ULID, with in, out and position: "No pixels or samples are touched until export" (**Exists today:** MI-WWAV-OS). A dropped `.wwav` stays one file, and its four stem lanes read it in place.
- **Media costs nothing twice.** In the library, `media/` holds APFS clones of the library's files, so a 210 MB song used in three sessions takes 210 MB. Media is never written after import, so Windows uses hard links. On exFAT the files are copied, and the sheet says so: "Copying 1.4 GB of media into the session."
- **Plugin state** records the plugin and the version that saved it:

```
{"format": "vst3", "uid": "<32 hex class id>", "name": "Tape Echo", "version": "2.1.4",
 "state": {"inline": "<base64>", "bytes": 18432, "sha256": "…"}}
{"format": "au", "uid": "aumf:TpEc:Vndr", "name": "Grand Piano", "version": "1.3.0",
 "state": {"file": "plugin-state/01JC….bin", "bytes": 41943040, "sha256": "…"}}
```

States up to 256 KB sit inline as base64; larger ones, such as a third-party sampler's 40 MB of mappings, are files, so an autosave never rewrites them unchanged. A built-in effect (5.9) has one amount and no state file. An older installed version is named: "Saved with 'Tape Echo' 2.1.4. You have 2.0.0, so its settings may not load." A missing plugin plays nothing and is named (see 5.7).

- **The lineage block** says what the session will become. A session reserves its `song_id` and `film_id` when it is made, so every export of it is a version of one work (6.8). `from` records a forked parent's version and sha256, so the exact parent can be found after its maker pushes a new one.

**Autosave and the undo journal.**

- Every change is a transaction recording the affected parts of the session before and after, as MI-WWAV-OS's `Txn` does, appended to `journal/undo.ndjson` at once. ⌘Z says what it will undo: "Undo move clip". The journal survives quitting, so the label is the same after a relaunch.
- `session.json` is rewritten 2 s after the last change and at every transport stop, to a temporary file that is then renamed, so a crash leaves the old file or the new one, never half of one. On open, newer journal entries are replayed: "Recovered 14 changes from 21:12."
- Deleting a clip removes it from the edit list, never from `media/`. **Clean up media…** lists unused files with their sizes and moves them to the trash on a press.
- `journal/` keeps a copy of `session.json` every 10 minutes of work for 30 days, under **File → Revert to**. ⌘S writes at once and says "Saved", though nothing ever needs it.

**On Windows** a `.wwavsession` is a plain folder. File → Open, dragging it onto the app, and a right-click **Open in Wi_WWAV** all open it. Names inside are 26-character ULIDs, so paths stay under Windows' 260-character default. Titles become folder names with `\ / : * ? " < > |` replaced by spaces, as PRANA's disc writer does.

**Sending.** **Send session…** writes one `Low Tide.wwavsession.zip`, stored rather than compressed (audio doesn't compress), without `cache/`. It offers no freeze: a plugin the other person doesn't have plays nothing on their machine and is named (5.7). Without the app, a session is still a folder of every recording you made and one JSON file any text editor reads.

### 6.6 Four stem buses: how N tracks become four stems (Proposed)

A `.wwav` holds four stems; a session holds any number of tracks. The rule that joins them lives in the mixer, not the export, so what you hear while working is what the file holds.

- **Every track routes to one of four stem buses**, vocals, drums, other or bass, by its role (see 5.3). A track routed into a group takes the group's role, and its role dot says so. Nothing is unassigned: a microphone track starts as vocals, a drum instrument as drums, lanes from a `.wwav` keep their stems, and everything else starts as other, including a video clip's sound.
- **The buses are the stems.** Each stem bus has its own copy of the built-in reverb and delay returns (5.9), fed only by its own tracks' sends, so each stem carries its own tail: "Returns: 2 × 4 stems".
- **The master is their sum**, then the master chain. Stems are written before the master chain and the master after it (see 5.13).
- **In the mixer** (see 5.8) the four stem buses stand between the tracks and the master, capped and metered in their stem colours. A bus with no tracks reads "silent" and is written as silence.
- **The fold check.** On export the app subtracts the summed stems from the master with the master chain bypassed. Under −80 dBFS the sheet says "Stems sum to the master." Above it, it names the cause, usually a non-linear plugin on a return.
- **Clipping is said out loud.** A stem bus over 0 dBFS can't be written at 16 bits: "drums peaks at +1.8 dBFS. **Lower all four stems by 2 dB**." The master is unchanged, and the sheet says a player summing the stems will play 2 dB quieter.

### 6.7 Sample rate, and conversions said out loud (Proposed)

- **New sessions run at 48 kHz,** the rate of video. A session opened from a `.wwav` runs at 44.1 kHz, so a remix makes the round trip without conversion. Clips at another rate are resampled as they play, and their header says "48 kHz → 44.1 kHz".
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

**Versions.** WWAV Push overwrote the audio under a permanent link and hid the old stems (**Exists today:** `vst_plugin/README.md`). Here each version is kept unchanged, with its own sha256, and the link plays the newest (see 5.16). Version 0.1 has no field for this, so **`wmet` 0.2** adds one optional key after `created`, `"version": 3` (absent means 1), and **`wlin` 0.2** adds `"parent_version"`. It is a minor bump: every 0.x reader still lists the file and plays its stems, because readers accept any JSON object and check only the major version.

**A remix from the Console** is any export from a session with a `from`. It is written with `type` `remix`, a new `song_id`, `parent_id` = the source's `song_id`, `root_id` = the source's root, generation + 1, and `creator` = your account name at export. `device_id` stays "": the desktop app isn't a device, and a per-install id would be a tracking number with no use. The export sheet's one secondary action switches between two ways of writing it (see 5.13):

- **As settings**, when the four stem lanes are the parent's stems untouched (plus takes) and every change is one that PRANA's `wrmx` can hold: a level, a mute, or a reverb, delay, distortion or tremolo amount on a stem, or a Filter on the master. `wrmx` has a level and a mute for each stem, four effect amounts for each stem (reverb, delay, distortion, tremolo), and the two filter amounts on the master. `wstm` holds the stems and `wrmx` the settings, as PRANA writes them, so the remix reopens with every control where you left it, on the device too.
- **Baked**, for anything else (new tracks, plugins, edits, automation). `wstm` holds the four rendered stem buses, with no `wrmx`: "This remix has 2 new tracks and a plugin, so its stems are rendered. It sounds the same everywhere."

**The master keeps the stems' length.** Readers use the stems only when `data`, `wstm` and `wmet` agree on the frame count, and speed and time change the length. The Console doesn't stretch audio or shift its pitch (5.4), so every `wrmx` it writes has `pitch` 0, `speed` 1.000 and `time` 1.000, and the master carries levels, mutes, effects and filters at the stems' length. PRANA's master leaves speed out for the same reason. A remix made on PRANA with speed, time or pitch set opens in the Console without them: "Not applied: speed 1.25×. The Console doesn't stretch audio." (see 5.14).

**Ids on the server.** The first upload of a `song_id` claims it for that account. The same id from the same account with a higher `version` is a new version; the same sha256 again does nothing ("Already up"); the same id from another account is refused: "This file's id belongs to another work, 'Low Tide' by LMY." Platform tracks keep their FNV-1a ids, so a WWAV Push track's versions and remixes still meet. **Exists today:** Wi's server keys each post by `workId`, `parentId` and `rootId` read from the file (`server/routes/wi.js`).

### 6.9 Thin remixes: PRANA's "Step 5" (Open)

A remix carries full stems at 53 MB a minute, even when it only moved four faders. PRANA's draft had a Step 5, never built: a remix placed as a song loads its parent's stems. Space's forks already work like this on the server: "A fork owns no audio" (**Exists today:** `server/routes/forks.js`).

Two facts narrow the choice:

- A settings remix with no takes has a `wstm` whose audio is byte for byte its parent's. The library and R2 can store that range once, keyed by its sha256, and assemble the whole file when it leaves, checked against the whole file's sha256.
- A thin file (master and settings, no `wstm`, about 10.6 MB a minute) plays its stems only where its parent already is. Leaving with it means leaving with something that needs the company, which gate 2.3 fails.

**Open:** thin remixes as files. Recommendation: thick files wherever a file leaves (exports, downloads, Export everything) and thin storage inside the library and on the server. The format doesn't change, and each set of stems is stored once.

### 6.10 .swav 0.2 (Proposed)

- **Remixes.** `type` gains `remix`, and pack and the app write `parent_id`, `root_id` and generation for films as `.wwav` does. A film cut in the Console to a song is born that song's child.
- **No grade.** Video is basic (5.11), so 0.2 has no box for a grade. A film made from a film is a new film file, with the parent's identity in `wlin`.
- **Open: film stems for Si_WWAV.** The app encodes its own exports, so it can write a layout it must never impose on a received file: one composite video track, enabled, which every player shows, plus up to four layer tracks marked disabled in their `tkhd`, which players skip **(verify)**. The composite is the master; the layers are the stems. The layers would be the session's video tracks, and the Console has no alpha (its composite is plain, 5.11), so they carry none. Which four tracks a longer stack gives up is part of the question. Recommendation: build it after one film has been cut in the Console and someone has asked to take a film apart.

### 6.11 Words and garments (Open)

| Family | Placeholder | Shape | Opens without WWAV in | A reply is |
|---|---|---|---|---|
| Ri_WWAV, writing | `.rwav` | Markdown with `wmet` and `wlin` keys in its front matter (`ri: "0.1"`, `work_id`, `parent_id`, `root_id`, `generation`, `creator`), as 4.7 recommends | any text editor | a link, which waits for consent (4.9) |
| Gi_WWAV, garments | `.gwav`, the "pattern file" | SVG pattern pieces, with the same JSON in SVG's own metadata element; DXF-AAMA for cutting tables as an export | any browser | a link, which waits for consent (4.9) |

Both join the shared id space, so a book can descend from an album. Until they exist, writing and fashion travel as Markdown and photos with identity in the database, the one place where "a post is a file" doesn't hold yet. Recommendation: keep `.md` for writing until a page must carry its images in one file, and name Gi's file when a Gi_cro_WWAV prototype cuts its first piece ("One pattern file works on all three tiers").

### 6.12 Tests written before the code

Rule 2 says to write down what a fail looks like before testing. Each of these fails as stated:

1. **Byte parity.** An original exported by the app, unpacked by `wwav_pack.py` and packed again, has a different sha256 (pack writes originals only). The same for `.swav` and `swav_pack.py`.
2. **One verdict.** Any of the four readers (Python, PRANA's C++, Wi's JavaScript, the app's Rust) gives a different sentence for any file in one shared test corpus.
3. **Plays without us.** ffprobe or ffmpeg reports an error on any export.
4. **The session.** Save, quit and reopen changes `session.json` by a byte or changes ⌘Z's label; killing the app mid-edit loses a journalled change.
5. **Settings hold.** A remix written As settings, read by `wwav_pack.py`, has a `wrmx` that differs from the session's stem levels, mutes and built-in amounts, or a `wstm` that differs from its parent's stems.

### 6.13 What the files leave out, and why

| Left out | Why |
|---|---|
| Compression inside `wstm` | PRANA streams raw frames, one read per 1024, and every WAV tool reads PCM |
| More than four stems | PRANA has four faders, and the mixer has four stem buses |
| A 24-bit or 48 kHz `.wwav` | no reader today would list it (6.7) |
| DRM and watermarks | they change the bytes |
| The `wgrd` box: a film's grade and new soundtrack as a recipe | Decided: video is basic, with no grade (5.11). Cut on 7 Oct 2026 to narrow v1. |
| Grade, titles and generators in `session.json` | Cut on 7 Oct 2026 to narrow v1. A video track holds cuts only (5.11). |
| Freezes in `renders/`, and the offer to freeze plugins in **Send session…** | Cut on 7 Oct 2026 to narrow v1. A missing plugin plays nothing and is named (5.7). |
| Pitch, speed and time in a settings remix | The Console doesn't stretch audio or shift its pitch; an opened `wrmx` lists them as "Not applied" (5.14). |
| A built-in EQ or compressor in `wrmx` or the session | Decided on 7 Oct 2026: the built-ins are PRANA's six, one amount each (5.9). |
| Files in the store: exact bytes sold, no per-buyer watermark, downloads checked against a receipt, ownership checked before a download, a purchase covering every version, and the $1 stem download | Commerce is later, inside Space, and nothing of it is designed now (4.13). Cut on 7 Oct 2026 to narrow v1. |


## 7. Look, sound and feel

The app speaks one design language in two registers. The case is the same in every view: a brushed-metal title bar, one light from the upper left, the same stem colours and the same words. What sits inside the case changes with the view. Heat and the Console's chrome are the **desk**, and Space is the **night**. Everything in this chapter is **Proposed** unless it carries another label. Where a value here differs from a raw token quoted in an earlier chapter, build this chapter's value.

### 7.1 Where it comes from

| System | Where it lives | The app takes | The app leaves |
|---|---|---|---|
| Heat's Aqua skin | Heat | the case: brushed metal, gel buttons, Aqua stripes, source list, sheets, the olive LCD, full dark tokens | the slate desk behind the window |
| MI-WWAV-OS style | `MI-WWAV-OS/app/lib/style/theme.dart` | "Aqua is the case; the Game Boy is inside it"; DMG screens; deck metal with pinstripes; the 140 ms beat | Helvetica Neue (7.4) |
| v3 iOS | `ios_v3/WWAV/Theme/`, `Visuals/` | the night palette, `WWAVLight.sun`, key colour, the starfield port, titles that end in a period | four pastel palettes, Nunito, the breathing tab pill |
| v4 web | `wwav/src/styles/tokens.css`, `planet.css` | seven ink opacities, one accent with three uses, Cormorant over Inter, 120/240/420 ms | the limousine-black ground |
| PRANA | `prana/SPEC.md` | the stem colours, state by shape | blinking LEDs (7.6), Jost (7.4) |
| Crater | `portfolio/src/crater.css`, the Sculptor of Time handoff | the 760 ms light-wash | the hourly supernova |
| Wi, the wall | `wi/src/wi.css` | paper and ink for the web face; a ring that carries a muted light's contrast | its adjusted stem hues (7.12) |
| Legacy icons | `assets/` | all five pictures | their old meanings (7.5) |

v1's scanlines and RGB fringe stay out; the founder called the old web look "so ugly and has such bad contrast that it makes things hard to see and use".

### 7.2 Two registers

| | Desk | Night |
|---|---|---|
| Views | Heat; the Console's chrome; Settings; every sheet | Space; the expanded player |
| Ground | brushed metal, white wells, `#edf3fe` stripes | `#070A18` under a key-tinted starfield |
| Ink | `#1b1b1b` | `#F4EFE6` |
| One accent | Aqua highlight `#3875D7` | royal blue `#2946FF` |
| Type | Lucida Grande, Menlo | Cormorant Garamond italic, Inter |
| Light appearance | Heat's light tokens | night |
| Dark appearance | Heat's dark tokens | night |

The web face at www.wi-wwav.com keeps Wi's own look (**Exists today:** `wi/src/wi.css`): paper `#f7f6f2`, ink `#161616`, the system's type at 17 px, light and dark from the phone. A share page is read by someone who never installed anything, so it borrows none of the app's costume.

#### The desk

**Exists today (Heat):** Lucida Grande 13 px at 1.35; brushed metal (1 px light and dark lines over a radial highlight on `#e4e4e4→#c9c9c9`); gel pills with a hard break at 45/55%; white and `#edf3fe` rows; a `#e7ecf2` source list; sheets that drop from the title bar over a 25% black backdrop; Stickies yellow; glass beads; a 3 px `rgba(56,117,215,.6)` focus ring; and a full set of dark tokens (7.11).

The desk has two metals with two jobs. **Case metal** is Heat's: title bar, toolbars, sheets, inspector. **Deck metal** is MI-WWAV-OS's paler `#FBFCFD`/`#E6EAEF`/`#C9D0D8`, with a 1 px pinstripe every 4 px that "reads as texture, never as stripes". It covers the Console's mixer strips and device cards (see 5.8 and 5.9).

**The Game Boy inside.** Every Console readout sits behind glass as a DMG screen: `#0B1F0B` and `#0F380F` grounds, `#306230` for unlit segments, `#8BAC0F` for secondary text and `#C6E24A` glow for primary, in uppercase Menlo inside the bezel's scanlines and vignette (**Exists today:** MI-WWAV-OS `ScreenBezel`). Meters stay "DMG green until it is about to be too loud" (see 5.2). Heat keeps the iTunes olive LCD, so the screen's colour names the view: olive is planning, green is making. DMG screens look the same in light and dark, because a lit screen doesn't change with the room's lamp.

Five fixes bring the desk's pairs up to gate 2.4's bars (ratios in 7.11):

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

### 7.3 What every register shares

- **One light.** `WWAVLight.sun` ≈ (0.30, 0.25), the upper left. It places the highlight on every orb and slider thumb and the gloss on every gel. In Space's 3D, every body is lit from it too (see 4.5).
- **A work's colour is its key colour:** hue = ((pc · 7) mod 12) · 30 with A = 0, major hsl(h, 72%, 58%), minor hsl(h, 58%, 42%), and night indigo, hue 232, for "still condensing" (**Exists today:** `keyColor.js`). A key colour is a fill, never a ground for text: in E minor and A major neither ink reaches 4.5:1.
- **Stem colours are PRANA's, "the same everywhere":** vocals `#D23C2A`, drums `#F0B90B`, other `#2E9A55`, bass `#1F4E9E`. The fill is exactly that hex in every view. No single hue clears 3:1 on every ground (bass is 2.5:1 on the night, drums 1.5:1 on deck metal), so every stem mark has a 1 pt ring in its register's ink, and the ring carries the contrast. Drums yellow is never type. State is shape:

```
audible               filled disc, ink ring
muted                 2 pt stem-colour ring, open centre, ink ring
soloed                filled, plus a 2 pt outer ring (royal blue in Space, ink elsewhere)
silent under a solo   filled at 45%, dashed ink ring
selected              filled, plus a 2 pt notch beneath (PRANA blinks; the app doesn't)
```

- **Heat's colours are fills.** Cool `#4f9be6`, warm `#efa431`, hot `#e0402c` and overdue `#8f1d16` fill tubes, pill borders and dots. The level word is ink, because hot on white is 4.25:1.
- **One accent per screen,** named in 7.2. v4's test holds: "If blue appears more than once or twice on a screen, something is wrong."
- **Spacing and corners.** A 4 px grid: 4, 8, 12, 16, 24, 32, 48, 64. Radii: chip 6, button 10 (gels are full pills), card 14, a sheet's lower corners 8.

### 7.4 Type

| Face | Where | Sizes | Why |
|---|---|---|---|
| Lucida Grande (Mac), Lucida Sans Unicode (Windows) | the desk: rows, labels, sheets, menus | 11, 13, 15, 20 pt; body 13 at 1.35 | Heat's face and Aqua's, until Yosemite (2014) moved to Helvetica Neue. Both are Lucida Sans and ship with their systems, so nothing is bundled. |
| Menlo (Mac), Consolas (Windows) | DMG screens, timecode | 11, 13, 20 pt, capitals | MI-WWAV-OS's dot-matrix face. Fixed width keeps "128.00 BPM" from jittering, PRANA's rule for "1.00x". |
| Cormorant Garamond italic, 300 and 400 | the name of a work, a person or a place, in the night | 20 pt minimum; 30 for headers; 46 for suns | v4's name face, from Crater. Line height 1.18 or more; weight 400 below 28 pt, where 300's hairlines thin. |
| Inter | running text and labels in the night | 11 (capitals, +0.08 em), 13, 15, 17 for reading | v4's one family, for anything read twice. |

Inter and Cormorant Garamond are OFL fonts bundled with the app. **Nunito retires:** its round terminals read soft against metal and crowd Cormorant in the night. What made v3's type its own, lowercase italic titles that end in a period ("lineage.", "time.", "vox."), moves to Cormorant. DM Mono (PRISMON) and Fraunces (the v3 mock) aren't used, because Menlo and Cormorant already do their jobs. **Jost retires:** it was for the shop's signs and price cards, and nothing left in the app uses it.

**Open: one sans or two.** Lucida in the desk and Inter elsewhere puts two sans faces in one window. Bundling Inter alone would be simpler and identical on Windows, but it would turn the Aqua case into a costume. Recommendation: keep Lucida for the desk, and recheck it on the Windows build, where Lucida Sans Unicode looks oldest.

### 7.5 Icons

**Exists today:** five 2000 × 2000 PNGs drawn in thick black lines (`assets/`), wired to a song card in `client/src/components/popups/SongInfoPopup.jsx`. The gates retired two of their old meanings, and the cuts retired a third, so each picture gets a job it can still honestly show:

| Picture | Was | Becomes |
|---|---|---|
| A red rose (`heart.png`) | like | **Add**, a private save that is never counted (see 4.11) |
| A two-masted schooner (`message.png`) | comments | the message door and letters: a ship carries post |
| A looped paperclip (`copy.png`) | share | **Copy link** to the web face |
| A plus (`plus.png`) | remix | **New**, the "+" on Heat's tabs and the **+ Lane** button (3.3, 5.5). Its tooltip says "New task" or "New lane", never "Add", which belongs to the rose |
| An up arrow (`up_arrow.png`) | unknown | **↑ Push** |

They are redrawn as vectors on 16, 20 and 28 pt grids, one stroke weight per size, in the register's ink; the rose keeps its red inside an ink line. Play, stop, record and loop are drawn in the same hand. SF Symbols aren't used, because their licence covers Apple platforms and the app goes to Windows.

### 7.6 Motion

Motion comes from sound. Nothing on screen moves while nothing plays, and nothing plays without a press (gate 2.4). The scale is built on MI-WWAV-OS's 140 ms beat with v4's curves: `cubic-bezier(0.22, 1, 0.36, 1)` for anything coming to rest, `cubic-bezier(0.32, 0.72, 0, 1)` for sheets. Nothing bounces.

| Token | ms | Use | From |
|---|---|---|---|
| `press` | 70 | a gel darkens to 88% | half a beat |
| `beat` | 140 | mute, solo, select, a view change; every move under Reduce Motion | MI-WWAV-OS |
| `fade` | 280 | fades, reveals, a sheet dropping | v4's 240 and Heat's 220, put on the beat |
| `morph` | 420 | the player opening | v4 |
| `wash` | 760 | the sky taking a key colour | the portfolio's light-wash |
| `dive` | 900 | entering a planet | the Works hub |

Toasts hold for 2600 ms. Every idle motion the app inherits gets a decision:

| Motion | Today | In the app |
|---|---|---|
| A planet breathing at its tempo (period 2 · 60 / BPM, scale 1.015) | v4, always | only while it plays |
| Moons floating ±3 pt over 5.4 s | v3 | gone; a moon's place is its level |
| Twinkle, breathing core, meteors, a sun's 9 s pulse | v3, v4, v5 | on the engine's clock, so they stop with the sound (Open in 4.4) |
| The tab pill breathing over 5.4 s | v3 | gone; the switcher is a gel control |
| The hourly supernova | portfolio | left out: an event on a timer is one nobody pressed |
| A planet pulsing for new work | portfolio | a still dot in "Since you last looked" |
| An LED blinking while selected | PRANA | a still notch |

Meters, playheads and waveforms move whenever audio moves, because they are information. Under Reduce Motion (Mac) or with animation effects off (Windows), every move becomes a 140 ms cross-fade, cameras lose their inertia, and breathing stops. A ceremony after a press is allowed if it ends within 900 ms, such as a planet condensing in as it lands.

### 7.7 Sound

The app is an instrument, so its own sounds must never be mistaken for the work or land in a take. **It makes no interface sounds**: no clicks, swooshes or alerts. There are two exceptions. Each starts only after a press and has its own switch.

- **The Console's click.** Off by default in each session (see 5.4). An accent at 1,568 Hz and a beat at 1,047 Hz, 12 ms sine bursts at −12 dBFS on the cue bus, which no recording or export taps.
- **Heat's chime.** One struck bell with a 1.2 s decay when a focus session ends, off by default (see 3.5). Nothing sounds to call you back from a break.

Space makes no sound but the works. Meteors are silent, because in a sky full of songs a sound effect would be one more song.

### 7.8 Haptics

On a Mac with a Force Touch trackpad, the Rust side calls `NSHapticFeedbackManager`. Haptics never fire on a timer, never carry information alone, and follow the system's trackpad setting. Windows gets none at first.

| Moment | Feel |
|---|---|
| A clip edge snaps to a grid line, the playhead or another clip | alignment |
| A dragged work enters a drop target: your galaxy, a system | alignment |
| A moon reaches either end of its arm | level change |
| Force click on a planet | opens its info card, as Quick Look does for a file |

### 7.9 Accessibility bars

The bars are gate 2.4's, set out in 2.12: body text 7:1 and secondary 4.5:1 in light and dark, in both registers; 44 × 44 pt hit areas; every gesture on a key; nothing moving while nothing plays; no state shown by colour alone. Two rules are added here, written down before testing as the gates require:

- **What counts as body text.** Running text and list rows are body text (7:1). Labels on controls, column headings and DMG secondary lines are secondary (4.5:1).
- **Desktop posture.** The gate's rule covers phones only: "body text under 16 px on a phone". That is about 0.46° of view at 33 cm. From 55 cm in front of a 13-inch MacBook Air (112 pt per inch), the same angle is about 19 pt. The rule here: text you read for more than a line (the reading room, letters, Mail) is at least 17 pt. Rows and labels, which are scanned, may be 13 pt, the Mac's own system size. Nothing is under 11 pt. Measure at 1024 × 680 and 1280 × 800, at default size and at ⌘+'s 20 pt, in light and dark, in all three views.

**Open: reading size.** 17 pt is 0.40°, under the phone rule's angle. Recommendation: test 17 and 19 pt with the founder at their own desk, and fix the number before any gate run.

Focus is the 3 px Aqua ring in the desk and a 2 pt ink ring in the night, since royal blue is spent on its three uses. Under Increase Contrast, deck metal drops its pinstripes and night text uses 100% ink only. Every stem mark speaks: "Vocals, 70 percent, audible", "Drums, muted", "Bass, soloed".

### 7.10 Words

The voice is the repo's: plain, second person, present tense, short. Real numbers replace adjectives. Every omission and every disabled control says why in one sentence. Endings are said out loud. No exclamation marks, no emoji. An ellipsis means a sheet follows ("Export everything…") or work is under way ("Syncing…").

- **Case.** Buttons and menus are sentence case, as Heat writes them. Headings in the night are lowercase italic and end in a period. Labels are tracked capitals, and so are DMG screens. Product and view names keep their case ("Heat", "Mi_WWAV"), and a work's name is set exactly as its maker typed it.
- **Claude.** The word "AI" never appears and nothing gets a sparkle; Counsel's spec already rules out "Any 'AI' branding in the UI". Claude is named wherever Claude acts, because you should know who made a change. Its work is labelled as an estimate or a draft, and it never gives a number it can't trace.
- **Undo instead of questions.** ⌘Z replaces "Are you sure?". Confirmation remains only where undo can't reach: money moving, and deleting an account (type DELETE).

| Moment | Copy | Source |
|---|---|---|
| Empty | "Nothing here right now." | Heat |
| The end | "That's everything." | Wi |
| A limit and its reason | "A solar system holds 21 worlds. Start another one." | v4 |
| A file that is less than it could be | "Plain audio comes in as master only." | 2.5 |
| A pause you didn't ask for | "The planet is paused while this plays." | v4 |
| Undo | menu "Undo move clip"; toast "Undone — move clip"; "Nothing to undo." | MI-WWAV-OS |
| Waiting, honestly | "Splitting 'break.wav' · segment 4 of 12" | 5.10; v4's rule: "anything finer than these stages would be invented" |
| Failing | "Couldn't save that change. Check your connection and try again." | Heat |
| Disabled | "Habit limit reached" | Heat |
| Claude | "Claude's estimate: 45m. It read the title, the notes and your past averages." | Proposed |

Never: "Oops", "Awesome", "AI-powered", "Trending", "Don't break your streak", "3 people are looking at this", or an unread count.

### 7.11 Tokens

Every token lives in one file, `design/tokens.json`. The build compiles it into CSS custom properties for the web UI, a Rust module for the wgpu compositor, and a C++ header for the engine, so no token can drift between processes.

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
| `stem.*` | `#D23C2A` `#F0B90B` `#2E9A55` `#1F4E9E` | same | carried by an ink ring | PRANA |
| `heat.*` | `#4f9be6` `#efa431` `#e0402c` `#8f1d16` | same | fills only | Heat |
| `light.sun` | (0.30, 0.25) | same | | v3 |

### 7.12 Left out, and open decisions

| Left out | Why |
|---|---|
| Palettes you pick (v2's four, v3's four pastels) | Each palette is another set of contrast checks, and every work already brings its own colour |
| A night mode that warms the screen | The system already does this (Wi's reason) |
| Heat's slate desk `#7d8a99` | It was the web page behind the window; a native window sits on your own desktop |
| The interior register: oak and felt, sand and clay, lamp light, PRANA's Turrell field, crema, the warm ink `#3D2E22`, Crater's amber lamps and sunset sky, "after hours" | The walkable shop is cut. "Eventually the space will hold commerce instead of having that be a 4th place." Cut on 7 Oct 2026 to narrow v1. |
| Jost, and the shop's signs, tags and price cards | They were the shop's type (7.4). Cut on 7 Oct 2026 to narrow v1. |
| The room sound, and house music from `portfolio/public/media/site-music/` | Cut with the shop. The app's only sounds are the Console's click and Heat's chime (7.7). |
| The shop's idle motions (field drift, steam, lamp flicker), and haptics for records, garments, shelves, the FX moons and the tempo and pitch dials | Cut on 7 Oct 2026 to narrow v1. |
| The astronaut and any avatar | Cut on 7 Oct 2026 to narrow v1. Your galaxy chip is who you are (4.14). |
| The plus as "＋ add a song" | "Remixing is Mi-WWAV and Console, not Space." The plus is now New (7.5). |

| Open | Recommendation |
|---|---|
| One sans or two | Lucida in the desk, Inter elsewhere; recheck on Windows |
| Reading size, 17 or 19 pt | Test both at the founder's desk before the gate run |
| Wi's adjusted stem hues (`#d93b30`, `#c99300`, `#2e9150`, `#2f6fd0`) | When the wall becomes the web face, move it to PRANA's hexes with ink rings, so the colours match on the device, in the app and on a phone |

## 8. Under the hood

This is the engineering plan. The two-process split, Tauri, JUCE hosting third-party VST3 and AU, FFmpeg with wgpu and VideoToolbox, three.js for Space, the engine as master clock, direct distribution, the existing mi-wwav.com server and the MCP server (Wi_WWAV is one; see 3.13) are **Decided**. Everything else is **Proposed** unless marked **Exists today** or **Open**. Numbers are targets to test against, not measurements. Nothing here has to be ready for a public launch (see 1.4), and the privacy defaults are conservative anyway.

### 8.1 Processes

| Process | Built with | Owns | Its main thread runs |
|---|---|---|---|
| `Wi_WWAV.app` | Tauri 2: a Rust core, and the web UI in WKWebView (WebView2 on Windows) | windows, menus, the three views, the library, undo, sync, the account, video | Tauri's event loop |
| `wwav-engine` (`Contents/Helpers/wwav-engine.app`) | JUCE 8 (C++) with PRANA's `prana/core` | the audio device, the graph, MIDI, third-party plugins and their windows | JUCE's message loop |
| `wwav-scan` | JUCE, command line | one pass over the plugin folders, then exits | none |
| `wi-mcp` (in `Contents/Helpers/`) | Rust, command line | the eight tools over the library (8.8); no window, no network | none; a stdio loop |

The app and the engine are the split. Why two:

1. **A plugin is someone else's code inside the audio callback.** A crashing plugin takes its process with it: in one process, the whole app, mid-sentence in Heat; split, only the engine, which the app restarts (8.3).
2. **Both frameworks want the main thread.** On macOS one framework drives `NSApplication` from it. In the app that is Tauri; wherever plugin editors live it must be JUCE, because AU and VST3 editors build their views on the message thread and many assume it is the main one.
3. **Real-time hygiene.** The audio thread shares its process with nothing that compiles JavaScript, collects garbage or decodes video.

The engine is a faceless helper app (`LSUIElement`, no Dock icon), so plugin windows belong to a real application. Keys a plugin window doesn't take go back to the app's single input router (MI-WWAV-OS's one raw-key reader, `router.dart`), so ⌘Z, ⌘K, ⌘1–⌘3 and Space still work.

`wi-mcp` is not part of the split. Claude Desktop or Claude Code starts it, not the app, and it is one more reader and writer of the library. It never talks to the engine, because none of the eight tools touches audio.

```
+---------------------------- Wi_WWAV.app (Tauri 2) ------------------------------+
| main thread: event loop, windows, menus, the one input router                   |
|                                                                                 |
| +------------------- web UI (WKWebView) ------------------+ +-- viewer -------+ |
| | Heat | Space | Console                                  | | CAMetalLayer    | |
| | timeline, mixer, three.js Space                         | | wgpu, one pass  | |
| +-----------------------------^---------------------------+ +-------^---------+ |
|          commands (JSON) / channels (raw bytes)                     |           |
| +-----------------------------+------- Rust core -------------------+---------+ |
| | library.sqlite + ULID media | undo journal | upload queue | Keychain auth   | |
| | video: FFmpeg decode -> wgpu -> VideoToolbox encode | take writer           | |
| | engine supervisor | Heat rules | mi-wwav.com and iCal clients               | |
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

  Claude Desktop / Claude Code --stdio--> wi-mcp (Rust, no window, no network)
                                            | the app's store code: one journal
                                            | transaction per write
                                            v
                                   library.sqlite (WAL), shared with the app
```

### 8.2 The wire and the clock

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

**The engine is the master clock** (**Decided**). Every callback publishes where the playhead will be when this block reaches the speaker, output latency and plugin delay already subtracted. The web UI gets clock anchors ten times a second and extrapolates the cursor at the display rate; the video presenter reads the clock every refresh (8.5); Space's orbits run on it, so a silent sky holds still (4.4). Meters reach the web UI once per frame through a Tauri channel as raw bytes, never JSON.

### 8.3 When the engine falls over

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
- **Heat and Claude carry on.** Heat lives in the app, so a focus countdown never stops, and `wi-mcp` has no use for the engine, so Claude's tools work while it restarts.

One process per plugin stays **Open** in 5.7, with its recommendation: not in v1; decide after a month of crash logs.

### 8.4 Audio

**The graph** is the tracks of 5.3, then the four stem buses, then the master (6.6).

- Clips stream PCM from the library: WAV, AIFF, CAF and `.wwav`. The app decodes compressed imports (MP3, AAC, FLAC) once into a WAV beside the untouched original, so the engine reads one kind of thing.
- Reader threads keep 2 s of every playing clip in memory ahead of the playhead and hold files under 32 MB whole. A `.wwav` is one file feeding four stem lanes, read in 1024-frame runs as PRANA reads `wstm`.
- **Splitting** runs in a background worker, never in the audio engine (5.10).
- Clips at another rate pass through the same polyphase resampler export uses (6.7).

**Built-in effects** are PRANA's set, as 5.9 has them, with `prana/core`'s DSP wrapped as JUCE processors:

- the reverb and the delay are returns, fed by sends, and each stem bus has its own copy, fed only by its own tracks' sends, so each stem carries its own tail (5.3, 6.6);
- distortion, tremolo and the filter (a low-pass and a high-pass amount) are inserts;
- the limiter is fixed and sits on the master only.

Each has one amount, and amounts glide over about 30 ms, so a move never zippers. There is no built-in EQ, compressor or instrument.

**Settings.** Sessions run at 48 kHz, or 44.1 kHz when opened from a `.wwav` (6.7). The block is 128 frames (2.67 ms at 48 kHz), 64 while monitoring an armed input, 256 or 512 for heavy mixes. The audio thread joins the device's audio workgroup; v1 renders on that one thread, and tracks go parallel only if the reference session (8.14) misses its budget.

**The click.** The metronome is off until you turn it on (5.4). The engine makes it as 12 ms sine bursts, 1,568 Hz on the accent and 1,047 Hz on the beat, at −12 dBFS, on a cue bus that no recording or export taps (7.7).

**Delay compensation.** When a node's reported latency changes, the graph recomputes each path off the audio thread and inserts delay so all paths meet aligned at every sum. Recordings are placed by CoreAudio's round trip plus the offset **Measure latency** finds (5.6).

**Plugins.** VST3 everywhere, AU (v2 and v3) on the Mac (**Decided**). CLAP is left out of v1: JUCE doesn't host it, and a third hosting layer would be ours to write and keep. `wwav-scan` checks one plugin at a time with a 30 s limit (5.7); a plugin that crashes or hangs is not checked again until its version or file date changes. Launch never waits for a scan.

**MIDI** comes through CoreMIDI via JUCE. Timestamps become sample offsets inside the block, so a note lands where it was played, not at the block's edge (5.6). Instruments are third-party VST3 and AU only (**Decided**). Quantize is an edit to the notes in the session, made in the app, with `played_at` kept beside them (5.4); the engine plays what the session says, and nothing ever moves audio. MIDI 2.0 is left out of v1: the controllers in reach speak MIDI 1.0, the path JUCE has tested longest.

**Rendering.** Export runs the same graph objects on a non-real-time thread as fast as the CPU allows, at the same block size, with plugins told `setNonRealtime(true)`, the hook WWAV Push uses to see an export (**Exists today:** `vst_plugin/README.md`). Automation is sample-accurate and commands carry sample times, so a session renders as it played: "what you hear is what you render, by construction" (`mixer.rs`). The export sheet names the exceptions: plugins that ask to render in real time (5.12), and plugins that change their own processing offline.

**Determinism, from PRANA** (**Exists today:** `prana/SPEC.md`, `prana/tests`). Built-in DSP compiles with no fast-math and `-ffp-contract=off`, uses its own sin, cos, exp and tanh, flushes denormals explicitly, and never calls Accelerate on those paths. A session of built-ins then renders bit-identically run to run and across arm64 and x86_64, which our own golden renders check (8.12). The app makes no promise of matching PRANA's device bit for bit, and PRANA's golden hashes don't hold sessions to anything. Third-party plugins can't promise determinism either, so their tests null against the last render instead.

### 8.5 Video, basic

Video lives in the Rust core (**Decided**). It is basic (5.11): clips on tracks, cut. Nothing is done to the picture.

- **Decode.** FFmpeg's libraries, with VideoToolbox decoding H.264, HEVC and ProRes in hardware and software as the fallback. Frames stay on the GPU as IOSurface-backed Metal textures that wgpu wraps without a copy. A decoder thread keeps about 250 ms of frames ahead of the clock for the clip that is the picture, and opens the next clip on the track before the cut, so a cut never waits.
- **Proxies.** Footage over 1080p gets half-resolution proxies in the background (5.11): ProRes 422 Proxy, every frame a keyframe, so scrubbing never waits on a long GOP. That is about 20 GB per hour of 4K at 30 fps, in the session's `cache/`, safe to delete. Export always uses originals. MI-WWAV-OS's rule holds: "Slow is acceptable, dropped frames during editing are not."
- **Composite.** A plain one. wgpu on Metal (DX12 on Windows) takes the picture from the top video track that has a clip under the playhead, skipping hidden tracks, and draws that clip's frame, scaled to the session's frame, in one pass. Where no clip is under the playhead the picture is black. There is no grade, transform, opacity or blend, so there is no layer maths, and a track below the top one is never decoded for the viewer.
- **Present.** The viewer is a native `CAMetalLayer` beside the web view, following the rectangle the web layout reports, or its own window on a second display (5.2). Space's screening room (4.7) uses the same presenter, with the layer sized to the stage. Each display refresh, the presenter asks the clock where the sound will be when this frame is on glass, shows the newest frame at or before it, and drops (and counts) frames more than one late. Bluetooth delay comes from the device's own report.
- **Encode.** Export doesn't follow the clock. Frame n sits at n ÷ fps and its sound starts at sample ⌊n × rate ÷ fps⌋; the engine renders the audio first and the encoder pulls frames by number, so picture and sound line up exactly. H.264 and HEVC go through VideoToolbox, AAC through AudioToolbox, into an MP4 with `+faststart`. `.swav`'s `wmet` and `wlin` are appended last, because nothing may follow them (`formats/swav/SPEC.md`).

**Space is drawn by three.js** in the web view, on WebGPU where the view has it and WebGL 2 otherwise (4.5). It draws Space and nothing else: the Console's timeline, mixer and piano roll are ordinary web UI, and the picture is the viewer's. Space's frame loop reads the engine's clock (4.4) and stops completely when nothing moves.

### 8.6 Storage

The library is the folder `~/Music/Wi_WWAV/` (2.5).

- **`library.sqlite`** holds clips, tags, sequences, Heat's records, the Claude tool switches, plugin scan results and the undo journal, in WAL mode with FTS5 for search. Every committed change is its own transaction, so nothing needs saving; the status bar says "Saved on this Mac". A nightly `VACUUM INTO` keeps seven dated copies. The app and `wi-mcp` both write this file (8.8).
- **`media/`** holds every imported, recorded and rendered file under a ULID, written once and never edited (**Exists today:** `MI-WWAV-OS/engine/src/ids.rs`).
- **`sessions/`** holds `.wwavsession` packages (6.5).

**Heat's records** are the tables of 3.16. Each record has a `public` flag, `false` by default. The two default public items are `ProfileShare` rows (3.16); each appears on your public Heat view once you press Show (3.15). The flag is part of the row, so switching it is one journalled change, "Undo make public", like any other.

**The undo journal** follows MI-WWAV-OS's design (**Exists today:** `MI-WWAV-OS/engine/src/store.rs`), reimplemented in the app's Rust core. MI-WWAV-OS contributes the idea, not the code (**Decided**). Every mutation runs in a transaction that snapshots the affected rows before and after.

```sql
CREATE TABLE txn     (id    TEXT PRIMARY KEY,   -- ULID, so order is time
                      label TEXT NOT NULL,      -- "move clip" -> "Undo move clip"
                      view  TEXT NOT NULL,      -- heat | space | console | library
                      actor TEXT NOT NULL,      -- you | claude
                      tool  TEXT,               -- the MCP tool, when actor is claude
                      state TEXT NOT NULL);     -- done | undone
CREATE TABLE txn_row (txn_id TEXT NOT NULL REFERENCES txn(id),
                      seq INTEGER NOT NULL, tbl TEXT NOT NULL, row_id TEXT NOT NULL,
                      before TEXT, after TEXT,  -- JSON; NULL means the row didn't exist
                      PRIMARY KEY (txn_id, seq));
```

- Undo writes `before` back in reverse; redo writes `after` forward. ⌘Z walks the current view's entries, labelled through the `history` op, and the journal survives relaunch. A change Claude made has the same shape, with `actor` `claude`, so its label reads "Undo Claude's estimate" (3.12).
- Settings → Claude lists the entries whose actor is `claude`, each with Undo, for when you have moved on to another view. Undoing an entry that is not the newest first checks that none of its rows has changed since. If one has, nothing is written: "Can't undo this. The task changed after Claude's estimate."
- No `ON DELETE CASCADE` anywhere: "an unjournaled row is an unrestorable one."
- A session keeps the same shape in its own `journal/undo.ndjson`, so its history travels with it (6.5).
- Work that has left the Mac is not a journal entry, and the Edit menu says so. Publishing is undoable only while the upload is queued: ⌘Z clears `published_at` and the queue loses it. Once the server has it, the Edit menu reads "Can't undo a publish. Unpublish 'World Ending'…" (see 2.7). Making a Heat item public is different: it is a journal entry, and undoing it removes the public copy at the next sync (8.7).

**Media is reclaimed only on a press.** Deleting removes rows, never files. **Clean up media…** lists files that no row and no journal entry names ("1.8 GB in 214 files") and moves them to `trash/`; emptying the trash deletes them. So undo can always bring a deleted clip back with its sound.

### 8.7 Sync and the server

The server stays (**Decided**): Express, Postgres and R2 on Heroku. What is local is the truth (2.8).

**The queue is a query** (**Exists today:** `MI-WWAV-OS/proto/README.md`): `published_at IS NOT NULL AND remote_id IS NULL`. Each 8 MiB part is signed just before it goes up, because presigned URLs last 300 s, and recorded in `upload_part(clip_id, n, etag)`, so a resumed upload skips it. Retries back off from 2 s to 5 min with jitter. The publish body carries `settings: {origin: "wi_wwav", clipId}` (2.8), so a retry never posts twice.

**Heat sync** goes to new per-user tables, because the PKM's tables have no `userId` (3.16). Each field carries a sequence number from a per-device counter, and the server keeps the higher one field by field, so a slow older write never overwrites a newer one (the PKM's `useAutosave.js` pattern). Only the app syncs. A change that `wi-mcp` made while the app was closed is an ordinary journalled change, and it goes up when the app next opens.

**What reaches the server from Heat** (**Decided**, from 3.15 and 3.16):

| What | Where it goes |
|---|---|
| A private record | your own rows, which only you can read |
| A private grade or course | nowhere. It stays on the Mac unless encrypted sync is on (**Open**, 8.9), and then it goes up as ciphertext |
| A record switched public | the same, plus a public copy holding only the fields 3.15 lists for that kind of record |
| A grade switched public | a public copy in the clear, because other people have to read it |
| The two default public items | a public copy each: the Now making line, and each project timeline you linked |
| An item switched back to private | its public copy is removed, whatever its kind |
| iCal addresses, the account token, the sync key | never; they stay in the Keychain (8.9) |

A public copy is written by the same push as any other change, and removed by the same push when the flag clears. The Now making copy carries its `clearsAt`, and the server stops returning it then, even if the Mac is off. The public view shows what the server has, so a new public item appears once it is up (2.8).

**Calendars.** The Rust core fetches each iCal address itself, on open if the last sync was over 15 minutes ago and then hourly (3.11). The address is read from the Keychain for the request and is never logged.

**Reused as they are** (**Exists today**):

| Endpoint | Used for |
|---|---|
| `/api/upload/sign`, `/process` | not used by the desktop, which splits locally (5.10); kept for cloud splits from phones and the web |
| `/api/upload/sign-stem`, `/process-stems` | the four stems unpacked from a `.wwav` for the web and iPhone players, uploaded beside the exact file (`/api/upload/parts`), so nothing is split twice |
| `/api/upload/sign-video` (2 GB cap) | films; the server also makes the streaming copy (4.12) |
| `/api/upload/sign-replace`, `/:trackId/replace-audio` | a new version under a permanent link, as WWAV Push does |
| `/api/publish`, `/api/unpublish` | a work into a system |
| `/api/lineage/global`, `/api/tracks/:id/lineage`, `/api/tracks/:id/fork` | family trees; ↑ Push (mix states) |
| `/api/v2/*` (galaxies, systems, planets, suns, lineage-links, saved, catalog, universe) | Space, as the iPhone uses it (4) |
| `/api/auth/refresh`, `/api/auth/me` | the 7-day JWT, refreshed after a 401 (2.4) |
| the devlog's read route, `server/routes/devlog.js` | letters on a sun (4.8) |
| `/api/messages` | the message door (4.11) |

**New** (**Proposed**):

| Endpoint | Why |
|---|---|
| desktop sign-in, authorization code with PKCE | below |
| `/api/upload/parts` | multipart straight to R2: `/sign` is one PUT capped at 250 MB, and a 6-minute `.wwav` is about 318 MB |
| `/api/heat/changes` | Heat sync, pulled with a cursor, pushed in batches; it carries public copies and their removal |
| `/api/heat/public/:userId` | reading a person's public Heat items, for the public Heat view on their sun (4.8) |
| `/desktop/latest.json` | the update manifest (8.10) |
| Stripe webhook sets `tier` | `server/routes/subscription.js` sets `isPro`/`proExpiresAt` today and must set `tier` as well, so a plan bought in the browser is the plan the app reads (9.4) |
| the web face (share pages) | share pages and public read routes on www.wi-wwav.com, because `WiPosts` serves one account today (below) |

`/api/heat/public/:userId` returns the person's public copies and nothing else, in the order 3.15 gives: the Now making line, the project timelines (each with its `targetId`, so a project sun picks out its own), then the other public records by kind. It is open to anyone who can open the sun, as `/api/v2` reads are. It returns records, never a total, a count or a comparison (3.15).

Production doesn't run `sync({ alter: true })` for new tables, so each new model gets an explicit `Model.sync()` at boot, as `DevlogPost` and `WiPost` do (`server/index.js`).

**Sign-in** opens the system browser on mi-wwav.com with a PKCE challenge and listens on a loopback address for the answer (RFC 8252). The password, Sign in with Apple and password managers stay in the browser; the app receives the account's usual JWT and a refresh token. The server already runs an OAuth 2.1 server with PKCE for the devlog's Claude connector (**Exists today:** `server/mcp/auth.js`); desktop sign-in adds a public client that checks real accounts.

**The web face.** Share links open on www.wi-wwav.com, so a friend on a phone installs nothing. Wi's player reads PCM a range at a time, "so nothing is decoded up front", under four faders (**Exists today:** `wi/`); a `.swav` plays as the MP4 it is. Wi serves one account today, so `WiPosts` needs an owner and public read routes.

### 8.8 The MCP server

**Decided:** Wi_WWAV is an MCP server, and the first one is local (3.13). The tools, their arguments, results and undo labels are 3.13's table, and this section does not repeat them. It says how they run.

**The helper.** `wi-mcp` is a small Rust program shipped inside the app, in `Contents/Helpers/`. Claude Desktop and Claude Code start it as a child process and speak MCP to it over stdio. Settings → Claude shows the lines to paste into Claude's config, with the helper's path as it is now, so they stay right after the app updates or moves. It opens no port and makes no network call.

**One tool call, one transaction** (**Proposed**). The helper links the same store code the app does, and the rules that Heat's tools depend on live there too: the heat algorithm, the estimate chain and Plan my day sit in the Rust core, and the web UI reaches them through commands. So Claude and the window never disagree about heat, and a rule such as "clamp 5–600" or "the same `source_id` twice makes one task" exists once. A call goes like this:

1. Claude sends `tools/call` on stdin.
2. The helper checks that the tool is switched on, then checks the arguments against 3.13's table.
3. It opens `library.sqlite`, starts a write transaction, and calls the store function the UI itself calls. The store writes the change and, in the same transaction, a `txn` row with `actor` `claude`, the tool's name and its fixed label, and the `txn_row` snapshots (8.6).
4. It commits and returns the plain JSON of 3.13, with `undo_label`.

If any step fails, the transaction rolls back and the helper returns one plain sentence with the error flag set. A change and its journal entry are never apart, so killing the helper mid-call leaves both or neither.

A read (`list_tasks`, `get_grades`) writes nothing. `plan_day` writes only the drafts, as `HeatState.planDrafts`, and no journal entry, because a draft is not a change until you press Return (3.13).

**When the app is open, and when it is not.** The app need not be open.

- **Closed.** The helper opens the library the app uses, the folder in Settings → Library. The change is in the file when the app next opens, and ⌘Z there already undoes it. The helper has no network, so sync waits for the app. If it finds no library, or a library newer than itself, it says so in one sentence and writes nothing: "No Wi_WWAV library yet. Open the app once."
- **Open.** There is still one library: one SQLite file in WAL mode, and two processes writing it through the same journal. WAL lets readers go on while one process writes, and a second writer waits for the first's lock, so both use a busy timeout rather than fail. Journal ids are ULIDs, so entries from both sit in one order in time. SQLite tells a connection when another has committed (`PRAGMA data_version`). The app checks it on a short timer and when its window comes forward, reads the journal rows newer than the last one it saw, and refreshes only the views those rows name. A change by Claude then shows on screen while Claude makes it.

**Security.**

- **Local only, by default.** stdio has no address to reach. Nothing outside your Mac can call a tool.
- **No network credentials.** The helper holds no account token, never opens the Keychain, and makes no network call. The iCal addresses and the account's JWT are out of its reach.
- **The door is the list.** The helper offers eight tools, and the store functions behind them take only the fields in 3.13's table. There is no argument that can set `done`, a score or the `public` flag, delete a record, change an existing due date, send mail or read the Public switch.
- **Switches.** Settings → Claude has a switch for each tool, kept in the library. The helper reads them on every list and every call, so a tool switched off is missing from what it offers, and a call to it is refused: "This tool is switched off in Wi_WWAV."
- **Claude's words are data.** Titles, notes and reasons Claude sends are length-limited and drawn as plain text, never HTML, like words other people wrote (8.9).
- **What Claude reads.** The Public switch decides what other people see. It does not stop Claude reading your Mac's library through a tool you allowed. That is Claude's own permission prompt (3.13).

**Remote, later.** A server through mi-wwav.com for claude.ai needs Heat sync first (8.7). It offers the same eight tools, arguments and labels, and signs in as the devlog's `/mcp/write` does (**Exists today:** `server/mcp/devlog.js`, `server/mcp/auth.js`), here against real accounts. It writes to the synced copy, and the Mac journals the change when it arrives, with the same "Undo Claude's…" label. Two limits follow from 8.7. It can't read private grades, which the server doesn't hold or holds only as ciphertext, so its `get_grades` returns the grades you made public and says so. And a change made while the Mac is off waits in `/api/heat/changes` until the app opens.

### 8.9 Security and privacy

- **Secrets live in the Keychain** (Credential Manager on Windows): the account token, every iCal address (the Brightspace link carries a private token) and, if encrypted sync is on, its key. Never in SQLite, logs, crash reports or `heat.json`. They are bound to the app's signing identity, so the engine, which runs strangers' code, can't read them; it holds no network credentials at all.
- **There is no Anthropic key anywhere in the app,** and the app calls no model. Claude comes to the app through `wi-mcp`, under Claude's own tool-permission prompts. Claude estimates and drafts, never decides; the tool descriptions keep "never invent metrics" (`rippleCreatorService.js`).
- **There are no Google tokens.** The app has no Google sign-in. Claude reads school mail through its own Gmail connector and records a thread with `record_mail_thread`, which takes the subject, sender, time and a reason, and never the message body. So an email's text never reaches the app or the server.
- **Library validation is relaxed only where plugins load,** in the engine and the scanner.
- **The web view is fenced.** Tauri 2 capabilities give each window only the commands it uses, no remote scripts load, and words other people wrote, and words Claude wrote, render as sanitized Markdown or plain text, never HTML.
- **Heat is private by default** (**Decided**). Every record is private. Two items make the simple public version: the Now making line and the project timelines you linked. Anything else goes public only when you switch that one item, and the switch is yours alone: no tool sets it (8.8).
- **A public copy holds only the fields 3.15 lists** for that kind of record, and exists only while the switch is on. The public copy never carries a notes field, an estimate or a streak. The server has no total, no count and no comparison to send, because none is ever made (3.15).
- **Grades default to private.** Grades are FERPA education records while a school holds them. Here the student chooses to show their own, and the switch says so: "Grades are private by default. Your school keeps them as education records. Turning this on shows this grade to anyone who opens your sun." A public grade is copied to the server in the clear, since other people have to read it, and the copy is removed when you switch it back. **Open:** whether private grade rows sync at all. Recommendation: off by default; when on, encrypted on the Mac with a Keychain key, so the server holds only ciphertext.
- **No analytics.** Crash reports are opt-in, carry no media, titles or tokens, and name the plugin, because that's what fixes the bug.

### 8.10 Shipping on the Mac, and Windows later

A direct download, not the Mac App Store, whose sandbox blocks plugin hosting (**Decided**).

| Item | Plan |
|---|---|
| Identity | Developer ID Application certificate (Apple Developer Program, $99 a year) |
| Runtime | hardened runtime on all four executables |
| `wwav-engine` | `com.apple.security.cs.disable-library-validation` so others' plugins load; `com.apple.security.device.audio-input` |
| `wwav-scan` | `disable-library-validation` |
| `Wi_WWAV.app` | library validation on |
| `wi-mcp` | library validation on; no entitlements, because it loads no plugins and opens no devices |
| Notarization | `xcrun notarytool submit --wait`, then `xcrun stapler staple` on the DMG |
| Architectures | universal: arm64 and x86_64 for Rust, JUCE and FFmpeg |
| Updates | Tauri's updater: an Ed25519-signed manifest at `mi-wwav.com/desktop/latest.json`, files on R2 |

- **Updates never interrupt.** They download in the background and install when you quit, never mid-playback: "Update ready · installs when you quit". Sparkle would add delta updates; Tauri's updater serves Windows from the same manifest and keys. The helper is replaced with the app, so the two always match.
- **The iCloud trap.** iCloud Desktop and Documents stamp `com.apple.FinderInfo` onto build output and codesign refuses it, as MI-WWAV-OS and WWAV Push found (`MI-WWAV-OS/README.md`, `vst_plugin/README.md`). Builds go to `~/Library/Developer/wi-wwav-build`; the release script runs `xattr -cr` before signing.
- **Protected plugins** sometimes need `allow-unsigned-executable-memory`. **Open.** Recommendation: add it only if a plugin in the test set fails without it.
- **Intel-only plugins** can't load into an arm64 engine. **Open.** Recommendation: offer "Open the engine under Rosetta" per session, as a bridge; Apple has said Rosetta's general use ends after macOS 27.
- **Minimum macOS. Open.** Recommendation: macOS 13, which runs on Macs back to 2017. "Wwav is about everybody" (devlog), and an old Intel laptop is what many people have.

**Windows** follows 3–6 weeks after the Mac (**Decided** estimate): WebView2; ASIO under Steinberg's free SDK agreement, WASAPI as fallback; VST3 only; wgpu on DX12; hardware decode and encode through FFmpeg (D3D11VA, Media Foundation, NVENC, Quick Sync, AMF); Authenticode through Microsoft's Trusted Signing or an OV certificate. The first build is x64, because an ARM64 engine can't load x64 plugins.

### 8.11 Licences

| Part | Licence | What it means here |
|---|---|---|
| JUCE 8 | AGPLv3 or commercial | Wi_WWAV is closed and takes money for Pro (9), so commercial; a free Starter tier applies under a revenue cap (check the cap at the version pinned). WWAV Push already uses JUCE 8.0.4 (**Exists today**) |
| VST3 SDK | MIT from 3.8 (October 2025); confirm at the version pinned | the logo needs Steinberg's separate agreement, so the UI writes "VST3" in plain text |
| Audio Units | Apple's SDK | no fee |
| FFmpeg | LGPL 2.1 or later | built `--disable-gpl --disable-nonfree --enable-shared`, dylibs in `Contents/Frameworks`, no libx264, libx265 or fdk-aac, source and build script published. A user may swap the libraries if they re-sign them; the notes say how. |
| Tauri, wgpu, three.js, Rust crates | MIT or Apache-2.0 | credited in About |
| Surge XT | GPLv3 | tests only, never shipped |

Codec patents (H.264, HEVC, AAC) are a question for a lawyer, logged in `docs/DECISIONS.md`. Encoding through the operating system's encoders means the app ships none of its own.

### 8.12 Tests

The gates' rule applies: write down what a fail looks like before testing, and "a check with no evidence is marked open. It is never marked passed" (`wi/GATES.md`). That is what makes long stretches of autonomous work checkable: each milestone in `docs/PLAN.md` names its fail criteria before any code.

| Suite | Proves | Runs on |
|---|---|---|
| Golden renders | sessions in `tests/sessions/` render to the same SHA-256 every time, on arm64 and x86_64; the goldens change only through `tools/update_golden.sh` in a commit that says why | Linux; macOS nightly |
| Plugin hosting | Surge XT loads, takes parameters, restores its state, renders, and nulls against its last render below −96 dBFS; its editor opens and closes | VST3 on Linux, AU on macOS |
| Delay compensation | test plugin `delay-n` adds a known latency; its sum with a dry copy nulls | Linux |
| Kill and restart | test plugins `crasher` (dies on a note) and `hanger` (never returns), plus 200 kills at random moments in playback, recording and render. Pass: back in 2 s, session hash unchanged, takes intact to the last block, transport stopped | Linux, macOS |
| Formats | the Rust `.wwav` and `.swav` writers match `wwav_pack.py` and `swav_pack.py` byte for byte; unpack then pack is identity; `ffprobe` accepts every export | Linux |
| Journal | random edit runs: undo all returns the first state byte for byte; redo all the last | Linux |
| MCP tools | `wi-mcp` runs over stdio against a test library, and every tool in 3.13 is called. Each write tool adds exactly one journal entry with `actor` `claude`, its fixed label and its result's `undo_label`, and undoing it returns the library byte for byte. The reads and `plan_day` add none. A repeated `source_id` or `thread_id` makes no second row. A tool switched off is missing from the list and refused when called. No argument sets `done`, a score or `public`. Kill the helper at 200 random moments: each call left both its change and its entry, or neither. With the app running, 1,000 interleaved writes from both lose none, and the app shows each | Linux; macOS nightly |
| Public and private | a private record, a private grade and a private note never appear in `/api/heat/public/:userId`; a public record shows only the fields 3.15 lists; switching it back removes its copy at the next sync; a Now making copy disappears at its `clearsAt` with the Mac off; no response carries a count or a total | Linux |
| Picture and sound | a test film with a flash and a click each second exports with zero offset; with two video tracks stacked, the viewer and the export show the top clip's frames, and a hidden track is skipped | macOS |
| UI | `mock-engine` speaks the socket and writes a fake clock and meters; the web UI runs in Playwright (WebKit, Chromium) with a fake Tauri bridge against stored screenshots of every view | Linux |
| Soak | 8 hours of the reference session: no dropouts, flat memory | a Mac, each release |

`tauri-driver` has no WebDriver for WKWebView, so Playwright's WebKit stands in, and a pass by hand on a real Mac covers the gap. The MCP Inspector, which the devlog's server is already tested with, gives a by-hand check of `wi-mcp` against a real Claude client.

**CI** is GitHub Actions. Everything that can run on Linux does; macOS runners take AU, VideoToolbox, signing and a nightly universal build. On a private repository a macOS minute bills at about ten Linux minutes, so macOS jobs on pull requests stay under 10 minutes. **Open:** a self-hosted Mac mini runner. Recommendation: once the macOS bill passes about $50 a month.

### 8.13 The repo

A new repository (**Decided**); `wi-wwav-desktop` is a working name.

```
wi-wwav-desktop/
  app/src-tauri/      Rust core: store, journal, Heat rules, sync, auth, video, engine supervisor
  app/ui/             web UI: shell/, heat/, space/, console/
  mcp/                wi-mcp, the stdio helper; builds against the store in app/src-tauri/
  engine/             wwav-engine and wwav-scan (JUCE, CMake); test-plugins/
  formats/            submodule: Mi-WWAV at a pinned commit (prana/core, prana/tools,
                      formats/swav, wi/src/formats)
  web/                shared UI parts and the share page for www.wi-wwav.com
  design/tokens.json  compiled to CSS, a Rust module and a C++ header (7.11)
  tests/  tools/  .github/workflows/
  docs/PLAN.md        milestones in build order, fail criteria written first
  docs/DECISIONS.md   date, choice, reason, what was turned down
  docs/QUESTIONS.md   Open decisions for the founder, each with a recommendation
```

An agent working alone writes its question into `QUESTIONS.md` and moves to other work rather than guessing.

**Open:** the submodule pulls the whole Mi-WWAV repo, iOS apps and portfolio included, for four folders. Recommendation: use a sparse checkout now, and split those folders into their own repository only if clone times start to hurt.

### 8.14 Budgets

Reference machine: a 2020 M1 MacBook Air with 8 GB. Reference session: 24 audio tracks, two `.wwav`s as stem lanes, 8 Surge XT instrument tracks, 40 built-in devices and two 4K video tracks.

| What | Budget |
|---|---|
| Audio callback | 2.67 ms at 128 frames, 48 kHz; DSP load at most 70% at the 99.9th percentile |
| Dropouts | none in an hour of the reference session |
| Click to sound | a mute is heard within one block and one socket hop, 10 ms at most |
| UI | 60 fps everywhere; 120 fps on ProMotion for timeline scrolling and the Space camera |
| Space | 60 fps at 2560 × 1600 with up to 2,000 galaxies as instanced points (4.5) |
| Video | a 4K stream with cuts, no dropped frames; footage over 1080p scrubs on proxies |
| Cold launch | Heat usable in 1.5 s; the engine opens its device in 0.8 s alongside, and Heat never waits for it or a scan |
| Engine restart | 2 s plus plugin load |
| Offline render | 10× real time or faster for sessions of built-ins |
| Library search | 50 ms at 50,000 clips |
| Memory | app 300 MB idle, not counting WebKit's processes; engine 150 MB empty |

CI measures what it can. The rest is measured by hand at each release and written into `docs/PLAN.md` beside its budget.

### 8.15 Left out, and why

| Left out | Why |
|---|---|
| The 3D shop's engineering: the hall budget (1,000 shops), the screen quad that seated a film in its room, `/api/store/*`, and store receipts in `library.sqlite` | The walkable shop is cut. "Eventually the space will hold commerce instead of having that be a 4th place." Cut on 7 Oct 2026 to narrow v1. |
| Purchases of works, Stripe Checkout for works and its web view in the app, Connect payouts, fashion listings, orders and tickets (`/api/purchase/*`, `/api/purchases/*`, `/api/connect/*`, `/api/fashion-listings`, `/api/orders`, `/api/events`) | Commerce is later, inside Space (4.13), and nothing of it is designed now. Cut on 7 Oct 2026 to narrow v1. Plans are still bought in the browser (9.4). |
| `/api/assist/:task`, its prompts and per-account limit, and the founder's Anthropic key on the server | Wi_WWAV is an MCP server instead (3.13). The app calls no model, and Claude's own permission prompts ask before each tool call. |
| A Google OAuth client, the `gmail.readonly` scope, Google verification and 7-day testing sign-ins, and Google tokens in the Keychain | Claude reads mail through its own Gmail connector, and calendars come in as iCal addresses, so the app needs no Google account. |
| A local network listener for MCP | Claude Desktop and Claude Code start the helper over stdio, so there is no port to defend. |
| Holding sessions to PRANA's golden hashes, and device parity | Cut on 7 Oct 2026 to narrow v1. Our own golden renders cover the built-ins. |
| Grade, transform, opacity and blend in the compositor, and `sanitizeGrade` | Decided: video is basic, with no grade and no effects (5.11). A film fork is a new film, not a recipe. |
| Camera takes and the camera entitlement | Cut on 7 Oct 2026 to narrow v1. |
| Location, snapped to 0.01° before it left the Mac | Its only use was "Near you", which is cut on 7 Oct 2026 to narrow v1 (4.14). |
| `/api/v2/travel` | It served the game layer, which is cut on 7 Oct 2026 to narrow v1 (4.14). |
| A device-code sign-in (RFC 8628) | It served PRANA's deferred account link, which waits for the device. The app signs in with PKCE alone. |
| The Unquantized room in the process diagram, the `unquantized` journal view, and the key ⌘4 | Unquantized is cut entirely on 7 Oct 2026 to narrow v1. |

## 9. Business, community and the gates

Wi_WWAV is a business. This chapter covers how it makes money, who it is for, and how each money-making part holds up against the founder's own written design philosophy, the four gates in `wi/GATES.md`. Everything here is **Proposed** unless it carries another label. Prices and fees that already run on the server are marked **Exists today**. Selling works is **later**, inside Space (**Decided**; see 4.13), so for now the money comes from hosting and cloud splits.

### 9.1 Why it is a business

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

1. **The artist keeps the money and the masters.** When Space sells works, a digital sale pays the seller 90%; the server already works that way (**Exists today:** `server/routes/purchase.js`, `connect.js`), and nothing in the app sells yet. WWAV takes no rights in a work beyond hosting and delivering it. Distribution without a label in the middle is the rebellion.
2. **Leaving costs nothing.** Export and Export everything are free and work while you are signed out (see 2.9).
3. **The price is a number everybody can reach.** The free app is a whole product: Heat, the Console with plugins, and a galaxy.
4. **It does what a phone can't.** Hosting VST3 and AU plugins, cutting a film against four stems on one clock, and drawing a universe in real 3D need a computer (see 1.4). That is "irreplaceable by the iphone" for software; the device line in 9.2 is the hardware answer. A direct download with no Apple cut (**Decided**) is the founder's "taking another bite out of apple" (devlog, Sep 29).
5. **Money comes from selling things, never from attention.** Every revenue line in 9.4 sells hosting, cloud splits, a seat, a device, an hour of someone's time or, later, a work. None of them sells a view. This is the app's hypothesis for gate 1.4 (9.6).

### 9.2 The family model, applied

"Every WWAV family follows one model: a device, a format, and a marketplace. You make the work on the device, it lives in a WWAV format, and creators can sell it." (`archive/src/content.js:369`). Wi_WWAV is where all four media will share one marketplace, which is **later**, inside Space (4.13).

| Family | Field | Hardware | Device in this app | Format | Marketplace |
|---|---|---|---|---|---|
| Mi_WWAV | sound | PRANA (Mi-WWAV Beta 1), in development; Mi_cro and Mi_pro tiers described | the Console (5) | `.wwav` (**Exists today**) | Space, later |
| Si_WWAV | sight | concept: "a camera and mobile editing terminal in one device" | the Console's video side (5.11) | `.swav` (**Exists today**) | Space, later |
| Ri_WWAV | words | concept: e-reader and writer | the suns' block editor (4.7, 4.8) | none yet; Markdown with `wmet` and `wlin` front matter (**Open**, see 6.11) | Space, later |
| Gi_WWAV | garments | concept: a design-to-garment machine | none; Space shows galleries and makes none (4.7) | the unnamed "pattern file" (**Open**) | Space, later |

Zi_WWAV (furniture) is left out, because the sketch names four media and nothing in the app makes a chair.

**The Console is Mi_pro_WWAV's software.** The archive describes Mi_pro_WWAV as "Desktop studio: a stronger processor, many inputs and outputs, instruments and sequencing. Makes discs." Three clauses map to something in the app:

- **A stronger processor:** the Mac.
- **Many inputs and outputs:** Core Audio devices.
- **Instruments and sequencing:** third-party instrument plugins, MIDI and the piano roll (**Decided**).

The fourth clause, **makes discs**, waits: "PRANA disc not ready yet, so definitely not writing software yet" (see 1.7). Shipping the software first gives Mi_pro_WWAV users before it has a case.

**The app does not sell the hardware,** and nothing in it is an advertisement. Two ways it would have are out: the disc round trip, because writing a PRANA's disc waits for the disc, and a drop page, because commerce is later (4.13). One stays: **founding members first.** A Founding seat (9.4) carries a reserved place in line for the device. It is not a discount on it.

**Post 34's roadmap.** Each step is quoted from the post, beside the app's part in it:

| Step | What the app does |
|---|---|
| "1. build beta v1" | Heat's WWAV space holds the milestones (its placeholder today reads "Beta v1 working"). |
| "2. ^ use beta v1 to find cofounders" | Letters on a sun show the build (4.8). The published gates and left-out tables show the inside, which is gate 2.5's question: "Would you show the inside to someone you respect, who knows what they're looking at?" |
| "3. build beta v2 with cofounders" | Testers' remixes come back as forks with lineage. Until PRANA discs can be written, those are remixes made in the Console. A survey can't give that test data. |
| "4. build out a kickstarter campaign" | The campaign runs on Kickstarter, as the post says. A letter to Wi-WWAV and the Founding list point to it. |
| "5. start selling preorders & manufacturing beta v3" | Heat holds the manufacturing timeline. Taking preorders in the app waits for commerce in Space. |

Kickstarter adds a 5% platform fee on top of payment processing. The old question, Kickstarter or the store's own preorders, is closed for now, because the app has no store.

### 9.3 Wi-WWAV: the community

"I think Wi-Wwav (we wave) is a perfect name for the people who are customers and fans of wwav, we are the wwav, so just shorten it and its Wi-WWAV." (devlog, Sep 30). The app takes its working name from this line: Wi_WWAV is the place and Wi-WWAV is the people in it. The letters keep the founder's hyphen. Hyphen or underscore is an **Open** naming decision (see 10.4).

- **Letters, not posts.** Since the second post, every devlog post opens "Dear Wi-WWAV," and is signed "LMY". In the app, a letter is a page with a greeting and a sign-off, read-only on a sun (see 4.8). The app's own release notes are letters too, and open the same way. They appear on LMY's sun and in "Since you last looked", and are never pushed.
- **Building in public.** The devlog is public, and Claude can read it through the `/mcp` connector (**Exists today**). The new repo publishes its `GATES.md` with fail criteria before results, as Wi did, and every chapter's "Left out, and why" table. That is what a future cofounder reads first.
- **No audience, only people.** There are no followers to count. You add a galaxy, and its letters reach you when you look (see 4.10 and 4.11), the founder's included.
- **Numbered first members.** A Founding member can show their number on their sun ("Founding #12"). It is off by default, because a badge that only some people can have works as status.

### 9.4 Money

#### Revenue lines now, and the one that waits

| Line | What is sold | Price | WWAV keeps | Status |
|---|---|---|---|---|
| Store fee, later | works, once Space sells them (4.13) | set by the seller; nothing is designed | 10% of the total, included in the price | **Proposed**, later. The server's 10% **Exists today** (`purchase.js`) |
| Pro | cloud splits and hosting room | one price list, **Open** below | all, less processing | exists in three versions |
| Founding Member | Pro for life and a number from 1 to 500 (**Exists today**); first in line for the device (**Proposed**) | $199.99 once | all, less processing | **Exists today** on iOS (`foundingMember.js`; the client says $199.99, the `iap.js` header $249.99) |
| Split packs | 50 cloud splits | $9.99 | all, less processing | **Exists today** on iOS |
| Hardware | Mi-WWAV preorders | **Open** (below) | the margin | planned (post 34) |
| Education | cohorts | $500 a seat, 12 seats | all, less processing | **Exists today**, out of date; the class systems it ran on are later |

The store fee is not a revenue line yet. Nothing in the app sells a work until commerce returns, and no plan here counts on it. Pro, Founding and split packs are what pays for the server now.

If all 500 Founding seats sell, they bring in $99,995 before fees, and that money arrives before the hardware does.

#### Three price generations, one price list

| Generation | Where | Prices |
|---|---|---|
| Web PRO (v1) | Stripe, with the amount kept in an environment variable; CLAUDE.md | $11 a month with a 14-day trial. The header of `iap.js` and `APP_STORE_READINESS_PLAN.md` extend it to $11 / $99 / $249.99 / $12 |
| iOS client | `ios_v3/…/Products.swift`, `MiWwav.storekit` | $7.99 a month, $69.99 a year, $199.99 Founding, $9.99 for 50 splits |
| Ops doc | `MONETIZATION_SETUP.md` | $4.99 a month, $19.99 a year, 3-day trial |

`Users` also carries three entitlement fields at once: `isPro`/`proExpiresAt`, `tier` and `credits`. The desktop app reads only `tier`. Today only Apple verification (`server/routes/iap.js`) sets it. The Stripe webhook (`server/routes/subscription.js`) sets `isPro`/`proExpiresAt`, so it must be changed to set `tier` as well, which is new server work. A plan bought through Stripe in the browser is the same plan on the iPhone, and the iOS app still sells it through Apple. In the desktop app, buying a plan opens mi-wwav.com in the browser, as sign-in does (8.7).

**Open: one desktop price list.** *Recommendation:*

| Plan | Price | What it pays for |
|---|---|---|
| Wi_WWAV | free | Everything that runs on your computer: Heat and its MCP server; the Console with plugins and unmetered local splits; export. Also 10 GB of published work hosted in your galaxy (about 45 songs as `.wwav`) and 10 cloud splits for life |
| Pro | $7.99 a month or $69.99 a year | What runs on WWAV's computers: 100 cloud splits a month from the iPhone or the web, and 100 GB of published work hosted |
| Founding | $199.99 once, seats 1–500 | Pro for life, the number, and a reserved place in line for the device |
| Split pack | $9.99 for 50 | Cloud splits that keep until you use them |

The plan page states the rule behind this list as Pro's own description: "Pro pays for the parts of WWAV that run on our computers. What runs on yours is free."

- **Storage cost.** At R2's list price of $0.015 per GB-month, a full Pro allowance costs $1.50 a month to store.
- **Why $7.99.** It is the newest price in shipped code, so one number can hold on every platform. A $7.99 month through Stripe nets about $7.46 after the standard US card fee (2.9% + 30¢). On iOS, Apple keeps 15–30%.

**Splits after local Demucs.** In v3 a split was the unit of currency, because each one costs a Replicate GPU run. On a Mac, Demucs runs in a background worker for free (see 5.10; v3's dev server took 30–60 s a song on Apple Silicon), so the desktop meters nothing local. Splits remain a unit only for cloud splitting, which phones and the web still need, and the quota in `server/utils/tier.js` and the reserve-and-refund transaction in `splits.js` stay as they are (**Exists today**).

**Open: expiring packs.** Packs expire 90 days after purchase today. *Recommendation:* they keep until used. An expiry date is a timer that pushes use, which is gate 1.1's concern, and the money for a pack has already been paid.

**Founding Member.** **Exists today:** the 500-seat counter is one server row behind a row lock, and a `foundingMemberNumber` never changes. The desktop sells the same seats through Stripe in the browser, against the same counter and 10-minute reservation. Checkout shows your number before you pay: "You'd be Founding #213." Nothing counts down the seats left. Once the last one goes, the plan page says what v3 says: "Program closed — all 500 slots claimed".

**Claude isn't sold.** The app calls no model at all. Claude runs in the person's own Claude (claude.ai, Claude Desktop or Claude Code) and reaches Wi_WWAV through MCP (see 2.11 and 3.13). There is no Anthropic key in the app, no daily limit and no credits, and what Claude costs is between the person and Claude. Ripple Creator's credits (15 for a chat, 40 for a strategy) stay with the old iOS app.

**Education.** The conservatory is already written (**Exists today:** `education.js`): 4 courses, 42 lessons, 126 assignments and about 157 hours, with each lesson's three assignments tagged study → make → refine.

- **Lessons.** Lessons are free, because they teach the tool and they are the shortest path to gate 4.
- **Cohorts.** What is sold is a person's time: 12 seats at $500, refundable until the start (as `cohort.js` does). Running a cohort as a class system in Space is later (see 10.1), so for now this line stays as it is. `cohort.js` holds one date, July 1, 2026, which has passed, so it needs a date per cohort.

#### Anti-luxury pricing

"teenage engineering only does one thing wrong in my opinion and its seriously wrong. There products are luxury items which is codename for inaccessible to poor ppl." (devlog, Sep 30). These rules make it checkable:

- **Never gate leaving with your own work.** Export and Export everything are free and work while signed out (see 2.9).
- **No feature works only with expensive gear:**
  - Demucs runs on the CPU on Intel Macs. It is slower and says so (5.10).
  - A song can be finished with no third-party plugin: record audio, split it, cut it, and use the six built-in effects (5.9). Instruments are third-party only, so MIDI needs one (5.3).
  - The Console records from the built-in microphone.
  - Newest gives every work in Space as a list, on any machine (4.10).
  - Performance targets a 2020 M1 MacBook Air, not a new Pro (8.14).
- **Free is a whole product.** Someone who never pays can plan a term, finish a song and put it in Space.
- **Every price is the whole price.** There is no "from $" and no fee added at checkout. When selling comes, the fee stays inside the price, as the server's line item says today: "(10% platform fee included)" (**Exists today**).
- **Nothing paid for expires.**
- **The fee is the same for everyone.** Pro buys storage and splits. It never buys a lower fee, better placement or more reach.

**Open: the device's price.** The devlog names no price. Its one signal is "TE's best products are like 2k". *Recommendation:* put a price ceiling in the campaign plan before step 4 and announce it in a letter to Wi-WWAV, so "Wwav is about everybody" is a number before it is a campaign page.

### 9.5 School

Heat started as a student's tool. Its prompts describe "a college student in computer engineering and Japanese", and the artifact's sync is hard-coded to the University of Rhode Island (see 3.11).

School matters to the business because of gate 4: "Did someone who doesn't consider themselves an artist make something with it?" The gate "fails until it happens". The Wi wall can't pass it, because it is locked to one account (`wi/GATES.md`). A university is full of people who don't consider themselves artists, and they already open a planner every day.

**Small groups come before the whole school.**

| Stage | Who | Needs | Limit |
|---|---|---|---|
| 1. One | the founder | the Brightspace iCal feed; Claude's Gmail connector, if wanted | none |
| 2. A small group | 5–20 classmates: a club or one section | nothing hard-coded to one school (3.11); each person pastes their own Brightspace link; Claude is optional, because Heat works without it (3.12) | none from Google, because the app asks Google for nothing |
| 3. The school | anyone at URI | registration in the school's Brightspace, through Valence or LTI 1.3 | the school's terms |

A class, with a teacher and a class system in Space, is later and not designed now (see 10.1).

Stage 2 is the first point where gate 4 can pass. Its fail criterion is written down first:

- Before they start, each person in the group is asked once, in person: "Do you consider yourself an artist?"
- The answers go into `GATES.md`, with each person's permission.
- The gate passes the first time someone who answered no makes something and keeps it.

**The Brightspace ask.** The iCal feed works now and needs no approval: each student pastes their own private calendar link, and Heat reads the due dates. Real scores, course lists and gradebook weights need D2L's Valence REST API or an LTI 1.3 tool, and the school's Brightspace administrator has to register either one. The ask, in plain words:

> We'd like Wi_WWAV registered as a read-only Brightspace app. Each student connects their own account and can disconnect it at any time. It reads their courses, due dates and grades so they can plan their week. Grades are private by default: they appear nowhere unless the student chooses to show one of their own, they are never sold, and they are deleted when the student asks. We'll sign the school's data-privacy agreement.

Grades are education records under FERPA. Keeping them private is part of the design, not a setting a student has to find: every grade starts private, and only the student can switch one public (see 3.15).

**Open: when to ask.** *Recommendation:* ask after stage 2, with the group's weekly reviews as evidence that Heat helps; twenty students make a different request from one.

**Open: money from the school.** Universities fund student ventures through innovation centres, pitch competitions and grants. *Recommendation:* apply to the ones that take no equity, and lead with gate 4 and the conservatory. Before taking school money or using school equipment, read the university's intellectual-property policy for student work, because some policies claim work made with significant school resources.

### 9.6 The gates, applied to the whole app

Rule 2 says to write down what a fail looks like before testing. The new repo's first commit holds a `GATES.md` with the fail criteria below and no results. A check with no evidence is marked open and is never marked passed.

| Gate | Fails if (desktop wording) | Proposed design that passes | Before any test |
|---|---|---|---|
| 1.1 Healthier | anything stretches use past what the person came for: autoplay, a list without an end, notifications, badges, streak counters, "up next" | "Since you last looked", pull only (2.10); every list ends "That's everything." (4.10); focus rounds never start on their own (3.5); nothing plays without a press | open; conflicts below |
| 1.2 Freer | any file can't leave as its exact bytes, or the whole account can't leave in one action | Export everything with ⌘⇧E, even when signed out (2.9) | open; testable at the first slice |
| 1.3 Not addicted | anyone, owners included, is shown a count of other people's attention, or anything is ordered by engagement or sales | private saves; Add galaxy; newest first; the public Heat view shows records, never totals or comparisons (3.15) | open; read every API field, `/api/heat/public/:userId` included |
| 1.4 Business | see the criterion proposed below | the revenue lines in 9.4; the store fee is later | open; no route until commerce returns |
| 2.1 Complexity | for any view's verb, steps after ≥ steps before, counted by doing them | Heat: *knowing what to do next*. Space: *hearing a song apart*. Console: *making a song that comes apart*, and *cutting a film and its four-stem score on one clock* | open; count at each view's first build |
| 2.2 Flourishing | no positive answer after a month | *a term planned and kept, and a song finished that comes apart*; Heat's own records are the month of evidence | open |
| 2.3 Freedom | Wi's (a)–(c), plus (d): a session or export needs WWAV's server to open | offline first (2.8); files play in `ffmpeg` and the pack tools; the local MCP helper needs no network (8.8) | open |
| 2.4 Being & body | chapter 2's desktop bars: body text 7:1 and secondary 4.5:1, 44 × 44 pt targets, every gesture on a key, nothing moves while nothing plays, nothing plays without a press | the starfield moves only while something plays (4.4); the app makes no interface sounds (7.7) | open; dense rows need their own rule (3.19) |
| 2.5 Craft | an independent review finds a bug that is left unfixed; the tests aren't green; one plugin can take the app down | the audio engine runs as a separate process and restarts with the session reloaded (**Decided**; 8.1, 8.3) | open |
| 2.6 Wisdom | no written list of what was left out | 9.7, plus every chapter's own table | passes once this document is committed |
| 3.1 Immortal | scored 1–10 | the value sits in files (RIFF/WAVE from 1991, ISO BMFF from 2001, JSON); the web UI dies first, then plugin formats | unscored |
| 3.2 Invisible | seconds spent thinking about the tool | proxy: presses from opening a `.wwav` in Finder to hearing it apart (target 2: double-click, Space) | unscored |
| 3.3 Paradigm | the behaviour that stops | a song shared as a sealed, flattened file; a label standing between an artist and a listener | unscored |
| 3.4 Fresh | how far outside its category it sits | it adopts the planner, the DAW and the social network, and moves outside them by sharing one file across all three views | unscored |
| 3.5 Anti-entropy | what accumulates and what decays | lineage links and Heat's records accumulate; signed links and plugin compatibility decay | unscored |
| 4 Mission | fails until it happens | the school stages (9.5) | fails |

**A fail criterion for 1.4.** On the Wi wall, 1.4 stays open because no business runs on it. In the app it has no route yet. The criterion below counts from the first payment for a work, commerce is later (4.13), and nothing in the app takes such a payment. Until commerce returns, 1.4 stays open and is never marked passed. The wording is committed now, so that it can't bend to fit the numbers later:

> Fails if, 12 months after Space takes its first payment for a work, the sales fee, Pro, Founding and split packs together don't cover the server's running costs (Heroku, Postgres, R2, Replicate, Apple's developer program); or if any revenue line depends on a count of attention.

Hardware and education are left out of the sum on purpose: the platform has to carry itself.

**Where the sketch and the gates disagree.** In each row, the Proposed default is the design that passes the gate, and each row is **Open**, with the recommendation shown. The last row is the exception: its switch is **Decided**, and its rule is **Proposed**.

| Sketch or earlier WWAV | Gate | Proposed default | Recommendation | See |
|---|---|---|---|---|
| Habit streaks | 1.1 names streaks | a record that only grows: "Done 41 days since August 26" | a streak counter as a per-habit setting, off by default | 3.9 |
| Deadline reminders | 1.1 | none | one alarm you set yourself, off by default | 2.10 |
| Likes | 1.3 | private saves, never counted | the default | 4.11 |
| Follows and follower counts | 1.3 | Add galaxy, kept private | if anything, a single line such as "LMY added your galaxy", never a number | 4.11 |
| Push notifications, badges | 1.1 | "Since you last looked" | the default | 2.10 |
| Popularity and trending | 1.3 | newest only | the default | 4.10 |
| "Heat is like a facebook typa thing": anything can be made public, grades included | 1.3, and FERPA for grades | everything private by default; the **Public** switch makes one item public at a time; items go public, never totals or comparisons | the switch is off by default and no tool sets it; a grade's switch says plainly what it does | 3.15 |

Six rows left with the features they were about. "Walk around infinitely", the shop's ambiance, Mii-like crowds, drop progress bars and paid promotion belonged to the walkable shop. They wait for commerce, and are asked again if it returns. Fuel economy and aimed comets went with the game layer (4.14).

### 9.7 What Wi_WWAV doesn't do, and why

| Left out | Why |
|---|---|
| Ads | An ad sells a person's attention to someone else, and the business has to work without selling attention (gate 1.4). |
| Selling or sharing data | A planner and a gradebook are private by default, and grades are FERPA records. Showing a grade is the student's own choice, one grade at a time. |
| Paid placement, boosts, featured slots | Money decides what a file costs, not who sees it. |
| Likes, plays, views, follower counts | "A count of other people's attention invites checking and comparing." |
| Rankings, "For You", trending | "Nothing decides for you what's worth seeing." |
| Notifications and Dock badges | Anything that reaches out keeps a person past what they came for (gate 1.1). |
| Streak counters by default | A streak that can break pulls you back to protect a number instead of to do the thing. |
| A paywall on export or on your own files | "Never gate leaving with your own work." |
| DRM | A file has to play in tools that know nothing of WWAV (gate 2.3). |
| The Mac App Store | Its sandbox blocks loading other developers' plugins, and Apple takes a cut of every purchase made inside it. |
| Credits or any in-app currency | A currency between a person and their money hides what things cost, and it can be paid in attention. A split pack isn't one: it buys one service, cloud splits, at a dollar price, and keeps until used. |
| A lower fee or better placement for Pro | The fee is the same for everyone, so paying never buys reach. |
| Purchases that expire | Money already paid shouldn't run out on a timer. |
| Rights to your masters | WWAV hosts and delivers your work; taking your masters is what labels do, and WWAV exists against that. |
| A device that needs the app | Mi-WWAV plays its discs (USB flash sticks in an acrylic case) without an account. The app is not a key to the device. |
| Accounts under 13 | The birthdate gate keeps them out (2.14). Younger students with a teacher belong to class systems, which are later. |
| Selling inside the app now: a walkable shop, the store fee as a revenue line, drops and preorders, and commissions through the store | "Eventually the space will hold commerce instead of having that be a 4th place." Commerce is later, inside Space (4.13). Cut on 7 Oct 2026 to narrow v1. |
| Selling the hardware, or writing PRANA discs, from the app | "PRANA disc not ready yet, so definitely not writing software yet." |
| Claude as a metered or sold feature | The app calls no model. Claude runs in the person's own Claude, through MCP, so there is nothing to meter or sell (2.11). |
| Google verification and a Gmail stage in the school plan | Claude reads mail through its own Gmail connector, so the app never asks Google for it (3.10). |
| Class systems and cohorts running in Space | Later, and not designed now. Cut on 7 Oct 2026 to narrow v1. |
| Challenges, such as v4's `wwav-2026` | Chapter 4 carries no contests, and the game layer is cut (4.14). Not designed now. |


## 10. Roadmap, open decisions and glossary

This chapter covers four things: the order things get built in, what the first two weeks must prove, every decision still open, and what the words in this document mean. The founder settled the build order and the estimates in conversation (**Decided**). The cut of 7 Oct 2026 changed what some stages hold, so an estimate that no longer matches its stage is marked **Proposed**. What each stage contains, and how each one is checked, is **Proposed**.

### 10.1 Build order

The order follows the file. Every view reads and writes `.wwav` and `.swav`, so the format spine comes first. Each later stage works on its own if work stops there.

| Stage | What exists at the end | Estimate |
|---|---|---|
| 0. Thin slice | the seven items in 10.3, end to end, passing on a second Mac | 1–2 weeks (**Decided**) |
| 1. Shell and library | the window, view switcher and Now strip; `~/Music/Wi_WWAV/` with import, reader verdicts, tags and pins; ⌘K, ⌘⇧N and labelled ⌘Z; sign-in through the browser; the upload queue; Export everything with its offline `index.html` (see 2); the Tauri updater with `/desktop/latest.json` (8.10; **Proposed**) | grows out of the slice, alongside stage 2; not estimated on its own |
| 2. Heat, with the MCP server | Today with the time column, Plan my day and the Pomodoro timer; Tasks, Calendar, Grades, Habits and Mail; spaces; the Brightspace and other iCal feeds; the artifact's records imported with every id kept (see 3.16); quick capture, notes and the weekly review (see 3.14); `wi-mcp` with its eight tools and the journal's Claude labels (see 3.13 and 8.8). Heat records are kept on the Mac, and sync arrives with stage 5 (see 8.7) | about 1 week for Heat (**Decided**); about 1 week more for the MCP server (**Proposed**) |
| 3. Console, audio | audio, instrument and stem-group tracks; VST3 and AU effects and instruments; MIDI and the piano roll; takes, one per pass; the four stem buses and the six built-in effects (see 5); the `.wwavsession` package with autosave and recovery (see 6.5); `.wwav` export; the upload queue sends exports to your library on mi-wwav.com | 2–4 months to finishing a song (**Decided**; the cut leaves less in this stage: no sampler, no comping, no Make a disc, and splitting has moved to stage 6) |
| 4. Console, video | basic cutting on video tracks, proxies for 4K, the viewer on the engine's clock, `.swav` export (see 5.11) | about 1 month, overlapping stage 3 (**Proposed**; the founder's 1–2 months was set when video held grade, titles and generators too) |
| 5. Space | the sky in three.js, the same on every machine; the planet player with level, mute and solo on the four moons, and ↑ Push; the four media to view; suns and letters, with the public Heat view and Heat sync; lineage and the family tree; Add, Add galaxy and the Saved shelf; the message door; Newest; "Since you last looked"; Open in Console (see 4); Push to Space (see 5.16); the web face and its share pages (8.7; **Proposed**); buying Pro or a Founding seat in the browser, with the Stripe webhook setting `tier` (9.4; **Proposed**) | 2–4 weeks (**Proposed**; the founder's estimate was set before real 3D, Heat sync and the public Heat view were added) |
| 6. Demucs splitting | Split into stems (⌃⌘S) in a background worker: the htdemucs model, the stem group made in place, the CPU fallback and its warning (see 5.10) | about 1–2 weeks (**Proposed**) |
| 7. Windows | the same app on WebView2, ASIO or WASAPI, VST3, DX12 and Authenticode (see 8.10) | 3–6 weeks after the Mac (**Decided**) |

Worked most days, that is roughly 3–6 months to a v1 of three views (**Proposed**). It is the founder's **Decided** 4–8 months for four rooms, less the shop's 1–2 months. Stage 5 grows with real 3D, Heat sync and the public Heat view, so the top of the range is the likelier end. Two stages are new, the MCP server and splitting as its own stage; the cut took more out of the Console than they add, so the range stays.

**Why this order.**

- **Heat comes second** because it takes a week and gets used every day. Its records become the month of evidence gate 2.2 asks for. The MCP server goes with it, because the eight tools work on Heat's records and need no engine and no server (8.8).
- **The Console comes third** because it is the longest and riskiest stage, and Space has nothing new to carry until it exports. Audio comes before video: a song is finished first, and a film is cut to it. A `.swav`'s sound is the mix (5.13).
- **Space comes after the Console** because a planet is an export, and **Open in Console** needs a Console to open. The server routes Space needs already exist, and the public Heat view needs Heat's records.
- **Splitting comes after** because a song can be made from recorded and imported audio without it, and it runs in its own background worker, apart from the audio engine (5.10).
- **Windows waits** for a Mac v1, so it ports a finished app rather than a moving one.

**What could move the dates.** Three things were named as uncertain, and none can be estimated from here. Each has a way to make it smaller:

| Uncertain | Why | How it is made smaller |
|---|---|---|
| The plugin long tail | every plugin breaks a host in its own way, and the failures show up in use, not in a scan | the founder's own plugin folder is the test set from stage 0; the engine logs crashes per plugin; `crasher` and `hanger` test the recovery path (8.3, 8.12) |
| Audio stability | dropouts, devices changing mid-session, and take alignment that v5's own commit calls "untested on hardware" | the loopback test (100 recordings within ±1 sample), the 8-hour soak and the 200 random kills, each with its fail criterion written first |
| Feel | whether 250 ms, 400 ms, 210 ms, 8 pt and the 140 ms beat feel right can only be judged by playing | a session with the founder at the end of every stage, and every timing kept as one named constant |

**After v1.** These wait until the three views run, each for the reason its chapter gives:

- commerce inside Space: works that carry a price and are bought where they are (4.13), and the sales fee in 9.4;
- school class systems: a class as a solar system, assignments and younger students (4.14, 9.5);
- "Near you", which is cut for now (4.14);
- writing PRANA discs, and bringing a PRANA's remixes home, once the disc exists (5.18);
- a PRANA view in the app, once the disc exists (5.18);
- film stems (6.10);
- Gi's pattern file (6.11);
- the remote MCP server for claude.ai, after Heat sync (3.13, 8.8);
- an ARM64 Windows engine (8.10).

#### Left out of the plan, and why

| Left out | Why |
|---|---|
| A stage for Unquantized: the door, plaza and halls, booths, the counter and bag, the back room, List view, and the fixes that had to close before it took money | Unquantized is cut entirely. "Eventually the space will hold commerce instead of having that be a 4th place." Cut on 7 Oct 2026 to narrow v1. |
| Make a disc, and the disc round trip, in the Console stage | "PRANA disc not ready yet, so definitely not writing software yet." |
| Video grade, titles and generators in the video stage | Decided: video is basic, with no effects (5.11). Cut on 7 Oct 2026 to narrow v1. |
| Google sign-in and Google verification in Heat's stage | Claude reads mail through its own Gmail connector, and calendars come in as iCal addresses (3.11). |
| The game layer, constellations, the astronaut and the remix deck in Space's stage | Cut on 7 Oct 2026 to narrow v1 (4.14). |
| Parties ("Come with me"), commissions for every maker, and PRANA as a USB controller | They belonged to the shop or to cut Console features. Cut on 7 Oct 2026 to narrow v1. |
| A clip-launch decision | Closed: no clip-launch view in v1 (5.2). |

### 10.2 Who can check what

An agent working alone can build and verify much of the app on Linux. Some checks need a Mac, and some can only be judged by the founder. When an agent reaches one of those, it writes the question into `docs/QUESTIONS.md` and moves on to other work rather than guessing (8.13).

| Area | Without the founder's Mac (Linux CI, an agent alone) | Needs a Mac | Only the founder can judge |
|---|---|---|---|
| Files | the Rust `.wwav` and `.swav` writers, byte for byte against `wwav_pack.py` and `swav_pack.py`; one verdict from all four readers; a session that reopens identical | the Finder package icon; double-click opening a `.wwav` as a planet | whether 44.1 kHz holds into 1.0 (6.7) |
| Audio | golden renders of our own sessions (8.12); Surge XT as VST3; `delay-n`; 200 kills with `crasher` and `hanger` | AU hosting; CoreAudio and CoreMIDI devices; the loopback latency test (an interface and a cable); the 8-hour soak on the reference M1 Air | whether a song can be finished in the Console; how the founder's own plugins behave |
| Video | frame-indexed export maths; FFmpeg software decode | VideoToolbox decode and encode; the `CAMetalLayer` presenter; the flash-and-click test, with two tracks stacked | whether cutting feels like cutting |
| Views | the web UI in Playwright against `mock-engine`; Heat's maths (heat, Plan my day, grades); Space's layout hashes; journal undo-all and redo-all | WKWebView behaviour; Keychain; Force Touch; Reduce Motion; 60 fps on the reference machine | gesture timings; Lucida or Inter; 17 or 19 pt at the founder's desk |
| MCP | `wi-mcp` over stdio against a test library: every tool called, one journal entry per write, a repeated `source_id` making no second row, a tool switched off refused, 200 random kills, 1,000 interleaved writes with the app running (8.12) | the same suite nightly; Claude Desktop and Claude Code starting the helper from the app bundle, and the lines in Settings → Claude staying right after the app moves | whether Claude's estimates are worth accepting; how the tool descriptions read in a real conversation |
| Public Heat view | a private record, grade or note never appears in `/api/heat/public/:userId`; a public record shows only its listed fields; switching back removes its copy; a Now making line disappears at its `clearsAt` with the Mac off; no response carries a count (8.12) | — | whether the view says enough and no more; which grades the founder is willing to show |
| Server | multipart parts, Heat sync and its public copies, and `tier` set by both Apple's and Stripe's paths, against test Postgres and an R2 stand-in, as Wi's gates were run | — | prices, the sales fee |
| Accounts | the iCal parser against a stored raw feed | — | the founder's Brightspace link; Stripe live keys; the Apple Developer account and its Developer ID |
| Shipping | the universal build script | signing, hardened runtime, entitlements, notarization, and Gatekeeper on a second Mac | when anyone else gets a copy |
| Gates | every fail criterion, written before testing | gate 2.4's measurements at 1024 × 680 and 1280 × 800 | gate 2.2's month of use; gate 4's question asked in person; every decision in 10.4 |

### 10.3 The first thin slice

The slice proves the architecture with the least of each part: two processes, the shared clock, one plugin, one picture, both formats and a signed build. These are the parts that cost the most to change later. Each item's fail criterion goes into `docs/PLAN.md` before its code is written. The slice is done only when all seven items pass on a second Mac that has never built the app.

| # | Item | Fails if |
|---|---|---|
| 1 | The app opens | a cold launch on the reference machine takes over 1.5 s to show the window, or ⌘1–⌘3 doesn't switch between three empty views |
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

**What it leaves out:** Heat and its MCP helper, Space, the library beyond one file, local splits, instrument tracks and the piano roll (MIDI and instrument plugins still arrive at the start of stage 3, as **Decided**, and the slice's engine protocol and session format carry MIDI events from the first commit), and Windows. Each is a later stage. The slice exists to learn early whether the two processes, the clock and signing behave.

### 10.4 Open decisions

This list holds every **Open** item in chapters 1–9, with duplicates merged. Each goes into `docs/QUESTIONS.md` with its recommendation. If a stage reaches one before it has an answer, the stage builds the recommendation as written and logs it in `docs/DECISIONS.md`, so no stage waits.

#### The whole app

| # | Decision | From | Recommendation |
|---|---|---|---|
| 1 | Hyphen or underscore: Wi-WWAV or Wi_WWAV | 1.5, 9.3 | Use the underscore for the app and the hyphen for the people in letters. Domains keep the hyphen either way, because hostnames can't contain underscores. |
| 2 | Which "v3" the sketch means | 1.5, 4.1 | Read "v3" as "the iPhone app", leave the archive's numbering alone, and give Wi_WWAV its own archive entry. |
| 3 | Copy or reference on import | 2.5 | Copy songs, films and anything under 2 GB, so the library and its export are complete. Offer "Leave in place" for camera folders and long raw video. |
| 4 | Using the app without an account | 2.14 | Yes. Heat, the library and the Console are fully local. Space can be looked at, but not published to or pushed from. |
| 5 | Reminders, deadline alerts and Dock badges | 2.10, 3.19, 9.6 | Never notify or badge about what other people do. Allow one alarm you set yourself on a single task or focus session; it is off by default and fires once. |
| 6 | One sans or two | 7.4, 7.12 | Lucida Grande (Lucida Sans Unicode on Windows) in the desk and Inter elsewhere; recheck on the Windows build. |
| 7 | Reading size: 17 or 19 pt | 7.9, 7.12 | Test both with the founder at their own desk, and fix the number before any gate run. |
| 8 | Wi's adjusted stem hues | 7.12 | When the wall becomes the web face, move it to PRANA's exact hexes with ink rings. |

#### Heat

| # | Decision | From | Recommendation |
|---|---|---|---|
| 9 | The streak counter | 3.9, 3.19, 9.6 | By default, a record that only grows ("Done 41 days since August 26"). The counter becomes a per-habit setting, off by default, and the log is no longer pruned at 400 days. |
| 10 | Valence or LTI 1.3: when to ask URI | 3.19, 9.5 | Ship on the iCal feed and Claude's Gmail connector. Ask URI's Brightspace admins after the small-group stage, with the group's weekly reviews as evidence. |
| 11 | Control size for dense rows | 3.19 | Keep 44 pt for buttons, tabs and orbs. Before testing, write a separate rule for rows and grids: 24 × 24 pt hit areas (WCAG 2.2) and every action on the keyboard. |
| 12 | The "Now making" line | 3.19 | Build it, shown only once you've written one, and clear it when its task is done or after 7 days. Drop it if it starts to feel like a status to keep up. |
| 13 | Who Heat is for | 3.19 | One account first, with nothing hard-coded to a school, so a classmate can use it next. |
| 14 | Whether private grade rows sync | 3.16, 8.9 | Off by default. When on, encrypt them on the Mac with a Keychain key, so the server holds only ciphertext. |
| 15 | The remote MCP server | 3.19, 8.8 | Build it after Heat sync, with the same eight tools and labels. A remote call writes to the synced copy, and the Mac journals it when it arrives, with the same "Undo Claude's…" label. Its `get_grades` returns only the grades you made public. |

#### Space

| # | Decision | From | Recommendation |
|---|---|---|---|
| 16 | Sky motion while nothing plays | 4.4, 7.6 | Still by default, with "Let the sky turn when it's quiet" in Appearance, off. |
| 17 | Which philosophy governs social features (likes, follows, counts, comments, notifications, popularity) | 4.11, 9.6 | Ship the defaults that pass the gates: Add, Add galaxy, replies as works, pull-only, newest first. If one returns, make it a single "LMY added your galaxy" line, never a number. |
| 18 | Unpublishing a work others have forked | 4.12 | It leaves the sky. Earlier forks keep playing its stems, and their trees read "withdrawn by its maker". |
| 19 | Writing's file format | 4.7, 6.11 | Markdown with `wmet` and `wlin` keys in its front matter (`ri: "0.1"`), until a page must carry its images in one file. `.rwav` stays a placeholder. |
| 20 | Gi's pattern file | 6.11 | Name it (placeholder `.gwav`) when a Gi_cro_WWAV prototype cuts its first piece. |
| 21 | Where pages and galleries are made | 4.7 | v1 shows galleries that already exist and makes none. A gallery maker waits until fashion has a format of its own. Pages are written in the suns' block editor. |

#### Console and files

| # | Decision | From | Recommendation |
|---|---|---|---|
| 22 | Metronome on for new sessions | 5.4, 7.7 | Off, so a recording starts as free as you played it. |
| 23 | One process per plugin | 5.7, 8.3 | Not in v1. Log a month of engine crashes per plugin, then decide. |
| 24 | What a local split costs | 5.10, 9.4 | Free and unmetered. The paid cloud split stays for phones and the web. |
| 25 | A 48 kHz or 24-bit `.wwav` | 5.13, 6.7 | Keep 44.1 kHz, 16-bit for all of 0.x and convert on export, saying so. Decide both changes together for 1.0, once PRANA's hardware is measured. |
| 26 | MP3 320 and a plain MP4 export | 5.13 | Not in v1. A `.swav` already plays as an MP4. |
| 27 | Versions and identity | 5.16, 6.8 | Keep versions on the server under one `song_id`. Add an optional `version` to `wmet` and a `parent_version` to `wlin` in 0.2. |
| 28 | Thin remixes | 6.9 | Thick files wherever a file leaves. Thin storage inside the library and R2, keyed by sha256. |
| 29 | Film stems for Si_WWAV | 6.10 | Build them after one film has been cut in the Console and someone has asked to take a film apart. |

#### Engineering and shipping

| # | Decision | From | Recommendation |
|---|---|---|---|
| 30 | `allow-unsigned-executable-memory` for copy-protected plugins | 8.10 | Add it only if a plugin in the test set fails without it. |
| 31 | Intel-only plugins | 8.10 | Offer "Open the engine under Rosetta" per session, as a bridge. |
| 32 | Minimum macOS | 8.10 | macOS 13. |
| 33 | A self-hosted Mac mini CI runner | 8.12 | Add one once macOS runner minutes pass about $50 a month. |
| 34 | The `formats/` submodule | 8.13 | A sparse checkout of Mi-WWAV now. Split the four folders out only if clone times start to hurt. |

#### Business and school

| # | Decision | From | Recommendation |
|---|---|---|---|
| 35 | One desktop price list | 9.4 | A free plan (all local features, 10 GB hosted, 10 cloud splits); Pro at $7.99 a month or $69.99 a year; Founding at $199.99 once, seats 1–500; a split pack at $9.99 for 50; one `tier` entitlement everywhere. |
| 36 | Expiring split packs | 9.4 | Packs keep until used. |
| 37 | The device's price | 9.4 | Write a price ceiling into the campaign plan before step 4, and announce it in a letter to Wi-WWAV. |
| 38 | Money from the school | 9.5 | No-equity grants and competitions, leading with gate 4. Read the university's IP policy for student work first. |
| 39 | A fail criterion for gate 1.4 | 9.6 | Adopt it: the gate fails if, 12 months after Space's first payment for a work, the sales fee, Pro, Founding and packs don't cover server running costs, or if any line depends on a count of attention. It has no route until commerce returns. |

### 10.5 Glossary

| Term | Meaning | See |
|---|---|---|
| `.gwav` | Placeholder name for Gi_WWAV's pattern file: SVG pattern pieces with identity in their metadata (**Open**) | 6.11 |
| `.rwav` | Placeholder name for a Ri_WWAV writing file; until it exists, writing is Markdown with `wmet` and `wlin` front matter | 6.11 |
| `.swav` | A film as one file: any ISO BMFF film (MP4, MOV) with `wmet` and `wlin` boxes appended; version 0.1 exists | 6.2 |
| `.wwav` | A song as one file: a RIFF WAVE whose `data` chunk is the master, with four stems in `wstm`, plus `wmet`, `wlin` and, for remixes, `wrmx`; 44.1 kHz, 16-bit; version 0.1 exists | 6.1 |
| `.wwavsession` | A Console session as a package: `session.json`, `media/`, `plugin-state/`, `renders/`, `journal/`, `cache/` | 6.5 |
| ↑ Push | Forks the mix you hear into the song's lineage without stopping playback; the fork owns no audio | 4.6 |
| ⌘K, ⌘⇧N, ⌘L | The command palette, quick capture, and the library drawer, in every view | 2.7 |
| Add | The private save that replaces the like; it goes to your Saved shelf and is never counted | 4.11 |
| Add galaxy | The private replacement for following; it feeds Newest and "Since you last looked" | 4.11 |
| Aqua | The classic Mac OS X look Heat wears: brushed metal, gel buttons, striped rows; its highlight `#3875D7` is the desk's one accent | 7.2 |
| AU, VST3 | The plugin formats the engine hosts: AU on the Mac, VST3 on both; CLAP and AAX are left out | 8.4 |
| Bio sun | The sun at the centre of your galaxy: your page, with your public Heat view under your bio blocks | 4.8 |
| Brightspace | D2L's learning system, used at URI; Heat reads its iCal feed now and Valence or LTI later | 3.11 |
| Built-in effects | PRANA's reverb, delay, distortion, tremolo, filter and master limiter, with one amount each; there is no built-in EQ, compressor or instrument | 5.9 |
| Case metal, deck metal | Heat's brushed metal (title bar, sheets) and MI-WWAV-OS's paler pinstriped metal (mixer strips, device cards) | 7.2 |
| Claude | Anthropic's model, run in the person's own Claude (claude.ai, Claude Desktop, Claude Code), which reaches Wi_WWAV through MCP. It estimates and drafts and never decides; the app calls no model and holds no key | 2.11, 3.12 |
| Clip | Any one thing in the library: a `.wwav`, a `.swav`, plain audio or video, an image or text (from MI-WWAV-OS) | 2.5 |
| Connected profiles | Heat and Space: your public Heat view crosses, and nothing private does | 1.3, 2.6 |
| Connected uploads | Console and Space: exports go up as exact bytes, and any song or film comes back as a session through Open in Console | 1.3, 2.6 |
| Console | The creation view (⌘3), and the software side of Mi_pro_WWAV | 5 |
| Cool, Warm, Hot, Overdue | Heat's four levels: under 0.34, from 0.34, from 0.70, and past due | 3.1 |
| Crater | The portfolio's Turrell-lit page, the source of the night's 760 ms light-wash and of its name face, Cormorant Garamond | 7.1, 7.4 |
| Current task | The one task that focus time is logged to; set with C | 3.5 |
| Decided, Proposed, Open, Exists today | This document's four status labels | front matter |
| Delay compensation | Automatic alignment of signal paths through plugins that report latency | 5.7, 8.4 |
| Demucs | The stem-splitting model (htdemucs, four stems); it runs locally in the Console and on Replicate for phones | 5.10 |
| Desk, night | The two visual registers: Heat and the Console's chrome, and Space | 7.2 |
| DISCMAN | The codename of the spring 2026 iPhone app (v3) | 1.5, 4.1 |
| DMG screen | A Game Boy-green readout behind glass in the Console | 5.2, 7.2 |
| Drop | How publishing works: drag a work onto a system or a view segment | 2.7 |
| Engine | `wwav-engine`, the separate JUCE process that owns audio, MIDI and plugins, and is the master clock | 8.1 |
| Estimate chain | A task's own estimate, else its type's average, else difficulty × 20 minutes | 3.1 |
| Export everything | One action (⌘⇧E) that writes every file, session and record, with an offline `index.html` | 2.9 |
| Family tree | One work's lineage laid out as its own system, the root at the centre and forks on rings | 4.9 |
| FERPA | The US law on education records; grades stay private by default | 3.15, 9.5 |
| Fold rule, fold check | How N tracks become four stems, and the export test that the stems sum to the master | 5.3, 6.6 |
| Fork | A work taken somewhere new, by anyone: a new id with a parent. Fork edges are facts, not claims | 4.9, 6.8 |
| Founding Member | $199.99 once, seats 1–500: Pro for life, a fixed number, and a place in line for the device | 9.4 |
| Galaxy | One person in Space: a bio sun and their solar systems | 4.2 |
| Galaxy chip | A 28 pt miniature of your own galaxy in the title bar; it opens your galaxy, your public Heat view and Settings | 2.1 |
| Gallery planet | A photo or fashion work: a lit sphere with up to 40 photos pinned on | 4.7 |
| Gates, the | The founder's written design audit (`wi/GATES.md`): 1 corruption, 2 good design, 3 great design (scored), 4 mission. Fail criteria are written before testing | 9.6 |
| Generation | How many forks a work is from its root (`wlin.generation`) | 6.1 |
| Get Info | Heat's inspector drawer for one record; it holds the Public switch | 3.1, 3.15 |
| Gi_WWAV | The garments family (fashion) | 1.2, 9.2 |
| Heat | The profile view (⌘1), for planning time, grown from the founder's claude.ai artifact of the same name | 3 |
| Heat algorithm | The rule that ranks tasks: v = 1 − days left ÷ runway, clamped to 0–1; 1.1 when overdue, 0.05 with no due date | 3.1 |
| JUCE | The C++ audio framework (version 8) the engine and the scanner are built on | 8.1 |
| Key colour | A work's colour: hue = ((pc · 7) mod 12) · 30 with A = 0; major hsl(h, 72%, 58%), minor hsl(h, 58%, 42%); night indigo, hue 232, when the key is unknown | 4.7, 7.3 |
| Kepler motion | Worlds move on real ellipses; in Space only while something plays | 4.4 |
| Letter | A page with a greeting and a sign-off; every devlog post is one ("Dear Wi-WWAV," … LMY) | 4.8, 9.3 |
| Library | The folder `~/Music/Wi_WWAV/`: `library.sqlite`, `media/`, `sessions/`, `trash/` | 2.5, 8.6 |
| Lineage | The family tree of works, carried in each file's `wlin`: "the only social graph" | 4.9 |
| Lineage link | A claim between works (influence, sample, collab, cover, custom) that waits for the other owner's **Agree** | 4.9 |
| LMY | Liam, the founder; how the letters are signed | 9.3 |
| Mail thread | A row in Mail made by Claude's `record_mail_thread`: subject, sender, time, state and a reason, never the body | 3.10 |
| MCP server | A server that speaks the Model Context Protocol so Claude can call its tools. Wi_WWAV is one, with eight tools | 2.11, 3.13, 8.8 |
| Message door | A message to one person. It opens once two people have added each other, with no read receipts and no unread badge | 4.11 |
| mi-wwav.com | The existing server, which stays: Heroku, Express, Postgres and Cloudflare R2 | 8.7 |
| Mi_cro_WWAV, Mi_pro_WWAV | Mi's pocket tier and its desktop-studio tier | 9.2 |
| Mi_WWAV | The sound family; PRANA is its first beta | 9.2 |
| Mi-WWAV | How the hardware line is written ("Mi-WWAV Beta 1") | 9.2 |
| MI-WWAV-OS | A separate handheld OS, simulated in Flutter and Rust; the app takes its grammar, timeline, undo journal and DMG screens | 2.7, 5.1 |
| Moon | One stem of a song, orbiting its planet; its distance from the planet is its volume | 4.6 |
| Newest | Space's one list: newest first, 30 at a time, then "That's everything." | 4.10 |
| Now making | A line on your sun naming what you're working on, shown once you have written one | 3.15 |
| Now strip | The 520 × 44 pt LCD in the title bar, with the task on the left and the track on the right | 2.2 |
| Open in Console | ⌘E on any song or film opens it as a session to tear apart; the file is never written to | 4.6, 5.14 |
| OUTPUT row | The arrangement's last row; select it and press Return to export | 5.13 |
| Page planet | A written work as a world: paper with ruled latitude lines | 4.7 |
| Pending grade | A grade notice with no score, made by Claude or by you; you type the score in | 3.8 |
| PKM | The founder's personal knowledge manager at `/admin/pkm`, the source of capture, recurrence and the weekly review | 3.14 |
| Plan my day | Heat's rule-based drafting of blocks into the day's free time | 3.5 |
| Planet | One work in Space: a song, film, page or gallery | 4.2 |
| Pomodoro | Heat's focus timer: 25-minute rounds and 5-minute breaks, with 15 minutes every fourth break | 3.5 |
| PRANA | Mi-WWAV Beta 1: a Teensy 4.1 stem player with four faders and a disc bay. Its C++ core runs the device, its simulator and the app's built-in effects | 5.9, 8.4 |
| PRISMON | The portfolio's three.js engine, which already runs the three.js version Space uses | 4.5 |
| Pro | The paid plan, which pays for what runs on WWAV's computers: cloud splits and hosting | 9.4 |
| Profile view, social view, creation view | Heat, Space and Console, each named for what you do there | 1.1 |
| Proxy | A half-resolution ProRes copy of footage over 1080p, used for editing | 5.11, 8.5 |
| Public Heat view | What anyone sees when they open your bio sun: your Now making line, the timelines of linked projects, and any record you switched public. It never shows a count or a comparison | 2.4, 3.15 |
| Public switch | The switch in Get Info on every record, off by default and available on grades too; no Claude tool can set it | 3.15 |
| Quantize | An act on selected MIDI notes (Q); each note keeps its played time, and ⌥Q returns to it. Audio is never quantized | 5.4 |
| Ri_WWAV | The words family (writing) | 9.2 |
| Ripple Creator | The server's release-plan writer (`rippleCreator.js`); Heat imports its plans as projects, and its rule "never invent metrics" is written into the MCP tool descriptions | 3.15 |
| Rule 2 | "Write down what a fail looks like before testing" | 9.6 |
| Runway | How many days before a due date heat starts rising: 2 × difficulty + 1 | 3.1 |
| Screening room | Where a film plays in Space: it fills the stage, and it is for viewing only | 4.7 |
| Secondary act | The one extra action each screen has, on ⇧Return | 2.7 |
| Sequence | An edit list, never media; a Console session is one | 2.5, 5.12 |
| Show / Keep private | The two buttons on the sheet that links a project or sets a Now making line: **Show** puts it on your public Heat view, and **Keep private** links it and shows nothing | 2.6, 3.15 |
| Si_WWAV | The sight family (film) | 9.2 |
| Since you last looked | The pull-only list at the top of Space: forks, links waiting on you, and new work and letters from galaxies you've added | 2.10 |
| SOLAR SYSTEM | The codename of WWAV v4 (web, summer 2026, `/summer_26`) and v5 (iOS): the galaxy model | 4.1 |
| Solar system | One project: a sun and up to 21 worlds, seven to a ring | 4.2 |
| `song_id`, `film_id` | 128-bit ids written as 32 hex characters; songs and films share one id space | 6.1 |
| Space | The social view (⌘2), named after v4's first tab | 4 |
| Spaces | User-defined filters that replace Heat's three fixed workspaces | 3.4 |
| Split, split pack | A Demucs run. Local splits are free, cloud splits are metered, and a pack of 50 costs $9.99 | 5.10, 9.4 |
| Stem | One of a song's four parts, always in the order vocals, drums, other, bass | 6.1 |
| Stem bus | One of the mixer's four buses; its output is that stem in the export | 6.6 |
| Stem colours | PRANA's vocals `#D23C2A`, drums `#F0B90B`, other `#2E9A55`, bass `#1F4E9E`, the same everywhere | 7.3 |
| Stem group | A Console track of four stem lanes, from a `.wwav` or a split | 5.3 |
| Stem lights | The four 8 pt lights in the Now strip and on a stem group's header, in PRANA's order and colours: filled is audible, a hollow ring is muted, a 2 pt outer ring is soloed | 2.2 |
| Stem player | WWAV's instrument; in Wi_WWAV, your skin for it wraps the Now strip's planet when nothing is loaded | 2.4 |
| Stem role | The stem a Console track folds into | 5.3 |
| Stripe | Takes the payment for Pro, Founding seats and split packs, in the browser | 9.4 |
| Sun | A page: a bio sun for a person, a project sun for a system | 4.8 |
| Take | One recording pass over a loop range; one take plays at a time, and takes are never comped | 5.6 |
| Tauri | The app framework (version 2): a Rust core, with the UI in the system web view | 8.1 |
| "That's everything." | How every list ends | 2.10, 4.10 |
| Thin remix | A remix stored without its own copy of the stems: "thick files, thin storage" | 6.9 |
| Tier | The one entitlement field the desktop app reads | 9.4 |
| Time block | A span of time in Today's time column, given to a task or a habit | 3.5 |
| ULID | The time-sortable id that names every media file and session | 2.5 |
| Undo journal | Every change as a labelled transaction, behind ⌘Z; a change Claude makes is labelled as Claude's | 2.7, 8.6 |
| Universe | Every galaxy on one spiral ("Everyone") | 4.2 |
| URI | The University of Rhode Island, the founder's school | 3.11, 9.5 |
| Valence, LTI 1.3 | D2L's REST API and the LTI standard: the route to real grades, registered by the school | 3.11 |
| Version | A new export of your own work: the same id, with the version number one higher | 6.8 |
| View | One of the app's three: Heat, Space or Console | 1.1 |
| Web face | www.wi-wwav.com as share pages and downloads | 1.5, 8.7 |
| wgpu | The Rust GPU layer that composites video (Metal on Mac, DX12 on Windows) | 8.5 |
| Wi | The existing private wall at www.wi-wwav.com (`wi/`): one account, every post a `.wwav` or `.swav`; Proposed to become the app's web face | 1.5, 8.7 |
| wi-mcp | The small Rust helper inside the app that Claude Desktop and Claude Code start to speak MCP over stdio; it has no window and no network | 3.13, 8.8 |
| Wi_WWAV | This app, said "we wave" | 1.5 |
| Wi-WWAV | The community of customers and fans: "we are the wwav" | 1.5, 9.3 |
| `wlin` | Lineage JSON: `parent_id`, `root_id`, `generation`, `creator`, `device_id` | 6.1 |
| `wmet` | Identity JSON: format version, `song_id` or `film_id`, title, artist, type, created; songs add bpm, key, frames and splitter | 6.1, 6.2 |
| `wrmx` | Remix settings JSON in a remix `.wwav`: levels, mutes, effects, pitch, speed, time and filters | 6.1 |
| `wstm` | The four interleaved stereo stems, aligned to 512 bytes | 6.1 |
| WWAV | World-Wide Audio-Visual, built on one idea: "every song comes apart" | 1.1 |
| WWAV Push | The JUCE plugin that gives a song a permanent link on its first export | 5.16 |
| `wwav-scan` | The short-lived process that checks plugins one at a time | 5.7, 8.1 |
| Zi_WWAV | The furniture family, left out of scope | 1.2 |

**Example names.** World Ending and More Love are labels of the founder's albums ("Listen to This When the World is Ending", "More Love Kills the Beast", `portfolio/src/albums.js`). Low Tide, glass hours, Ana, Tape Echo and Grand Piano are examples made up for this document. KeyStep stands in for any USB keyboard controller.

### 10.6 Sources

Every **Exists today** claim in this document comes from these sources, read in October 2026. The cut of 7 Oct 2026 is the founder's conversation, recorded in `docs/SCOPE_CUT.md`.

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
  - `server/routes/v2/`
- **MI-WWAV-OS:**
  - `MI-WWAV-OS/ARCHITECTURE.md`, `MI-WWAV-OS/proto/README.md`
  - `MI-WWAV-OS/engine/src/` (`model.rs`, `store.rs`, `ids.rs`, `api.rs`, `mixer.rs`)
  - `MI-WWAV-OS/app/lib/style/theme.dart`
- **WWAV Push:** `vst_plugin/`.
- **Money, social and Claude:**
  - routes in `server/routes/`: `purchase.js`, `connect.js`, `films.js`, `stems.js`, `foundingMember.js`, `splits.js`, `forks.js`, `cohort.js`, `rippleCreator.js`, `wi.js`
  - service in `server/services/`: `rippleCreatorService.js`
  - also `server/mcp/auth.js` and `server/utils/tier.js`
- **Portfolio:**
  - `portfolio/src/pkm/`, `portfolio/src/education.js`, `portfolio/src/albums.js`, `portfolio/src/crater.css`
  - `portfolio/src/components/Hub.jsx`, `portfolio/src/components/rooms/prismon/`
- **Notes and the rest:** `TODO_2.md`, `todo.md`, `assets/`.
