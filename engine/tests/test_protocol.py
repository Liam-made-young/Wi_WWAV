"""The socket, the frames and the handshake (docs/ENGINE.md 1-3.2), and the
engine side of F8: every message round-trips through the length-prefixed
envelope, bad frames close the connection, and the shared-memory header sits
at the documented offsets.

Fails if: the listening line or the hello result differ from the contract; a
client speaking another protocol isn't refused with the contract's sentence;
a frame at the 16 MiB limit doesn't round-trip; a frame over it, a short
read, or a payload that isn't a JSON object leaves the connection open; the
socket isn't mode 0600; a second client is served while the first is
connected; an op the engine doesn't do yet is silently accepted.
"""
import json
import os
import socket
import stat
import struct
import time
import unittest

from wwav_client import MAX_FRAME, Client, Engine, EngineError, Shm


class Protocol(unittest.TestCase):
    def setUp(self):
        self.shm = Shm()
        self.engine = Engine(self.shm, rate=48000, block=256)
        self.addCleanup(self.shm.close)
        self.addCleanup(self.engine.stop)

    def test_listening_line_and_nothing_else_on_stdout(self):
        self.assertEqual(self.engine.listening_line, f"wwav-engine listening {self.engine.sock_path}\n")
        c = self.engine.connect()
        c.call("ping")
        c.close()
        self.engine.close_stdin()
        self.engine.proc.wait(timeout=2)
        self.assertEqual(self.engine.stdout_rest(), b"")

    def test_socket_is_owner_only(self):
        mode = os.stat(self.engine.sock_path).st_mode
        self.assertTrue(stat.S_ISSOCK(mode))
        self.assertEqual(stat.S_IMODE(mode), 0o600)

    def test_hello(self):
        c = self.engine.connect(hello=False)
        r = c.call("hello", {"protocol": 1, "client": "wi_wwav 0.1.0"})
        self.assertEqual(r["protocol"], 1)
        self.assertTrue(r["engine"].startswith("wwav-engine "), r)
        self.assertEqual(r["pid"], self.engine.proc.pid)
        self.assertEqual(r["sample_rate"], 48000)
        self.assertEqual(r["block"], 256)
        self.assertEqual(r["device"], "null")
        self.assertEqual(r["shm_layout"], 1)

    def test_shared_memory_header(self):
        h = self.shm.header()
        self.assertEqual(h["magic"], b"WWAV")
        self.assertEqual(h["layout"], 1)
        self.assertEqual(h["sample_rate"], 48000)
        self.assertEqual(h["block"], 256)
        self.assertEqual(h["meter_slots"], 260)
        self.assertEqual(h["meter_ring"], 64)
        self.assertEqual(h["peaks_bytes"], 64 * 1024)
        self.assertEqual(h["input_bytes"], 2 * 96000 * 2 * 4)
        self.assertEqual(h["pid"], self.engine.proc.pid)
        self.assertGreater(h["start_ns"], 0)
        self.assertLessEqual(h["start_ns"], time.monotonic_ns())

    def test_protocol_mismatch_is_refused_and_closed(self):
        c = self.engine.connect(hello=False)
        with self.assertRaises(EngineError) as e:
            c.call("hello", {"protocol": 2, "client": "wi_wwav 9.0.0"})
        self.assertEqual(e.exception.code, "protocol")
        self.assertEqual(e.exception.message, "This engine speaks protocol 1; the app speaks 2.")
        self.assertTrue(c.closed())
        # The engine itself carries on: the next client is served.
        self.engine.connect().call("ping")

    def test_hello_comes_first(self):
        c = self.engine.connect(hello=False)
        with self.assertRaises(EngineError) as e:
            c.call("ping")
        self.assertEqual(e.exception.code, "hello_first")
        c.hello()
        c.call("ping")

    def test_ping_counts_monotonic_nanoseconds(self):
        c = self.engine.connect()
        before = time.monotonic_ns()
        t1 = c.call("ping")["t"]
        t2 = c.call("ping")["t"]
        after = time.monotonic_ns()
        self.assertIsInstance(t1, int)
        self.assertLessEqual(before, t1)
        self.assertLessEqual(t1, t2)
        self.assertLessEqual(t2, after)

    def test_unknown_op(self):
        c = self.engine.connect()
        with self.assertRaises(EngineError) as e:
            c.call("transport.fly")
        self.assertEqual(e.exception.code, "unknown_op")
        self.assertTrue(e.exception.message.endswith("."), e.exception.message)

    def test_frame_at_the_limit_round_trips(self):
        c = self.engine.connect()
        head = b'{"id": 77, "op": "ping", "args": {"pad": "'
        tail = b'"}}'
        payload = head + b"x" * (MAX_FRAME - len(head) - len(tail)) + tail
        self.assertEqual(len(payload), MAX_FRAME)
        c.send_raw(payload)
        r = c.response(77)
        self.assertTrue(r["ok"], r)
        self.assertIn("t", r["result"])

    def test_bad_frames_close_the_connection(self):
        bad = {
            "empty": struct.pack("<I", 0),
            "over 16 MiB": struct.pack("<I", MAX_FRAME + 1) + b"{",
            "not JSON": struct.pack("<I", 8) + b"not json",
            "a JSON array": struct.pack("<I", 6) + b"[1, 2]",
            "a JSON string": struct.pack("<I", 4) + b'"hi"',
        }
        for name, raw in bad.items():
            with self.subTest(name):
                c = self.engine.connect()
                c.sock.sendall(raw)
                self.assertTrue(c.closed(), name)
                c.close()
        # A short read: the length says more than ever arrives.
        c = self.engine.connect()
        c.sock.sendall(struct.pack("<I", 100) + b'{"id": 1')
        c.sock.shutdown(socket.SHUT_WR)
        self.assertTrue(c.closed())
        c.close()
        self.engine.connect().call("ping")

    def test_a_client_that_stops_reading_cant_wedge_the_socket(self):
        a = self.engine.connect()
        # Thousands of quick errors whose answers a never reads, until the engine's writes block...
        flood = b"".join(struct.pack("<I", len(m)) + m for m in (
            json.dumps({"id": i, "op": "param.set", "args": {"node": "master", "param": "gain_db", "value": 0}})
            .encode() for i in range(2, 20002)))
        a.sock.setblocking(False)
        sent = 0
        deadline = time.monotonic() + 5
        while sent < len(flood) and time.monotonic() < deadline:
            try:
                sent += a.sock.send(flood[sent:])
            except BlockingIOError:
                time.sleep(0.01)
        a.sock.setblocking(True)
        time.sleep(0.3)
        # ...then a bad frame: the engine drops a, and the next client is served.
        a.sock.sendall(struct.pack("<I", 0))
        b = Client(self.engine.sock_path)
        self.engine.clients.append(b)
        self.assertEqual(b.call("hello", {"protocol": 1, "client": "next"}, timeout=2)["protocol"], 1)

    def test_utf8_round_trips(self):
        c = self.engine.connect(hello=False)
        r = c.call("hello", {"protocol": 1, "client": "wi_wwav 0.1.0 · Wi-WWAV ✓"})
        self.assertEqual(r["protocol"], 1)

    def test_one_client_at_a_time(self):
        a = self.engine.connect()
        b = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        b.connect(self.engine.sock_path)
        msg = json.dumps({"id": 1, "op": "hello", "args": {"protocol": 1, "client": "second"}}).encode()
        b.sendall(struct.pack("<I", len(msg)) + msg)
        b.settimeout(0.3)
        with self.assertRaises(socket.timeout):
            b.recv(4)
        a.close()
        b.settimeout(5)
        n = struct.unpack("<I", b.recv(4))[0]
        reply = b""
        while len(reply) < n:
            reply += b.recv(n - len(reply))
        self.assertTrue(json.loads(reply)["ok"])
        b.close()

    def test_device_list_and_open(self):
        c = self.engine.connect()
        devices = c.call("device.list")["devices"]
        null = [d for d in devices if d["name"] == "null"]
        self.assertEqual(len(null), 1, devices)
        self.assertEqual(null[0]["outputs"], 2)
        self.assertIn(44100, null[0]["rates"])
        self.assertIn(48000, null[0]["rates"])
        r = c.call("device.open", {"name": "null", "sample_rate": 44100, "block": 512})
        self.assertEqual(r, {"name": "null", "sample_rate": 44100, "block": 512, "output_latency": 0,
                             "input_latency": 0})
        h = self.shm.header()
        self.assertEqual((h["sample_rate"], h["block"]), (44100, 512))
        self.shm.wait_callbacks(3)
        # null: the default device, which is the null device when the engine started with --device null.
        r = c.call("device.open", {"name": None, "sample_rate": 48000, "block": 128})
        self.assertEqual(r["name"], "null")
        with self.assertRaises(EngineError) as e:
            c.call("device.open", {"name": "No Such Device 9000", "sample_rate": 48000, "block": 128})
        self.assertEqual(e.exception.code, "no_such_device")

    def test_later_stages_say_unsupported(self):
        c = self.engine.connect()
        for op, args in [("plugin.state", {"node": "x"}), ("plugin.editor.open", {"node": "x"}),
                         ("plugin.editor.close", {"node": "x"}), ("plugin.params", {"node": "x"}),
                         ("midi.inputs", None), ("midi.route", {"input": "a", "track": "b", "channel": None})]:
            with self.subTest(op):
                with self.assertRaises(EngineError) as e:
                    c.call(op, args)
                self.assertEqual(e.exception.code, "unsupported")
                self.assertTrue(e.exception.message, op)

    def test_shutdown(self):
        c = self.engine.connect()
        self.assertEqual(c.call("shutdown"), {})
        self.assertEqual(self.engine.proc.wait(timeout=1.0), 0)


class WithoutTestFlag(unittest.TestCase):
    def test_debug_ops_are_unknown_without_test(self):
        shm = Shm()
        self.addCleanup(shm.close)
        engine = Engine(shm, test=False)
        self.addCleanup(engine.stop)
        c = engine.connect()
        for op in ("debug.crash", "debug.hang", "debug.crumb"):
            with self.subTest(op):
                with self.assertRaises(EngineError) as e:
                    c.call(op, {"in": "audio", "node": "x"})
                self.assertEqual(e.exception.code, "unknown_op")
        c.call("ping")

    def test_missing_shared_memory_is_refused(self):
        class Gone:
            name = "/wwav-no-such-region-0"

        with self.assertRaises(RuntimeError):
            Engine(Gone())


if __name__ == "__main__":
    unittest.main()
