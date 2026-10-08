# The core's commands

How the web UI talks to the app's Rust core (`crates/wi-core`). The same
calls travel two ways:

- **In the app**, through one Tauri command, `core`, which takes
  `{cmd, args}` and returns the result or an error. Each window may call only
  the commands its capability lists (`docs/SPEC.md` 9.8); a window in no
  capability file, such as one that showed someone else's page, calls none.
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
  current view, so ⌘Z can name it ("Undo mark done"). The core never asks
  "Are you sure?": undo replaces it (8.10). A view is Learn, Space or the
  Console (`heat`, `space`, `console`: there are three, and no fourth), or
  `library` for the drawer over them. The commands still call it `room`
  (`{room}`), the journal's own name for where a change was made; the spec's
  word is view (docs/DECISIONS.md).
- Events are `{event, payload}`. Meters are not events: they arrive once per
  frame as raw bytes on their own channel (`meters`), the newest meter entry
  of `docs/ENGINE.md` 4.4.
- Nothing the UI shows as a count of other people's attention exists in any
  result (gate 1.3).
- The app holds no key and calls no model's API. Where it asks Claude (Mail's
  two jobs, a syllabus, the prompt box) it runs the person's own Claude Code,
  signed in as them, with only the tools that job names (docs/HEAT.md,
  docs/ASK.md).

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
| `history.get` | `{room}` | `{undo: "Undo move clip" \| null, redo: "Redo move clip" \| null, cant: "Can't undo a message." \| null}` |
| `history.undo` | `{room}` | `{label: "move clip"}` or error `nothing_to_undo` ("Nothing to undo.") |
| `history.redo` | `{room}` | `{label}` or error `nothing_to_redo` |
| `history.undoEntry` | `{txnId}` | `{label}` or error `cant_undo` ("This changed again since. Undo the later change first."). One entry out of order: Settings → Claude's list |

`room` is `heat`, `space`, `console` or `library` (anything else is
`bad_args`: "There is no view called 'kitchen'."). After any change the core
sends `history` with the new labels for every view. Changes that arrive from
the person's other devices are journaled where no view's ⌘Z acts, so a sync
never takes the redo they have waiting; an undo that would put back a record
such a change touched is held, and says it "changed on another device since".

### records (journaled documents for the views)

Learn's records (3.15) are documents of a `kind` with an `id`, kept in
`library.sqlite` and journaled. They sync with the account whichever view
wrote them (a milestone planned from Space is Learn's as much as one planned
in Learn), except private grades and courses, which stay on the Mac.

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
`verdict` is the reader's sentence, word for word: `wwav_pack.py info`'s for a
`.wwav` or a plain WAV ("the master only: no wmet and wlin, so a plain WAV",
or "not listed: the master isn't 44.1 kHz 16-bit stereo PCM"), `swav_pack.py
info`'s for a film. Other plain audio, which the reference can't read, says
"Plain audio comes in as master only." The import summary says it too.

### player (2.3): the one listening player

| cmd | args | result |
|---|---|---|
| `player.load` | `{clip}` | `{state}` |
| `player.play` / `player.pause` | — | `{state}` |
| `player.seek` | `{seconds}` | `{state}` |
| `player.stem` | `{stem, level? , mute?, solo?}` | `{state}` |

`state` is `{clip, title, key, bpm, playing, position, duration, stems: [{stem, level, mute, solo}], pausedFor: null \| "console" \| "film"}`.
A song runs at its own sample rate (44.1 kHz for a `.wwav`, a plain WAV's own),
and the engine's device follows it: `position` and `duration` are in seconds of
that song, and `engine.status`'s `sampleRate` changes when a song at another
rate loads.
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
| `publish.drop` | `{clip, target: {kind: "system", id}, label}` | `{queued}`, or error `already_placed` |
| `publish.queue` | — | `{waiting: [...], sentence}` |

A drop tags the clip with its system and sets `published_at`. The worker
uploads it, publishes it and puts it in the system, and a placement the server
refused for a moment is retried like an upload. A song is one world, so a drop
onto a second system is refused at once with `already_placed` ("'Low Tide' is
already a world in another system. A song is one world, so it can't be in
two."). Dropping a work again also starts a new attempt after a refusal, and a
relaunch does too. `waiting` counts works still to go up and works that are up
but not yet in their system.

Events: `status` `{area: "upload", sentence: "Uploading World Ending · part 14 of 27"}`.
⌘Z clears `published_at` while the upload is queued; after the server has it
`history.get` returns `cant: "Can't undo a publish. Unpublish 'World Ending'…"`.

### heat (3, 8.8)

Learn's reads and writes, its calendars and its Claude pane: `heat.snapshot`,
`heat.put`, `heat.patch`, `heat.delete`, `heat.done`, `heat.plan.*`,
`heat.focus.*`, `heat.capture.*`, `heat.public.set`, `heat.calendars.*`,
`heat.claude.*` and the rest. `docs/HEAT.md` lists each one's arguments,
result and refusals, and what the snapshot carries. Every write answers
`{..., undo}` with the Edit menu's text, and the core sends `heat`
`{kinds: [...]}` after any Learn change, the app's own, Claude's (`wi-mcp`)
or a sync's, so views refetch.

### ask, db, wiki

The prompt box on ⌘K (`ask.*`), the Database tab (`db.*`) and the Wiki tab
(`wiki.*`). `docs/ASK.md` lists each one's arguments, result and refusals.
`ask.send` sends `ask` events `{id, doing}` while Claude works. What Claude
would change is staged and made by `ask.apply`, as one journal entry. A
write through `db.*` to one of Learn's records is the `heat.*` command a
view would use, and sends the same `heat` event.

### export (2.9)

| cmd | args | result |
|---|---|---|
| `export.everything` | `{to, zip}` | `{path, files, bytes}`; progress events `export` |

### status

Event `status` `{area: "sync" \| "save" \| "upload" \| "room" \| "calendar", sentence}` drives
the status bar: "Saved on this Mac", "Synced 3:41 PM", "Offline. 2 works wait
to go up; they leave when you're back."

The views add their own areas (`heat.*`, `space.*`, `console.*`) under the
same conventions, each listed here when it is built.

## What the shell adds

The Tauri app (`app/src-tauri/README.md`) answers two more commands of its own
through the same `core` command, and sends two more events. The dev bridge has
none of them: a browser has no menus.

| cmd | args | result |
|---|---|---|
| `meters.listen` | `{channel}` | `{}`: meters arrive on that channel, as raw bytes |
| `shell.room` | `{room}` | `{}`: the view now showing (`heat`, `space`, `console`, or `library`), so the Edit menu names its undo and View ticks it |

Events `menu` `{action}` (`room.heat`, `room.space`, `room.console` on
⌘1–⌘3, `history.undo`, `history.redo`, `library.toggle`, `export.everything`,
`session.new`) and `open` `{clips} \| {sessions} \| {error}` (files opened from
outside).
