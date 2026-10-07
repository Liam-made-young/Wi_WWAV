"""Offline render (docs/ENGINE.md 3.6) and the fold rule (docs/SPEC.md 5.3, 6.6).

Fails if: the four stems of a render don't sum to its master within float
rounding; a stem read in place from a .wwav differs from the stem that was
packed; two renders of the same session, in one engine or in two, differ by
any byte; a file's sha256 differs from the one in the result; the WAVE
files aren't 32-bit float or 16-bit PCM at the session rate; playback keeps
running through a render or comes back playing; pings go unanswered while a
render runs.
"""
import hashlib
import os
import shutil
import tempfile
import time
import unittest

from wwav_client import (RATE, STEMS, TRACK_IDS, Engine, EngineError, Shm, first_difference, read_wav, sine_song,
                         stem_session)

SECONDS = 3


class Render(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-render-")
        cls.path, cls.frames, stems = sine_song(os.path.join(cls.dir, "01 Engine Test"), SECONDS + 1)
        cls.stems = {name: [v / 32768.0 for v in s] for name, s in zip(STEMS, stems)}

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def setUp(self):
        self.out = tempfile.mkdtemp(prefix="wwav-out-")
        self.addCleanup(shutil.rmtree, self.out)
        self.engine, self.c, self.shm = self.start()

    def start(self):
        shm = Shm()
        self.addCleanup(shm.close)
        engine = Engine(shm, rate=RATE, block=1024)
        self.addCleanup(engine.stop)
        c = engine.connect()
        c.call("session.load", {"graph": stem_session(self.path, self.frames), "playhead": 0, "off": []})
        return engine, c, shm

    def render(self, c=None, sub="a", **kw):
        args = {"out_dir": os.path.join(self.out, sub), "start": 0, "len": SECONDS * RATE, "master": True,
                "stems": True, "format": "f32"}
        args.update(kw)
        return (c or self.c).call("render", args, timeout=120)

    def check_hashes(self, r):
        for name, path in r["files"].items():
            self.assertTrue(os.path.isabs(path), path)
            with open(path, "rb") as f:
                self.assertEqual(hashlib.sha256(f.read()).hexdigest(), r["sha256"][name], name)

    def test_fold_rule_and_stems_read_in_place(self):
        r = self.render()
        self.assertEqual(set(r["files"]), {"master", *STEMS})
        self.assertEqual(r["frames"], SECONDS * RATE)
        self.check_hashes(r)
        audio = {}
        for name, path in r["files"].items():
            self.assertEqual(os.path.basename(path), name + ".wav")
            tag, ch, rate, bits, samples = read_wav(path)
            self.assertEqual((tag, ch, rate, bits), (3, 2, RATE, 32), name)
            self.assertEqual(len(samples), 2 * r["frames"], name)
            audio[name] = samples
        n = 2 * r["frames"]
        # Each stem is its track at unity: exactly the 16-bit stem that was packed.
        for name in STEMS:
            self.assertIsNone(first_difference(audio[name], self.stems[name][:n]), name)
        # The fold rule: the master before its chain is the sum of the four stems.
        worst = max(abs(audio["master"][i] - sum(audio[s][i] for s in STEMS)) for i in range(n))
        self.assertLess(worst, 1e-6)
        progress = [e for e in self.c.events if e["ev"] == "render.progress"]
        self.assertTrue(progress)
        self.assertTrue(all(e["stage"] in ("master", "stems") and e["total"] == r["frames"] for e in progress))
        self.assertEqual(progress[-1]["done"], r["frames"])
        self.assertEqual([e["done"] for e in progress], sorted(e["done"] for e in progress))

    def test_identical_across_runs_and_engines(self):
        first = self.render(sub="1")["sha256"]
        self.assertEqual(self.render(sub="2")["sha256"], first)
        _, c2, _ = self.start()
        self.assertEqual(self.render(c=c2, sub="3")["sha256"], first)

    def test_an_offset_render_with_a_muted_track(self):
        self.c.call("param.set", {"node": TRACK_IDS[1], "param": "mute", "value": True})
        r = self.render(start=1000, len=RATE)
        audio = {name: read_wav(path)[4] for name, path in r["files"].items()}
        self.assertIsNone(first_difference(audio["drums"], [0.0] * (2 * RATE)))
        self.assertIsNone(first_difference(audio["vocals"], self.stems["vocals"][2000:2000 + 2 * RATE]))
        worst = max(abs(audio["master"][i] - audio["vocals"][i] - audio["other"][i] - audio["bass"][i])
                    for i in range(2 * RATE))
        self.assertLess(worst, 1e-6)

    def test_s16(self):
        r = self.render(format="s16", len=RATE)
        self.check_hashes(r)
        audio = {}
        for name, path in r["files"].items():
            tag, ch, rate, bits, samples = read_wav(path)
            self.assertEqual((tag, ch, rate, bits), (1, 2, RATE, 16), name)
            audio[name] = samples
        for name in STEMS:
            self.assertIsNone(first_difference(audio[name], self.stems[name][:2 * RATE]), name)
        # 16-bit stems at unity sum exactly in float, so the 16-bit master is their exact sum.
        for i in range(2 * RATE):
            self.assertEqual(audio["master"][i], sum(audio[s][i] for s in STEMS))

    def test_master_only_and_stems_only(self):
        self.assertEqual(set(self.render(sub="m", stems=False, len=RATE)["files"]), {"master"})
        r = self.render(sub="s", master=False, len=RATE)
        self.assertEqual(set(r["files"]), set(STEMS))
        self.assertEqual(set(r["sha256"]), set(STEMS))

    def test_playback_stops_for_a_render_and_comes_back_stopped(self):
        self.c.call("transport.play")
        self.c.wait_event("transport", state="playing")
        time.sleep(0.2)
        self.c.events.clear()
        self.render(len=RATE)
        stopped = self.c.wait_event("transport", state="stopped")
        self.shm.wait_callbacks(2)
        clock = self.shm.clock()
        self.assertEqual((clock["state"], clock["rate"]), (0, 0.0))
        self.assertEqual(clock["sample_pos"], stopped["sample"])
        self.assertGreater(stopped["sample"], 0)

    def test_pings_are_answered_while_a_render_runs(self):
        rid = 900
        self.c.send({"id": rid, "op": "render", "args": {"out_dir": os.path.join(self.out, "long"), "start": 0,
                                                          "len": 120 * RATE, "master": True, "stems": False}})
        ping = self.c.request("ping", timeout=2)
        self.assertTrue(ping["ok"])
        self.assertNotIn(rid, self.c.responses, "the render finished before the ping was answered")
        r = self.c.response(rid, timeout=120)
        self.assertTrue(r["ok"], r)
        self.assertEqual(r["result"]["frames"], 120 * RATE)

    def test_bad_renders(self):
        cases = [({"len": 0}, "bad_args"), ({"format": "mp3"}, "bad_args"), ({"out_dir": "relative/dir"}, "bad_args"),
                 ({"master": False, "stems": False}, "bad_args"), ({"start": -1}, "bad_args")]
        for kw, code in cases:
            with self.subTest(kw=kw):
                with self.assertRaises(EngineError) as e:
                    self.render(**kw)
                self.assertEqual(e.exception.code, code)
        self.c.call("session.unload")
        with self.assertRaises(EngineError) as e:
            self.render()
        self.assertEqual(e.exception.code, "no_session")


if __name__ == "__main__":
    unittest.main()
