# Wi_WWAV

One desktop app, said "we wave", with three views: **Learn** (⌘1, the profile
view; WWL, Wi-WWAV-Learn) plans your time, **Space** (⌘2, the social view) holds people and their
work as a universe in 3D, and the **Console** (⌘3, the creation view) makes
songs and films on one clock. Every view reads and writes the same two files:
a song is a `.wwav` and a film is a `.swav` (`docs/SPEC.md` chapter 6). The app
calls no model and takes no money: Claude works on Learn through the app's MCP
server (`docs/HEAT.md`), and commerce comes later, inside Space
(`docs/SCOPE_CUT.md`).

- `docs/SPEC.md` is the design, written before any code.
- `docs/GATES.md` holds the founder's four gates, with fail criteria written first.
- `docs/PLAN.md` lists the milestones in build order, each with its fail criteria and status.
- `docs/QUESTIONS.md` lists every open decision with its recommendation.
- `docs/DECISIONS.md` logs what was decided while building, and why.
- `docs/HEAT.md` says how Learn's store, commands and MCP helper fit together.
- `docs/ASK.md` says how the prompt box on ⌘K, the Database tab and the Wiki tab are built.

## Getting the sources

```
git clone https://github.com/Liam-made-young/Wi_WWAV
cd Wi_WWAV
tools/bootstrap.sh   # formats/: Mi-WWAV at its pinned commit, sparse (private: needs access)
```

The app itself builds without `formats/`; only the format parity tests need it.

## Run it on your Mac

This builds the app from source and opens it. The first build takes about 10 minutes; later ones take seconds. Nothing here is signed yet, so this is the developer build, not a download.

**Once:**

1. Xcode's command-line tools: `xcode-select --install`
2. Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`, then open a new terminal.
3. Node 22: from nodejs.org, or `brew install node@22`.
4. The Tauri command: `cargo install tauri-cli --version "^2" --locked`

**Get the code and run it:**

```sh
git clone https://github.com/Liam-made-young/Wi_WWAV
cd Wi_WWAV
git checkout claude/relaxed-cori-x2igz9
npm ci --prefix app/ui
cd app && cargo tauri dev
```

The window opens on Learn. Your library is made at `~/Music/Wi_WWAV/` the first time.

**What this build holds** (7 Oct 2026; `docs/PLAN.md` has the details):

- **Learn** (⌘1): Today with the time column, Plan my day and the Pomodoro timer; Tasks; Calendar; Grades; Habits; Mail; Notes; Get Info with the Public switch; capture (⌘⇧N). Not built yet: the Settings panes for Learn, Claude and Privacy, sharing to your galaxy and the public view page, the space sheet and the weekly review.
- **Space** (⌘2): the frame, drawing a sample sky in 3D.
- **Console** (⌘3): the frame, listing your library. The audio engine isn't built by these steps (it needs JUCE and CMake), so it says the engine is off; Learn never needs it.

**Calendars.** Settings → Learn (⌘,) takes your Brightspace calendar link and any other calendar's iCal address. Each is kept in the Keychain. In Brightspace: Calendar → Subscribe, and copy the link.

**Commitments.** Your classes, shifts and commutes go in from Calendar's sidebar (Schedule): by hand, by pasting a schedule, by dropping a photo of one, or from an `.ics` file or address. Plan my day and P then keep clear of them, of the travel time around them and of sleep, and Today says what the day has left. `docs/COMMITMENTS.md` has the details.

**Notes.** Notes are markdown files in `~/Music/Wi_WWAV/Notes`, which any editor can open; Learn keeps them in step with what it shows. To get a notebook page in from an iPhone, add the Shortcut in `tools/shortcut/` (its README says how), then take a photo, tap Share, and tap Send to Wi-WWAV. On the Mac, drop or paste an image on the Notes tab, or put it in `Wi-WWAV Inbox` in iCloud Drive. The page is read on this Mac (the first time, the app builds its reader with `swiftc`, which takes a few seconds and needs Xcode's command-line tools) and filed to the class it was taken in. `docs/NOTES.md` has the details.

**Mail.** The app holds no password for any mailbox. It runs your own Claude Code (`claude`, signed in, with its Gmail connector on) to read mail every half hour while it is open and to send what you write. Add your addresses in Settings → Learn → Mail accounts: one is the mailbox Claude's Gmail connector is signed in to, and any other is forwarded into it at a plus address. In Mail you can read a thread, reply, compose, archive, mark unread, search and sort; "Read mail now" reads at once, and the Outbox shows what is waiting or failed.

**Claude.** Build the helper once, from the `Wi_WWAV` folder in a second terminal:

```sh
cargo build --release -p wi-mcp
```

Then tell Claude where it is (Settings → Claude shows these lines with the helper's real path). Use the full path of your `Wi_WWAV` folder.

- Claude Code: `claude mcp add --scope user wi-wwav -- /path/to/Wi_WWAV/target/debug/wi-mcp`
- Claude Desktop: in `~/Library/Application Support/Claude/claude_desktop_config.json`, add `{"mcpServers": {"wi-wwav": {"command": "/path/to/Wi_WWAV/target/debug/wi-mcp"}}}`, then restart Claude Desktop.

Open the app once first so your library exists, then ask Claude "What's on my list today?".
