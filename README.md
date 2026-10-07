# Wi_WWAV

One desktop app, said "we wave", with three views: **Heat** (⌘1, the profile
view) plans your time, **Space** (⌘2, the social view) holds people and their
work as a universe in 3D, and the **Console** (⌘3, the creation view) makes
songs and films on one clock. Every view reads and writes the same two files:
a song is a `.wwav` and a film is a `.swav` (`docs/SPEC.md` chapter 6). The app
calls no model and takes no money: Claude works on Heat through the app's MCP
server (`docs/HEAT.md`), and commerce comes later, inside Space
(`docs/SCOPE_CUT.md`).

- `docs/SPEC.md` is the design, written before any code.
- `docs/GATES.md` holds the founder's four gates, with fail criteria written first.
- `docs/PLAN.md` lists the milestones in build order, each with its fail criteria and status.
- `docs/QUESTIONS.md` lists every open decision with its recommendation.
- `docs/DECISIONS.md` logs what was decided while building, and why.
- `docs/HEAT.md` says how Heat's store, commands and MCP helper fit together.

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

The window opens on Heat. Your library is made at `~/Music/Wi_WWAV/` the first time.

**What this build holds.** It is being finished today (7 Oct 2026): Heat in full, plus the frames of Space and the Console. The checklist at the top of `docs/PLAN.md` says what has landed so far. The audio engine isn't built by these steps (it needs JUCE and CMake), so the Console says the engine is off; Heat never needs it.

**Calendars.** Settings → Heat: paste your Brightspace calendar's iCal link (Brightspace → Calendar → Subscribe). It stays in the Keychain.

**Claude.** Build the helper once, from the `Wi_WWAV` folder in a second terminal:

```sh
cargo build -p wi-mcp
```

Then open Settings → Claude in the app. It shows the exact line for Claude Code and the block for Claude Desktop's config, with this helper's path filled in. Restart Claude Desktop after adding it. Ask Claude "What's on my list today?" to check that it works.
