"""Clips the engine can't play where they lie (docs/ENGINE.md 3.3): audio at
another rate than the session's, and audio in another format. Each is made
once into a WAV at the session's rate and played from there.

Fails if: a clip at another rate is refused, or plays off pitch, at the
wrong level or from the wrong place; a .wwav's stems don't play in a session
at 48 kHz, or don't come back within a thousandth when that session is
written as a .wwav again; FLAC doesn't decode to the samples it was made
from, or MP3 and AAC to the tone they were made from; a file is made again
when the cache already has it; two renders of a converted clip differ.

Compressed fixtures are made with tools on this machine (flac, lame,
afconvert); a test whose tool is missing is skipped and says so.
"""
import math
import os
import shutil
import subprocess
import tempfile
import time
import unittest

from wwav_client import (RATE, STEMS, TONES, Engine, EngineError, Shm, first_difference, read_wav, sine, sine_song,
                         stem_session, wav_bytes)

from test_wwav import reference_verdict, stems_of


def tool(*names):
    for n in names:
        found = shutil.which(n) or (n if os.path.isabs(n) and os.path.exists(n) else None)
        if found:
            return found
    return None


def audio_track(path, frames, at=0, start=0):
    return {"id": "01JTESTAUDIO00000000000000", "kind": "audio", "role": "other", "gain_db": 0.0, "pan": 0.0,
            "mute": False, "solo": False, "sends": {"reverb": 0.0, "delay": 0.0}, "devices": [],
            "clips": [{"id": "01JTESTAUDIOCLIP0000000000", "path": path, "source": "master", "at": at, "in": start,
                       "len": frames, "gain_db": 0.0, "reverse": False}]}


def session(tracks, rate=RATE):
    return {"sample_rate": rate, "tempo_map": [{"at_beats": 0, "bpm": 120.0}], "tracks": tracks,
            "master": {"gain_db": 0.0, "devices": []}}


def tone_of(samples, rate):
    """(frequency by zero crossings, peak) of the left channel of interleaved stereo samples."""
    left = samples[0::2]
    ups = sum(1 for a, b in zip(left, left[1:]) if a < 0 <= b)
    return ups * rate / len(left), max(abs(v) for v in left)


