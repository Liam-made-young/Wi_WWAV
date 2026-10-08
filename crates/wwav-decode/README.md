# wwav-decode

Decodes the common audio files to float frames, and converts one to a WAV the
engine plays. The engine plays WAV and `.wwav` directly; every other file, and
any file whose rate differs from the session's, is converted once to a 32-bit
float WAV at the session's rate and played like any other WAV.

```rust
let info = wwav_decode::probe(path)?;            // rate, channels, frames, codec
let mut decoder = wwav_decode::Decoder::open(path)?;
while decoder.read(&mut frames)? > 0 {}          // interleaved f32, a packet a call
let done = wwav_decode::to_wav(src, dst, 48_000, &cancel, &mut |done, total| {})?;
```

- Integer samples come out divided by their full scale: a 16-bit `v` is
  exactly `v / 32768.0`, 24-bit `v / 8388608.0`, 32-bit `v / 2147483648.0`
  (rounded to the nearest float), 8-bit `(v - 128) / 128.0`. Floats are passed
  through as they are, over full scale included.
- `to_wav` writes the plain 44-byte kind of WAV: `RIFF`, `WAVE`, a 16-byte
  `fmt ` with format tag 3, then `data`. At the file's own rate the samples
  are the decoded ones, bit for bit; at another they go through
  `wwav_dsp::resample::Resampler`, which has no delay and gives
  `resampled_len` frames. The same file gives the same bytes every run.
- The WAV is written beside `dst` as `.<name>.<pid>-<n>.part`, flushed to the
  disk, and renamed into place. Cancelled or failed, the part file is deleted
  and a file already at `dst` is left as it was.
- `Info.frames` is the length the container states, and `None` where it only
  could be estimated (below). `Converted.frames` is what was written.
- Errors are sentences: `Error`'s `Display` is fit to show.

## What it reads

Decoding is symphonia 0.5.5, all in Rust. "Exact" means every sample equal,
bit for bit, to the 16-bit WAV the file was made from. Fixtures are made when
the tests run and nothing binary is in the repo. On this Mac (8 Oct 2026)
every encoder was present and no test was skipped.

| File | `codec` | Proved by | Made with |
|---|---|---|---|
| WAV: 8, 16, 24, 32-bit, float 32 and 64, plain and extensible headers | `pcm` | exact, mono and stereo | by hand |
| AIFF 16 and 24-bit, AIFF-C little-endian | `pcm` | exact | by hand, afconvert, ffmpeg |
| CAF, PCM | `pcm` | exact | afconvert, ffmpeg |
| FLAC; FLAC in Ogg and in Matroska | `flac` | exact | flac, ffmpeg |
| ALAC in MP4 (`.m4a`) and in CAF | `alac` | exact | afconvert |
| MP3 with a LAME header | `mp3` | same length, 1 kHz within 1 Hz and 1 dB, sweep at lag 0 | lame |
| MP3 without one | `mp3` | on pitch; delay left in | lame `-t` |
| AAC in MP4 (`.m4a`, `.mp4`) | `aac` | the same | afconvert, ffmpeg |
| AAC, raw (ADTS, `.aac`) | `aac` | on pitch; delay left in | ffmpeg |
| Vorbis in Ogg | `vorbis` | same length, on pitch, sweep at lag 0 | libvorbis, built by the test with `cc` |
| Vorbis in Ogg | `vorbis` | on pitch, as long as it says | ffmpeg's own encoder |
| WAV: MS and IMA ADPCM | `adpcm` | on pitch, sweep at lag 0 | ffmpeg |
| WAV: A-law, mu-law | `alaw`, `mulaw` | the same, same length | ffmpeg |

A test whose encoder is missing prints `skipped: no <tool>` and passes; the
suite was also run with every tool hidden to check that. The tools are
`/usr/bin/afconvert`, and `flac`, `lame` and `ffmpeg` in `/opt/homebrew/bin`.
The ffmpeg here has no libvorbis encoder, and its own Vorbis encoder smears a
sweep (0.79 alike to its source, where libvorbis gives 0.9998), so the Vorbis
test that measures placement builds `tests/common/vorbis_encode.c` against
Homebrew's libvorbis.

## What it adds to symphonia

A clip has to start and end where its file says, to the frame. Symphonia
0.5.5 leaves these unused, and `Decoder` applies them (`src/container.rs`):

- **AAC encoder delay in MP4.** `enable_gapless` trims MP3 and Ogg; the MP4
  reader ignores it. Apple's encoder says its delay (2112 frames) and true
  length in an `iTunSMPB` tag, ffmpeg's (1024) in an edit list. Both are read,
  the edit list first. Without this an `.m4a` clip from afconvert starts
  44 ms late at 48 kHz and runs on into its padding.
- **The end of an AIFF.** Symphonia counts the sound chunk 8 bytes too long
  and decodes that far, into the next chunk's header: two frames of noise at
  the end of a 16-bit stereo AIFF with a chunk after its sound. The frame
  count in `COMM` is used instead.
