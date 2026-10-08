# wwav-engine

The audio engine `Wi_WWAV.app` runs as a child process, in Rust. It is a
server of `docs/ENGINE.md`: the app talks to it over a Unix socket and reads
its clock, meters and takes from shared memory. This is Console Phase 3a,
the audio half of the Audiovisual tool: playback and recording, decoding and
resampling, and reading and writing `.wwav`. The timeline, the mixer's
effects and video are later phases.

`engine/` holds the first cut of the same contract in C++ on JUCE. It is
kept, and nothing here depends on it; its test suites in `engine/tests` are
this engine's tests too.

## Running it

The app starts it (`wi_core::Config::engine_path`), with a pipe on its stdin:

```
wwav-engine --socket <dir>/engine.sock --shm /wwav-<pid>-<n> [--device <name|null>]
            [--rate 48000] [--block 128] [--cache <dir>] [--test]
```

From a terminal, `wwav-engine-cli` starts it the same way and sends any op:

```
cargo build --release -p wwav-engine -p wwav-engine-cli
printf 'hello {"protocol": 1, "client": "terminal"}\ndevice.list\ntransport.play\nsleep 200ms\nclock\n' |
  target/release/wwav-engine-cli --spawn target/release/wwav-engine --device null -
```

## The threads

```
main      accepts one client at a time and reads its frames: hello, ping and
          param.set are answered there, the rest handed on
writer    sends every frame out, so no other thread waits on the client
worker    runs ops in the order they came: devices, sessions, the transport,
          renders, takes, exports
audio     the device's callback, or the timer when no device is open
reader    keeps streamed clips ahead of the playhead
take      writes the take being recorded, and shows it in shared memory
```

The audio thread never allocates, locks, logs or waits. It shares atomics,
two single-producer queues and the graph's pointer with the rest
(`src/audio.rs` says how each is kept safe). `tests/no_alloc.rs` holds the
first of those as a number.

| File | What is in it |
|---|---|
| `audio.rs` | the audio thread: the clock, the transport, the meters, the graph swap, the timer |
| `graph.rs` | tracks into the four stem buses into the master; the fold rule |
| `media.rs` | finding a clip in a WAV or `.wwav`; clips held whole or streamed; the reader thread |
| `build.rs` | `session.load`'s graph, checked and built |
| `convert.rs` | other formats and other rates, made once into a WAV in the cache |
| `device.rs` | sound cards, through cpal |
| `record.rs` | the capture ring, where a take belongs, the take's writer |
| `export.rs` | writing a `.wwav` |
| `engine.rs` | the ops, and which thread answers each |
| `server.rs` | the way out of the socket |

## Tests

```
tools/local_formats.sh                      # in a git worktree: fills formats/
cargo test -p wwav-engine                   # about 40 s
cargo test -p wwav-engine --test contract -- --ignored   # the two slow suites: 20 s to 3 min
```

- `tests/contract.rs` runs `engine/tests/*.py` against the binary: the
  suites the first engine was built to (`test_protocol`, `test_playback`,
  `test_render`, `test_restart`, and behind `--ignored` `test_streaming` and
  `test_review`), and three written with this engine (`test_record`,
  `test_wwav`, `test_convert`). They need `python3` and `formats/`.
- `tests/no_alloc.rs` is the audio thread, with an allocator that counts.
- Unit tests sit beside what they test.

One test in `engine/tests` changed: `test_playback.test_bad_sessions` no
longer expects a clip at another rate to be refused by an engine whose
`hello` says it resamples.

## What was checked, and what wasn't

Measured on a MacBook Pro (Apple silicon), release build, 8 Oct 2026:

- All nine suites pass: 90 tests.
- The MacBook's speakers opened at 44.1 kHz in blocks of 256; over a second
  the clock moved 44,288 samples in 1.00426 s (44,100.0 a second), with no
  dropouts and a DSP load of 0.2 % for a four-stem song.
- A four-stem session rendered as five float files at 139 times real time.
- Two minutes of that session looping on the timer in blocks of 128: no
  dropouts in 41,393 blocks, and a DSP load of 1.4 % at the worst of 480
  readings. These are in `docs/PLAN.md`'s budgets too.

Not checked, because only a person or other hardware can:

- **How it sounds.** Nothing here has ears. The played session was held at
  −70 dB during the check.
- **The microphone.** `record.start` on a real input was not run: opening it
  makes macOS ask for the microphone, which is the person's to answer. The
  path it takes is tested with the loopback, but the latency a card reports,
  and so where a real take lands, is unchecked. A loopback cable from an
  output to an input would settle it.
- **An hour without a dropout.** Two minutes were run.
- **Linux and Windows.** cpal has backends for both; nothing was built off
  the Mac. The socket and shared memory are Unix-only, as in `wwav-wire`.
- **The JUCE engine.** The brief asked for the suites to run against both
  engines. The JUCE one has never been built on a Mac and wasn't here.

## Limits

- Two channels out. A wider card gets the master on its first two.
- One input device at a time, at the output's rate. An input on another
  clock than the output drifts against it over a long take; an aggregate
  device is the fix, and that is the person's to set up.
- A device that changes its own rate under a loaded session is not played
  (silence, transport stopped) until `device.open` or `session.load` puts
  the two at one rate. The app isn't told with an event yet.
- The conversion cache only grows (see `docs/DECISIONS.md`).
- No effects, sends, instruments, MIDI, plugins, automation or reversed
  clips: each is refused as `unsupported` with a sentence, not ignored.
- Denormals aren't flushed. Nothing in the graph feeds back yet; the first
  effect that does should set that.

## Five minutes, for Liam

1. `cargo build --release -p wwav-engine -p wwav-engine-cli`
2. Play a song you know through the speakers. Put its path in place of
   `/path/to/song.wwav`, and its length in frames (seconds × 44100) for `len`:

   ```
   target/release/wwav-engine-cli --spawn target/release/wwav-engine --rate 44100 - <<'EOF'
   hello {"protocol": 1, "client": "terminal"}
   session.load {"graph": {"sample_rate": 44100, "tracks": [{"id": "T", "kind": "audio", "role": "other", "clips": [{"id": "C", "path": "/path/to/song.wwav", "source": "master", "at": 0, "in": 0, "len": 2646000}]}]}}
   transport.play
   sleep 10s
   param.set {"node": "T", "param": "pan", "value": -1.0}
   sleep 3s
   param.set {"node": "T", "param": "mute", "value": true}
   sleep 2s
   EOF
   ```

   Listen for: it starts at once and at the right pitch; no clicks or gaps
   in the first ten seconds; it moves to the left ear without a click; the
   mute is a quick fade, not a tick.
3. The same with an MP3 or an `.m4a`: the first load takes a moment (it is
   being decoded), the second none.
4. Record yourself. Add these lines after `transport.play`, with a path of
   your own, and say yes when macOS asks for the microphone:

   ```
   record.start {"path": "/tmp/take.wav"}
   sleep 5s
   record.stop
   ```

   Listen to `/tmp/take.wav`: it is you, at the right speed, with no gaps.
   `record.stop`'s answer should say `"dropped": 0`.
