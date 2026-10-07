#!/usr/bin/env python3
"""F1 and F2 on files nobody wrote by hand: random variations of the corpus,
random song folders and random film.txt files, through the reference tools
and the Rust `wwav`, which have to print, write and refuse the same.

  fuzz.py [--cases N] [--seed S] [--wwav PATH] [--keep DIR]

Each case is one of: info of a corpus file broken in up to three random ways
(a cut, a changed byte, a changed size, a chunk or box added, a JSON body
swapped for an odd one), unpack of one, pack of a song folder with a random
song.txt and folder name, swav pack of a corpus film with a random
film.txt, or a random command line (options, abbreviations, values with "="
or glued on, -h in every form, "--") beside a song, a .wwav and a film. Both programs run on identical copies of the case, from inside it,
so their lines and the files they write can be compared as they are.

A wrong command line (exit 2) and a -h (exit 0, "usage: ...") are compared
by their exit codes only, since the usage texts differ; so is a python traceback (a value no writer makes, such
as a JSON escape for half a surrogate pair, which python can't print).

Needs python3, formats/ and the wwav binary (cargo build -p wwav-formats,
unless --wwav names it). Exits 1 on any difference, keeping those cases.
"""
import argparse
import filecmp
import os
import random
import shutil
import struct
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CORPUS = os.path.join(ROOT, "tests", "corpus")
WWAV_PACK = os.path.join(ROOT, "formats", "prana", "tools", "wwav_pack.py")
SWAV_PACK = os.path.join(ROOT, "formats", "formats", "swav", "swav_pack.py")
sys.path.insert(0, os.path.dirname(WWAV_PACK))
import wwav_pack as wp  # noqa: E402  the reference's own wav_header

# JSON bodies that probe python's json and str(): versions of every type,
# frames that equal 600 only to python, and texts that aren't JSON
ODD_JSON = [b'{"wwav": "0.1"}', b'{"wwav": 0}', b'{"wwav": 1}', b'{"wwav": 0.5, "frames": 600}',
            b'{"wwav": "00.9", "frames": 600.0}', b'{"wwav": "\\u0661"}', b'{"wwav": true}', b'{"wwav": [1]}',
            b'{"wwav": null}', b'{"wwav": {"a": 1}}', b'{"wwav": "0.1", "frames": true}', b'{"wwav": "0x", "frames": "600"}',
            b'{"wwav": "0.1", "frames": 6e2}', b'null', b'[]', b'{}', b'"s"', b'1e400', b'{"wwav": 1e400}', b'{"wwav": -1}',
            b'{"wwav": " 0"}', b'{"wwav": "0.1", "wwav": "2"}', b'{"a": NaN, "wwav": "0"}', b'{"swav": "0.1"}',
            b'{"swav": "1"}', b'{"swav": 0}', b'{"parent_id": null}', b'\xff\xfe', b'{"wwav": 1e16}',
            b'{"wwav": 12345678901234567890}', b'{"wwav": "0.1", "frames": 600.5, "title": "\\u00e9\\n\\t\\"q\\" \\u2028"}']
WEIRD = "abc XYZ 019 =#\t\\\"'é漢🎵  \u0085\x1c\x01\x7f_-.٣"
BPMS = ["120", "120.125", "97.335", "1_20", "١٢٠", " 99.999 ", "nan", "inf", "-120", "20", "20.0001", "399.995",
        "1e2", "0x10", "", "abc", "+88.885", "１２０", "60.005", "100.015"]


def is_film(name):
    return name.endswith((".mp4", ".swav"))


def mutate(rng, data, film):
    b = bytearray(data)
    k = rng.randrange(6)
    if k == 0 and len(b) > 12:
        del b[rng.randrange(12, len(b)):]
    elif k == 1 and b:
        b[rng.randrange(len(b))] = rng.randrange(256)
    elif k == 2 and len(b) > 16:
        struct.pack_into(">I" if film else "<I", b, rng.randrange(len(b) - 4),
                         rng.choice([0, 1, 2, 7, 8, 16, 600, 0xFFFFFFFF, rng.randrange(1 << 32)]))
    elif k == 3:
        body = rng.choice(ODD_JSON)
        cid = rng.choice([b"wmet", b"wlin", b"wrmx", b"wstm", b"data", b"fmt ", b"JUNK", b"free", b"\xff\x00ab"])
        b += struct.pack(">I", 8 + len(body)) + cid + body if film else cid + struct.pack("<I", len(body)) + body
    elif k == 4:
        for cid in (b"wmet", b"wlin", b"wrmx"):
            i = b.find(cid)
            # a box's size is before its type, a chunk's after its id: an id
            # found where a cut left no room for the size isn't swapped
            if i < 4 or i + 8 > len(b) or rng.random() < 0.4:
                continue
            body = rng.choice(ODD_JSON)
            if film:
                old = struct.unpack(">I", b[i - 4:i])[0]
                if i - 4 + old > len(b):
                    continue
                b[i - 4:i - 4 + old] = struct.pack(">I", 8 + len(body)) + cid + body
            else:
                old = struct.unpack("<I", b[i + 4:i + 8])[0]
                b[i:i + 8 + old + (old & 1)] = cid + struct.pack("<I", len(body)) + body + b"\0" * (len(body) & 1)
            break
    else:
        i = rng.randrange(len(b) + 1)
        b[i:i] = bytes(rng.randrange(256) for _ in range(rng.randrange(1, 9)))
    return bytes(b)


