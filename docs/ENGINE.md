# The engine contract

How `Wi_WWAV.app` and `wwav-engine` talk (`docs/SPEC.md` 9.1–9.3). These
programs implement it and must agree byte for byte:

| Program | Side | Where |
|---|---|---|
| the app's Rust core | client | `crates/wi-core` (`engine` module), over `crates/wwav-wire` |
| `wwav-engine` | server | `crates/wwav-engine` (Rust; the one the app runs) |
| `wwav-engine`, the first cut | server | `engine/` (JUCE 8 + `prana/core`; kept, not built on) |
| `mock-engine` | server, for tests | `crates/mock-engine` |
| `wwav-engine-cli` | client, for a terminal | `crates/wwav-engine-cli` |

"Anything provable in a terminal is true in the app": every op below can be
sent from `wwav-engine-cli`.

Protocol version: **1**. Shared-memory layout version: **1**.

---

## 1. Starting the engine

The app creates two things before it starts the engine, so both outlive it:

1. **A shared-memory region** (§4), named `/wwav-<app pid>-<n>` (POSIX
   `shm_open`; a named file mapping on Windows), mode 0600.
2. **A private directory** `$TMPDIR/wwav-<app pid>/`, mode 0700, for the
   socket.

Then it runs:

```
wwav-engine --socket <dir>/engine-<n>.sock --shm <name> [--device <name|null>]
            [--rate 48000] [--block 128] [--cache <dir>] [--test]
```

`<n>` is the region's (`/wwav-<app pid>-<n>`), so every engine the app
starts has a socket and a region of its own.

- The engine **listens** on `--socket` (a Unix domain socket, mode 0600; a
  named pipe on Windows) and accepts one client at a time.
- The engine's **stdin is a pipe from the app**. When it reads end-of-file,
  the app has gone: the engine stops audio and exits within 1 s. Nothing else
  is read from stdin.
- `--device null` opens no hardware. A timer thread calls the graph every
  block at the nominal rate, so playback, the clock and meters behave as with
  a real device. Tests and CI use it.
