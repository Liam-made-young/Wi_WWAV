"""Writing a .wwav (docs/ENGINE.md 3.11; docs/SPEC.md 6.1, 6.6, 6.7).

Fails if: a .wwav the engine writes isn't one the reference reader lists
with its four stems; written without dither from 16-bit stems at unity, its
stems and master aren't the ones it was made from, bit for bit; written
with dither, any sample is more than two codes from its source, or two
writes of the same song differ by a byte; the engine can't play what it
wrote; the fold check doesn't notice a master that isn't the sum of its
stems; an original without a title is written anyway.
"""
import ast
import hashlib
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest

from wwav_client import (PACK, RATE, STEMS, Engine, EngineError, Shm, first_difference, read_wav, sine_song,
                         stem_session)

SONG_ID = "00112233445566778899aabbccddeeff"


def chunks(path):
    with open(path, "rb") as f:
        raw = f.read()
    out, at = {}, 12
    while at + 8 <= len(raw):
        cid, n = struct.unpack_from("<4sI", raw, at)
        out.setdefault(cid, raw[at + 8: at + 8 + n])
        at += 8 + n + (n & 1)
    return out


def reference_verdict(path):
    """What formats/prana/tools/wwav_pack.py makes of the file."""
    code = "import sys, runpy; print(ascii(runpy.run_path(sys.argv[1])['Wwav'](sys.argv[2]).verdict()))"
    out = subprocess.run([sys.executable, "-I", "-c", code, PACK, path], capture_output=True, text=True, check=True)
    return ast.literal_eval(out.stdout)


def stems_of(path):
    """The four stems of a .wwav as lists of int16, and its master."""
    c = chunks(path)
    master = list(struct.unpack(f"<{len(c[b'data']) // 2}h", c[b"data"]))
    pad = struct.unpack_from("<H", c[b"wstm"], 6)[0]
    frames = struct.unpack_from("<I", c[b"wstm"], 12)[0]
    body = struct.unpack_from(f"<{frames * 8}h", c[b"wstm"], 16 + pad)
    return master, [[v for f in range(frames) for v in body[8 * f + 2 * s: 8 * f + 2 * s + 2]] for s in range(4)]


