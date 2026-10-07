"""A .wwav plays its four stems (S0.3, engine side), with the clock and meters
in shared memory (docs/ENGINE.md 3.3-3.5, 4.2-4.4).

Fails if: the clock doesn't advance at the session rate while playing, or
uses another clock than the reader's monotonic one; the meters don't show
each stem on its own bus; a mute or solo isn't heard within one block of the
command's arrival (the block after it fades, PRANA's one-block ramp, and
every block after that is silent); a reader ever sees a torn clock; a
session.load during playback stops it, moves the playhead or plays a block
of the old graph after it has answered; a session.load that fails changes
what plays; a feature of a later stage is silently ignored instead of
refused as "unsupported".
"""
import math
import os
import shutil
import tempfile
import time
import unittest
from array import array

from wwav_client import (RATE, STEMS, TONES, TRACK_IDS, Engine, EngineError, Shm, sine, sine_song, stem_session,
                         wav_bytes)

BUS = {s: 4 + i for i, s in enumerate(STEMS)}  # meter slots: four tracks, then the buses, then master
MASTER = 8
AMP = {s: a for s, (_, a) in zip(STEMS, TONES)}


def song(seconds=6):
    d = tempfile.mkdtemp(prefix="wwav-song-")
    path, frames, stems = sine_song(os.path.join(d, "01 Engine Test"), seconds)
    return d, path, frames


