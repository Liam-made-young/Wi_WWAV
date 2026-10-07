"""Defects an independent review found in the first cut of wwav-engine, kept
as tests now that they are fixed. Each test's docstring names its finding.
"""
import ast
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

    def test_a_clip_longer_than_its_file_does_not_kill_the_engine(self):
        """Finding: a clip held whole allocates its len, not its file: a long len on a small file kills the engine
        (std::bad_alloc, SIGABRT), and a restart that reloads the session dies the same way."""
        graph = stem_session(self.path, self.frames)
        for t in graph["tracks"]:
            t["clips"][0]["len"] = 10 ** 12  # frames past the file's end are silence (media.h)
        try:
            self.c.request("session.load", {"graph": graph, "playhead": 0, "off": []}, timeout=30)
        except ConnectionError:
            pass
        time.sleep(0.2)
        self.assertIsNone(self.engine.proc.poll(), f"the engine died (status {self.engine.proc.poll()})")

    def test_stem_verdicts_match_the_reference_reader(self):
        """Finding: wstm stem verdicts differ from wwav_pack.py's (atoi overflow on the version, json::whole on a float
        frame count, PRANA's JSON taking trailing text, a 1 MiB cap on wmet)."""
        with open(self.path, "rb") as f:
            raw = f.read()
        chunks, at = [], 12
        while at + 8 <= len(raw):
            cid, n = struct.unpack_from("<4sI", raw, at)
            chunks.append((cid, raw[at + 8: at + 8 + n]))
            at += 8 + n + (n & 1)
        wmet = json.loads(dict(chunks)[b"wmet"])
        n = wmet["frames"]

        def with_chunks(new):
            body = b"WAVE"
            for cid, p in chunks:
                p = new.get(cid, p)
                body += struct.pack("<4sI", cid, len(p)) + p + (b"\0" if len(p) & 1 else b"")
            return b"RIFF" + struct.pack("<I", len(body)) + body

        def met(**fields):
            return json.dumps({**wmet, **fields}).encode()

        def extra(text):  # wmet with more members written as they are
            return json.dumps(wmet).encode()[:-1] + b", " + text + b"}"

        variants = {
            # the review's four
            "a 20-digit version": met(wwav="99999999999999999999"),
            "frames written as 44100.0": met(frames=float(n)),
            "text after the object": json.dumps(wmet).encode() + b" }x",
            "a wmet over 1 MiB": met(notes="x" * (1 << 20)),
            # versions as Python's str() prints them, and \\d's digits of any script
            "a float version": met(wwav=1.0),
            "an int version of 0": met(wwav=0),
            "an int version of 2": met(wwav=2),
            "a version of 1e16": met(wwav=1e16),
            "a version of 1e-05": met(wwav=1e-05),
            "a version of 1e400": extra(b'"wwav": 1e400'),
            "a negative version": met(wwav=-1),
            "a NaN version": met(wwav=float("nan")),
            "a null version": met(wwav=None),
            "a true version": met(wwav=True),
            "a list version": met(wwav=[0]),
            "an Arabic-Indic version": met(wwav="\u0663.0"),
            "a fullwidth zero version": met(wwav="\uff10.1"),
            "a version of 0 then 1": extra(b'"wwav": "1.0"'),
            # frame counts as Python compares and prints them
            "frames of 1 then the right count": json.dumps({**wmet, "frames": 1}).encode()[:-1] +
            f', "frames": {n}}}'.encode(),
            "frames written 4.41e4": extra(b'"frames": 4.41e4'),
            "frames as a string": met(frames=str(n)),
            "frames as true": met(frames=True),
            "frames as NaN": met(frames=float("nan")),
            "frames as a list": met(frames=[n, "it's", None, {"k": 'a"b'}, -0.0]),
            # what json.loads takes, and what it refuses
            "a lone surrogate": met(title="\ud800"),
            "an Infinity": met(x=float("inf")),
            "a 4300-digit int": extra(b'"x": ' + b"7" * 4300),
            "a 4301-digit int": extra(b'"x": ' + b"7" * 4301),
            "a 4301-digit float": extra(b'"x": ' + b"7" * 4301 + b".0"),
            "a byte-order mark": b"\xef\xbb\xbf" + met(),
            "invalid UTF-8": extra(b'"x": "\xff"'),
            "an encoded surrogate": extra(b'"x": "\xed\xa0\x80"'),
            "a raw control character": extra(b'"x": "a\x01b"'),
            "a trailing comma": extra(b'"x": 1,'),
            "an empty wmet": b"",
            "a wmet that is a list": b"[1]",
        }
        wlins = {
            "a wlin that is a list": b"[]",
            "a wlin with a trailing comma": b'{"a": 1,}',
        }
        cases = [(name, {b"wmet": text}) for name, text in variants.items()]
        cases += [(name, {b"wlin": text}) for name, text in wlins.items()]
        for name, new in cases:
            with self.subTest(name):
                path = os.path.join(self.dir, name.replace(" ", "_") + ".wwav")
                with open(path, "wb") as f:
                    f.write(with_chunks(new))
                # The reference reader's own verdict (not its info output, which can't print every wmet).
                verdict = "import sys, runpy; print(ascii(runpy.run_path(sys.argv[1])['Wwav'](sys.argv[2]).verdict()))"
                reference = ast.literal_eval(subprocess.run([sys.executable, "-I", "-c", verdict, PACK, path],
                                                            capture_output=True, text=True, check=True).stdout)
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

    def test_a_loop_over_a_streamed_clip_never_drops_audio(self):
        """Finding: a loop that jumps back into a streamed clip plays silence after every wrap: the reader's window only
        holds frames ahead of the playhead, so the loop start is reread after the jump."""
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
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-review-")
        cls.path, cls.frames, _ = sine_song(os.path.join(cls.dir, "01 Held"), 30)  # 26 MB: held whole, decoded at load

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def _playing(self):
        """An engine playing the song at block 128, and the bigger graph the app sends next (a track added, say):
        the four tracks again and 32 more, which takes the engine a few hundred ms to load."""
        shm = Shm()
        self.addCleanup(shm.close)
        engine = Engine(shm, rate=RATE, block=128)
        self.addCleanup(engine.stop)
        c = engine.connect()
        c.call("session.load", {"graph": stem_session(self.path, self.frames), "playhead": 0, "off": []})
        c.call("transport.play")
        shm.wait_callbacks(5)
        bigger = stem_session(self.path, self.frames)
        for i in range(8):
            for t in stem_session(self.path, self.frames)["tracks"]:
                t["id"] = t["id"][:-2] + f"{i + 1:02d}"
                bigger["tracks"].append(t)
        return shm, c, bigger

    def test_a_mute_is_heard_within_10_ms_while_a_load_runs(self):
        """Finding: param.set waits behind whatever op the worker is running, so a mute sent during a session.load is
        heard only after the whole load, far past one block (S0.3's 10 ms)."""
        shm, c, bigger = self._playing()
        # The app sends the new graph, and the user clicks a stem light.
        c.send({"id": 100, "op": "session.load", "args": {"graph": bigger, "off": []}})
        sent = time.monotonic()
        c.send({"id": 101, "op": "param.set", "args": {"node": "bus:vocals", "param": "mute", "value": True}})
        while time.monotonic() - sent < 5:
            # The newest entry, polled without a pause: meters_since sleeps 5 ms when it looks in the middle of
            # a callback (the clock counts the block before its meters are out), which is the test's lag, not
            # the engine's.
            e = shm.meter_entry(shm.meter_write() - 1)
            if e["used"] >= 9 and e["slots"][e["used"] - 5][0] == 0.0:  # bus:vocals, in either graph
                break
        took = time.monotonic() - sent
        self.assertTrue(c.response(100, timeout=30)["ok"])
        self.assertTrue(c.response(101, timeout=30)["ok"])
        self.assertLess(took, 0.010, f"the mute was heard {took * 1000:.0f} ms after it was sent")

    def test_changes_sent_during_a_load_land_in_the_new_graph(self):
        """A change sent while a load runs is heard at once in the graph playing, and kept when the new graph
        (compiled before the change) takes over. A change to a track only the new graph has waits for it."""
        shm, c, bigger = self._playing()
        new_track = bigger["tracks"][4]["id"]
        c.send({"id": 100, "op": "session.load", "args": {"graph": bigger, "off": []}})
        c.send({"id": 101, "op": "param.set", "args": {"node": TRACK_IDS[0], "param": "mute", "value": True}})
        c.send({"id": 102, "op": "param.set", "args": {"node": "bus:bass", "param": "gain_db", "value": -6.0}})
        c.send({"id": 103, "op": "param.set", "args": {"node": new_track, "param": "mute", "value": True}})
        c.send({"id": 104, "op": "param.set", "args": {"node": new_track, "param": "mute", "value": False}})
        c.send({"id": 105, "op": "param.set", "args": {"node": new_track, "param": "gain_db", "value": -20.0}})
        replies = {i: c.response(i, timeout=30) for i in range(100, 106)}
        for i, r in replies.items():
            self.assertTrue(r["ok"], (i, r))
        slots = replies[100]["result"]["meter_slots"]
        other_vocals = bigger["tracks"][8]["id"]  # untouched; 440 Hz peaks in every block of 128
        k = shm.clock()["callbacks"]
        shm.wait_callbacks(4)
        for e in shm.meters_since(k + 1):
            def peak(node, e=e):
                return e["slots"][slots[node]][0]

            self.assertEqual(e["used"], len(bigger["tracks"]) + 5)
            self.assertEqual(e["slots"][slots[TRACK_IDS[0]]], (0.0, 0.0, 0.0, 0.0))
            self.assertAlmostEqual(peak(other_vocals), TONES[0][1], delta=0.002)
            self.assertAlmostEqual(peak(new_track), peak(other_vocals) / 10, delta=0.001)
            # nine bass tracks at 0 dB, the same sine, into a bus at -6 dB
            self.assertAlmostEqual(peak("bus:bass"), peak(TRACK_IDS[3]) * 9 * 10 ** (-6 / 20), delta=0.002)


