"""A client for wwav-engine, for the tests. Python 3 standard library only.

It starts the engine the way the app does (docs/ENGINE.md 1): it creates the
shared-memory region and a private socket directory first, starts the engine
with a pipe on its stdin, waits for the one "listening" line, then speaks
length-prefixed JSON frames (2).

The shared-memory offsets below are copied from docs/ENGINE.md 4, not from
engine/include/wwav_shm.h, so the tests check the engine against the contract.

WWAV_ENGINE names the engine binary and WWAV_PACK formats/prana/tools/wwav_pack.py.
"""
import itertools
import json
import math
import mmap
import os
import select
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import time
from array import array

ENGINE = os.environ.get("WWAV_ENGINE", "")
PACK = os.environ.get("WWAV_PACK", "")

# docs/ENGINE.md 4
CLOCK_AT = 64
CRUMB_AT = 128
METER_WRITE_AT = 192
RING_AT = 256
MS = 260  # meter slots
MR = 64  # meter ring entries
ENTRY = 32 + MS * 16
PEAKS_BYTES = 64 * 1024
INPUT_BYTES = 2 * 96000 * 2 * 4
TOTAL = RING_AT + MR * ENTRY + PEAKS_BYTES + INPUT_BYTES

MAX_FRAME = 16 * 1024 * 1024
STEMS = ["vocals", "drums", "other", "bass"]

_names = itertools.count()


def fnv1a64(text):
    h = 0xCBF29CE484222325
    for b in text.encode("utf-8"):
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


class Shm:
    """The region the app creates and zeroes before it starts the engine."""

    def __init__(self):
        self.name = f"/wwav-{os.getpid()}-{next(_names)}"
        if os.path.isdir("/dev/shm"):
            # Linux: shm_open's names live in /dev/shm.
            self._path = "/dev/shm" + self.name
            fd = os.open(self._path, os.O_RDWR | os.O_CREAT | os.O_EXCL, 0o600)
            try:
                os.ftruncate(fd, TOTAL)
                self.map = mmap.mmap(fd, TOTAL)
            finally:
                os.close(fd)
            self._shared = None
        else:
            from multiprocessing import shared_memory

            self._path = None
            self._shared = shared_memory.SharedMemory(self.name.lstrip("/"), create=True, size=TOTAL)
            self.map = self._shared.buf

    def close(self):
        if self._shared is not None:
            self._shared.close()
            self._shared.unlink()
        else:
            self.map.close()
            os.unlink(self._path)

    def header(self):
        magic, layout, rate, block, slots, ring, peaks, inputs, pid, start = struct.unpack_from(
            "<4sIIIIIIIQQ", self.map, 0)
        return {"magic": magic, "layout": layout, "sample_rate": rate, "block": block, "meter_slots": slots,
                "meter_ring": ring, "peaks_bytes": peaks, "input_bytes": inputs, "pid": pid, "start_ns": start}

    def clock(self):
        """One consistent read of the seqlock (4.2)."""
        while True:
            s1 = struct.unpack_from("<Q", self.map, CLOCK_AT)[0]
            if s1 & 1:
                continue
            pos, host, rate, state, dropouts, callbacks = struct.unpack_from("<qQdIIQ", self.map, CLOCK_AT + 8)
            s2 = struct.unpack_from("<Q", self.map, CLOCK_AT)[0]
            if s1 == s2:
                return {"seq": s1, "sample_pos": pos, "host_time_ns": host, "rate": rate, "state": state,
                        "dropouts": dropouts, "callbacks": callbacks}

    def crumb(self):
        return struct.unpack_from("<QQ", self.map, CRUMB_AT)

    def meter_write(self):
        return struct.unpack_from("<Q", self.map, METER_WRITE_AT)[0]

    def meter_entry(self, i):
        """Entry i of the meter ring (4.4): callback, load, dropouts and the used slots."""
        at = RING_AT + (i % MR) * ENTRY
        callback, load, dropouts, used = struct.unpack_from("<QfII", self.map, at)
        slots = [struct.unpack_from("<4f", self.map, at + 32 + 16 * s) for s in range(min(used, MS))]
        return {"callback": callback, "load": load, "dropouts": dropouts, "used": used, "slots": slots}

    def meters_since(self, after_callback, timeout=5.0):
        """Every meter entry written for callbacks after `after_callback`, oldest first,
        waiting until at least one exists."""
        deadline = time.monotonic() + timeout
        while True:
            w = self.meter_write()
            entries = [self.meter_entry(i) for i in range(max(0, w - MR + 1), w)]
            entries = [e for e in entries if e["callback"] > after_callback]
            if entries or time.monotonic() > deadline:
                return sorted(entries, key=lambda e: e["callback"])
            time.sleep(0.005)

    def wait_callbacks(self, n, timeout=5.0):
        """Waits until the clock has counted n more callbacks; returns the count."""
        start = self.clock()["callbacks"]
        deadline = time.monotonic() + timeout
        while self.clock()["callbacks"] < start + n:
            if time.monotonic() > deadline:
                raise AssertionError(f"the clock stopped at {self.clock()['callbacks']} callbacks")
            time.sleep(0.002)
        return self.clock()["callbacks"]