class Playback(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir, cls.path, cls.frames = song()

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def setUp(self):
        self.shm = Shm()
        self.addCleanup(self.shm.close)
        self.engine = Engine(self.shm, rate=RATE, block=1024)
        self.addCleanup(self.engine.stop)
        self.c = self.engine.connect()
        self.load_result = self.c.call("session.load", {"graph": stem_session(self.path, self.frames), "playhead": 0,
                                                         "off": []})

    def play(self):
        self.c.call("transport.play")
        self.shm.wait_callbacks(3)

    def latest(self):
        k = self.shm.clock()["callbacks"]
        return self.shm.meters_since(k - 1)[-1]

    def level(self, entry, slot):
        peak_l, peak_r, rms_l, rms_r = entry["slots"][slot]
        return peak_l, peak_r, rms_l, rms_r

    def test_load_result(self):
        r = self.load_result
        self.assertEqual(r["nodes"], 9)
        self.assertEqual(r["latency"], {})
        want = {tid: i for i, tid in enumerate(TRACK_IDS)}
        want.update({f"bus:{s}": BUS[s] for s in STEMS})
        want["master"] = MASTER
        self.assertEqual(r["meter_slots"], want)

    def test_clock_advances_at_the_session_rate(self):
        self.play()
        a = self.shm.clock()
        time.sleep(1.0)
        b = self.shm.clock()
        now = time.monotonic_ns()
        self.assertEqual(b["state"], 1)
        self.assertEqual(b["rate"], float(RATE))
        moved = b["sample_pos"] - a["sample_pos"]
        took = (b["host_time_ns"] - a["host_time_ns"]) / 1e9
        self.assertAlmostEqual(moved / took, RATE, delta=RATE * 0.01)
        self.assertAlmostEqual(moved, RATE * 1.0, delta=RATE * 0.1)
        # host_time_ns is on the reader's monotonic clock (CLOCK_MONOTONIC here).
        self.assertLess(abs(now - b["host_time_ns"]), 200_000_000)
        self.assertGreaterEqual(b["callbacks"] - a["callbacks"], 40)

    def test_meters_show_the_four_buses(self):
        self.play()
        e = self.latest()
        self.assertEqual(e["used"], 9)
        self.assertGreaterEqual(e["load"], 0.0)
        self.assertLess(e["load"], 1.0)
        for s in STEMS:
            peak_l, peak_r, rms_l, rms_r = self.level(e, BUS[s])
            with self.subTest(s):
                self.assertAlmostEqual(peak_l, AMP[s], delta=AMP[s] * 0.02)
                self.assertAlmostEqual(peak_r, AMP[s], delta=AMP[s] * 0.02)
                self.assertAlmostEqual(rms_l, AMP[s] / math.sqrt(2), delta=AMP[s] * 0.03)
                # each track meters the same as its bus: one track per role
                self.assertEqual(self.level(e, STEMS.index(s)), self.level(e, BUS[s]))
        master_peak = self.level(e, MASTER)[0]
        self.assertGreater(master_peak, 0.3)
        self.assertLessEqual(master_peak, sum(AMP.values()) + 1e-6)

    def _wait_for_block_edge(self):
        """Returns the callback count just after a block, so a command sent now lands between blocks."""
        k = self.shm.clock()["callbacks"]
        while self.shm.clock()["callbacks"] == k:
            time.sleep(0.0002)
        return self.shm.clock()["callbacks"]

    def _heard_within_one_block(self, op_args, silent_slots, untouched_slots):
        self.play()
        normal = self.latest()
        k0 = self._wait_for_block_edge()
        self.c.call("param.set", op_args)
        k1 = self.shm.clock()["callbacks"]  # every block after k1 started after the command arrived
        self.shm.wait_callbacks(5)
        entries = self.shm.meters_since(k0)
        self.assertEqual([e["callback"] for e in entries], list(range(k0 + 1, k0 + 1 + len(entries))))
        slot = silent_slots[0]
        rms = self.level(normal, slot)[2]
        fade = next(e["callback"] for e in entries if self.level(e, slot)[2] < rms * 0.9)
        self.assertGreater(fade, k0)
        self.assertLessEqual(fade, k1 + 1, f"the change began {fade - k1} blocks after the command arrived")
        fading = next(e for e in entries if e["callback"] == fade)
        self.assertGreater(self.level(fading, slot)[2], 0.0, "a cut instead of a one-block fade")
        for e in entries:
            for s in untouched_slots:
                self.assertAlmostEqual(self.level(e, s)[0], self.level(normal, s)[0], delta=0.01)
            if e["callback"] > fade:
                for s in silent_slots:
                    self.assertEqual(self.level(e, s), (0.0, 0.0, 0.0, 0.0), f"slot {s} at callback {e['callback']}")

    def test_mute_is_heard_within_one_block(self):
        self.assertEqual(self.c.call("param.set", {"node": TRACK_IDS[0], "param": "mute", "value": False}), {})
        self._heard_within_one_block({"node": TRACK_IDS[0], "param": "mute", "value": True},
                                     silent_slots=[BUS["vocals"], 0], untouched_slots=[BUS["drums"], BUS["bass"]])

    def test_solo_is_heard_within_one_block(self):
        self._heard_within_one_block({"node": TRACK_IDS[1], "param": "solo", "value": True},
                                     silent_slots=[BUS["vocals"], BUS["other"], BUS["bass"], 0, 2, 3],
                                     untouched_slots=[BUS["drums"], 1])

    def test_unmute_comes_back(self):
        self.c.call("param.set", {"node": TRACK_IDS[3], "param": "mute", "value": True})
        self.play()
        self.assertEqual(self.level(self.latest(), BUS["bass"])[0], 0.0)
        self.c.call("param.set", {"node": TRACK_IDS[3], "param": "mute", "value": False})
        self.shm.wait_callbacks(3)
        self.assertAlmostEqual(self.level(self.latest(), BUS["bass"])[0], AMP["bass"], delta=0.01)

    def test_gain_pan_buses_and_master(self):
        self.play()
        half = -20 * math.log10(2)
        self.c.call("param.set", {"node": "bus:vocals", "param": "gain_db", "value": half})
        self.c.call("param.set", {"node": TRACK_IDS[1], "param": "pan", "value": 1.0})
        self.c.call("param.set", {"node": "bus:other", "param": "mute", "value": True})
        self.c.call("param.set", {"node": "master", "param": "gain_db", "value": -120.0})
        self.shm.wait_callbacks(3)
        e = self.latest()
        self.assertAlmostEqual(self.level(e, BUS["vocals"])[0], AMP["vocals"] / 2, delta=0.003)
        self.assertAlmostEqual(self.level(e, 0)[0], AMP["vocals"], delta=0.003)  # the track is before its bus
        drums = self.level(e, 1)
        self.assertLess(drums[0], 1e-3)  # hard right: the left side is gone
        self.assertAlmostEqual(drums[1], AMP["drums"], delta=0.003)
        self.assertEqual(self.level(e, BUS["other"]), (0.0, 0.0, 0.0, 0.0))
        self.assertLess(self.level(e, MASTER)[0], 1e-5)
        # bus solo: only that bus reaches the master
        self.c.call("param.set", {"node": "master", "param": "gain_db", "value": 0.0})
        self.c.call("param.set", {"node": "bus:other", "param": "mute", "value": False})
        self.c.call("param.set", {"node": "bus:bass", "param": "solo", "value": True})
        self.shm.wait_callbacks(3)
        e = self.latest()
        for s in ("vocals", "drums", "other"):
            self.assertEqual(self.level(e, BUS[s])[0], 0.0, s)
        self.assertAlmostEqual(self.level(e, MASTER)[0], AMP["bass"], delta=0.003)

    def test_param_errors(self):
        cases = [
            ({"node": "01JNOSUCHNODE", "param": "mute", "value": True}, "no_such_node"),
            ({"node": TRACK_IDS[0], "param": "wobble", "value": 1.0}, "no_such_param"),
            ({"node": "master", "param": "solo", "value": True}, "no_such_param"),
            ({"node": TRACK_IDS[0], "param": "mute", "value": "yes"}, "bad_args"),
            ({"node": TRACK_IDS[0], "param": "gain_db", "value": "loud"}, "bad_args"),
            ({"node": TRACK_IDS[0], "param": "pan", "value": 2.0}, "bad_args"),
            ({"node": TRACK_IDS[0], "param": "send.reverb", "value": 0.5}, "unsupported"),
            ({"node": TRACK_IDS[0], "param": "gain_db", "value": -3.0, "at": 48000}, "unsupported"),
            ({"node": 5, "param": "mute", "value": True}, "bad_args"),
            ({"param": "mute", "value": True}, "bad_args"),
            ({"node": "x\u0000", "param": "mute", "value": True}, "bad_args"),
        ]
        for args, code in cases:
            with self.subTest(args=args):
                with self.assertRaises(EngineError) as e:
                    self.c.call("param.set", args)
                self.assertEqual(e.exception.code, code)

    def test_transport_and_its_events(self):
        r = self.c.call("transport.locate", {"sample": RATE})
        self.assertEqual(r, {"sample": RATE})
        self.c.wait_event("transport", state="stopped", sample=RATE)
        self.shm.wait_callbacks(2)
        clock = self.shm.clock()
        self.assertEqual((clock["state"], clock["sample_pos"], clock["rate"]), (0, RATE, 0.0))

        self.assertEqual(self.c.call("transport.play"), {"sample": RATE})
        self.c.wait_event("transport", state="playing", sample=RATE)
        time.sleep(0.25)
        stopped = self.c.call("transport.stop")["sample"]
        self.c.wait_event("transport", state="stopped", sample=stopped)
        self.assertGreater(stopped, RATE + RATE // 10)
        self.shm.wait_callbacks(2)
        clock = self.shm.clock()
        self.assertEqual((clock["state"], clock["sample_pos"], clock["rate"]), (0, stopped, 0.0))
        # stopped: the buses are silent
        e = self.latest()
        for s in STEMS:
            self.assertEqual(self.level(e, BUS[s])[0], 0.0)

    def test_loop(self):
        start, end = RATE // 2, RATE
        self.assertEqual(self.c.call("transport.loop", {"on": True, "start": start, "end": end}), {})
        self.c.call("transport.locate", {"sample": start})
        self.c.call("transport.play")
        seen, wrapped, last = [], False, None
        deadline = time.monotonic() + 1.5
        while time.monotonic() < deadline:
            pos = self.shm.clock()["sample_pos"]
            seen.append(pos)
            if last is not None and pos < last:
                wrapped = True
            last = pos
            time.sleep(0.003)
        self.assertTrue(wrapped, "the playhead never jumped back to the loop start")
        self.assertGreaterEqual(min(seen), start)
        self.assertLess(max(seen), end)
        self.c.call("transport.loop", {"on": False, "start": 0, "end": 0})
        for op, args in [("transport.loop", {"on": True, "start": end, "end": start}),
                         ("transport.loop", {"on": True, "start": 0, "end": 2 ** 50 + 1}),
                         ("transport.locate", {"sample": 2 ** 60})]:
            with self.subTest(op=op, args=args):
                with self.assertRaises(EngineError) as e:
                    self.c.call(op, args)
                self.assertEqual(e.exception.code, "bad_args")

    def test_a_load_while_playing_swaps_without_stopping(self):
        self.play()
        self.c.events.clear()
        before = self.shm.clock()
        graph = stem_session(self.path, self.frames)
        graph["tracks"][0]["gain_db"] = -20 * math.log10(2)
        r = self.c.call("session.load", {"graph": graph, "off": []})  # no playhead: keep it
        self.assertEqual(r["nodes"], 9)
        k = self.shm.clock()["callbacks"]
        self.shm.wait_callbacks(3)
        after = self.shm.clock()
        self.assertEqual(after["state"], 1)
        self.assertGreater(after["sample_pos"], before["sample_pos"])
        entries = self.shm.meters_since(k)
        # session.load answers once the swap is done, so every block after its answer plays the
        # new graph: vocals at half, the rest as they were.
        for e in entries:
            self.assertAlmostEqual(self.level(e, BUS["vocals"])[0], AMP["vocals"] / 2, delta=0.003)
            self.assertAlmostEqual(self.level(e, BUS["bass"])[0], AMP["bass"], delta=0.003)
        self.assertEqual([e["callback"] for e in entries], list(range(k + 1, k + 1 + len(entries))))
        self.assertEqual([e for e in self.c.events if e["ev"] == "transport" and e["state"] == "stopped"], [])

    def test_unload(self):
        self.play()
        self.assertEqual(self.c.call("session.unload"), {})
        self.shm.wait_callbacks(2)
        self.assertEqual(self.latest()["used"], 0)
        with self.assertRaises(EngineError) as e:
            self.c.call("param.set", {"node": "master", "param": "gain_db", "value": 0.0})
        self.assertEqual(e.exception.code, "no_session")

    def _device(self, **kw):
        d = {"id": "01JTESTDEVICE0000000000000", "format": "vst3", "uid": "ABCDEF0123456789ABCDEF0123456789",
             "path": "/Library/Audio/Plug-Ins/VST3/Tape Echo.vst3", "params": {}, "state": None, "on": True}
        d.update(kw)
        return d

    def test_later_stages_are_refused_not_ignored(self):
        g = lambda **kw: stem_session(self.path, self.frames, **kw)  # noqa: E731
        master_device = stem_session(self.path, self.frames)
        master_device["master"]["devices"] = [self._device(format="builtin", uid="reverb", path=None)]
        instrument = stem_session(self.path, self.frames)
        instrument["tracks"].append({"id": "01JTESTSYNTH00000000000000", "kind": "instrument", "role": "other",
                                     "gain_db": 0.0, "pan": 0.0, "mute": False, "solo": False,
                                     "sends": {"reverb": 0.0, "delay": 0.0}, "devices": [],
                                     "notes": [{"pitch": 60, "vel": 100, "at": 0, "len": 1000}]})
        reverse = stem_session(self.path, self.frames)
        reverse["tracks"][0]["clips"][0]["reverse"] = True
        for name, graph in [("a VST3 insert", g(devices=[self._device()])),
                            ("a built-in on the master", master_device),
                            ("a send", g(sends={"reverb": 0.5, "delay": 0.0})),
                            ("an instrument track", instrument),
                            ("a reversed clip", reverse)]:
            with self.subTest(name):
                with self.assertRaises(EngineError) as e:
                    self.c.call("session.load", {"graph": graph, "playhead": 0, "off": []})
                self.assertEqual(e.exception.code, "unsupported")
                self.assertTrue(e.exception.message.endswith("."), e.exception.message)
        # A device that loads switched off, or is off, needs nothing from this stage.
        self.c.call("session.load", {"graph": g(devices=[self._device()]), "playhead": 0,
                                     "off": ["01JTESTDEVICE0000000000000"]})
        self.c.call("session.load", {"graph": g(devices=[self._device(on=False)]), "playhead": 0, "off": []})

    def test_a_failed_load_changes_nothing(self):
        self.play()
        bad = stem_session("/no/such/file.wwav", self.frames)
        with self.assertRaises(EngineError) as e:
            self.c.call("session.load", {"graph": bad, "playhead": 0, "off": []})
        self.assertEqual(e.exception.code, "no_such_file")
        self.shm.wait_callbacks(2)
        e = self.latest()
        self.assertEqual(e["used"], 9)
        self.assertAlmostEqual(self.level(e, BUS["bass"])[0], AMP["bass"], delta=0.01)

    def test_bad_sessions(self):
        plain = os.path.join(self.dir, "plain.wav")
        with open(plain, "wb") as f:
            f.write(wav_bytes(sine(1000.0, 0.5, RATE), 2))
        fast = os.path.join(self.dir, "fast.wav")
        with open(fast, "wb") as f:
            f.write(wav_bytes(sine(1000.0, 0.5, 48000, rate=48000), 2, rate=48000))
        dup = stem_session(self.path, self.frames)
        dup["tracks"][1]["id"] = dup["tracks"][0]["id"]
        role = stem_session(self.path, self.frames)
        role["tracks"][0]["role"] = "keys"
        stem_of_plain = stem_session(plain, RATE)
        at_48k = stem_session(self.path, self.frames, rate=48000)
        clip_48k = stem_session(fast, 48000)
        for t in clip_48k["tracks"]:
            t["kind"] = "audio"
            t["clips"][0]["source"] = "master"
        long_id = stem_session(self.path, self.frames)
        long_id["tracks"][0]["id"] = "\x7f" * 1025  # its answer would list it as \u007f, six times as long
        far = stem_session(self.path, self.frames)
        far["tracks"][0]["clips"][0]["len"] = 2 ** 50 + 1
        cases = [("duplicate ids", dup, "bad_session"), ("an unknown role", role, "bad_session"),
                 ("an id over 1 KiB", long_id, "bad_session"), ("a clip past 2^50 frames", far, "bad_session"),
                 ("a stem of a plain WAV", stem_of_plain, "bad_clip"),
                 ("a session rate the device isn't at", at_48k, "rate_mismatch"),
                 ("a clip at another rate", clip_48k, "unsupported")]
        for name, graph, code in cases:
            with self.subTest(name):
                with self.assertRaises(EngineError) as e:
                    self.c.call("session.load", {"graph": graph, "playhead": 0, "off": []})
                self.assertEqual(e.exception.code, code)

    def test_audio_track_of_a_mono_wav(self):
        mono = array("h", [round(0.5 * 32767 * math.sin(2 * math.pi * 1000 * n / RATE)) for n in range(RATE * 2)])
        path = os.path.join(self.dir, "mono.wav")
        with open(path, "wb") as f:
            f.write(wav_bytes(mono, 1))
        graph = stem_session(self.path, self.frames)
        graph["tracks"] = [{"id": "01JTESTMONO000000000000000", "kind": "audio", "role": "other", "gain_db": 0.0,
                            "pan": 0.0, "mute": False, "solo": False, "sends": {"reverb": 0.0, "delay": 0.0},
                            "devices": [], "clips": [{"id": "01JTESTMONOCLIP00000000000", "path": path,
                                                      "source": "master", "at": RATE // 2, "in": 0, "len": RATE,
                                                      "gain_db": 0.0, "reverse": False}]}]
        r = self.c.call("session.load", {"graph": graph, "playhead": 0, "off": []})
        self.assertEqual(r["meter_slots"]["bus:other"], 3)
        self.c.call("transport.play")
        time.sleep(0.8)
        e = self.latest()
        peak_l, peak_r, _, _ = self.level(e, 3)
        self.assertAlmostEqual(peak_l, 0.5, delta=0.01)
        self.assertAlmostEqual(peak_r, 0.5, delta=0.01)
        self.assertEqual(self.level(e, 1)[0], 0.0)  # bus:vocals
        time.sleep(1.0)  # past the clip's end
        self.assertEqual(self.level(self.latest(), 3)[0], 0.0)


class Clock(unittest.TestCase):
    def test_a_reader_never_sees_a_torn_clock(self):
        """F8, engine side: 3,000 clock writes a second against a reader in another process."""
        shm = Shm()
        self.addCleanup(shm.close)
        engine = Engine(shm, rate=48000, block=16)
        self.addCleanup(engine.stop)
        c = engine.connect()
        c.call("transport.play")
        shm.wait_callbacks(10)
        first = shm.clock()
        reads, deadline, last = 0, time.monotonic() + 0.5, first
        while time.monotonic() < deadline:
            k = shm.clock()
            reads += 1
            self.assertEqual(k["seq"] % 2, 0)
            self.assertEqual((k["state"], k["rate"]), (1, 48000.0))
            # one block per callback while playing: every consistent read sits on this line
            self.assertEqual(k["sample_pos"] - first["sample_pos"], 16 * (k["callbacks"] - first["callbacks"]))
            self.assertGreaterEqual(k["callbacks"], last["callbacks"])
            self.assertGreaterEqual(k["host_time_ns"], last["host_time_ns"])
            last = k
        self.assertGreater(last["callbacks"] - first["callbacks"], 1000)
        self.assertGreater(reads, 1000)


if __name__ == "__main__":
    unittest.main()
