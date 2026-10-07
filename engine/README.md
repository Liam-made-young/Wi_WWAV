# wwav-engine

The audio engine that `Wi_WWAV.app` runs as a child process (`docs/SPEC.md`
9.1–9.4). The app talks to it over a Unix socket and reads its clock and
meters from shared memory; `docs/ENGINE.md` is the contract, and this is its
server. C++17, JUCE 8.0.15 and PRANA's `prana/core`, built with CMake and
Ninja.

## Build and test on Linux

```
tools/bootstrap.sh          # formats/ (in a git worktree: tools/local_formats.sh)
bash tools/fetch_juce.sh    # JUCE into engine/third_party/JUCE, which git ignores
cmake -S engine -B engine/build -G Ninja -DCMAKE_BUILD_TYPE=Release
ninja -C engine/build
(cd engine/build && ctest --output-on-failure)
```

It needs a C++17 compiler, CMake 3.22 or later, Ninja, Python 3 (the tests
use its standard library only) and the ALSA headers (`libasound2-dev`); JUCE's
`docs/Linux Dependencies.md` lists the rest. JACK is left out
(`JUCE_JACK=0`). A clean build takes about a minute on four cores, half of
it configuring (JUCE builds its `juceaide` tool then); with sccache, add
`-DCMAKE_CXX_COMPILER_LAUNCHER=sccache`.

The binary is `engine/build/wwav-engine_artefacts/Release/wwav-engine`, beside
`wwav-scan` and the unit tests in `engine/build/`.

On a machine without `/dev/snd/seq`, ALSA prints one line about its MIDI
sequencer at startup. It is harmless: JUCE's device manager looks for MIDI
devices as it starts.

## Build on macOS

The same commands. On a Mac the engine builds as `wwav-engine.app` with
`LSUIElement` in its `Info.plist`, so it has no Dock icon and the plugin
windows it will open belong to a real application; the app embeds it as
`Contents/Helpers/wwav-engine.app`. The minimum is macOS 13. For a universal
build add `-DCMAKE_OSX_ARCHITECTURES="arm64;x86_64"`. **None of this has been
built or run on a Mac yet.**

## Running it by hand

The app creates the shared memory and the socket's folder, then starts the
engine with a pipe on its stdin:

```
wwav-engine --socket <dir>/engine.sock --shm /wwav-<pid>-<n> [--device <name|null>]
            [--rate 48000] [--block 128] [--test]
```

It prints `wwav-engine listening <socket>` once it is ready, and nothing
else on stdout; logs go to stderr. When its stdin reaches end of file it
stops the audio and exits; if tearing down hangs (a hung audio thread, say),
it leaves anyway 0.75 s after it began, inside the contract's second.
`--device null` opens no hardware: a timer thread calls the graph every block
at the nominal rate. `--test` turns on the `debug.*` ops.

`tests/wwav_client.py` does all of that from Python, and is the quickest
way to try an op:

```
cd engine/tests
WWAV_ENGINE=../build/wwav-engine_artefacts/Release/wwav-engine python3 -i -c '
from wwav_client import *
shm = Shm(); engine = Engine(shm); c = engine.connect()'
>>> c.call("device.list")
>>> shm.clock()
```

## How it is put together

| File | What it holds |
|---|---|
| `src/main.cpp` | the JUCE application: arguments, the listening line, the stdin watchdog |
| `src/engine.cpp` | the ops, the worker thread that runs them in order, startup and shutdown |
| `src/server.cpp` | the socket: one client at a time, frames, the handshake, a writer thread |
| `src/json.cpp` | a strict JSON parser without recursion, for frames and `.wwav` chunks |
| `src/pyjson.cpp` | JSON as the reference reader's Python reads and prints it, for stem verdicts |
| `src/audio.cpp` | the device (JUCE's or the null timer), the callback, the transport, the graph swap |
| `src/build.cpp` | `session.load`'s graph, from JSON to a `Graph` |
| `src/graph.cpp` | tracks into stem buses into the master; meters and crumbs |
| `src/media.cpp` | finding a clip in a WAV or `.wwav`, holding it whole or streaming it |
| `src/shm.cpp` | the shared memory: header, clock seqlock, crumb, meter ring |
| `src/wav.cpp` | the WAVE writer for renders |
| `include/wwav_shm.h` | the shared-memory layout, shared with the app's Rust mirror |
| `scan/main.cpp` | `wwav-scan`, a placeholder until plugins arrive |

Threads: the main thread runs JUCE's message loop (where plugin windows will
live) and answers `ping`, so a hung message loop shows as an unanswered
ping. A socket thread reads frames and answers `hello` and `param.set`, and
a writer thread sends every frame out, so a client that stops reading never
blocks either the socket thread or the worker. One worker thread runs every
other op in the order it came, so a long load or render holds up neither a
ping nor a mute. The audio thread is the device's callback, or the null
device's timer. A reader thread keeps streamed clips ahead of the playhead.