class EngineError(Exception):
    def __init__(self, code, message):
        super().__init__(f"{code}: {message}")
        self.code = code
        self.message = message


class Client:
    """One connection to the engine's socket."""

    def __init__(self, path):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(path)
        self.ids = itertools.count(1)
        self.events = []
        self.responses = {}

    def close(self):
        self.sock.close()

    def send_raw(self, payload):
        self.sock.sendall(struct.pack("<I", len(payload)) + payload)

    def send(self, obj):
        self.send_raw(json.dumps(obj).encode("utf-8"))

    def _read(self, n, deadline):
        buf = b""
        while len(buf) < n:
            left = deadline - time.monotonic()
            if left <= 0:
                raise TimeoutError("no frame from the engine in time")
            ready, _, _ = select.select([self.sock], [], [], left)
            if not ready:
                continue
            chunk = self.sock.recv(min(1 << 20, n - len(buf)))
            if not chunk:
                return None
            buf += chunk
        return buf

    def recv(self, timeout=10.0):
        """The next frame as a dict, or None when the engine closed the connection."""
        deadline = time.monotonic() + timeout
        head = self._read(4, deadline)
        if head is None:
            return None
        (n,) = struct.unpack("<I", head)
        assert 1 <= n <= MAX_FRAME, f"the engine sent a frame of {n} bytes"
        body = self._read(n, deadline)
        if body is None:
            return None
        msg = json.loads(body.decode("utf-8"))
        assert isinstance(msg, dict), msg
        return msg

    def closed(self, timeout=2.0):
        """True when the engine closes the connection within `timeout` (frames before that are kept).
        A reset is a close too: closing with unread bytes in the socket resets it."""
        deadline = time.monotonic() + timeout
        try:
            while True:
                msg = self.recv(max(0.0, deadline - time.monotonic()))
                if msg is None:
                    return True
                self._keep(msg)
        except ConnectionResetError:
            return True
        except TimeoutError:
            return False

    def _keep(self, msg):
        if "ev" in msg:
            assert "id" not in msg, msg
            self.events.append(msg)
        else:
            self.responses[msg["id"]] = msg

    def request(self, op, args=None, timeout=10.0):
        """Sends one request and returns its full response."""
        rid = next(self.ids)
        msg = {"id": rid, "op": op}
        if args is not None:
            msg["args"] = args
        self.send(msg)
        return self.response(rid, timeout)

    def response(self, rid, timeout=10.0):
        deadline = time.monotonic() + timeout
        while rid not in self.responses:
            msg = self.recv(max(0.0, deadline - time.monotonic()))
            if msg is None:
                raise ConnectionError("the engine closed the connection")
            self._keep(msg)
        return self.responses.pop(rid)

    def call(self, op, args=None, timeout=10.0):
        """Sends one request; returns its result or raises EngineError."""
        r = self.request(op, args, timeout)
        if r.get("ok"):
            assert isinstance(r.get("result"), dict), r
            return r["result"]
        err = r.get("error") or {}
        raise EngineError(err.get("code"), err.get("message"))

    def hello(self):
        return self.call("hello", {"protocol": 1, "client": "engine tests"})

    def wait_event(self, ev, timeout=5.0, **match):
        deadline = time.monotonic() + timeout
        while True:
            for i, e in enumerate(self.events):
                if e["ev"] == ev and all(e.get(k) == v for k, v in match.items()):
                    return self.events.pop(i)
            msg = self.recv(max(0.0, deadline - time.monotonic()))
            if msg is None:
                raise ConnectionError("the engine closed the connection")
            self._keep(msg)