- With no `--device`, the engine opens the system's default output. A
  device that won't open is said on stderr and the same timer runs unnamed
  (`hello`'s `device` is null), so there is always a clock; `device.open`
  can try again.
- `--cache` names the folder for audio made ready to play (§3.3). Without
  it: `$TMPDIR/wwav-engine-cache`.
- `--test` enables the `debug.*` ops (§3.9). Without it they return
  `{"code": "unknown_op"}`.
- On macOS the engine is `Contents/Helpers/wwav-engine.app` with
  `LSUIElement`, so it has no Dock icon and its plugin windows belong to a
  real application.

The engine prints one line to stdout when it is listening:
`wwav-engine listening <socket path>`, and nothing else on stdout. Logs go to
stderr.

## 2. Framing

Every message in either direction is a frame:

```
u32 little-endian   length of the payload in bytes (1 .. 16 MiB)
payload             one JSON object, UTF-8
```

A frame over 16 MiB, a payload that isn't exactly one JSON object (RFC 8259:
valid UTF-8, nothing after it) or that nests more than 127 levels deep
(serde_json's limit), or a short read closes the connection. Large data
(plugin state over 256 KB, audio) is never sent in a frame: it goes through
a file whose path is in the frame.

Three kinds of payload:

```
-> {"id": 412, "op": "param.set", "args": {"node": "01JC5R…", "param": "gain_db", "value": -3.0}}
<- {"id": 412, "ok": true, "result": {}}
<- {"id": 413, "ok": false, "error": {"code": "no_such_node", "message": "No node 01JC5R…"}}
<- {"ev": "plugin.latency", "node": "01JC5T…", "samples": 2048}
```

- `id` is a positive integer chosen by the client, unique per connection.
  Every request gets exactly one response with the same `id`, in any order.
- `args` may be omitted when an op takes none. `result` is always an object.
- An event has `ev` and no `id`. Events may arrive at any time, between
  responses.
- Error codes are snake_case words. Messages are sentences a person can read.

Commands run at tens a second at most. Fast data (the clock, meters, peaks,
input audio, the crumb) never travels as frames: it is in shared memory (§4).

## 3. Ops

### 3.1 Handshake and health

| Op | Args | Result |
|---|---|---|
| `hello` | `{"protocol": 1, "client": "wi_wwav 0.1.0"}` | `{"protocol": 1, "engine": "wwav-engine 0.1.0", "pid": 1234, "sample_rate": 48000, "block": 128, "device": "null", "shm_layout": 1}` |
| `ping` | — | `{"t": <engine monotonic ns>}` |
| `shutdown` | — | `{}`, then the engine stops audio and exits |

The Rust engine's `hello` also carries `"features": ["decode", "resample",
"record", "wwav.write"]`: what it does beyond the first cut of this contract
(§3.3's clips in other formats and at other rates, §3.10, §3.11). A client,
or a test both engines share, asks there instead of guessing.

`hello` must be the first request. A client whose `protocol` differs gets
`{"code": "protocol", "message": "This engine speaks protocol 1; the app speaks 2."}`
and the connection closes.

The app pings once a second while connected. No answer within 1 s, or a clock
(§4.2) that stalls for 500 ms while `state` is playing, means the engine has
hung: the app kills it and restarts it (§5).

### 3.2 Devices

| Op | Args | Result |
|---|---|---|
| `device.list` | — | `{"devices": [{"name": "MacBook Air Speakers", "inputs": 0, "outputs": 2, "rates": [44100, 48000]}]}` |
| `device.open` | `{"name": "…" \| null, "sample_rate": 48000, "block": 128}` | `{"name": "…", "sample_rate": 48000, "block": 128, "output_latency": 312, "input_latency": 296}` |

Latencies are in samples, as the device reports them. The clock subtracts
`output_latency` (§4.2).

### 3.3 The session

The session lives in the app; the engine only renders (9.3). The app compiles
its session into a **graph** and sends the whole graph. The engine builds it on
a worker thread and swaps it in with one atomic pointer write, keeping the
playhead. Parameter changes after that are `param.set`, never a rebuild.

| Op | Args | Result |
|---|---|---|
| `session.load` | `{"graph": Graph, "playhead": <sample>, "off": ["<node id>", …]}` | `{"nodes": <count>, "latency": {"<node id>": <samples>}, "meter_slots": {"<node id>": <slot>}}` |
| `session.unload` | — | `{}` |

`off` lists device nodes that load switched off (a plugin that crashed, 9.3).

```
Graph {
  "sample_rate": 48000,
  "tempo_map": [{"at_beats": 0, "bpm": 86.0}],
  "tracks": [Track],
  "master": {"gain_db": 0.0, "devices": [Device]}
}
Track {
  "id": "<ULID>", "kind": "audio" | "instrument" | "stem" | "bus",
  "role": "vocals" | "drums" | "other" | "bass",
  "gain_db": 0.0, "pan": 0.0, "mute": false, "solo": false,
  "sends": {"reverb": 0.0, "delay": 0.0},        // 0..1, to the role's returns
  "devices": [Device],
  "clips": [Clip],                                 // audio, stem
  "notes": [Note]                                  // instrument
}
Clip {
  "id": "<ULID>", "path": "/abs/path/to/media/01J….wwav",
  "source": "master" | "vocals" | "drums" | "other" | "bass",   // which part of the file
  "at": <session sample>, "in": <file frame>, "len": <frames>,
  "gain_db": 0.0, "reverse": false
}
Note { "pitch": 60, "vel": 100, "at": <session sample>, "len": <samples> }
Device {
  "id": "<ULID>", "format": "builtin" | "vst3" | "au",
  "uid": "reverb" | "<vst3 class id>" | "aumf:TpEc:Vndr",
  "path": "/Library/Audio/Plug-Ins/VST3/Tape Echo.vst3",   // vst3, au
  "params": {"<name>": <value>},
  "state": {"inline": "<base64>"} | {"file": "/abs/path"} | null,
  "on": true
}
```

- A `.wwav` dropped on a session becomes one track of kind `stem` per stem,
  each with a clip whose `source` is that stem and whose `path` is the same
  file. The engine reads the file in place (`wstm` in 1024-frame runs).
- Every track routes to the stem bus of its `role`. Each stem bus has its own
  reverb and delay returns fed only by its tracks' sends; the master is the sum
  of the four buses, then `master.devices` (the fold rule, 5.3 and 6.6).
- Clip times are in samples at `sample_rate`: `at` and `len` count the
  session's samples, `in` the file's own frames.
- WAV (16-, 24- or 32-bit PCM, 32-bit float; mono or stereo) and `.wwav` at
  the session's rate are read where they lie. Anything else is made ready
  first: a file at another rate is resampled (`wwav-dsp`'s resampler, the
  export's), and FLAC, ALAC, AIFF, CAF, MP3, AAC in MP4, Ogg Vorbis and the
  other WAVE codings are decoded (`wwav-decode`). Each becomes one 32-bit
  float WAV at the session's rate in the cache folder (§1), keyed by the
  file's path, size and modified time, and is played from there, so a seek,
  a loop and a render of it are as exact as of any WAV. `session.load`
  answers once every clip is ready; while a long file is being made it sends
  `{"ev": "convert.progress", "path": "…", "done": <source frames>, "total": <frames | null>}`.
  A file that can't be made ready is refused: `unsupported` (a format or
  more than two channels), `bad_clip` (damaged) or `no_such_file`.

### 3.4 Parameters

| Op | Args | Result |
|---|---|---|
| `param.set` | `{"node": "<id>", "param": "gain_db", "value": -3.0, "at": <sample, optional>}` | `{}` |

`node` is a track, device, stem bus (`"bus:vocals"` …) or `"master"`. Track
params: `gain_db`, `pan`, `mute`, `solo`, `send.reverb`, `send.delay`.
Built-in device params are named in `engine/BUILTINS.md`. Third-party plugin
params are `p<index>`. Without `at` a change applies at the next block;
mute and solo are heard within one block (S0.3's 10 ms is one block plus
one socket hop).

### 3.5 Transport

| Op | Args | Result |
|---|---|---|
| `transport.play` | — | `{"sample": <playhead>}` |
| `transport.stop` | — | `{"sample": <playhead>}` |
| `transport.locate` | `{"sample": <n>}` | `{"sample": <n>}` |
| `transport.loop` | `{"on": true, "start": <n>, "end": <n>}` | `{}` |

Every transport change also sends `{"ev": "transport", "state": "stopped" | "playing", "sample": <n>}`.

### 3.6 Rendering

| Op | Args | Result |
|---|---|---|
| `render` | `{"out_dir": "/abs", "start": <n>, "len": <n>, "master": true, "stems": true, "format": "f32" \| "s16"}` | `{"files": {"master": "/abs/master.wav", "vocals": "…", "drums": "…", "other": "…", "bass": "…"}, "frames": <n>, "sha256": {"master": "<hex>", …}}` |

Render runs the same graph objects on a non-real-time thread, as fast as the
CPU allows, at the session's block size, with plugins told
`setNonRealtime(true)`. Files are WAVE at the session rate: 32-bit float
(`f32`, the default) or 16-bit PCM. Stems are written before the master chain
and the master after it. Progress events: `{"ev": "render.progress", "stage": "master" | "stems", "done": <frames>, "total": <frames>}`.
Live playback stops for the length of a render and comes back stopped.

### 3.7 Plugins

| Op | Args | Result |
|---|---|---|
| `plugin.state` | `{"node": "<id>"}` | `{"state": {"inline": "<base64>"} \| {"file": "/abs"}, "bytes": <n>, "sha256": "<hex>"}` |
| `plugin.editor.open` | `{"node": "<id>"}` | `{"window": "<platform id>"}` |
| `plugin.editor.close` | `{"node": "<id>"}` | `{}` |
| `plugin.params` | `{"node": "<id>"}` | `{"params": [{"index": 0, "name": "Mix", "value": 0.5, "automatable": true}]}` |

States over 256 KB are written to a file in the session's `plugin-state/`
folder (the app passes `state_dir` in `session.load`'s `graph`), never inline.
The engine sends `{"ev": "plugin.state", "node": "…", "state": …}` for every
plugin on every transport stop and every 60 s while anything changed, and the
app keeps the last one (9.3).

Events: `{"ev": "plugin.latency", "node": "…", "samples": 2048}` whenever a
node's reported latency changes; the engine has already recomputed delay
compensation off the audio thread.

Keys a plugin window doesn't use go back to the app:
`{"ev": "key", "key": " ", "code": "Space", "meta": false, "shift": false, "alt": false, "ctrl": false}`.

### 3.8 MIDI

| Op | Args | Result |
|---|---|---|
| `midi.inputs` | — | `{"inputs": [{"id": "…", "name": "KeyStep"}]}` |
| `midi.route` | `{"input": "<id>", "track": "<id>", "channel": 1 \| null}` | `{}` |

Events: `{"ev": "midi.device", "id": "…", "name": "KeyStep", "connected": true}`.

### 3.9 Test ops (only with `--test`)

| Op | Args | Does |
|---|---|---|
| `debug.crash` | `{"in": "audio" \| "message"}` | `abort()`s on the named thread |
| `debug.hang` | `{"in": "audio" \| "message"}` | never returns on the named thread |
| `debug.crumb` | `{"node": "<id>"}` | writes that node to the crumb and crashes inside it |

The test plugins `crasher` (dies on a note) and `hanger` (never returns from
its process call) do the same from inside a real plugin.

### 3.10 Recording

One take at a time, written as it comes to a 32-bit float WAVE file at the
device's rate. The input never touches the audio thread's timing: it goes
through a ring to a writer thread.

| Op | Args | Result |
|---|---|---|
| `record.start` | `{"path": "/abs/take.wav", "input": "<name>" \| null}` | `{"path": "…", "input": "MacBook Pro Microphone", "channels": 1, "sample_rate": 48000, "at": <session sample>, "input_latency": 480}` |
| `record.stop` | — | `{"path": "…", "frames": <n>, "at": <session sample>, "sample_rate": 48000, "channels": 1, "input": "…", "input_latency": 480, "peak": 0.71, "dropped": 0}` |

- `input` null is the system's default input. On the null device it is
  `"null"`: the null device's input is its own output, the master, frame for
  frame, so a take can be held against what was played without hardware.
  (`"input": "null"` asks for that loopback on any device.)
- `at` is where the take's first frame belongs in the session: the sample
  that was being heard when that frame was captured. The audio thread says
  which sample reaches the speaker at which moment (the clock's own facts,
  §4.2), the input stamps each buffer with when it was captured, and the
  engine places the take by the two. With the transport stopped, `at` is the
  playhead. It can be below 0 for a take begun at the very start.
- A take has the input's one or two channels (the first two of a wider one).
- `dropped` counts input frames lost because the writer fell more than five
  seconds behind. It should be 0.
- While a take is being recorded and the transport plays, the clock's
  `state` is 2.
- `device.open` and a second `record.start` are refused with `recording`;
  `record.stop` with no take, `not_recording`. A path that can't be written
  is `record_failed`, before any input is opened.
- An engine told to exit finishes the take's header first.

Events: `{"ev": "record", "state": "recording", "path": "…", "at": <n>}` and
`{"ev": "record", "state": "stopped", "path": "…", "at": <n>, "frames": <n>}`.

### 3.11 Writing a .wwav

| Op | Args | Result |
|---|---|---|
| `wwav.write` | `{"path": "/abs/Low Tide.wwav", "start": <n>, "len": <n>, "meta": Meta, "creator": "liam_made_young", "parent": Parent \| null, "dither": true}` | `{"path": "…", "song_id": "<32 hex>", "frames": <n at 44.1 kHz>, "bytes": <n>, "sample_rate": 44100, "folds": true, "fold_dbfs": -138.5 \| null, "peak": 0.82}` |

```
Meta   { "song_id": "<32 hex>", "title": "Low Tide", "artist": "…", "bpm": 86.0, "key": "…",
         "type": "original" | "split" | "remix", "splitter": "…", "created": "2026-10-08" }