class Convert(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-convert-")
        cls.at_48k = os.path.join(cls.dir, "tone 48k.wav")
        with open(cls.at_48k, "wb") as f:
            # 1 kHz for the first second, then 2 kHz: so where a clip starts in the file can be heard
            f.write(wav_bytes(sine(1000.0, 0.5, 48000, rate=48000) + sine(2000.0, 0.25, 48000, rate=48000), 2,
                              rate=48000))
        cls.song, cls.frames, cls.stems = sine_song(os.path.join(cls.dir, "01 Engine Test"), 2)
        cls.plain = os.path.join(cls.dir, "plain.wav")
        cls.plain_samples = sine(1000.0, 0.5, RATE)
        with open(cls.plain, "wb") as f:
            f.write(wav_bytes(cls.plain_samples, 2))

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def setUp(self):
        self.cache = tempfile.mkdtemp(prefix="wwav-cache-")
        self.addCleanup(shutil.rmtree, self.cache)
        self.out = tempfile.mkdtemp(prefix="wwav-out-")
        self.addCleanup(shutil.rmtree, self.out)

    def start(self, rate=RATE):
        shm = Shm()
        self.addCleanup(shm.close)
        engine = Engine(shm, rate=rate, block=512, extra=["--cache", self.cache])
        self.addCleanup(engine.stop)
        return engine.connect()

    def render(self, c, frames, start=0, sub="r", stems=False):
        r = c.call("render", {"out_dir": os.path.join(self.out, sub), "start": start, "len": frames, "master": True,
                              "stems": stems, "format": "f32"}, timeout=60)
        return r, read_wav(r["files"]["master"])[4]

    def test_a_clip_at_another_rate_plays_on_pitch_from_the_right_place(self):
        c = self.start()
        self.assertIn("resample", c.hello()["features"])
        c.call("session.load", {"graph": session([audio_track(self.at_48k, RATE)]), "playhead": 0, "off": []})
        _, first = self.render(c, RATE // 2, start=RATE // 4, sub="a")
        hz, peak = tone_of(first, RATE)
        self.assertAlmostEqual(hz, 1000.0, delta=4.0)
        self.assertAlmostEqual(peak, 0.5, delta=0.005)
        # `in` counts the file's own frames: 48000 of them is its second second, at 2 kHz
        c.call("session.load", {"graph": session([audio_track(self.at_48k, RATE, start=48000)]), "playhead": 0,
                                "off": []})
        r1, second = self.render(c, RATE // 2, start=RATE // 4, sub="b")
        hz, peak = tone_of(second, RATE)
        self.assertAlmostEqual(hz, 2000.0, delta=4.0)
        self.assertAlmostEqual(peak, 0.25, delta=0.005)
        r2, _ = self.render(c, RATE // 2, start=RATE // 4, sub="c")
        self.assertEqual(r1["sha256"], r2["sha256"])
        # made once: one WAV for the file, however many sessions and clips use it
        self.assertEqual(sorted(os.path.splitext(n)[1] for n in os.listdir(self.cache)), [".rate", ".wav"])

    def test_the_cache_is_used_by_the_next_engine_and_not_for_a_changed_file(self):
        graph = session([audio_track(self.at_48k, RATE)])
        self.start().call("session.load", {"graph": graph, "playhead": 0, "off": []})
        made = {n: os.path.getmtime(os.path.join(self.cache, n)) for n in os.listdir(self.cache)}
        c = self.start()
        c.call("session.load", {"graph": graph, "playhead": 0, "off": []})
        self.assertEqual({n: os.path.getmtime(os.path.join(self.cache, n)) for n in os.listdir(self.cache)}, made)
        changed = os.path.join(self.dir, "changing.wav")
        shutil.copy(self.at_48k, changed)
        c.call("session.load", {"graph": session([audio_track(changed, RATE)]), "playhead": 0, "off": []})
        self.assertEqual(len(os.listdir(self.cache)), 4)
        with open(changed, "wb") as f:
            f.write(wav_bytes(sine(500.0, 0.5, 48000, rate=48000), 2, rate=48000))
        os.utime(changed, (time.time() + 5, time.time() + 5))
        c.call("session.load", {"graph": session([audio_track(changed, RATE)]), "playhead": 0, "off": []})
        hz, _ = tone_of(self.render(c, RATE // 2)[1], RATE)
        self.assertAlmostEqual(hz, 500.0, delta=4.0, msg="the old file's audio was played")

    def test_a_wwavs_stems_play_at_48k_and_come_back_as_a_wwav(self):
        c = self.start(rate=48000)
        frames = self.frames * 48000 // RATE
        c.call("session.load", {"graph": stem_session(self.song, frames, rate=48000), "playhead": 0, "off": []})
        r = c.call("render", {"out_dir": os.path.join(self.out, "s"), "start": 12000, "len": 24000, "master": False,
                              "stems": True, "format": "f32"}, timeout=60)
        for name, (freq, amp) in zip(STEMS, TONES):
            hz, peak = tone_of(read_wav(r["files"][name])[4], 48000)
            self.assertAlmostEqual(hz, freq, delta=4.0, msg=name)
            self.assertAlmostEqual(peak, amp, delta=0.003, msg=name)
        w = c.call("wwav.write", {"path": os.path.join(self.out, "back.wwav"), "start": 0, "len": frames,
                                  "dither": False, "meta": {"title": "Back", "created": "2026-10-08"}}, timeout=60)
        self.assertEqual(w["frames"], self.frames)
        self.assertEqual(reference_verdict(w["path"]), "4 stems, and the master")
        _, stems = stems_of(w["path"])
        # 44.1 -> 48 -> 44.1: away from the two ends, where the filter reads past the file, within 33 codes (0.001)
        edge = 2 * 2000
        for name, got, want in zip(STEMS, stems, self.stems):
            worst = max(abs(g - v) for g, v in zip(got[edge:-edge], list(want)[edge:-edge]))
            self.assertLessEqual(worst, 33, name)

    def lossless(self, path):
        c = self.start()
        c.call("session.load", {"graph": session([audio_track(path, RATE)]), "playhead": 0, "off": []})
        _, got = self.render(c, RATE)
        self.assertIsNone(first_difference(got, [v / 32768.0 for v in self.plain_samples]))

    def lossy(self, path):
        c = self.start()
        c.call("session.load", {"graph": session([audio_track(path, RATE)]), "playhead": 0, "off": []})
        hz, peak = tone_of(self.render(c, RATE // 2, start=RATE // 4)[1], RATE)
        self.assertAlmostEqual(hz, 1000.0, delta=4.0)
        self.assertAlmostEqual(peak, 0.5, delta=0.03)

    def made(self, name, command):
        path = os.path.join(self.dir, name)
        if not os.path.exists(path):
            subprocess.run(command + [path], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return path

    def test_flac(self):
        flac = tool("flac", "/opt/homebrew/bin/flac")
        if not flac:
            self.skipTest("no flac on this machine")
        self.lossless(self.made("plain.flac", [flac, "--silent", "-f", self.plain, "-o"]))

    def test_aiff_and_alac(self):
        afconvert = tool("/usr/bin/afconvert")
        if not afconvert:
            self.skipTest("no afconvert on this machine")
        self.lossless(self.made("plain.aiff", [afconvert, "-f", "AIFF", "-d", "BEI16", self.plain]))
        self.lossless(self.made("plain alac.m4a", [afconvert, "-f", "m4af", "-d", "alac", self.plain]))

    def test_mp3(self):
        lame = tool("lame", "/opt/homebrew/bin/lame")
        if not lame:
            self.skipTest("no lame on this machine")
        self.lossy(self.made("plain.mp3", [lame, "--quiet", "-b", "192", self.plain]))

    def test_aac(self):
        afconvert = tool("/usr/bin/afconvert")
        if not afconvert:
            self.skipTest("no afconvert on this machine")
        self.lossy(self.made("plain.m4a", [afconvert, "-f", "m4af", "-d", "aac", "-b", "192000", self.plain]))

    def test_files_that_arent_audio_are_refused_with_a_reason(self):
        c = self.start()
        text = os.path.join(self.dir, "notes.mp3")
        with open(text, "w") as f:
            f.write("not audio at all\n" * 100)
        for name, path, codes in [("text called .mp3", text, {"unsupported", "bad_clip"}),
                                  ("a missing file", os.path.join(self.dir, "gone.flac"), {"no_such_file"})]:
            with self.subTest(name):
                with self.assertRaises(EngineError) as e:
                    c.call("session.load", {"graph": session([audio_track(path, RATE)]), "playhead": 0, "off": []})
                self.assertIn(e.exception.code, codes)
                self.assertTrue(e.exception.message.endswith("."), e.exception.message)
        self.assertEqual([n for n in os.listdir(self.cache) if n.endswith(".wav")], [], "a refused file left a WAV")


if __name__ == "__main__":
    unittest.main()