class Engine:
    """A running wwav-engine with --device null, as the app starts it."""

    def __init__(self, shm, rate=44100, block=1024, test=True, device="null", sock_dir=None):
        if not ENGINE:
            raise RuntimeError("set WWAV_ENGINE to the wwav-engine binary")
        self.dir = sock_dir or tempfile.mkdtemp(prefix=f"wwav-{os.getpid()}-")
        os.chmod(self.dir, 0o700)
        self.sock_path = os.path.join(self.dir, "engine.sock")
        args = [ENGINE, "--socket", self.sock_path, "--shm", shm.name, "--rate", str(rate), "--block", str(block)]
        if device is not None:
            args += ["--device", device]
        if test:
            args.append("--test")
        self.clients = []
        self.proc = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        try:
            self.listening_line = self._listening_line()
        except Exception:
            self.kill()
            self.proc.stdin.close()
            self.proc.stdout.close()
            if sock_dir is None:
                shutil.rmtree(self.dir, ignore_errors=True)
            raise

    def _listening_line(self, timeout=10.0):
        deadline = time.monotonic() + timeout
        line = b""
        while not line.endswith(b"\n"):
            left = deadline - time.monotonic()
            ready, _, _ = select.select([self.proc.stdout], [], [], max(0.0, left))
            if not ready:
                self.kill()
                raise TimeoutError("the engine never said it was listening")
            chunk = os.read(self.proc.stdout.fileno(), 1)
            if not chunk:
                raise RuntimeError(f"the engine exited before listening (status {self.proc.wait()})")
            line += chunk
        return line.decode("utf-8")

    def connect(self, hello=True):
        c = Client(self.sock_path)
        self.clients.append(c)
        if hello:
            c.hello()
        return c

    def stdout_rest(self):
        """Whatever the engine wrote to stdout after the listening line (it must be nothing)."""
        return self.proc.stdout.read()

    def close_stdin(self):
        self.proc.stdin.close()

    def kill(self):
        if self.proc.poll() is None:
            self.proc.kill()
        self.proc.wait()

    def stop(self):
        """Ends the engine as the app does when it quits: by closing its stdin."""
        for c in self.clients:
            c.close()
        try:
            self.proc.stdin.close()
            self.proc.wait(timeout=2)
        except (subprocess.TimeoutExpired, BrokenPipeError):
            pass
        self.kill()
        self.proc.stdout.close()
        shutil.rmtree(self.dir, ignore_errors=True)


# ---- songs and sessions ---------------------------------------------------------

RATE = 44100
# One sine per stem, at levels the meters can tell apart (vocals, drums, other, bass).
TONES = [(440.0, 0.25), (220.0, 0.20), (330.0, 0.15), (110.0, 0.30)]


def wav_bytes(frames, channels, rate=RATE):
    """A canonical 16-bit PCM WAV of interleaved int16 `frames` (an array('h'))."""
    data = frames.tobytes() if sys.byteorder == "little" else _swapped(frames)
    head = struct.pack("<4sI4s4sIHHIIHH4sI", b"RIFF", 36 + len(data), b"WAVE", b"fmt ", 16, 1, channels, rate,
                       rate * channels * 2, channels * 2, 16, b"data", len(data))
    return head + data


def _swapped(a):
    b = array(a.typecode, a)
    b.byteswap()
    return b.tobytes()


def sine(freq, amp, frames, rate=RATE, phase=0):
    """Stereo int16 frames of a sine; the right channel lags a quarter turn so L and R differ."""
    out = array("h", bytes(frames * 4))
    w = 2.0 * math.pi * freq / rate
    for n in range(frames):
        out[2 * n] = round(amp * 32767.0 * math.sin(w * (n + phase)))
        out[2 * n + 1] = round(amp * 32767.0 * math.cos(w * (n + phase)))
    return out


def periodic(cycle, frames):
    """`cycle` (stereo int16 frames) repeated to `frames` frames: long test audio, made fast."""
    reps = frames * 2 // len(cycle) + 1
    return array("h", (cycle * reps)[: frames * 2])


