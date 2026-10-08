"""Recording a take (docs/ENGINE.md 3.10, 4.5), on the null device, whose
input is its own output: so a take can be held against what was played,
sample for sample.

Fails if: a take recorded while playing isn't the master that was played,
frame for frame, from the sample `at` names; the clock doesn't say recording
while a take runs and the transport plays; the take's audio and its peaks
aren't in shared memory as it is recorded; a take stopped leaves a WAVE file
whose header isn't its length; a second take, a stop without a take, or a
device change in the middle of one is accepted.
"""
import os
import shutil
import struct
import tempfile
import time
import unittest

from wwav_client import (MR, ENTRY, PEAKS_BYTES, RATE, RING_AT, Engine, EngineError, Shm, first_difference, read_wav,
                         sine_song, stem_session)

PEAKS_AT = RING_AT + MR * ENTRY  # docs/ENGINE.md 4.5
INPUT_AT = PEAKS_AT + PEAKS_BYTES
RING_HEADER = 64
PEAK_ENTRIES = (PEAKS_BYTES - RING_HEADER) // 16


class Record(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-record-")
        cls.path, cls.frames, _ = sine_song(os.path.join(cls.dir, "01 Engine Test"), 6)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def setUp(self):
        self.shm = Shm()
        self.addCleanup(self.shm.close)
        self.engine = Engine(self.shm, rate=RATE, block=256)
        self.addCleanup(self.engine.stop)
        self.c = self.engine.connect()
        self.c.call("session.load", {"graph": stem_session(self.path, self.frames), "playhead": 0, "off": []})
        self.out = tempfile.mkdtemp(prefix="wwav-take-")
        self.addCleanup(shutil.rmtree, self.out)
        self.take = os.path.join(self.out, "take.wav")

    def rendered(self, start, frames):
        """The master of the session from `start`, as the engine renders it."""
        r = self.c.call("render", {"out_dir": os.path.join(self.out, "r"), "start": start, "len": frames,
                                   "master": True, "stems": False, "format": "f32"}, timeout=60)
        return read_wav(r["files"]["master"])[4]

    def test_a_take_is_what_was_played_from_the_sample_it_names(self):
        self.c.call("transport.locate", {"sample": RATE})
        self.c.call("transport.play")
        time.sleep(0.2)
        started = self.c.call("record.start", {"path": self.take})
        self.assertEqual((started["input"], started["channels"], started["sample_rate"]), ("null", 2, RATE))
        self.assertEqual(started["input_latency"], 0)
        self.assertGreater(started["at"], RATE)
        self.c.wait_event("record", state="recording", at=started["at"])
        self.shm.wait_callbacks(3)
        self.assertEqual(self.shm.clock()["state"], 2, "the clock doesn't say recording")
        time.sleep(0.6)
        stopped = self.c.call("record.stop")
        self.c.wait_event("record", state="stopped", frames=stopped["frames"])
        self.shm.wait_callbacks(2)
        self.assertEqual(self.shm.clock()["state"], 1, "recording ended, the transport plays on")
        self.c.call("transport.stop")
        self.assertEqual((stopped["path"], stopped["at"], stopped["dropped"]), (self.take, started["at"], 0))
        self.assertGreater(stopped["frames"], RATE // 2)
        self.assertEqual(stopped["frames"] % 256, 0, "a take on the timer is whole blocks")
        self.assertAlmostEqual(stopped["peak"], 0.9, delta=0.2)

        tag, channels, rate, bits, samples = read_wav(self.take)
        self.assertEqual((tag, channels, rate, bits), (3, 2, RATE, 32))
        self.assertEqual(len(samples), 2 * stopped["frames"])
        self.assertIsNone(first_difference(samples, self.rendered(started["at"], stopped["frames"])))

    def test_a_take_shows_in_shared_memory_as_it_is_recorded(self):
        self.c.call("transport.play")
        started = self.c.call("record.start", {"path": self.take})
        time.sleep(0.5)
        stopped = self.c.call("record.stop")
        self.c.call("transport.stop")
        n = stopped["frames"]
        write, take, at, per_peak, channels = struct.unpack_from("<QQqII", self.shm.map, PEAKS_AT)
        self.assertEqual((take, at, per_peak, channels), (1, started["at"], 256, 2))
        self.assertEqual(write, n // 256)
        frames, take, rate, channels = struct.unpack_from("<QQII", self.shm.map, INPUT_AT)
        self.assertEqual((frames, take, rate, channels), (n, 1, RATE, 2))
        samples = read_wav(self.take)[4]
        ring = list(struct.unpack_from(f"<{2 * n}f", self.shm.map, INPUT_AT + RING_HEADER))
        self.assertIsNone(first_difference(ring, samples))
        for i in (0, write // 2, write - 1):
            lo_l, hi_l, lo_r, hi_r = struct.unpack_from("<4f", self.shm.map, PEAKS_AT + RING_HEADER + 16 * i)
            left = samples[2 * 256 * i: 2 * 256 * (i + 1): 2]
            right = samples[2 * 256 * i + 1: 2 * 256 * (i + 1): 2]
            self.assertEqual((lo_l, hi_l, lo_r, hi_r), (min(left), max(left), min(right), max(right)), f"peak {i}")
        self.assertLess(write, PEAK_ENTRIES)

    def test_a_take_while_stopped_starts_where_the_playhead_stands(self):
        self.c.call("transport.locate", {"sample": 5000})
        started = self.c.call("record.start", {"path": self.take})
        self.assertEqual(started["at"], 5000)
        self.shm.wait_callbacks(4)
        self.assertEqual(self.shm.clock()["state"], 0)
        stopped = self.c.call("record.stop")
        self.assertGreater(stopped["frames"], 0)
        self.assertEqual(stopped["peak"], 0.0)
        self.assertEqual(set(read_wav(self.take)[4]), {0.0})

    def test_one_take_at_a_time(self):
        with self.assertRaises(EngineError) as e:
            self.c.call("record.stop")
        self.assertEqual(e.exception.code, "not_recording")
        for args, code in [({"path": "relative/take.wav"}, "bad_args"), ({}, "bad_args"),
                           ({"path": self.take, "input": 5}, "bad_args"),
                           ({"path": os.path.join(self.out, "no", "such", "dir", "t.wav")}, "record_failed"),
                           ({"path": self.take, "input": "No Such Input 9000"}, "no_such_device")]:
            with self.subTest(args=args):
                with self.assertRaises(EngineError) as e:
                    self.c.call("record.start", args)
                self.assertEqual(e.exception.code, code)
        self.c.call("record.start", {"path": self.take})
        for op, args in [("record.start", {"path": self.take + "2"}),
                         ("device.open", {"name": "null", "sample_rate": RATE, "block": 512})]:
            with self.subTest(op):
                with self.assertRaises(EngineError) as e:
                    self.c.call(op, args)
                self.assertEqual(e.exception.code, "recording")
        self.c.call("record.stop")
        # and another after it
        again = os.path.join(self.out, "again.wav")
        self.c.call("record.start", {"path": again})
        self.shm.wait_callbacks(3)
        self.assertGreater(self.c.call("record.stop")["frames"], 0)
        self.assertEqual(struct.unpack_from("<QQ", self.shm.map, PEAKS_AT)[1], 2, "the second take")

    def test_a_take_in_hand_when_the_engine_goes_keeps_a_true_header(self):
        self.c.call("transport.play")
        self.c.call("record.start", {"path": self.take})
        time.sleep(0.3)
        self.engine.close_stdin()
        self.engine.proc.wait(timeout=1.0)
        tag, channels, rate, bits, samples = read_wav(self.take)  # read_wav checks the RIFF size
        self.assertEqual((tag, channels, rate, bits), (3, 2, RATE, 32))
        self.assertGreater(len(samples), RATE // 4)


if __name__ == "__main__":
    unittest.main()
