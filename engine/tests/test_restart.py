"""When the engine falls over (docs/ENGINE.md 1, 5; docs/SPEC.md 9.3): S0.2's
Linux part, the restart-ready engine.

Fails if: after kill -9 during playback, a fresh engine isn't listening,
loaded with the same session and stopped at the last sample_pos the clock
showed, within 2 s of the kill; closing the engine's stdin doesn't make it
exit within 1 s; shutdown doesn't; the crumb doesn't name the node that was
running when debug.crumb crashed it; debug.crash and debug.hang don't do
what they say on the thread they name.
"""
import os
import shutil
import signal
import tempfile
import time
import unittest

from wwav_client import RATE, TRACK_IDS, Engine, EngineError, Shm, fnv1a64, sine_song, stem_session


class Restart(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-restart-")
        cls.path, cls.frames, _ = sine_song(os.path.join(cls.dir, "01 Engine Test"), 4)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def setUp(self):
        self.shm = Shm()
        self.addCleanup(self.shm.close)

    def start(self, **kw):
        engine = Engine(self.shm, rate=RATE, block=256, **kw)
        self.addCleanup(engine.stop)
        return engine

    def load(self, c, playhead=0):
        return c.call("session.load", {"graph": stem_session(self.path, self.frames), "playhead": playhead,
                                       "off": []})

    def test_kill_9_during_playback_then_back_stopped_at_the_playhead_within_2_s(self):
        sock_dir = tempfile.mkdtemp(prefix=f"wwav-{os.getpid()}-")
        old = self.start(sock_dir=sock_dir)
        c = old.connect()
        self.load(c)
        c.call("transport.play")
        time.sleep(0.6)
        last = self.shm.clock()
        self.assertEqual(last["state"], 1)
        self.assertGreater(last["sample_pos"], 0)

        os.kill(old.proc.pid, signal.SIGKILL)
        killed = time.monotonic()
        old.proc.wait()
        # As the app does (docs/ENGINE.md 5): the last sample_pos the clock showed, a new
        # engine on the same socket path and region, hello, device.open, session.load, locate.
        last = self.shm.clock()
        new = Engine(self.shm, rate=RATE, block=256, sock_dir=sock_dir)
        self.addCleanup(new.stop)
        c2 = new.connect()
        c2.call("device.open", {"name": "null", "sample_rate": RATE, "block": 256})
        self.load(c2, playhead=last["sample_pos"])
        self.assertEqual(c2.call("transport.locate", {"sample": last["sample_pos"]}), {"sample": last["sample_pos"]})
        self.shm.wait_callbacks(1)  # a block of the new engine's has written the clock since the locate
        clock = self.shm.clock()
        took = time.monotonic() - killed
        self.assertEqual(self.shm.header()["pid"], new.proc.pid)
        self.assertEqual(clock["state"], 0, "the transport came back running")
        self.assertEqual(clock["sample_pos"], last["sample_pos"])
        self.assertLess(took, 2.0)
        print(f"\n  back in {took * 1000:.0f} ms, stopped at {clock['sample_pos']}", end=" ")
        # Nothing resumes on its own.
        time.sleep(0.2)
        self.assertEqual(self.shm.clock()["state"], 0)
        self.assertEqual(self.shm.clock()["sample_pos"], last["sample_pos"])

    def test_closing_stdin_exits_within_1_s(self):
        engine = self.start()
        c = engine.connect()
        self.load(c)
        c.call("transport.play")
        self.shm.wait_callbacks(3)
        closed = time.monotonic()
        engine.close_stdin()
        self.assertEqual(engine.proc.wait(timeout=1.0), 0)
        self.assertLess(time.monotonic() - closed, 1.0)
        # and the audio stopped with it
        k = self.shm.clock()["callbacks"]
        time.sleep(0.1)
        self.assertEqual(self.shm.clock()["callbacks"], k)

    def test_closing_stdin_mid_render_still_exits_within_1_s(self):
        engine = self.start()
        c = engine.connect()
        self.load(c)
        out = tempfile.mkdtemp(prefix="wwav-out-")
        self.addCleanup(shutil.rmtree, out)
        c.send({"id": 50, "op": "render", "args": {"out_dir": out, "start": 0, "len": 600 * RATE, "master": True,
                                                    "stems": True}})
        c.wait_event("render.progress")
        closed = time.monotonic()
        engine.close_stdin()
        engine.proc.wait(timeout=1.0)
        self.assertLess(time.monotonic() - closed, 1.0)

    def test_shutdown_exits_within_1_s(self):
        engine = self.start()
        c = engine.connect()
        self.load(c)
        c.call("transport.play")
        asked = time.monotonic()
        self.assertEqual(c.call("shutdown"), {})
        self.assertEqual(engine.proc.wait(timeout=1.0), 0)
        self.assertLess(time.monotonic() - asked, 1.0)

    def test_debug_crumb_names_the_node(self):
        engine = self.start()
        c = engine.connect()
        self.load(c)
        c.call("transport.play")
        self.shm.wait_callbacks(2)
        c.send({"id": 9, "op": "debug.crumb", "args": {"node": TRACK_IDS[2]}})
        self.assertTrue(c.closed(timeout=3))
        self.assertEqual(engine.proc.wait(timeout=3), -signal.SIGABRT)
        crumb, seq = self.shm.crumb()
        self.assertEqual(crumb, fnv1a64(TRACK_IDS[2]))
        self.assertGreater(seq, 0)

    def test_crumb_is_cleared_between_nodes(self):
        engine = self.start()
        c = engine.connect()
        self.load(c)
        self.shm.wait_callbacks(5)
        _, seq1 = self.shm.crumb()
        self.shm.wait_callbacks(5)
        crumb, seq2 = self.shm.crumb()
        self.assertGreater(seq2, seq1)  # written around every node, every block
        # read between blocks, it is almost always clear
        clear = sum(1 for _ in range(200) if self.shm.crumb()[0] == 0)
        self.assertGreater(clear, 150)
        with self.assertRaises(EngineError) as e:
            c.call("debug.crumb", {"node": "01JNOSUCHNODE"})
        self.assertEqual(e.exception.code, "no_such_node")

    def test_debug_crash(self):
        for where in ("audio", "message"):
            with self.subTest(where):
                engine = self.start()
                c = engine.connect()
                c.send({"id": 3, "op": "debug.crash", "args": {"in": where}})
                self.assertTrue(c.closed(timeout=3))
                self.assertEqual(engine.proc.wait(timeout=3), -signal.SIGABRT)

    def test_debug_hang_audio_stalls_the_clock(self):
        engine = self.start()
        c = engine.connect()
        self.load(c)
        c.call("transport.play")
        self.assertEqual(c.call("debug.hang", {"in": "audio"}), {})
        time.sleep(0.1)
        k = self.shm.clock()["callbacks"]
        time.sleep(0.6)
        self.assertEqual(self.shm.clock()["callbacks"], k, "the clock kept moving")
        self.assertEqual(self.shm.clock()["state"], 1)
        c.call("ping")  # the message thread is fine
        engine.kill()

    def test_debug_hang_message_stops_pings(self):
        engine = self.start()
        c = engine.connect()
        self.assertEqual(c.call("debug.hang", {"in": "message"}), {})
        with self.assertRaises(TimeoutError):
            c.call("ping", timeout=1.0)
        self.shm.wait_callbacks(3)  # the audio thread is fine
        engine.kill()

    def test_bad_debug_args(self):
        engine = self.start()
        c = engine.connect()
        with self.assertRaises(EngineError) as e:
            c.call("debug.crash", {"in": "disk"})
        self.assertEqual(e.exception.code, "bad_args")


if __name__ == "__main__":
    unittest.main()
