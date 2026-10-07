"""Reader threads (docs/SPEC.md 9.4): a .wwav over 32 MB is not held whole;
its stems stream from the file in 1024-frame runs, kept ahead of the playhead.

Fails if: a streamed stem drops out (a silent block) during steady playback,
after a load, or after a locate while stopped; a locate during playback
doesn't recover within a tenth of a second; a render of streamed stems
differs from the stems that were packed.
"""
import os
import shutil
import tempfile
import time
import unittest

from wwav_client import (RATE, STEMS, TONES, Engine, Shm, first_difference, make_song, mix, periodic, read_wav,
                         sine, stem_session)

SECONDS = 40  # 20 bytes a frame: about 35 MB, over the 32 MB held whole
# Whole frames per cycle, so long stems are one cycle repeated: 441, 220.5, 315 and 110.25 Hz.
CYCLES = [100, 200, 140, 400]
AMP = {s: a for s, (_, a) in zip(STEMS, TONES)}


class Streaming(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-stream-")
        cls.frames = SECONDS * RATE
        lcm = 2800
        cycles = [periodic(sine(RATE / n, a, n), lcm) for n, (_, a) in zip(CYCLES, TONES)]
        cls.stems = [periodic(c, cls.frames) for c in cycles]
        master = periodic(mix(cycles, lcm), cls.frames)
        cls.path = make_song(os.path.join(cls.dir, "01 Long"), cls.frames, cls.stems, master)
        assert os.path.getsize(cls.path) > 32 * 1024 * 1024

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def setUp(self):
        self.shm = Shm()
        self.addCleanup(self.shm.close)
        self.engine = Engine(self.shm, rate=RATE, block=512)
        self.addCleanup(self.engine.stop)
        self.c = self.engine.connect()

    def load(self, playhead):
        self.c.call("session.load", {"graph": stem_session(self.path, self.frames), "playhead": playhead, "off": []})

    def assert_no_gaps(self, entries):
        self.assertTrue(entries)
        for e in entries:
            for i, s in enumerate(STEMS):
                peak = e["slots"][4 + i][0]
                self.assertAlmostEqual(peak, AMP[s], delta=AMP[s] * 0.05,
                                       msg=f"{s} at callback {e['callback']}: peak {peak}")

    def collect(self, after, seconds):
        """Every meter entry after callback `after` for `seconds`, read often enough that the
        64-entry ring never laps the reader."""
        seen = {}
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            for e in self.shm.meters_since(after):
                seen[e["callback"]] = e
            time.sleep(0.15)
        entries = [seen[k] for k in sorted(seen)]
        self.assertEqual([e["callback"] for e in entries], list(range(after + 1, after + 1 + len(entries))))
        return entries

    def play_for(self, seconds):
        self.c.call("transport.play")
        k = self.shm.clock()["callbacks"]  # play answers once a block has started playing
        return self.collect(k, seconds)

    def test_steady_playback_after_a_load(self):
        self.load(playhead=30 * RATE)
        self.assert_no_gaps(self.play_for(1.2))

    def test_locate_while_stopped_then_play(self):
        self.load(playhead=0)
        self.c.call("transport.locate", {"sample": 12 * RATE})
        self.assert_no_gaps(self.play_for(1.0))
        self.c.call("transport.stop")
        self.c.call("transport.locate", {"sample": 3 * RATE})
        self.assert_no_gaps(self.play_for(0.5))

    def test_locate_while_playing_recovers(self):
        self.load(playhead=0)
        self.c.call("transport.play")
        time.sleep(0.3)
        self.c.call("transport.locate", {"sample": 25 * RATE})
        located = self.shm.clock()["callbacks"]
        time.sleep(0.1)
        k = self.shm.clock()["callbacks"]
        self.assertGreater(k, located)
        self.assert_no_gaps(self.collect(k, 0.5))

    def test_a_load_with_a_playhead_during_playback_plays_on_from_there(self):
        # A restart while playing, or the app moving the playhead with a new graph: every block
        # after the answer plays the new session from the new playhead, already read ahead.
        self.load(playhead=0)
        self.c.call("transport.play")
        time.sleep(0.3)
        self.load(playhead=30 * RATE)
        k = self.shm.clock()["callbacks"]
        clock = self.shm.clock()
        self.assertEqual(clock["state"], 1)
        self.assertGreaterEqual(clock["sample_pos"], 30 * RATE)
        self.assertLess(clock["sample_pos"], 30 * RATE + RATE // 10)
        self.assert_no_gaps(self.collect(k, 0.5))

    def test_render_of_streamed_stems(self):
        self.load(playhead=0)
        out = tempfile.mkdtemp(prefix="wwav-out-")
        self.addCleanup(shutil.rmtree, out)
        start, n = 35 * RATE + 77, 2 * RATE
        r = self.c.call("render", {"out_dir": out, "start": start, "len": n, "master": True, "stems": True,
                                   "format": "s16"}, timeout=60)
        for name, stem in zip(STEMS, self.stems):
            got = read_wav(r["files"][name])[4]
            want = [v / 32768.0 for v in stem[2 * start: 2 * (start + n)]]
            self.assertIsNone(first_difference(got, want), name)


if __name__ == "__main__":
    unittest.main()