The audio thread never allocates or logs, and the engine's own code never
locks on it. Commands reach it through two single-producer queues, one from
the worker (the transport) and one from the socket thread (`param.set`); a
new graph arrives with one atomic pointer exchange, and the worker frees the
old one once the callback that might hold it has ended. The one lock the
audio thread takes is JUCE's own: `AudioDeviceManager` holds its callback
lock around each callback of a real device, contended only while a device
opens or closes.

A `param.set` that arrives while a `session.load` is being built changes the
graph that is playing at once, and the new graph replays it before it is
swapped in, since the app compiled that graph before the change. One for a
track only the new graph has waits for the load (and later ones for the same
node wait behind it, so a node's changes land in the order sent).

## What it does now

- Frames as `docs/ENGINE.md` 2 has them, parsed strictly (RFC 8259): a
  payload that isn't exactly one JSON object in valid UTF-8 (text after it,
  a trailing comma, a comment, a lone surrogate escape), or one nested more
  than 127 deep (serde_json's limit, which the app parses with), closes the
  connection; nothing a client sends can overflow a stack. A string holding `\u0000` is valid and is answered, though no
  name, path or id the engine knows can hold one. No answer goes out over
  16 MiB: an error's sentence quotes at most 16 KiB of what was sent, and
  track ids are at most 1 KiB.
- Every op in `docs/ENGINE.md` 3.1–3.6 and 3.9: `hello` (and the protocol
  refusal), `ping`, `shutdown`, `device.list`, `device.open`, `session.load`,
  `session.unload`, `param.set`, `transport.play`, `stop`, `locate` and
  `loop` with their `transport` events, `render` with its progress events,
  and `debug.crash`, `debug.hang` and `debug.crumb`.
- Tracks of kind `audio` and `stem`, with clips from WAV files (16-, 24- and
  32-bit PCM, 32-bit float, mono or stereo) and `.wwav` files. A stem clip
  reads its stem from `wstm` in place; a master clip reads the `data`
  chunk. Whether a `.wwav` has stems is decided by the reference reader's
  rules, and a clip asking for one that isn't there gets that reader's
  verdict as its error, word for word: `wmet` and `wlin` are read as
  Python's `json` module reads them, and values printed as `str()` prints
  them (`engine_review` checks 38 kinds of chunk against the reader itself).
- Files under 32 MB are held whole: the frames the file has, whatever the
  clip's `len` (past the file's end is silence). Bigger ones stream in
  1024-frame runs, about 2 s ahead of the playhead; `transport.play` waits
  (up to 0.3 s) for the first quarter second to be read. While a loop is
  on, each streamed clip also keeps about 1.5 s from the loop's start
  ready, so the jump back never waits on the disk.
- Positions and lengths (clip `at`, `in` and `len`, locates, loops,
  renders) are refused past 2^50 samples, so no sum of them overflows.
- The fold rule: every track sums into the stem bus of its role, and the
  master is the sum of the four buses, then the master's gain. A render
  writes the buses as the stems and the sum as the master, so the stems add
  up to the master.
- `param.set` for tracks and buses (`gain_db`, `pan`, `mute`, `solo`) and the
  master (`gain_db`, `mute`). A change is heard from the first block after
  it arrives, whatever the worker is doing: like PRANA's mixer, a gain ramps
  to its new value over that block, so nothing clicks, and a mute is silent
  from the block after. Solo works within tracks and within buses.
- A real device that opens at another rate than the loaded session's (JUCE
  picks a rate the device has when it hasn't the one asked for) is closed
  again, and `device.open` answers `rate_mismatch`. One that changes its own
  rate later stops the transport and plays silence, with a `transport`
  event, rather than play the session fast or slow.
- Renders run the live graph on the worker, block by block at the session's
  block size, as fast as the CPU allows, into 32-bit float or 16-bit WAVE
  files with their sha256. Live playback stops for the render and comes back
  stopped. 16-bit files are rounded, not dithered: export dithers once, at
  its last step (`docs/SPEC.md` 6.7).