def mix(stems, frames):
    master = array("h", bytes(frames * 4))
    for i in range(frames * 2):
        master[i] = max(-32768, min(32767, sum(s[i] for s in stems)))
    return master


def make_song(folder, frames, stems, master=None):
    """Packs a .wwav with wwav_pack.py from four stereo int16 stems; the master is their sum
    unless given. Returns the .wwav's path."""
    if not PACK:
        raise RuntimeError("set WWAV_PACK to formats/prana/tools/wwav_pack.py")
    os.makedirs(folder)
    if master is None:
        master = mix(stems, frames)
    with open(os.path.join(folder, "master.wav"), "wb") as f:
        f.write(wav_bytes(master, 2))
    for name, s in zip(STEMS, stems):
        with open(os.path.join(folder, name + ".wav"), "wb") as f:
            f.write(wav_bytes(s, 2))
    with open(os.path.join(folder, "song.txt"), "w") as f:
        f.write("title = Engine Test\nartist = Tests\nsong_id = 0123456789abcdef0123456789abcdef\n"
                "created = 2026-10-07\n")
    out = folder + ".wwav"
    subprocess.run([sys.executable, "-I", PACK, "pack", folder, "-o", out], check=True, stdout=subprocess.DEVNULL)
    return out


def sine_song(folder, seconds):
    frames = int(seconds * RATE)
    stems = [sine(f, a, frames) for f, a in TONES]
    return make_song(folder, frames, stems), frames, stems


TRACK_IDS = ["01JTESTVOCALS00000000000000", "01JTESTDRUMS000000000000000",
             "01JTESTOTHER000000000000000", "01JTESTBASS0000000000000000"]


def stem_session(path, frames, rate=RATE, **track_overrides):
    """A dropped .wwav as the app compiles it: one stem track per stem, reading the file in place."""
    tracks = []
    for tid, stem in zip(TRACK_IDS, STEMS):
        t = {"id": tid, "kind": "stem", "role": stem, "gain_db": 0.0, "pan": 0.0, "mute": False, "solo": False,
             "sends": {"reverb": 0.0, "delay": 0.0}, "devices": [],
             "clips": [{"id": tid[:-4] + "CLIP", "path": path, "source": stem, "at": 0, "in": 0, "len": frames,
                        "gain_db": 0.0, "reverse": False}]}
        t.update(track_overrides)
        tracks.append(t)
    return {"sample_rate": rate, "tempo_map": [{"at_beats": 0, "bpm": 120.0}], "tracks": tracks,
            "master": {"gain_db": 0.0, "devices": []}}


def first_difference(got, want):
    """None when the two sample lists are equal, else where they first differ. (unittest's own
    diff of lists this long takes minutes.)"""
    if len(got) != len(want):
        return f"{len(got)} samples, not {len(want)}"
    for i, (a, b) in enumerate(zip(got, want)):
        if a != b:
            return f"sample {i} is {a}, not {b}"
    return None


def read_wav(path):
    """(format, channels, rate, bits, samples as a list of floats in -1..1) of a WAVE file."""
    with open(path, "rb") as f:
        data = f.read()
    assert data[:4] == b"RIFF" and data[8:12] == b"WAVE", path
    assert struct.unpack_from("<I", data, 4)[0] == len(data) - 8, f"{path}: RIFF size"
    at, fmt, samples = 12, None, None
    while at + 8 <= len(data):
        cid, n = struct.unpack_from("<4sI", data, at)
        body = data[at + 8: at + 8 + n]
        if cid == b"fmt ":
            fmt = struct.unpack_from("<HHIIHH", body, 0)
        elif cid == b"data":
            tag, ch, rate, _, _, bits = fmt
            if tag == 3 and bits == 32:
                samples = array("f", body)
                scale = 1.0
            elif tag == 1 and bits == 16:
                samples = array("h", body)
                scale = 1.0 / 32768.0
            else:
                raise AssertionError(f"{path}: format {tag}, {bits}-bit")
            if sys.byteorder != "little":
                samples.byteswap()
            return tag, ch, rate, bits, [s * scale for s in samples] if scale != 1.0 else list(samples)
        at += 8 + n + (n & 1)
    raise AssertionError(f"{path}: no data chunk")
