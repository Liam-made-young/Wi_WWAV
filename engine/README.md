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
stops the audio and exits. `--device null` opens no hardware: a timer thread
calls the graph every block at the nominal rate. `--test` turns on the
`debug.*` ops.

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
| `src/server.cpp` | the socket: one client at a time, frames, the handshake |
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
ping. A socket thread reads frames and answers `hello`. One worker thread
runs every other op in the order it came, so a long render never holds up a
ping. The audio thread is the device's callback, or the null device's timer.
A reader thread keeps streamed clips ahead of the playhead.

The audio thread never allocates or logs, and the engine's own code never
locks on it. Commands reach it through a single-producer queue whose only
producer is the worker; a new graph arrives with one atomic pointer exchange,
and the worker frees the old one once the callback that might hold it has
ended. The one lock the audio thread takes is JUCE's own: `AudioDeviceManager`
holds its callback lock around each callback of a real device, contended
only while a device opens or closes.

## What it does now

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
  verdict as its error.
- Files under 32 MB are held whole. Bigger ones stream in 1024-frame runs,
  about 2 s ahead of the playhead; `transport.play` waits (up to 0.3 s) for
  the first quarter second to be read.
- The fold rule: every track sums into the stem bus of its role, and the
  master is the sum of the four buses, then the master's gain. A render
  writes the buses as the stems and the sum as the master, so the stems add
  up to the master.
- `param.set` for tracks and buses (`gain_db`, `pan`, `mute`, `solo`) and the
  master (`gain_db`, `mute`). A change is heard from the first block after
  it arrives: like PRANA's mixer, a gain ramps to its new value over that
  block, so nothing clicks, and a mute is silent from the block after.
  Solo works within tracks and within buses.
- Renders run the live graph on the worker, block by block at the session's
  block size, as fast as the CPU allows, into 32-bit float or 16-bit WAVE
  files with their sha256. Live playback stops for the render and comes back
  stopped. 16-bit files are rounded, not dithered: export dithers once, at
  its last step (`docs/SPEC.md` 6.7).
- Shared memory as 4.1–4.4 lay it out: the header, the clock seqlock every
  callback (`sample_pos` with the device's output latency subtracted), the
  crumb around every node, and a meter entry every block.

## Tests

`ctest` runs seven suites. Each test was written before the code it checks.

| Suite | Proves |
|---|---|
| `engine_unit` | a reader never sees a torn clock against a writer at full rate (F8); the command queue keeps order; crumbs are standard FNV-1a 64; unity gain is exact; tracks fold into buses and buses into the master; the WAVE headers; probing WAV and `.wwav` files and the reference reader's verdicts; a streamed clip never returns wrong audio while its reader races it |
| `engine_protocol` | the listening line and nothing else on stdout; the socket is 0600; `hello` and the protocol refusal; a 16 MiB frame round-trips and bad frames close the connection (F8); one client at a time; devices; later stages answer `unsupported` |
| `engine_playback` | a `.wwav` made by `wwav_pack.py` plays: the clock runs at the session rate on the reader's monotonic clock, each stem shows on its own bus, and a mute or solo is heard within one block of arriving (S0.3); a `session.load` during playback swaps the graph without stopping or moving the playhead; the transport and its events; loops; refusals |
| `engine_render` | the stems sum to the master and each stem is exactly the one packed; renders are identical across runs and engines; 16-bit; playback stops for a render; pings are answered during one |
| `engine_restart` | after `kill -9` during playback a new engine is back, loaded and stopped at the last `sample_pos`, within 2 s (S0.2, Linux part; it takes about 30 ms here); closing stdin exits within 1 s, even mid-render; `shutdown`; the crumb names the crashed node; `debug.crash` and `debug.hang` |
| `engine_streaming` | a 35 MB `.wwav` streams with no silent block after a load, a locate or a play, recovers within 0.1 s of a locate during playback, and renders exactly |
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
  device has run.
- A loop longer than about a second that jumps back into a streamed clip
  misses a few milliseconds after each jump: the reader doesn't read ahead
  across the loop's end yet. Those misses, and any other disk misses, are not
  counted in the clock's `dropouts`, which counts the device's xruns and the
  null device's late blocks.
- One audio thread renders every track, as `docs/SPEC.md` 9.4 plans for v1.
- Windows (named pipes, file mappings) waits for Stage 6.