def weird(rng, n):
    return "".join(rng.choice(WEIRD) for _ in range(rng.randrange(n)))


def key_lines(rng, keys, fixed):
    lines = [f"{k}{rng.choice(['=', ' = ', '= '])}{rng.choice(BPMS) if 'bpm' in k.lower() else weird(rng, 10)}"
             for k in (rng.choice(keys) for _ in range(rng.randrange(7)))] + fixed
    rng.shuffle(lines)
    sep = rng.choice(["\n", "\r\n", "\r"])
    return (sep.join(lines) + sep).encode()


def song(case, folder, frames, seed=0):
    os.makedirs(os.path.join(case, folder))
    for i, n in enumerate(["master"] + wp.STEMS):
        with open(os.path.join(case, folder, n + ".wav"), "wb") as f:
            f.write(wp.wav_header(frames) + bytes((i * 7 + j + seed) % 256 for j in range(frames * 4)))


# what a command line is made of: every option, abbreviated, given a value
# with "=" or glued on, -h in each form, "--", and things that look like
# options but aren't (a negative number, a space)
ARGS = ["-h", "--help", "--he", "-h=", "-h=x", "--help=x", "-hx", "-h-", "-hh", "-ho", "-hox", "-oh", "-o", "--out",
        "--o", "-ox", "-o=x", "-o=", "--out=x", "--out=", "-o-x", "--creator", "--cr=a", "--c", "--splitter", "--s",
        "--title", "--t=x", "--artist", "--a", "--=x", "--", "-", "", "-x", "--bogus", "-1", "-1.5", "-.5", "-٣",
        "a b", "-a b", "x", "x.wwav", "pack", "info", "unpack", "01 Song", "song.wwav", "film.mp4", "film.swav"]


def make_case(rng, case):
    """Fills `case` with one random case; returns the python command and
    the matching wwav arguments, both relative to the case."""
    names = sorted(n for n in os.listdir(CORPUS) if n != "manifest.json")
    kind = rng.choice(["info", "unpack", "pack", "swav pack", "arguments"])
    if kind == "arguments":
        song(case, "01 Song", 3)
        with open(os.path.join(case, "01 Song", "song.txt"), "w") as f:
            f.write("song_id = 0123456789abcdef0123456789abcdef\ncreated = 2026-10-03\n")
        shutil.copy(os.path.join(CORPUS, "original.wwav"), os.path.join(case, "song.wwav"))
        shutil.copy(os.path.join(CORPUS, "plain.mp4"), os.path.join(case, "film.mp4"))
        shutil.copy(os.path.join(CORPUS, "plain.swav"), os.path.join(case, "film.swav"))
        with open(os.path.join(case, "film.txt"), "w") as f:
            f.write("film_id = 00112233445566778899aabbccddeeff\ncreated = 2026-10-03\n")
        args = [rng.choice(ARGS) for _ in range(rng.randrange(6))]
        if rng.random() < 0.8:
            args.insert(0, rng.choice(["pack", "info", "unpack"]))
        film = rng.random() < 0.3
        return [SWAV_PACK if film else WWAV_PACK] + args, (["swav"] if film else []) + args
    if kind in ("info", "unpack"):
        name = rng.choice(names)
        data = open(os.path.join(CORPUS, name), "rb").read()
        for _ in range(rng.randrange(1 if kind == "info" else 0, 4)):
            data = mutate(rng, data, is_film(name))
        src = "in.swav" if is_film(name) else "in.wwav"
        with open(os.path.join(case, src), "wb") as f:
            f.write(data)
        tool, swav = (SWAV_PACK, ["swav"]) if is_film(name) else (WWAV_PACK, [])
        args = [kind, src] + (["-o", "out.mp4" if swav else "out"] if kind == "unpack" else [])
        return [tool] + args, swav + args
    if kind == "pack":
        folder = rng.choice(["01 Song", "1-x", "Song", "0001 ", "٠٣ x", "12345 a", "01 . -_y", weird(rng, 6) or "z"])
        folder = folder.replace("/", "") if folder.strip(".") else "s"
        song(case, folder, rng.choice([0, 1, 7, 600]))
        created = ["created = 2026-10-03"] if rng.random() < 0.9 else []
        with open(os.path.join(case, folder, "song.txt"), "wb") as f:
            f.write(key_lines(rng, ["title", "artist", "bpm", "key", "created", "Title", " BPM", "x", "#title"],
                              ["song_id = 0123456789abcdef0123456789abcdef"] + created))
        args = ["pack", folder, "-o", "out.wwav"]
        args += [x for opt in ("--creator", "--splitter") if rng.random() < 0.4 for x in (opt, weird(rng, 5))]
        return [WWAV_PACK] + args, args
    film = rng.choice(["film.mp4", "a.b.mov", ".hidden", "x", (weird(rng, 5).replace("/", "") or "f") + ".m4v"])
    shutil.copy(os.path.join(CORPUS, rng.choice([n for n in names if is_film(n)])), os.path.join(case, film))
    with open(os.path.join(case, os.path.splitext(film)[0] + ".txt"), "wb") as f:
        f.write(key_lines(rng, ["title", "artist", "created", "creator", "Title", " ARTIST", "x", "#title"],
                          ["film_id = 00112233445566778899aabbccddeeff", "created = 2026-10-03"]))
    args = ["pack", film, "-o", "out.swav"]
    args += [x for opt in ("--title", "--artist", "--creator") if rng.random() < 0.3 for x in (opt, weird(rng, 5))]
    return [SWAV_PACK] + args, ["swav"] + args