- Shared memory as 4.1–4.4 lay it out: the header, the clock seqlock every
  callback (`sample_pos` with the device's output latency subtracted), the
  crumb around every node, and a meter entry every block.

## Tests

`ctest` runs eight suites. Each test was written before the code it checks.

| Suite | Proves |
|---|---|
| `engine_unit` | a reader never sees a torn clock against a writer at full rate (F8), nor a torn meter entry when the writer laps it; the command queue keeps order; crumbs are standard FNV-1a 64; unity gain is exact; tracks fold into buses and buses into the master; the WAVE headers; probing WAV and `.wwav` files and the reference reader's verdicts; a streamed clip never returns wrong audio while its reader races it, and a loop finds its start ready; the JSON parser is strict, and its Python dialect reads and prints as Python 3.13 does |
| `engine_protocol` | the listening line and nothing else on stdout; the socket is 0600; `hello` and the protocol refusal; a 16 MiB frame round-trips and bad frames close the connection (F8); one client at a time, and one that stops reading can't keep the next one out; devices; later stages answer `unsupported` |
| `engine_playback` | a `.wwav` made by `wwav_pack.py` plays: the clock runs at the session rate on the reader's monotonic clock, each stem shows on its own bus, and a mute or solo is heard within one block of arriving (S0.3); a `session.load` during playback swaps the graph without stopping or moving the playhead; the transport and its events; loops; refusals |
| `engine_render` | the stems sum to the master and each stem is exactly the one packed; renders are identical across runs and engines; 16-bit; playback stops for a render; pings are answered during one |
| `engine_restart` | after `kill -9` during playback a new engine is back, loaded and stopped at the last `sample_pos`, within 2 s (S0.2, Linux part; it takes about 30 ms here); closing stdin exits within 1 s, even mid-render or with the audio thread hung; `shutdown`; the crumb names the crashed node; `debug.crash` and `debug.hang` |
| `engine_streaming` | a 35 MB `.wwav` streams with no silent block after a load (also one with a new playhead during playback), a locate or a play, recovers within 0.1 s of a locate during playback, and renders exactly |
| `engine_review` | what an independent review found, kept as tests: a clip's long `len` doesn't exhaust memory; stem verdicts match the reference reader's on 38 kinds of `wmet` and `wlin`; a loop over a streamed clip never drops audio; a mute sent during a 36-track load is heard within 10 ms, and changes sent during a load land in the new graph; deep nesting, text after the object, two objects, a trailing comma and invalid UTF-8 close the connection, `\u0000` doesn't, and no answer is over 16 MiB; an exiting engine leaves its replacement's socket |
| `engine_scan_usage` | `wwav-scan` says it scans nothing yet |

## Not done yet

- Plugins (VST3 and AU), built-in devices, sends and their returns,
  instrument and bus tracks, MIDI and recording are later stages. The engine
  reads their JSON and answers `unsupported`; a device that is off, or in
  `session.load`'s `off`, loads as nothing to do. `plugin.*` and `midi.*`
  answer `unsupported`. `session.load`'s `latency` is always empty, and
  there is no delay compensation, because nothing has latency yet.
- Clips at a rate other than the session's are refused until the export
  resampler (F9) exists to play them through. AIFF and CAF files, reversed
  clips, and `param.set` with `at` are refused too.
- The peaks and input rings in shared memory are left as reserved space;
  they come with recording.
- `wwav-scan` only prints its usage.
- Nothing has run on a Mac: the bundle and its `LSUIElement`, CoreAudio, and
  the clock. On a Mac the clock should take CoreAudio's own timestamp rather
  than the moment the callback runs, and the audio thread should join the
  device's workgroup; neither is done.
- Real devices on Linux are untested here (no sound card); only the null
  device has run. That includes the rate checks above.
- Disk misses (a streamed clip whose reader fell behind plays silence) are
  not counted in the clock's `dropouts`, which counts the device's xruns
  and the null device's late blocks.
- The null device's timer thread isn't a real-time thread. On a machine
  whose every core is busy it can run a block late, and a change heard
  "within one block" then takes longer; a real device's callback runs at
  the system's real-time priority.
- A stem verdict prints a list or object nested in `wmet`'s `frames` as
  Python's `repr()` would, except that a code point Unicode hasn't assigned
  is printed as it is where Python escapes it.
- One audio thread renders every track, as `docs/SPEC.md` 9.4 plans for v1.
- Windows (named pipes, file mappings) waits for Stage 6.
