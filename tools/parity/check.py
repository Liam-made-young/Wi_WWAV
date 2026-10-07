#!/usr/bin/env python3
"""F3, one verdict (docs/PLAN.md): every file in tests/corpus/ through the
four readers, which have to say the same thing about it.

  check.py [--wwav PATH] [--allow-known] [--corpus DIR]

1. The reference: the last line of wwav_pack.py info or swav_pack.py info.
2. Wi's JavaScript readers under Node (wi_verdicts.mjs).
3. PRANA's C++, built here from formats/prana/core (prana_verdict.cpp).
   .wwav and .wav only: the device doesn't read films.
4. The Rust crate: the last line of `wwav info` or `wwav swav info`, built
   with cargo unless --wwav names the binary.

PRANA decides what to list, not what to say, so its outcome is compared
with the start of the reference's sentence, by the table DEVICE below.
Every other reader has to give the sentence character for character.

manifest.json names any reader known to say something else about a file,
and why ("differs"). Those are differences all the same, so they fail the
check unless --allow-known is given. The Rust test passes it, so the test
stays green while they are open questions and fails if any of them changes
or a new one appears. The Rust reader is never allowed to differ.

Needs python3, node (22, for fs.openAsBlob), a C++17 compiler (CXX, or c++)
and formats/ checked out.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
HERE = os.path.join(ROOT, "tools", "parity")
WWAV_PACK = os.path.join(ROOT, "formats", "prana", "tools", "wwav_pack.py")
SWAV_PACK = os.path.join(ROOT, "formats", "formats", "swav", "swav_pack.py")
WI = os.path.join(ROOT, "formats", "wi", "src", "formats")
PRANA = os.path.join(ROOT, "formats", "prana")
PRANA_SOURCES = ["disc/library.cpp", "disc/wav.cpp", "disc/wwav.cpp", "base/json.cpp", "base/text.cpp",
                 "io/storage.cpp"]

# PRANA's Library either drops a file, lists its master, or lists its stems.
# Each outcome stands for every sentence that starts with it.
DEVICE = {
    "not listed": "not listed: the master isn't 44.1 kHz 16-bit stereo PCM",
    "the master only": "the master only: (any reason)",
    "4 stems, and the master": "4 stems, and the master",
}
READERS = ("wi", "prana", "rust")


def device_outcome(sentence):
    return sentence.split(":", 1)[0]


def is_film(name):
    return name.lower().endswith((".mp4", ".m4v", ".mov", ".swav"))


def last_line(cmd):
    """The verdict an info command ends with, or its error."""
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        return "error: " + r.stderr.strip()
    m = re.search(r"^  (?:on PRANA|reads as): (.*)\Z", r.stdout.rstrip("\n"), re.M)
    return m.group(1) if m else "error: no verdict in " + repr(r.stdout)


def reference(path):
    return last_line([sys.executable, SWAV_PACK if is_film(path) else WWAV_PACK, "info", path])


def rust(binary, path):
    return last_line([binary, *(["swav"] if is_film(path) else []), "info", path])


def wi(paths):
    r = subprocess.run(["node", os.path.join(HERE, "wi_verdicts.mjs"), WI, *paths],
                       capture_output=True, text=True, check=True)
    return {j["file"]: j["verdict"] for j in map(json.loads, r.stdout.splitlines())}


def prana(paths, build):
    exe = os.path.join(build, "prana_verdict")
    core = os.path.join(PRANA, "core")
    subprocess.run([os.environ.get("CXX", "c++"), "-std=c++17", "-O1", "-ffp-contract=off", "-w",
                    "-I", core, "-I", os.path.join(PRANA, "tests"), os.path.join(HERE, "prana_verdict.cpp"),
                    *[os.path.join(core, s) for s in PRANA_SOURCES], "-o", exe], check=True)
    r = subprocess.run([exe, *paths], capture_output=True, text=True, check=True)
    return {path: outcome for outcome, path in (line.split("\t", 1) for line in r.stdout.splitlines())}


def cargo_wwav():
    subprocess.run(["cargo", "build", "-q", "-p", "wwav-formats", "--bin", "wwav"], cwd=ROOT, check=True)
    return os.path.join(os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target")), "debug", "wwav")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--wwav", help="the Rust binary (default: cargo build it)")
    ap.add_argument("--allow-known", action="store_true", help="pass when the only differences are in manifest.json")
    ap.add_argument("--corpus", default=os.path.join(ROOT, "tests", "corpus"))
    a = ap.parse_args()

    with open(os.path.join(a.corpus, "manifest.json"), encoding="utf-8") as f:
        files = json.load(f)["files"]
    paths = [os.path.join(a.corpus, e["file"]) for e in files]
    binary = a.wwav or cargo_wwav()
    with tempfile.TemporaryDirectory(prefix="wwav-parity-") as build:
        device = prana([p for p in paths if not is_film(p)], build)
    js = wi(paths)

    unexpected, known = [], []
    for e, path in zip(files, paths):
        ref = reference(path)
        said = {"wi": js[path], "rust": rust(binary, path)}
        if not is_film(path):
            said["prana"] = device[path]
        if ref != e["verdict"]:
            unexpected.append(f"{e['file']}: the reference now says {ref!r}; manifest.json says {e['verdict']!r} "
                              "(run tools/corpus/make_corpus.py)")
        differs = e.get("differs", {})
        for reader in READERS:
            if reader not in said:
                continue
            want = device_outcome(ref) if reader == "prana" else ref
            got = said[reader]
            if got == want and reader not in differs:
                continue
            line = f"{e['file']}: {reader} says {got!r}, the reference {want!r}"
            if reader in differs and reader != "rust" and got == differs[reader]:
                known.append(f"{line}\n    because {differs['because']}")
            elif reader in differs and got != differs[reader]:
                unexpected.append(f"{line} (manifest.json expects {differs[reader]!r})")
            else:
                unexpected.append(line)

    readers = "the reference, Wi's JavaScript, PRANA's C++ (.wwav and .wav) and the Rust crate"
    print(f"{len(files)} files in {os.path.relpath(a.corpus, ROOT)}, read by {readers}")
    if known:
        print(f"\n{len(known)} known differences (manifest.json):")
        for k in known:
            print("  " + k)
    if unexpected:
        print(f"\n{len(unexpected)} unexpected differences:")
        for u in unexpected:
            print("  " + u)
    if unexpected or (known and not a.allow_known):
        print("\nF3 fails: the readers don't give one verdict for every file.")
        sys.exit(1)
    print("\nOne verdict for every file." if not known else "\nOne verdict for every file but the known differences.")


if __name__ == "__main__":
    main()