def files(top):
    out = {}
    for d, _, names in os.walk(top):
        for n in names:
            p = os.path.join(d, n)
            out[os.path.relpath(p, top)] = p
    return out


def differs(py, rs, a, b):
    """What differs between the python run in `a` and the Rust run in `b`."""
    if py.returncode != rs.returncode:
        return f"exit {py.returncode} and {rs.returncode}: {py.stderr[-300:]!r} and {rs.stderr[-300:]!r}"
    tb = b"Traceback (most recent call last)" in py.stderr
    if py.returncode == 0 and py.stdout.startswith(b"usage:") and rs.stdout.startswith(b"usage:"):
        return None  # -h: each prints its own usage, and nothing is written
    if py.returncode != 2 and not tb and (py.stdout, py.stderr) != (rs.stdout, rs.stderr):
        return f"python said {py.stdout + py.stderr!r}, wwav {rs.stdout + rs.stderr!r}"
    if not tb and py.stdout != rs.stdout:
        return f"python printed {py.stdout!r}, wwav {rs.stdout!r}"
    fa, fb = files(a), files(b)
    if sorted(fa) != sorted(fb):
        return f"python wrote {sorted(fa)}, wwav {sorted(fb)}"
    bad = [n for n in fa if not filecmp.cmp(fa[n], fb[n], shallow=False)]
    return f"different bytes in {bad}" if bad else None


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--cases", type=int, default=1000)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--wwav", help="the Rust binary (default: cargo build it)")
    ap.add_argument("--keep", help="where to keep the cases that differ (default: a temporary folder)")
    a = ap.parse_args()
    binary = a.wwav
    if not binary:
        subprocess.run(["cargo", "build", "-q", "-p", "wwav-formats", "--bin", "wwav"], cwd=ROOT, check=True)
        binary = os.path.join(os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target")), "debug", "wwav")
    binary = os.path.abspath(binary)
    keep = a.keep or tempfile.mkdtemp(prefix="wwav-fuzz-")
    rng = random.Random(a.seed)
    bad = 0
    for i in range(a.cases):
        case = os.path.join(keep, f"{i:05}")
        py_dir, rs_dir = os.path.join(case, "py"), os.path.join(case, "rs")
        os.makedirs(py_dir)
        py_cmd, rs_args = make_case(rng, py_dir)
        shutil.copytree(py_dir, rs_dir)
        py = subprocess.run([sys.executable, *py_cmd], cwd=py_dir, capture_output=True)
        rs = subprocess.run([binary, *rs_args], cwd=rs_dir, capture_output=True)
        why = differs(py, rs, py_dir, rs_dir)
        if why:
            bad += 1
            print(f"{case}: wwav {' '.join(rs_args)}\n  {why}")
        else:
            shutil.rmtree(case)
    print(f"{a.cases} cases (seed {a.seed}): {bad} differ" + (f", kept in {keep}" if bad else ""))
    if not bad and not a.keep:
        shutil.rmtree(keep)
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
