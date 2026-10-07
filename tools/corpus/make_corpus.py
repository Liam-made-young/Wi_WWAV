#!/usr/bin/env python3
"""Makes tests/corpus/: the .wwav, .wav, .mp4 and .swav files every reader
is checked against (docs/PLAN.md F1-F3), and manifest.json, which says why
each file exists and what the reference tool says of it.

  make_corpus.py [--out DIR] [--check]

Needs Python 3 (no packages), the reference tools in formats/ and ffmpeg.
Nothing is random: the audio is integer patterns, the ids and dates are
fixed, and the films are encoded single-threaded with ffmpeg's bit-exact
flags, so a second run gives the same bytes. --check makes the corpus again
in a temporary folder and says whether it matches the committed one (the
films only match with the same ffmpeg and libx264 builds).

The .wwav files start from one song packed by wwav_pack.py; the rest are
that song taken apart and put back together with the tool's own helpers
(wav_header, wmet_json, wlin_json, interleave), then broken in one way each.
The films are packed by swav_pack.py.
"""
import argparse
import filecmp
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from array import array

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
WWAV_PACK = os.path.join(ROOT, "formats", "prana", "tools", "wwav_pack.py")
SWAV_PACK = os.path.join(ROOT, "formats", "formats", "swav", "swav_pack.py")
sys.path.insert(0, os.path.dirname(WWAV_PACK))
import wwav_pack as wp  # noqa: E402  the reference's own helpers

FRAMES = 600  # a short song keeps every file small; nothing here depends on length
SONG_ID = "0123456789abcdef0123456789abcdef"
REMIX_ID = "fedcba9876543210fedcba9876543210"
FILM_ID = "00112233445566778899aabbccddeeff"
CREATED = "2026-10-03"
CREATOR = "liam_made_young"
PCM_GUID = bytes.fromhex("0100000000001000800000aa00389b71")  # KSDATAFORMAT_SUBTYPE_PCM

# Each file's verdict from the reference tool; the C++ device and Wi's
# JavaScript are expected to say the same unless "differs" says otherwise.
STEMS = "4 stems, and the master"
PLAIN = "the master only: no wmet and wlin, so a plain WAV"
NOT_LISTED = "not listed: the master isn't 44.1 kHz 16-bit stereo PCM"
SWAV = "a .swav: the film, its wmet and wlin"
PLAIN_MP4 = "a plain MP4: no wmet and wlin"


def pcm(track, frames=FRAMES, channels=2, bytes_per_sample=2):
    """A different integer sawtooth per track and channel, full scale."""
    out = bytearray()
    for f in range(frames):
        for c in range(channels):
            v = (f * (131 + 37 * track) + c * 9973 + track * 4099) % 65536 - 32768
            out += (v * (1 << (8 * bytes_per_sample - 16))).to_bytes(bytes_per_sample, "little", signed=True)
    return bytes(out)


def fmt_body(rate=44100, bits=16, channels=2):
    block = channels * bits // 8
    return struct.pack("<HHIIHH", 1, channels, rate, rate * block, block, bits)


def extensible_fmt_body():
    """WAVE_FORMAT_EXTENSIBLE, 40 bytes, with PCM in its subformat."""
    return struct.pack("<HHIIHHHHI", 0xFFFE, 2, 44100, 176400, 4, 16, 22, 16, 3) + PCM_GUID


class Stems:
    """A wstm payload: its header and zeros up to the next 512-byte boundary
    in the file, wherever the chunk lands, then the frames."""

    def __init__(self, audio, frames, version=1):
        self.audio, self.frames, self.version = audio, frames, version

    def payload(self, at):
        pad = (wp.ALIGN - (at + wp.WSTM_HEADER) % wp.ALIGN) % wp.ALIGN
        return struct.pack("<HBBBBHII", self.version, 4, 2, 16, 0, pad, 44100, self.frames) + bytes(pad) + self.audio


def riff(chunks, pad_last=True):
    """A RIFF WAVE of (id, payload) chunks in this order, each padded to an
    even length (the last one only if pad_last), with a RIFF size that fits."""
    out = bytearray(b"RIFF\0\0\0\0WAVE")
    for i, (cid, body) in enumerate(chunks):
        if isinstance(body, Stems):
            body = body.payload(len(out) + 8)
        out += cid + struct.pack("<I", len(body)) + body
        if len(body) & 1 and (pad_last or i < len(chunks) - 1):
            out += b"\0"
    struct.pack_into("<I", out, 4, len(out) - 8)
    return bytes(out)