class Write(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dir = tempfile.mkdtemp(prefix="wwav-write-")
        cls.path, cls.frames, cls.stems = sine_song(os.path.join(cls.dir, "01 Engine Test"), 2)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.dir)

    def setUp(self):
        self.shm = Shm()
        self.addCleanup(self.shm.close)
        self.engine = Engine(self.shm, rate=RATE, block=1024)
        self.addCleanup(self.engine.stop)
        self.c = self.engine.connect()
        self.c.call("session.load", {"graph": stem_session(self.path, self.frames), "playhead": 0, "off": []})
        self.out = tempfile.mkdtemp(prefix="wwav-out-")
        self.addCleanup(shutil.rmtree, self.out)

    def write(self, name="Low Tide.wwav", **kw):
        args = {"path": os.path.join(self.out, name), "start": 0, "len": self.frames,
                "meta": {"song_id": SONG_ID, "title": "Low Tide", "artist": "Tests", "created": "2026-10-08"},
                "creator": "liam_made_young"}
        args.update(kw)
        return self.c.call("wwav.write", args, timeout=60)

    def test_without_dither_16_bit_stems_at_unity_come_back_bit_for_bit(self):
        r = self.write(dither=False)
        self.assertEqual((r["frames"], r["sample_rate"], r["song_id"]), (self.frames, 44100, SONG_ID))
        self.assertEqual(r["bytes"], os.path.getsize(r["path"]))
        self.assertTrue(r["folds"], r)
        self.assertEqual(reference_verdict(r["path"]), "4 stems, and the master")
        master, stems = stems_of(r["path"])
        source_master, source_stems = stems_of(self.path)
        for name, got, want in zip(STEMS, stems, source_stems):
            self.assertIsNone(first_difference(got, want), name)
        self.assertIsNone(first_difference(master, source_master), "master")
        c = chunks(r["path"])
        wmet, wlin = json.loads(c[b"wmet"]), json.loads(c[b"wlin"])
        self.assertEqual((wmet["title"], wmet["artist"], wmet["frames"], wmet["type"]),
                         ("Low Tide", "Tests", self.frames, "original"))
        self.assertEqual((wlin["parent_id"], wlin["root_id"], wlin["generation"], wlin["creator"]),
                         (None, SONG_ID, 0, "liam_made_young"))
        progress = [e for e in self.c.events if e["ev"] == "render.progress"]
        self.assertTrue(progress and all(e["stage"] == "wwav" for e in progress))
        self.assertEqual(progress[-1]["done"], self.frames)

    def test_dither_stays_within_two_codes_and_is_the_same_every_time(self):
        a = self.write("a.wwav")
        b = self.write("b.wwav")
        self.assertEqual(reference_verdict(a["path"]), "4 stems, and the master")
        with open(a["path"], "rb") as fa, open(b["path"], "rb") as fb:
            self.assertEqual(hashlib.sha256(fa.read()).hexdigest(), hashlib.sha256(fb.read()).hexdigest())
        _, stems = stems_of(a["path"])
        _, source = stems_of(self.path)
        differs = False
        for name, got, want in zip(STEMS, stems, source):
            worst = max(abs(g - w) for g, w in zip(got, want))
            self.assertLessEqual(worst, 2, name)
            differs = differs or worst > 0
        self.assertTrue(differs, "nothing was dithered")
        # another song's noise is its own
        other = self.write("c.wwav", meta={"song_id": "ff" * 16, "title": "Low Tide", "created": "2026-10-08"})
        self.assertNotEqual(stems_of(other["path"])[1][0], stems[0])

    def test_the_engine_plays_what_it_wrote(self):
        r = self.write(dither=False, start=1000, len=RATE)
        self.c.call("session.load", {"graph": stem_session(r["path"], RATE), "playhead": 0, "off": []})
        out = self.c.call("render", {"out_dir": os.path.join(self.out, "r"), "start": 0, "len": RATE,
                                     "master": False, "stems": True, "format": "s16"})
        for name, stem in zip(STEMS, self.stems):
            want = [v / 32768.0 for v in stem[2000: 2000 + 2 * RATE]]
            self.assertIsNone(first_difference(read_wav(out["files"][name])[4], want), name)

    def test_a_remix_names_its_parent(self):
        r = self.write(meta={"title": "Low Tide (remix)", "type": "remix"},
                       parent={"song_id": SONG_ID, "root_id": "ab" * 16, "generation": 2})
        self.assertRegex(r["song_id"], "^[0-9a-f]{32}$")
        c = chunks(r["path"])
        wlin = json.loads(c[b"wlin"])
        self.assertEqual((wlin["parent_id"], wlin["root_id"], wlin["generation"]), (SONG_ID, "ab" * 16, 3))
        self.assertEqual(json.loads(c[b"wmet"])["type"], "remix")
        self.assertEqual(reference_verdict(r["path"]), "4 stems, and the master")

    def test_a_master_that_isnt_the_sum_of_its_stems_is_said(self):
        self.c.call("param.set", {"node": "master", "param": "gain_db", "value": -6.0})
        r = self.write(dither=False)
        self.assertFalse(r["folds"])
        self.assertGreater(r["fold_dbfs"], -20.0)

    def test_bad_writes(self):
        cases = [({"path": "relative.wwav"}, "bad_args"), ({"len": 0}, "bad_args"), ({"dither": "yes"}, "bad_args"),
                 ({"meta": {"title": "T", "type": "bootleg"}}, "bad_args"),
                 ({"parent": {"root_id": "ab" * 16}}, "bad_args"),
                 ({"meta": {"song_id": SONG_ID}}, "write_failed"),  # an original needs a title
                 ({"meta": {"song_id": "not hex", "title": "T"}}, "write_failed")]
        for kw, code in cases:
            with self.subTest(kw=kw):
                with self.assertRaises(EngineError) as e:
                    self.write(**kw)
                self.assertEqual(e.exception.code, code)
                self.assertTrue(e.exception.message.endswith("."), e.exception.message)
        self.assertEqual(os.listdir(self.out), [], "a refused write left a file")
        self.c.call("session.unload")
        with self.assertRaises(EngineError) as e:
            self.write()
        self.assertEqual(e.exception.code, "no_session")


if __name__ == "__main__":
    unittest.main()
