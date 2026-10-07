# The core's commands

How the web UI talks to the app's Rust core (`crates/wi-core`). The same
calls travel two ways:

- **In the app**, through one Tauri command, `core`, which takes
  `{cmd, args}` and returns the result or an error. Each window may call only
  the commands its capability lists (`docs/SPEC.md` 9.8); the checkout window
  calls none.
- **In development and tests**, through `wi-devbridge`, a WebSocket server
  that hosts the same core, so Playwright drives the real core (with
  `mock-engine` and `tools/mock-server` behind it) from Chromium.

`app/ui/src/bridge/` hides the difference: `call(cmd, args)` and
`on(event, handler)`.

## Conventions

- `cmd` is `<area>.<verb>`, lower case: `library.import`, `history.undo`.
- `args` and the result are JSON objects. Ids are ULIDs (clips, sessions,
  records) or 32-hex work ids (`song_id`, `film_id`).
- An error is `{code, message}`: `code` is a snake_case word the UI can
  branch on, `message` a sentence it can show as is ("A solar system holds 21
  worlds. Start another one.").
- Every mutation takes a `label` and runs in one journal transaction in the
  current room, so ⌘Z can name it ("Undo mark done"). The core never asks
  "Are you sure?": undo replaces it (8.10).
- Events are `{event, payload}`. Meters are not events: they arrive once per
  frame as raw bytes on their own channel (`meters`), the newest meter entry
  of `docs/ENGINE.md` 4.4.
- Nothing the UI shows as a count of other people's attention exists in any
  result (gate 1.3).

## Commands

### app

| cmd | args | result |
|---|---|---|
| `app.hello` | — | `{version, library, signedIn, platform, reduceMotion}` |
| `app.settings.get` | — | the settings object (2.13) |
| `app.settings.set` | `{patch}` | the settings object |

### history (2.7, 9.6)

| cmd | args | result |
|---|---|---|
| `history.get` | `{room}` | `{undo: "Undo move clip" \| null, redo: "Redo move clip" \| null, cant: "Can't undo a purchase." \| null}` |
| `history.undo` | `{room}` | `{label: "move clip"}` or error `nothing_to_undo` ("Nothing to undo.") |
| `history.redo` | `{room}` | `{label}` or error `nothing_to_redo` |

`room` is `heat`, `space`, `console`, `unquantized` or `library`. After any
change the core sends `history` with the new labels for that room.

### records (journaled documents for the rooms)

Heat's records (3.15) and the other rooms' local records are documents of a
`kind` with an `id`, kept in `library.sqlite` and journaled.

| cmd | args | result |
|---|---|---|
| `records.list` | `{kind, where?}` | `{records: [...]}` |
| `records.get` | `{kind, id}` | `{record}` |
| `records.mutate` | `{label, room, ops: [{op: "put" \| "patch" \| "delete", kind, id, value?}]}` | `{records: [...]}` (the records as they now stand) |

Event `records` `{kinds: [...]}` after any change, so views refetch.

### library (2.5)

| cmd | args | result |
|---|---|---|
| `library.list` | `{filter?, smart?, sort?, limit?, after?}` | `{clips: [Clip], next?}` |
| `library.search` | `{q, limit?}` | `{clips: [Clip]}` |
| `library.get` | `{id}` | `{clip, verdict, chunks}` |
| `library.inspect` | `{path}` | `{summary: "214 files: 38 .wwav, 12 .swav, 160 plain audio, 4 other. Plain audio comes in as master only."}` |
| `library.import` | `{paths, label}` | `{clips}`; progress events `library.import` |
| `library.tag` | `{ids, add?, remove?, label}` | `{clips}` |
| `library.colour` | `{ids, colour \| null, label}` | `{clips}` |
| `library.pin` | `{id, slot \| null, label}` | `{pins}` |
| `library.smart.save` | `{id?, name, rules, label}` | `{folder}` |
| `library.delete` | `{ids, label}` | `{}` (rows only; files stay) |
| `library.cleanup.preview` | — | `{files, bytes, sentence: "1.8 GB in 214 files"}` |
| `library.cleanup.run` | — | `{moved}` |
| `library.trash.empty` | — | `{deleted}` |

`Clip` is `{id, kind, title, artist, bpm, key, duration, colour, tags, pinned, verdict, published, sha256, bytes, created}`.

### player (2.3): the one listening player

| cmd | args | result |
|---|---|---|
| `player.load` | `{clip}` | `{state}` |
| `player.play` / `player.pause` | — | `{state}` |
| `player.seek` | `{seconds}` | `{state}` |
| `player.stem` | `{stem, level? , mute?, solo?}` | `{state}` |

`state` is `{clip, title, key, bpm, playing, position, duration, stems: [{stem, level, mute, solo}], pausedFor: null \| "console" \| "film"}`.
Events: `player` with the state on every change; `clock` ten times a second
with `{sample, hostTimeNs, rate, state}` (the shared-memory clock, for the UI
to extrapolate the cursor).

### engine (9.3)

| cmd | args | result |
|---|---|---|
| `engine.status` | — | `{state: "starting" \| "running" \| "restarting" \| "stopped", device, sampleRate, block}` |
| `engine.plugin.keepOff` / `engine.plugin.tryAgain` | `{device}` | `{}` |

Events: `engine.stopped` `{plugin, track, sentence: "The audio engine stopped. 'Tape Echo' on the track 'Keys' was running when it did. Restarting…"}`,
`engine.back` `{sentence: "Back. 'Tape Echo' is off until you turn it on. Changes made inside its own window in the last 41 s may be lost."}`.

### account (2.4, 9.7)

| cmd | args | result |
|---|---|---|
| `account.status` | — | `{signedIn, username?, galaxy?}` |
| `account.signIn` | — | `{signedIn, username}` (opens the system browser, waits on the loopback) |
| `account.signOut` | — | `{signedIn: false}` |

The token lives in the operating system's keychain, never in SQLite, logs
or results.

### publish and the upload queue (2.8)

| cmd | args | result |
|---|---|---|
| `publish.drop` | `{clip, target: {kind: "system" \| "shelf", id}, terms?, label}` | `{queued}` |
| `publish.queue` | — | `{waiting: [...], sentence}` |

Events: `status` `{area: "upload", sentence: "Uploading World Ending · part 14 of 27"}`.
⌘Z clears `published_at` while the upload is queued; after the server has it
`history.get` returns `cant: "Can't undo a publish. Unpublish 'World Ending'…"`.

### export (2.9)

| cmd | args | result |
|---|---|---|
| `export.everything` | `{to, zip}` | `{path, files, bytes}`; progress events `export` |

### status

Event `status` `{area: "sync" \| "save" \| "upload" \| "room", sentence}` drives
the status bar: "Saved on this Mac", "Synced 3:41 PM", "Offline. 2 works wait
to go up; they leave when you're back."

Rooms add their own areas (`heat.*`, `space.*`, `console.*`, `store.*`) under
the same conventions, each listed here when it is built.