def chunk_bytes(cid, body):
    return cid + struct.pack("<I", len(body)) + body + b"\0" * (len(body) & 1)


def chunks_of(b):
    """[(id, payload)] of a whole RIFF file, as wwav_pack.py walks it."""
    out, at = [], 12
    while at + 8 <= len(b):
        cid, n = struct.unpack("<4sI", b[at:at + 8])
        out.append((cid, b[at + 8:at + 8 + n]))
        at += 8 + n + (n & 1)
    return out


def tool(script, *args):
    return subprocess.run([sys.executable, script, *args], capture_output=True, text=True, check=True).stdout


def verdict(path):
    if path.endswith((".mp4", ".swav")):
        return re.search(r"^  reads as: (.*)$", tool(SWAV_PACK, "info", path), re.M).group(1)
    return re.search(r"^  on PRANA: (.*)$", tool(WWAV_PACK, "info", path), re.M).group(1)


def song_folder(work, name, txt):
    folder = os.path.join(work, name)
    os.makedirs(folder)
    for t, n in enumerate(["master"] + wp.STEMS):
        with open(os.path.join(folder, n + ".wav"), "wb") as f:
            f.write(wp.wav_header(FRAMES) + pcm(t))
    with open(os.path.join(folder, "song.txt"), "w", encoding="utf-8") as f:
        f.write(txt)
    return folder


def ffmpeg(out, *flags):
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y",
         "-f", "lavfi", "-i", "testsrc=duration=1:size=64x64:rate=10",
         "-f", "lavfi", "-i", "sine=frequency=440:duration=1:sample_rate=44100",
         "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p", "-threads", "1",
         "-c:a", "aac", "-b:a", "32k", "-ac", "1",
         "-fflags", "+bitexact", "-flags:v", "+bitexact", "-flags:a", "+bitexact", "-map_metadata", "-1",
         *flags, out],
        check=True)


def boxes_of(b):
    """[(type, start, size)] of an MP4's top-level boxes (32-bit sizes only)."""
    out, at = [], 0
    while at < len(b):
        n, kind = struct.unpack(">I4s", b[at:at + 8])
        out.append((kind, at, n))
        at += n
    return out


def box(kind, text):
    body = text.encode()
    return struct.pack(">I", 8 + len(body)) + kind + body