class Envelope(unittest.TestCase):
    def setUp(self):
        self.shm = Shm()
        self.addCleanup(self.shm.close)
        self.engine = Engine(self.shm, rate=48000, block=256)
        self.addCleanup(self.engine.stop)

    def test_deep_nesting_does_not_kill_the_engine(self):
        """Finding: JUCE's recursive JSON parser overflows the socket thread's stack: a 200 KB frame of nested arrays
        kills the engine (SIGSEGV) instead of closing the connection."""
        c = self.engine.connect()
        depth = 100000
        c.send_raw(b'{"id": 1, "op": "ping", "args": {"a": ' + b"[" * depth + b"]" * depth + b"}}")
        c.closed(timeout=2)
        time.sleep(0.2)
        self.assertIsNone(self.engine.proc.poll(), f"the engine died (status {self.engine.proc.poll()})")
        self.engine.connect().call("ping")

    def test_nesting_is_answered_to_serde_jsons_depth(self):
        """127 levels, the most serde_json (the app's parser) takes, are answered; 128 close the connection."""
        for depth, answered in [(127, True), (128, False)]:
            with self.subTest(depth=depth):
                c = self.engine.connect()
                self.engine.clients.remove(c)
                try:
                    inner = b"[" * (depth - 2) + b"]" * (depth - 2)  # the request and its args are two levels
                    c.send_raw(b'{"id": 7, "op": "ping", "args": {"a": ' + inner + b"}}")
                    if answered:
                        self.assertTrue(c.response(7, timeout=2)["ok"])
                    else:
                        self.assertTrue(c.closed())
                        self.assertEqual(c.responses, {})
                finally:
                    c.close()

    def test_a_payload_that_isnt_one_json_object_closes_the_connection(self):
        """Finding: payloads that aren't one JSON object are answered, not closed: text after the object, two objects
        (the second request then never gets a response), a trailing comma, invalid UTF-8."""
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

    def test_a_nul_escape_is_valid_json(self):
        """Finding: a JSON object whose string holds the escape \\u0000 is valid JSON, but the engine closes the
        connection as if it weren't."""
        c = self.engine.connect()
        r = c.request("hello", {"protocol": 1, "client": "wi_wwav\u00000.1.0"}, timeout=2)
        self.assertTrue(r["ok"], r)

    def test_no_reply_is_over_16_mib(self):
        """Finding: an error message echoes the request's strings, so a request at the 16 MiB limit gets a reply frame
        over it, which the app must treat as a broken connection."""
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
    def test_an_exiting_engine_leaves_its_replacements_socket(self):
        """Finding: an engine that exits cleanly unlinks its socket path even when a replacement engine has since bound
        the same path, so the app can no longer reach the replacement."""
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