- **The end of a short Ogg.** When the sound fits in one Ogg page (a second
  or two), symphonia decodes to the end of the last block, past where the page
  says the sound ends: 24 to 636 frames long on 18 of 32 files from libvorbis.
  Decoding stops at the stated length.
- **Rate and channels** are taken from the first decoded packet, not the
  header. Symphonia's MP4 reader calls every ALAC track 44.1 kHz.
- **Panics.** Symphonia panics on some damaged files (24 of 540 in the
  test: index out of bounds, arithmetic overflow, an `unwrap`). They are
  caught and reported as `Error::Bad`. Rust still prints the panic to stderr,
  and this needs the default `panic = "unwind"`.

Checked once by hand, not in the suite: symphonia's AAC decode of a file is
ffmpeg's to 4.2e-7, and its Vorbis decode is libvorbis's to 4.2e-7 with the
same length on all 32 files once stopped at the stated end.

## Limits

Not read, each refused as `Unsupported` in a sentence (tested, but for the
last line):

- Opus, WMA, MP2 (symphonia has an MP2 decoder behind a feature that isn't
  switched on here).
- RF64 and W64, so no WAV over 4 GB.
- AAC in CAF, and PCM in Matroska: symphonia has the codec but can't start it
  from what the container gives.
- A raw AAC stream as afconvert writes it (the MPEG-2 bit set). ffmpeg's is
  read.
- More than two channels. Nothing is mixed down.
- A chained Ogg (two streams end to end). `probe` and the first stream read
  normally, and the error comes when the second stream starts.
- Not tried: AC-3, DTS, WavPack, APE, Musepack, DSD, Speex. Symphonia 0.5 has
  no decoder for them.

Read, with something missing:

- **HE-AAC** decodes without its high band, at half its rate: symphonia has
  no SBR. An afconvert `aach` file at 48 kHz came out as 24 kHz, the right
  length. Its placement wasn't measured.
- **Encoder delay stays in** where the file doesn't say it or symphonia
  doesn't read it: an MP3 with no LAME header (1105 frames in the test), a raw
  AAC stream (1024), AAC or MP3 in Matroska, a fragmented MP4, and an MP4
  with several edits or two sound tracks. For those `Info.frames` is `None`
  or includes the delay.
- **`Info.frames` is `None`** for an MP3 with no LAME header and a raw AAC
  stream (symphonia estimates them from the bitrate; for the raw AAC in the
  test it was 2048 frames short in two seconds), and for Matroska (which
  counts in milliseconds).
- **A file cut short** decodes as far as it goes with no error; symphonia
  ends a damaged file the way it ends a whole one. Compare
  `Converted.frames` with `Info.frames` to tell.
- **A packet that won't decode** is skipped, as symphonia's players do, so
  the sound after it moves earlier by that packet. If no packet decodes, the
  file is `Bad`.
- A file of several tracks plays its first track with sound.
- A damaged header can still ask symphonia for a great deal of memory; that
  isn't guarded.
- AAC with an `iTunSMPB` tag written by iTunes itself wasn't tried, only
  afconvert's. An AIFF behind an ID3 tag is tested with an empty tag only.

## Dependencies

| Crate | Licence | For |
|---|---|---|
| symphonia 0.5.5, with symphonia-core, -metadata, -utils-xiph, -bundle-flac, -bundle-mp3, -codec-aac, -codec-adpcm, -codec-alac, -codec-pcm, -codec-vorbis, -format-caf, -format-isomp4, -format-mkv, -format-ogg, -format-riff | MPL-2.0 | every decoder and container |
| arrayvec, lazy_static, log, bitflags | MIT OR Apache-2.0 | symphonia's |
| bytemuck | Zlib OR Apache-2.0 OR MIT | symphonia's |
| extended | MIT | AIFF's 80-bit rate |
| encoding_rs | (Apache-2.0 OR MIT) AND BSD-3-Clause | tag text |
| cfg-if, scopeguard, simdutf8, multiversion_no_op | MIT OR Apache-2.0 | encoding_rs's |
| wwav-dsp (libm, MIT) | this repo's | the resampler |
| hound 3.5.1 | Apache-2.0 | tests only: a second reader of the WAV written |
| rustfft 6.4, tempfile 3 | MIT OR Apache-2.0 | tests only |

AAC, like every codec here, is decoded in software by symphonia; no system
decoder and no ffmpeg is linked. The licences are from each crate's
`Cargo.toml` in `~/.cargo/registry`. New to `Cargo.lock`: the 16 symphonia
crates, arrayvec, extended, lazy_static and hound; the rest were there.

## Tests

```sh
export CARGO_TARGET_DIR=~/Library/Developer/wi-wwav-build/target-console-av
cargo test -p wwav-decode                 # 40 tests, about 3 s
cargo test -p wwav-decode -- --nocapture  # to see what was skipped
```