Parent { "song_id": "<32 hex>", "root_id": "<32 hex>", "generation": 2 }
```

The four stem buses and the master of session samples `start` to `start +
len`, rendered by the graph that plays them as `render` does (playback stops
for it and comes back stopped), resampled to 44.1 kHz when the session is at
another rate, taken to 16 bits once at the last step, and laid out by
`wwav-formats`, which owns the format: the file is written beside its path
and renamed into place when whole.

- `song_id` and `created` are made up when left out (a new id; today).
  `parent` makes the song a child: its parent's root, one generation on.
  What `wwav-formats` refuses is refused here as `write_failed` with its
  words: an original without a title, an id that isn't 32 hex, a song over
  what one RIFF file holds.
- `dither` (the default) adds TPDF dither, each stem and the master with its
  own noise, seeded from the song's id so the same song written twice is the
  same bytes. Without it samples are rounded, which gives 16-bit stems at
  unity back bit for bit.
- `folds` is the fold check (`docs/SPEC.md` 6.6): whether the master is the
  sum of the four stems to within −80 dBFS, measured before either is taken
  to 16 bits. `fold_dbfs` is how far apart they are at worst, null when not
  at all. The file is written either way; the export sheet decides what to
  say. `peak` over 1.0 means samples were clipped.
- Progress: `{"ev": "render.progress", "stage": "wwav", "done": <session frames>, "total": <len>}`.

## 4. Shared memory

Little-endian, every field naturally aligned. The app creates the region and
zeroes it; the engine maps it, writes the header, then writes the rest from
the audio thread without locks. Readers never lock either.

`crates/wwav-wire/src/shm.rs` and `engine/include/wwav_shm.h` define this
layout, and a test compiles the header and compares every `offsetof` with the
Rust mirror.

### 4.1 Header (offset 0, 64 bytes)

| Offset | Type | Field |
|---|---|---|
| 0 | `[u8; 4]` | magic `"WWAV"` |
| 4 | u32 | layout version, 1 |
| 8 | u32 | sample rate |
| 12 | u32 | block size |
| 16 | u32 | meter slots (`MS`, 260) |
| 20 | u32 | meter ring length (`MR`, 64) |
| 24 | u32 | peaks ring bytes |
| 28 | u32 | input ring bytes |
| 32 | u64 | engine pid |
| 40 | u64 | engine start, monotonic ns |
| 48 | 16 bytes | reserved, zero |

### 4.2 Clock (offset 64, 64 bytes): a seqlock

| Offset | Type | Field |
|---|---|---|
| 64 | u64 | `seq`: odd while being written |
| 72 | i64 | `sample_pos`: where the playhead will be when this block reaches the speaker |
| 80 | u64 | `host_time_ns`: the monotonic time at which `sample_pos` is at the speaker |
| 88 | f64 | `rate`: samples per second of playhead travel (0 when stopped) |
| 96 | u32 | `state`: 0 stopped, 1 playing, 2 recording |
| 100 | u32 | `dropouts`: count since the engine started |
| 104 | u64 | `callbacks`: blocks processed since start |
| 112 | 16 bytes | reserved |

Every audio callback writes it: `seq += 1` (odd), the fields, `seq += 1`
(even), with release ordering. A reader loads `seq`, reads the fields, loads
`seq` again, and retries if it changed or is odd. An engine killed between
the two increments leaves `seq` odd: before it starts the next engine on the
region, the app writes a whole clock over it (stopped, at the `sample_pos` it
holds) with `seq` even, and an engine that finds `seq` odd rounds it up to
even before its first write. `sample_pos` has output latency and plugin delay
already subtracted. The UI extrapolates the cursor from
`sample_pos + (now - host_time_ns) × rate / 1e9`.

### 4.3 Crumb (offset 128, 64 bytes)

| Offset | Type | Field |
|---|---|---|
| 128 | u64 | `crumb`: FNV-1a 64 of the id of the graph node now processing, 0 when none |
| 136 | u64 | `crumb_seq`: incremented with every write |

The audio thread writes `crumb` before every call into a device node and
clears it after. After a crash the app reads it and names the plugin instance
(9.3), by hashing each device id in its session with the same FNV-1a 64
(`wwav_ids::fnv1a64`).

### 4.4 Meters (offset 192)

A ring of `MR` entries; `meter_write` (u64 at offset 192) counts entries
written. Entry `i` is at `256 + (i mod MR) × (32 + MS × 16)`:

| Offset in entry | Type | Field |
|---|---|---|
| 0 | u64 | callback number |
| 8 | f32 | DSP load, 0..1 (time in the callback ÷ the block's duration) |
| 12 | u32 | dropouts so far |
| 16 | u32 | slots used |
| 20 | 12 bytes | reserved |
| 32 + 16·s | 4 × f32 | slot `s`: peak L, peak R, RMS L, RMS R (linear) |

Slots come from `session.load`'s `meter_slots`: one per track, then the four
stem buses (`bus:vocals`, `bus:drums`, `bus:other`, `bus:bass`), then
`master`. The app sends the newest entry to the web UI once per frame as raw
bytes on a Tauri channel, never JSON.

### 4.5 Peaks and input rings

After the meters come the peaks ring, then the input ring, at the sizes in
the header. Each starts with a 64-byte header. The take's writer thread
fills them as a take is recorded (§3.10); a reader loads `write`, copies,
loads `write` and `take` again, and throws the copy away if the ring came
round or another take began.

**Peaks** (at 256 + `MR` × (32 + `MS` × 16); 64 KiB): the take's waveform.

| Offset | Type | Field |
|---|---|---|
| 0 | u64 | `write`: peaks written since the take began |
| 8 | u64 | `take`: counts takes; `write` starts over with each |
| 16 | i64 | `at`: the session sample of the take's first frame |
| 24 | u32 | frames per peak, 256 |
| 28 | u32 | the take's channels |
| 32 | 32 bytes | reserved |
| 64 + 16·(i mod 4092) | 4 × f32 | peak `i`: least L, most L, least R, most R of its frames |

A mono take has the same on both sides. 4092 peaks are about 22 s at 48 kHz;
a reader that draws a take keeps what it has read.

**Input** (after the peaks; 2 × 96000 × 2 × 4 bytes): the take's audio.

| Offset | Type | Field |
|---|---|---|
| 0 | u64 | `write`: frames written since the take began |
| 8 | u64 | `take` |
| 16 | u32 | sample rate |
| 20 | u32 | the take's channels |
| 24 | 40 bytes | reserved |
| 64 + 8·(i mod 191992) | 2 × f32 | frame `i`, left and right |

Two samples a frame whatever the take's channels. `crates/wwav-wire`'s
`PeaksRing` and `InputRing` are this layout, with both sides' reads and
writes.

## 5. When the engine falls over

1. The engine dies. The app sees the socket close and the child exit (or kills
   it after a hang, §3.1).
2. It reads `crumb` and names the device node that was running.
3. It starts a new engine, sends `hello`, `device.open` with the last device,
   and `session.load` built from the session as it stands, with each plugin's
   last reported state. The instance that crashed is in `off`.
4. The transport comes back stopped at the same playhead (`transport.locate`
   to the last `sample_pos` the clock showed). Nothing resumes on its own.
5. The app shows the sheet in 5.7: "The audio engine stopped. 'Tape Echo' on
   the track 'Keys' was running when it did. Restarting…", then **Keep it off**
   and **Try it again**.

Three crashes in ten minutes from one plugin and it stays off: "crashed 3
times". The target is 2 s plus the plugins' own load time.