def make(out):
    work = tempfile.mkdtemp(prefix="wwav-corpus-")
    entries = []

    def add(name, data, why, want, differs=None):
        path = os.path.join(out, name)
        if data is not None:
            with open(path, "wb") as f:
                f.write(data)
        got = verdict(path)
        if got != want:
            sys.exit(f"make_corpus: {name}: the reference says {got!r}, not {want!r}")
        e = {"file": name, "why": why, "verdict": got}
        if differs:
            e["differs"] = differs
        entries.append(e)

    # ---- .wwav: one song, packed by the tool -------------------------------------

    folder = song_folder(work, "01 Corpus Song",
                         f'title = Tést "Song"\nartist = Mi\nbpm = 120.125\nkey = A minor\n'
                         f"song_id = {SONG_ID}\ncreated = {CREATED}\n")
    original = os.path.join(out, "original.wwav")
    tool(WWAV_PACK, "pack", folder, "-o", original, "--creator", CREATOR)
    with open(original, "rb") as f:
        song = f.read()
    parts = dict(chunks_of(song))
    fmt, data, wmet, wlin = parts[b"fmt "], parts[b"data"], parts[b"wmet"], parts[b"wlin"]
    stem_audio = parts[b"wstm"][-FRAMES * 16:]
    stems = Stems(stem_audio, FRAMES)
    # the assembler reproduces the tool's bytes, so every variant below is
    # the tool's song with exactly one thing changed
    assert riff([(b"fmt ", fmt), (b"data", data), (b"wmet", wmet), (b"wstm", stems), (b"wlin", wlin)]) == song
    add("original.wwav", None,
        "An original packed by wwav_pack.py: a quote and an accent in the title, bpm 120.125 (an exact binary tie, "
        "written 120.12), a creator.", STEMS)

    odd = song_folder(work, "02 Odd", f"title = Odds\nartist = Mi\nsong_id = {SONG_ID}\ncreated = {CREATED}\n")
    odd_out = os.path.join(out, "odd-chunks.wwav")
    tool(WWAV_PACK, "pack", odd, "-o", odd_out, "--creator", "od")
    with open(odd_out, "rb") as f:
        odd_parts = dict(chunks_of(f.read()))
    assert len(odd_parts[b"wmet"]) & 1 and len(odd_parts[b"wlin"]) & 1, "both odd: each takes a pad byte"
    assert not len(wmet) & 1, "original.wwav's wmet is even, so both cases are here"
    add("odd-chunks.wwav", None, "An original whose wmet and wlin both have odd lengths, so each is followed by a pad byte.",
        STEMS)

    uni = song_folder(work, "03 Unicode",
                      "title = Ünïcödé 🎵\ttab \\ backslash\nartist = Ωmega\x01\nbpm = 98\n"
                      f"song_id = {SONG_ID}\ncreated = {CREATED}\n")
    tool(WWAV_PACK, "pack", uni, "-o", os.path.join(out, "unicode.wwav"), "--splitter", "demucs")
    add("unicode.wwav", None,
        "An original whose wmet holds UTF-8 written as itself, a tab, a backslash and a control character "
        "(escaped as \\u0001), a whole bpm, a splitter and no key.", STEMS)

    # A remix as PRANA's RemixSaver writes one: wlin names its parent, and
    # wrmx (the device's writeWrmx text) comes last.
    remix_wmet = wp.wmet_json({"song_id": REMIX_ID, "title": 'Tést "Song" (remix-001)', "artist": "Mi", "bpm": 120.125,
                               "key": "A minor", "frames": FRAMES, "type": "remix", "created": CREATED}).encode()
    remix_wlin = ('{"parent_id": "%s", "root_id": "%s", "generation": 1, "creator": "", "device_id": ""}'
                  % (SONG_ID, SONG_ID)).encode()
    track = '{"stem": "%s", "vol": %s, "mute": %s, "fx": {"reverb": %s, "delay": 0.000, "distortion": 0.000, "tremolo": 0.000}}'
    wrmx = ('{"tracks": [' + ", ".join([track % ("vocals", "0.800", "false", "0.250"), track % ("drums", "0.650", "true", "0.000"),
                                        track % ("other", "0.800", "false", "0.000"), track % ("bass", "1.000", "false", "0.125")])
            + '], "master": {"vol": 1.0, "pitch": -2, "speed": 1.000, "time": 1.000, "lpf": 1.000, "hpf": 0.000}, '
            '"mode": "stems", "start": 0, "length": %d}' % FRAMES).encode()
    remix = [(b"fmt ", fmt), (b"data", data), (b"wmet", remix_wmet), (b"wstm", stems), (b"wlin", remix_wlin)]
    add("remix.wwav", riff(remix + [(b"wrmx", wrmx)]),
        "A remix in PRANA's chunk order (fmt, data, wmet, wstm, wlin, wrmx): type remix, the original as its parent and "
        "root, generation 1, and wrmx as the device writes it.", STEMS)
    add("remix-wrmx-not-json.wwav", riff(remix + [(b"wrmx", b"vocals.vol = 0.8")]),
        "A remix whose wrmx isn't JSON: info says so, and the verdict doesn't look at wrmx.", STEMS)

    add("master-only.wwav", riff([(b"fmt ", fmt), (b"data", data), (b"wmet", wmet), (b"wlin", wlin)]),
        "The original without its stems (the store's '.wwav, master only'): the same wmet and wlin, no wstm.",
        "the master only: no wstm")
    add("no-wstm.wwav",
        riff([(b"fmt ", fmt), (b"data", data), (b"wmet", wmet), (b"xstm", stems), (b"wlin", wlin)]),
        "The original with its wstm chunk renamed: identity, an unknown chunk, and no stems.", "the master only: no wstm")

    # ---- plain WAVs ------------------------------------------------------------------

    add("plain.wav", wp.wav_header(FRAMES) + data, "A plain 44.1 kHz 16-bit stereo WAV.", PLAIN)
    add("odd-list.wav", riff([(b"fmt ", fmt), (b"data", data), (b"LIST", b"abc")], pad_last=False),
        "A plain WAV ending in an odd-length LIST chunk with no pad byte, so the file's size is odd.", PLAIN)
    add("48k.wav", riff([(b"fmt ", fmt_body(rate=48000)), (b"data", pcm(0))]), "A 48 kHz WAV.", NOT_LISTED)
    add("24bit.wav", riff([(b"fmt ", fmt_body(bits=24)), (b"data", pcm(0, bytes_per_sample=3))]), "A 24-bit WAV.",
        NOT_LISTED)
    add("mono.wav", riff([(b"fmt ", fmt_body(channels=1)), (b"data", pcm(0, channels=1))]), "A mono WAV.", NOT_LISTED)
    add("extensible.wav", riff([(b"fmt ", extensible_fmt_body()), (b"data", data)]),
        "A plain WAV whose fmt is WAVE_FORMAT_EXTENSIBLE (40 bytes) with PCM in its subformat.", PLAIN)
    add("two-fmt-last-44.wav", riff([(b"fmt ", fmt_body(rate=48000)), (b"fmt ", fmt), (b"data", data)]),
        "Two fmt chunks, 48 kHz then 44.1 kHz: the last fmt wins.", PLAIN)
    add("two-fmt-last-48.wav", riff([(b"fmt ", fmt), (b"fmt ", fmt_body(rate=48000)), (b"data", data)]),
        "Two fmt chunks, 44.1 kHz then 48 kHz: the last fmt wins.", NOT_LISTED)

    # ---- the song, broken one way at a time ------------------------------------------

    def with_wmet(body):
        return riff([(b"fmt ", fmt), (b"data", data), (b"wmet", body), (b"wstm", stems), (b"wlin", wlin)])

    add("extensible.wwav",
        riff([(b"fmt ", extensible_fmt_body()), (b"data", data), (b"wmet", wmet), (b"wstm", stems), (b"wlin", wlin)]),
        "The original with a WAVE_FORMAT_EXTENSIBLE fmt, so the master starts at byte 68.", STEMS)
    add("wmet-not-json.wwav", with_wmet(b"not JSON, just words"), "A wmet that isn't JSON.", PLAIN)
    add("wmet-array.wwav", with_wmet(b"[0.1, 600]"), "A wmet that is JSON but not an object.", PLAIN)
    add("wmet-almost-json.wwav", with_wmet(wmet[:-1] + b", }"),
        "A wmet with a trailing comma, which isn't JSON.", PLAIN,
        {"prana": "4 stems, and the master",
         "because": "PRANA's JSON reader (core/base/json.cpp) looks values up without checking the rest of the text, "
                    "so it reads this wmet's version and frames and lists the stems."})
    add("wmet-nan.wwav", with_wmet(wmet.replace(b'"bpm": 120.12', b'"bpm": NaN')),
        "A wmet whose bpm is NaN, which python's json and PRANA read and JSON.parse doesn't.", STEMS,
        {"wi": PLAIN,
         "because": "JSON.parse rejects NaN, so Wi's reader (wi/src/formats/wwav.js) finds no wmet; wwav_pack.py's "
                    "json.loads takes NaN as a float, and PRANA skips a bpm it can't parse."})
    add("version-1.0.wwav", with_wmet(wmet.replace(b'"wwav": "0.1"', b'"wwav": "1.0"')),
        "A newer major version.", "the master only: version 1.0 is newer than this reader (0.x)")
    add("no-version.wwav", with_wmet(wmet.replace(b'"wwav": "0.1", ', b"")), "A wmet without its wwav key.",
        "the master only: wmet has no version")
    add("frames-wmet.wwav", with_wmet(wmet.replace(b'"frames": %d' % FRAMES, b'"frames": %d' % (FRAMES + 1))),
        "wmet's frames one more than the master's.",
        f"the master only: frame counts differ (data {FRAMES}, wstm {FRAMES}, wmet {FRAMES + 1})")

    short = Stems(stem_audio, FRAMES)
    short.frames = FRAMES - 1  # the header says one frame fewer; the chunk still holds them all
    add("frames-wstm.wwav",
        riff([(b"fmt ", fmt), (b"data", data), (b"wmet", wmet), (b"wstm", short), (b"wlin", wlin)]),
        "wstm's header one frame short of the master's.",
        f"the master only: frame counts differ (data {FRAMES}, wstm {FRAMES - 1}, wmet {FRAMES})")
    add("wstm-v2.wwav",
        riff([(b"fmt ", fmt), (b"data", data), (b"wmet", wmet), (b"wstm", Stems(stem_audio, FRAMES, version=2)),
              (b"wlin", wlin)]),
        "A wstm header of version 2.", "the master only: wstm isn't v1, 4 stereo 16-bit stems at 44.1 kHz")
    add("wstm-cut-off.wwav",
        riff([(b"fmt ", fmt), (b"data", data), (b"wmet", wmet), (b"wstm", Stems(stem_audio[:-100 * 16], FRAMES)),
              (b"wlin", wlin)]),
        "A wstm chunk 100 frames short of its header's count, with wlin whole after it.",
        "the master only: wstm is cut off")

    stems_at = song.index(stem_audio)
    add("cut-in-stems.wwav", song[:stems_at + 1000],
        "The original cut off in the middle of its stems: wstm is the last chunk, cut off, and wlin is gone.", PLAIN)
    add("wlin-cut-off.wwav", song[:-5], "The original missing its last 5 bytes: wlin is the last chunk, cut off.", PLAIN)
    add("junk-cut-off.wwav", song + b"JUNK" + struct.pack("<I", 4096) + bytes(100),
        "The original, then a JUNK chunk that says 4096 bytes and has 100: a cut-off last chunk after a whole song.",
        STEMS)

    add("dup-data.wwav", song + chunk_bytes(b"data", data[:400]),
        "A second, shorter data chunk after wlin: the first chunk of each id is the one read, so the master is the "
        "first.", STEMS)

    second = wmet.replace('"title": "Tést \\"Song\\""'.encode(), b'"title": "Second"')
    assert second != wmet
    add("dup-wmet.wwav",
        riff([(b"fmt ", fmt), (b"data", data), (b"wmet", wmet), (b"wstm", stems), (b"wlin", wlin), (b"wmet", second)]),
        "A second wmet after wlin with another title: the first chunk of each id is the one read.", STEMS)
    add("dup-wmet-newer.wwav",
        riff([(b"fmt ", fmt), (b"data", data), (b"wmet", wmet), (b"wstm", stems), (b"wlin", wlin),
              (b"wmet", wmet.replace(b'"wwav": "0.1"', b'"wwav": "1.0"'))]),
        "A second wmet after wlin saying version 1.0: the first chunk of each id is the one read.", STEMS,
        {"prana": "the master only",
         "because": "PRANA's chunk walk (core/disc/wwav.cpp wwavChunk) keeps the last chunk of each id, so it reads "
                    "the second wmet's version 1.0."})

    # ---- films ------------------------------------------------------------------------

    plain_mp4 = os.path.join(out, "plain.mp4")
    ffmpeg(plain_mp4)
    fast_mp4 = os.path.join(out, "fast.mp4")
    ffmpeg(fast_mp4, "-movflags", "+faststart")
    with open(plain_mp4, "rb") as f:
        plain = f.read()
    with open(fast_mp4, "rb") as f:
        fast = f.read()
    assert [b[0] for b in boxes_of(plain)] == [b"ftyp", b"free", b"mdat", b"moov"]
    assert [b[0] for b in boxes_of(fast)] == [b"ftyp", b"moov", b"free", b"mdat"]
    add("plain.mp4", None, "A tiny H.264 and AAC film from ffmpeg: ftyp, free, mdat, moov.", PLAIN_MP4)
    add("fast.mp4", None, "The same film with fast start: ftyp, moov, free, mdat.", PLAIN_MP4)

    _, last, _ = boxes_of(fast)[-1]
    zero = bytearray(fast)
    struct.pack_into(">I", zero, last, 0)
    add("zero.mp4", bytes(zero), "fast.mp4 with its last box (mdat) of size 0: to the end of the file.", PLAIN_MP4)

    # ffmpeg leaves an 8-byte free box before mdat so the mdat header can grow
    # to 16 bytes without moving the media: grow it, as a film over 4 GB has it
    (_, free_at, _), (_, mdat_at, mdat_n) = boxes_of(plain)[1:3]
    assert mdat_at == free_at + 8
    large = plain[:free_at] + struct.pack(">I4sQ", 1, b"mdat", mdat_n + 8) + plain[mdat_at + 8:]
    add("large.mp4", large, "plain.mp4 with a 64-bit mdat size (size 1, then the size after the type).", PLAIN_MP4)

    film_txt = f'title = Film "One" — é\nartist = Mi\nfilm_id = {FILM_ID}\ncreated = {CREATED}\ncreator = {CREATOR}\n'
    for name in ("plain", "zero", "large"):
        src = os.path.join(work, name + ".mp4")
        shutil.copy(os.path.join(out, name + ".mp4"), src)
        with open(os.path.join(work, name + ".txt"), "w", encoding="utf-8") as f:
            f.write(film_txt)
        tool(SWAV_PACK, "pack", src, "-o", os.path.join(out, name + ".swav"))
    add("plain.swav", None, "plain.mp4 packed by swav_pack.py with a film.txt.", SWAV)
    add("zero.swav", None, "zero.mp4 packed by swav_pack.py: its last box gets its real size first.", SWAV)
    add("large.swav", None, "large.mp4 packed by swav_pack.py: a 64-bit box before wmet and wlin.", SWAV)

    with open(os.path.join(out, "plain.swav"), "rb") as f:
        swav = f.read()
    swav_wmet = ('{"swav": "0.1", "film_id": "%s", "title": "x", "artist": "", "type": "original", "created": "%s"}'
                 % (FILM_ID, CREATED))
    swav_wlin = '{"parent_id": null, "root_id": "%s", "generation": 0, "creator": "", "device_id": ""}' % FILM_ID
    add("version-1.0.swav", swav.replace(b'"swav": "0.1"', b'"swav": "1.0"'), "A newer major version.",
        "the film only: version 1.0 is newer than this reader (0.x)")
    add("no-version.swav", plain + box(b"wmet", swav_wmet.replace('"swav": "0.1", ', "")) + box(b"wlin", swav_wlin),
        "A wmet without its swav key.", "the film only: wmet has no version")
    add("nan.swav", plain + box(b"wmet", swav_wmet[:-1] + ', "fps": NaN}') + box(b"wlin", swav_wlin),
        "A wmet with NaN, which isn't JSON to any .swav reader.", PLAIN_MP4)
    add("wlin-not-json.swav", plain + box(b"wmet", swav_wmet) + box(b"wlin", "parent: none"),
        "A wlin that isn't JSON.", PLAIN_MP4)
    add("not-at-end.swav", swav + struct.pack(">I4s", 8, b"free"),
        "plain.swav with a box after wlin: it reads, but unpack refuses to move the film.", SWAV)
    child_wlin = ('{"parent_id": "%s", "root_id": "%s", "generation": 1, "creator": "%s", "device_id": ""}'
                  % (SONG_ID, SONG_ID, CREATOR))
    add("child.swav", plain + box(b"wmet", swav_wmet) + box(b"wlin", child_wlin),
        "A film whose parent is original.wwav's song (one id space): what the Console's export writes.", SWAV)

    shutil.rmtree(work)
    for e in entries:
        with open(os.path.join(out, e["file"]), "rb") as f:
            e["sha256"] = hashlib.sha256(f.read()).hexdigest()
    with open(os.path.join(out, "manifest.json"), "w", encoding="utf-8") as f:
        json.dump({"about": "Made by tools/corpus/make_corpus.py. verdict is what wwav_pack.py or swav_pack.py info says; "
                            "differs names a reader known to say something else, and why.",
                   "files": entries}, f, ensure_ascii=False, indent=2)
        f.write("\n")
    for e in entries:
        size = os.path.getsize(os.path.join(out, e["file"]))
        assert size < 200_000, f"{e['file']} is {size} bytes"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--out", default=os.path.join(ROOT, "tests", "corpus"))
    ap.add_argument("--check", action="store_true", help="make it again elsewhere and compare")
    a = ap.parse_args()
    if not a.check:
        if os.path.isdir(a.out):
            shutil.rmtree(a.out)
        os.makedirs(a.out)
        make(a.out)
        print(f"{a.out}: {len(os.listdir(a.out)) - 1} files and manifest.json")
        return
    fresh = tempfile.mkdtemp(prefix="wwav-corpus-check-")
    try:
        make(fresh)
        names = sorted(set(os.listdir(fresh)) | set(os.listdir(a.out)))
        bad = [n for n in names if not (os.path.exists(os.path.join(a.out, n)) and os.path.exists(os.path.join(fresh, n))
                                        and filecmp.cmp(os.path.join(a.out, n), os.path.join(fresh, n), shallow=False))]
    finally:
        shutil.rmtree(fresh)
    if bad:
        sys.exit(f"make_corpus: these differ from a fresh run: {', '.join(bad)}")
    print(f"{a.out}: the same as a fresh run")


if __name__ == "__main__":
    main()
