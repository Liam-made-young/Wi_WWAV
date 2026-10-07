"""Defects an independent review found in the first cut of wwav-engine.

Every test here fails against the engine as it stands, so each is skipped
unless WWAV_REVIEW_FINDINGS=1; the skip reason names the finding. Run them
with:

  cd engine/build && WWAV_REVIEW_FINDINGS=1 ctest -R engine_review --output-on-failure

Remove a test's skip once its finding is fixed.
"""
import json
import math
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import unittest

from wwav_client import (MAX_FRAME, PACK, RATE, STEMS, TONES, TRACK_IDS, Client, Engine, EngineError, Shm, make_song,
                         mix, periodic, sine, sine_song, stem_session)

FINDINGS = bool(os.environ.get("WWAV_REVIEW_FINDINGS"))


def finding(text):
    return unittest.skipUnless(FINDINGS, "finding: " + text)


BUS_VOCALS = 4  # meter slots of a four-track stem session: tracks 0-3, then the buses


class Sessions(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-review-")
        cls.path, cls.frames, _ = sine_song(os.path.join(cls.dir, "01 Short"), 1)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def setUp(self):
        self.shm = Shm()
        self.addCleanup(self.shm.close)
        self.engine = Engine(self.shm, rate=RATE, block=512)
        self.addCleanup(self.engine.stop)
        self.c = self.engine.connect()

    @finding("a clip held whole allocates its len, not its file: a long len on a small file kills the engine "
             "(std::bad_alloc, SIGABRT), and a restart that reloads the session dies the same way")
    def test_a_clip_longer_than_its_file_does_not_kill_the_engine(self):
        graph = stem_session(self.path, self.frames)
        for t in graph["tracks"]:
            t["clips"][0]["len"] = 10 ** 12  # frames past the file's end are silence (media.h)
        try:
            self.c.request("session.load", {"graph": graph, "playhead": 0, "off": []}, timeout=30)
        except ConnectionError:
            pass
        time.sleep(0.2)
        self.assertIsNone(self.engine.proc.poll(), f"the engine died (status {self.engine.proc.poll()})")

    @finding("wstm stem verdicts differ from wwav_pack.py's (atoi overflow on the version, json::whole on a "
             "float frame count, PRANA's JSON taking trailing text, a 1 MiB cap on wmet)")
    def test_stem_verdicts_match_the_reference_reader(self):
        raw = open(self.path, "rb").read()
        chunks, at = [], 12
        while at + 8 <= len(raw):
            cid, n = struct.unpack_from("<4sI", raw, at)
            chunks.append((cid, raw[at + 8: at + 8 + n]))
            at += 8 + n + (n & 1)
        wmet = json.loads(dict(chunks)[b"wmet"])

        def with_wmet(text):
            body = b"WAVE"
            for cid, p in chunks:
                p = text if cid == b"wmet" else p
                body += struct.pack("<4sI", cid, len(p)) + p + (b"\0" if len(p) & 1 else b"")
            return b"RIFF" + struct.pack("<I", len(body)) + body

        variants = {
            "a 20-digit version": json.dumps({**wmet, "wwav": "99999999999999999999"}).encode(),
            "frames written as 44100.0": json.dumps({**wmet, "frames": float(wmet["frames"])}).encode(),
            "text after the object": json.dumps(wmet).encode() + b" }x",
            "a wmet over 1 MiB": json.dumps({**wmet, "notes": "x" * (1 << 20)}).encode(),
        }
        for name, text in variants.items():
            with self.subTest(name):
                path = os.path.join(self.dir, name.replace(" ", "_") + ".wwav")
                with open(path, "wb") as f:
                    f.write(with_wmet(text))
                info = subprocess.run([sys.executable, "-I", PACK, "info", path], capture_output=True, text=True,
                                      check=True).stdout
                reference = info.split("on PRANA: ")[-1].strip()
                try:
                    self.c.call("session.load", {"graph": stem_session(path, self.frames), "playhead": 0, "off": []})
                    engine = "4 stems, and the master"
                except EngineError as e:
                    engine = e.message.split(": ", 1)[-1].rstrip(".")
                self.assertEqual(engine, reference)


class Streaming(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-review-")
        cls.frames = 40 * RATE  # about 35 MB: streamed, as every song over about 1:40 is
        cycles = [periodic(sine(RATE / n, a, n), 2800) for n, (_, a) in zip([100, 200, 140, 400], TONES)]
        stems = [periodic(c, cls.frames) for c in cycles]
        cls.path = make_song(os.path.join(cls.dir, "01 Long"), cls.frames, stems, periodic(mix(cycles, 2800),
                                                                                           cls.frames))

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    @finding("a loop that jumps back into a streamed clip plays silence after every wrap: the reader's window "
             "only holds frames ahead of the playhead, so the loop start is reread after the jump")
    def test_a_loop_over_a_streamed_clip_never_drops_audio(self):
        shm = Shm()
        self.addCleanup(shm.close)
        engine = Engine(shm, rate=RATE, block=512)
        self.addCleanup(engine.stop)
        c = engine.connect()
        c.call("session.load", {"graph": stem_session(self.path, self.frames), "playhead": 10 * RATE, "off": []})
        c.call("transport.loop", {"on": True, "start": 10 * RATE, "end": 13 * RATE})
        c.call("transport.play")
        k = shm.clock()["callbacks"]
        seen, deadline = {}, time.monotonic() + 7.0  # two wraps
        while time.monotonic() < deadline:
            for e in shm.meters_since(k):
                seen[e["callback"]] = e
            time.sleep(0.1)
        entries = [seen[x] for x in sorted(seen)]
        self.assertEqual([e["callback"] for e in entries], list(range(k + 1, k + 1 + len(entries))))
        rms = TONES[0][1] / math.sqrt(2)
        short = [(e["callback"], round(e["slots"][BUS_VOCALS][2], 3)) for e in entries
                 if e["slots"][BUS_VOCALS][2] < rms * 0.9]
        self.assertEqual(short, [], "blocks where the vocals bus lost audio (callback, RMS)")


class Mute(unittest.TestCase):
    @finding("param.set waits behind whatever op the worker is running, so a mute sent during a session.load "
             "is heard only after the whole load, far past one block (S0.3's 10 ms)")
    def test_a_mute_is_heard_within_10_ms_while_a_load_runs(self):
        d = tempfile.mkdtemp(prefix="wwav-review-")
        self.addCleanup(shutil.rmtree, d)
        path, frames, _ = sine_song(os.path.join(d, "01 Held"), 30)  # 26 MB: held whole, decoded at load
        shm = Shm()
        self.addCleanup(shm.close)
        engine = Engine(shm, rate=RATE, block=128)
        self.addCleanup(engine.stop)
        c = engine.connect()
        c.call("session.load", {"graph": stem_session(path, frames), "playhead": 0, "off": []})
        c.call("transport.play")
        shm.wait_callbacks(5)
        # The app sends a new graph (a track added, say), and the user clicks a stem light.
        bigger = stem_session(path, frames)
        for i in range(8):
            for t in stem_session(path, frames)["tracks"]:
                t["id"] = t["id"][:-2] + f"{i:02d}"
                bigger["tracks"].append(t)
        c.send({"id": 100, "op": "session.load", "args": {"graph": bigger, "off": []}})
        sent = time.monotonic()
        c.send({"id": 101, "op": "param.set", "args": {"node": "bus:vocals", "param": "mute", "value": True}})
        while time.monotonic() - sent < 5:
            e = shm.meters_since(shm.clock()["callbacks"] - 1)[-1]
            if e["used"] >= 9 and e["slots"][e["used"] - 5][0] == 0.0:  # bus:vocals, in either graph
                break
        took = time.monotonic() - sent
        c.response(100, timeout=30)
        c.response(101, timeout=30)
        self.assertLess(took, 0.010, f"the mute was heard {took * 1000:.0f} ms after it was sent")


class Envelope(unittest.TestCase):
    def setUp(self):
        self.shm = Shm()
        self.addCleanup(self.shm.close)
        self.engine = Engine(self.shm, rate=48000, block=256)
        self.addCleanup(self.engine.stop)

    @finding("JUCE's recursive JSON parser overflows the socket thread's stack: a 200 KB frame of nested "
             "arrays kills the engine (SIGSEGV) instead of closing the connection")
    def test_deep_nesting_does_not_kill_the_engine(self):
        c = self.engine.connect()
        depth = 100000
        c.send_raw(b'{"id": 1, "op": "ping", "args": {"a": ' + b"[" * depth + b"]" * depth + b"}}")
        c.closed(timeout=2)
        time.sleep(0.2)
        self.assertIsNone(self.engine.proc.poll(), f"the engine died (status {self.engine.proc.poll()})")
        self.engine.connect().call("ping")

    @finding("payloads that aren't one JSON object are answered, not closed: text after the object, two objects "
             "(the second request then never gets a response), a trailing comma, invalid UTF-8")
    def test_a_payload_that_isnt_one_json_object_closes_the_connection(self):
        bad = {
            "text after the object": b'{"id": 5, "op": "ping"} trailing',
            "two objects": b'{"id": 5, "op": "ping"}{"id": 6, "op": "ping"}',
            "a trailing comma": b'{"id": 5, "op": "ping",}',
            "invalid UTF-8": b'{"id": 5, "op": "ping", "args": {"x": "\xff\xfe"}}',
        }
        for name, payload in bad.items():
            with self.subTest(name):
                c = self.engine.connect()
                self.engine.clients.remove(c)
                try:
                    c.send_raw(payload)
                    self.assertTrue(c.closed(), name)
                    self.assertEqual(c.responses, {}, f"{name} was answered")
                finally:
                    c.close()  # one client at a time: the next subtest's connect waits for this one

    @finding("a JSON object whose string holds the escape \\u0000 is valid JSON, but the engine closes the "
             "connection as if it weren't")
    def test_a_nul_escape_is_valid_json(self):
        c = self.engine.connect()
        r = c.request("hello", {"protocol": 1, "client": "wi_wwav\u00000.1.0"}, timeout=2)
        self.assertTrue(r["ok"], r)

    @finding("an error message echoes the request's strings, so a request at the 16 MiB limit gets a reply "
             "frame over it, which the app must treat as a broken connection")
    def test_no_reply_is_over_16_mib(self):
        d = tempfile.mkdtemp(prefix="wwav-review-")
        self.addCleanup(shutil.rmtree, d)
        path, frames, _ = sine_song(os.path.join(d, "01 S"), 1)
        c = self.engine.connect()
        c.call("device.open", {"name": "null", "sample_rate": RATE, "block": 256})
        c.call("session.load", {"graph": stem_session(path, frames), "playhead": 0, "off": []})
        head = b'{"id":4,"op":"param.set","args":{"node":"master","value":1,"param":"'
        tail = b'"}}'
        c.send_raw(head + b"x" * (MAX_FRAME - len(head) - len(tail)) + tail)
        (n,) = struct.unpack("<I", c._read(4, time.monotonic() + 20))
        self.assertLessEqual(n, MAX_FRAME)


class Replacement(unittest.TestCase):
    @finding("an engine that exits cleanly unlinks its socket path even when a replacement engine has since "
             "bound the same path, so the app can no longer reach the replacement")
    def test_an_exiting_engine_leaves_its_replacements_socket(self):
        sock_dir = tempfile.mkdtemp(prefix=f"wwav-{os.getpid()}-")
        self.addCleanup(shutil.rmtree, sock_dir, True)
        old_shm, new_shm = Shm(), Shm()
        self.addCleanup(old_shm.close)
        self.addCleanup(new_shm.close)
        old = Engine(old_shm, rate=48000, block=256, sock_dir=sock_dir)
        self.addCleanup(old.stop)
        c = old.connect()
        new = Engine(new_shm, rate=48000, block=256, sock_dir=sock_dir)
        self.addCleanup(new.stop)
        c.call("shutdown")
        old.proc.wait(timeout=2)
        n = Client(new.sock_path)
        new.clients.append(n)
        self.assertEqual(n.call("hello", {"protocol": 1, "client": "app"})["protocol"], 1)


if __name__ == "__main__":
    unittest.main()
