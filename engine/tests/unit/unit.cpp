// Unit tests for the parts of wwav-engine that need neither JUCE nor a
// device: the clock's seqlock (F8: a reader never sees a torn clock under a
// writer at full rate), the command queue, gains and the graph's sums (the
// fold rule), the WAV writer, finding a clip in a file, and the streamed
// ring under a racing reader thread.
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <filesystem>
#include <string>
#include <thread>
#include <vector>

#include "graph.h"
#include "json.h"
#include "media.h"
#include "pyjson.h"
#include "shm.h"
#include "spsc.h"
#include "wav.h"

using namespace wwav;

static int failures = 0;

#define CHECK(cond)                                                      \
  do {                                                                   \
    if (!(cond)) {                                                       \
      fprintf(stderr, "%s:%d: failed: %s\n", __FILE__, __LINE__, #cond); \
      failures++;                                                        \
    }                                                                    \
  } while (0)

// Folders the tests write files into; main removes them at the end.
static std::vector<std::string> tempDirs;

static std::string tempDir() {
  char tmpl[] = "/tmp/wwav-unit-XXXXXX";
  const char* d = mkdtemp(tmpl);
  if (!d) abort();
  tempDirs.push_back(d);
  return d;
}

static void writeFile(const std::string& path, const std::vector<uint8_t>& bytes) {
  FILE* f = fopen(path.c_str(), "wb");
  if (!f) abort();
  fwrite(bytes.data(), 1, bytes.size(), f);
  fclose(f);
}

static std::vector<uint8_t> readFile(const std::string& path) {
  std::vector<uint8_t> out;
  FILE* f = fopen(path.c_str(), "rb");
  if (!f) return out;
  int c;
  while ((c = fgetc(f)) != EOF) out.push_back((uint8_t)c);
  fclose(f);
  return out;
}

static void put16(std::vector<uint8_t>& b, uint32_t v) {
  b.push_back(v & 255);
  b.push_back((v >> 8) & 255);
}
static void put32(std::vector<uint8_t>& b, uint32_t v) {
  put16(b, v & 0xffff);
  put16(b, v >> 16);
}
static void putId(std::vector<uint8_t>& b, const char* id) { b.insert(b.end(), id, id + 4); }
static uint32_t get32(const std::vector<uint8_t>& b, size_t at) {
  return b[at] | (b[at + 1] << 8) | (b[at + 2] << 16) | ((uint32_t)b[at + 3] << 24);
}

// A WAV: fmt (16 bytes, or 40 for WAVE_FORMAT_EXTENSIBLE), then data.
static std::vector<uint8_t> wav(uint16_t tag, uint16_t channels, uint32_t rate, uint16_t bits,
                                const std::vector<uint8_t>& data, bool extensible = false) {
  std::vector<uint8_t> b;
  uint32_t fmtSize = extensible ? 40 : 16;
  putId(b, "RIFF");
  put32(b, 4 + 8 + fmtSize + 8 + (uint32_t)data.size());
  putId(b, "WAVE");
  putId(b, "fmt ");
  put32(b, fmtSize);
  put16(b, extensible ? 0xFFFE : tag);
  put16(b, channels);
  put32(b, rate);
  put32(b, rate * channels * bits / 8);
  put16(b, channels * bits / 8);
  put16(b, bits);
  if (extensible) {
    put16(b, 22);
    put16(b, bits);
    put32(b, 3);
    put16(b, tag);  // the subformat GUID starts with the format tag
    static const uint8_t rest[14] = {0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80,
                                     0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71};
    b.insert(b.end(), rest, rest + 14);
  }
  putId(b, "data");
  put32(b, (uint32_t)data.size());
  b.insert(b.end(), data.begin(), data.end());
  return b;
}

// A .wwav laid out as wwav_pack.py lays one out. `frames` are the master's;
// wstm holds `wstmFrames` and wmet may say otherwise. `claim` adds to the
// frames wstm's header says it holds.
static std::vector<uint8_t> wwavFile(uint32_t frames, uint32_t wstmFrames, uint32_t wmetFrames, uint32_t claim = 0) {
  std::vector<uint8_t> master;
  for (uint32_t f = 0; f < frames; f++) {
    put16(master, (f * 3) & 0xffff);
    put16(master, (f * 5) & 0xffff);
  }
  std::vector<uint8_t> b = wav(1, 2, 44100, 16, master);
  std::string meta =
      "{\"wwav\": \"0.1\", \"song_id\": \"0123456789abcdef0123456789abcdef\", \"title\": \"T\", "
      "\"artist\": \"\", \"frames\": " +
      std::to_string(wmetFrames) + ", \"type\": \"original\", \"created\": \"\"}";
  putId(b, "wmet");
  put32(b, (uint32_t)meta.size());
  b.insert(b.end(), meta.begin(), meta.end());
  if (meta.size() & 1) b.push_back(0);
  uint32_t payload = (uint32_t)b.size() + 8;
  uint16_t pad = (uint16_t)((512 - (payload + 16) % 512) % 512);
  putId(b, "wstm");
  put32(b, 16 + pad + wstmFrames * 16);
  put16(b, 1);
  b.push_back(4);
  b.push_back(2);
  b.push_back(16);
  b.push_back(0);
  put16(b, pad);
  put32(b, 44100);
  put32(b, wstmFrames + claim);
  b.insert(b.end(), pad, 0);
  for (uint32_t f = 0; f < wstmFrames; f++) {
    for (int s = 0; s < 4; s++) {
      put16(b, (f + 1000 * s) & 0x7fff);                         // left
      put16(b, (0x10000 - ((f + 1000 * s) & 0x7fff)) & 0xffff);  // right: its negative
    }
  }
  std::string lin =
      "{\"parent_id\": null, \"root_id\": \"0123456789abcdef0123456789abcdef\", \"generation\": 0, "
      "\"creator\": \"\", \"device_id\": \"\"}";
  putId(b, "wlin");
  put32(b, (uint32_t)lin.size());
  b.insert(b.end(), lin.begin(), lin.end());
  if (lin.size() & 1) b.push_back(0);
  uint32_t riff = (uint32_t)b.size() - 8;
  b[4] = riff & 255;
  b[5] = (riff >> 8) & 255;
  b[6] = (riff >> 16) & 255;
  b[7] = riff >> 24;
  return b;
}

// ---- the clock -------------------------------------------------------------------

static void seqlockNeverTears() {
  wwav_shm_clock clock;
  memset(&clock, 0, sizeof clock);
  std::atomic<bool> stop{false};
  std::atomic<uint64_t> writes{0};
  std::thread writer([&] {
    uint64_t k = 0;
    while (!stop.load(std::memory_order_relaxed)) {
      k++;
      ClockState s;
      s.samplePos = (int64_t)k * 128;
      s.hostTimeNs = k * 3;
      s.rate = (double)k * 0.5;
      s.state = (uint32_t)(k & 1);
      s.dropouts = (uint32_t)(k * 7);
      s.callbacks = k;
      writeClock(&clock, s);
    }
    writes = k;
  });
  uint64_t reads = 0, torn = 0, last = 0;
  auto until = std::chrono::steady_clock::now() + std::chrono::milliseconds(400);
  while (std::chrono::steady_clock::now() < until) {
    ClockState s = readClock(&clock);
    uint64_t k = s.callbacks;
    reads++;
    if (k == 0) continue;
    if (s.samplePos != (int64_t)k * 128 || s.hostTimeNs != k * 3 || s.rate != (double)k * 0.5 ||
        s.state != (uint32_t)(k & 1) || s.dropouts != (uint32_t)(k * 7))
      torn++;
    CHECK(k >= last);
    last = k;
  }
  stop = true;
  writer.join();
  CHECK(torn == 0);
  CHECK(reads > 10000);
  CHECK(writes > 100000);
  printf("  %llu writes, %llu reads, %llu torn\n", (unsigned long long)writes.load(), (unsigned long long)reads,
         (unsigned long long)torn);
  CHECK(clock.seq % 2 == 0);
}

static void clockRecoversFromAWriterKilledMidWrite() {
  wwav_shm_clock clock;
  memset(&clock, 0, sizeof clock);
  clock.seq = 41;  // odd: the last engine died between the two halves
  ClockState s;
  s.samplePos = 99;
  s.callbacks = 1;
  writeClock(&clock, s);
  CHECK(clock.seq % 2 == 0);
  CHECK(clock.seq > 41);
  CHECK(readClock(&clock).samplePos == 99);
}

// The meter ring as the app reads it (crates/wwav-wire, Region::newest_meters):
// the newest entry, copied, then kept only if meter_write shows the writer
// hadn't come round the ring to it meanwhile. Every copy kept must be one
// whole block's entry. On x86 stores are never reordered, so this fails only
// if the engine publishes an entry before it is complete or in the wrong
// place; the ordering for arm64 is the fences in shm.cpp.
static void meterRingNeverTearsForALappedReader() {
  const std::string name = "/wwav-unit-" + std::to_string(getpid());
  int fd = shm_open(name.c_str(), O_RDWR | O_CREAT | O_EXCL, 0600);
  if (fd < 0 || ftruncate(fd, WWAV_SHM_TOTAL_BYTES) != 0) abort();
  void* view = mmap(nullptr, WWAV_SHM_TOTAL_BYTES, PROT_READ, MAP_SHARED, fd, 0);
  close(fd);
  SharedRegion region;
  const std::string opened = region.open(name);
  shm_unlink(name.c_str());
  CHECK(opened.empty() && view != MAP_FAILED);
  if (!opened.empty() || view == MAP_FAILED) return;
  const wwav_shm* shm = static_cast<const wwav_shm*>(view);

  const uint32_t used = 9;
  std::atomic<bool> stop{false};
  std::thread writer([&] {
    for (uint32_t k = 1; !stop.load(std::memory_order_relaxed); k = k % 1000000 + 1) {
      wwav_shm_meter_entry* e = region.meterEntry();
      e->callback = k;
      e->slots_used = used;
      for (uint32_t s = 0; s < used; s++) e->slots[s] = wwav_shm_meter_slot{(float)k, (float)k, (float)k, (float)k};
      region.publishMeters();
    }
  });
  uint64_t kept = 0, lapped = 0, torn = 0;
  auto until = std::chrono::steady_clock::now() + std::chrono::milliseconds(300);
  while (std::chrono::steady_clock::now() < until) {
    const uint64_t n = __atomic_load_n(&shm->meters.meter_write, __ATOMIC_ACQUIRE);
    if (n == 0) continue;
    const wwav_shm_meter_entry* e = &shm->ring[(n - 1) % WWAV_METER_RING];
    const uint64_t callback = __atomic_load_n(&e->callback, __ATOMIC_RELAXED);
    const uint32_t slots = __atomic_load_n(&e->slots_used, __ATOMIC_RELAXED);
    bool whole = slots == used;
    for (uint32_t s = 0; s < used; s++) {
      float v[4];
      __atomic_load(&e->slots[s].peak_l, &v[0], __ATOMIC_RELAXED);
      __atomic_load(&e->slots[s].peak_r, &v[1], __ATOMIC_RELAXED);
      __atomic_load(&e->slots[s].rms_l, &v[2], __ATOMIC_RELAXED);
      __atomic_load(&e->slots[s].rms_r, &v[3], __ATOMIC_RELAXED);
      for (float x : v) whole = whole && x == (float)callback;
    }
    __atomic_thread_fence(__ATOMIC_ACQUIRE);
    if (__atomic_load_n(&shm->meters.meter_write, __ATOMIC_RELAXED) >= n - 1 + WWAV_METER_RING) {
      lapped++;
      continue;
    }
    kept++;
    if (!whole || callback != (n - 1) % 1000000 + 1) torn++;
  }
  stop = true;
  writer.join();
  munmap(view, WWAV_SHM_TOTAL_BYTES);
  printf("  %llu entries kept, %llu lapped, %llu torn\n", (unsigned long long)kept, (unsigned long long)lapped,
         (unsigned long long)torn);
  CHECK(torn == 0);
  CHECK(kept > 10000);
}

// ---- the command queue -----------------------------------------------------------

static void spscKeepsOrder() {
  static Spsc<uint64_t, 256> q;
  const uint64_t n = 2000000;
  std::thread producer([&] {
    for (uint64_t i = 1; i <= n; i++)
      while (!q.push(i)) std::this_thread::yield();
  });
  uint64_t want = 1, v;
  bool inOrder = true;
  while (want <= n) {
    if (q.pop(&v)) {
      if (v != want) inOrder = false;
      want++;
    }
  }
  producer.join();
  CHECK(inOrder);
  CHECK(!q.pop(&v));
}

// ---- gains and the graph ---------------------------------------------------------

// The crumb is the node id's FNV-1a 64, as wwav_ids::fnv1a64 computes it:
// the app names the crashed node by hashing its own ids the same way.
static void crumbsAreStandardFnv1a64() {
  CHECK(fnv1a64("") == 0xcbf29ce484222325ull);
  CHECK(fnv1a64("a") == 0xaf63dc4c8601ec8cull);
  CHECK(fnv1a64("foobar") == 0x85944171f73967e8ull);
}

static void gainsAreExactWhereTheyMustBe() {
  CHECK(dbGain(0.0f) == 1.0f);
  CHECK(fabsf(dbGain(-6.0206f) - 0.5f) < 1e-4f);
  CHECK(fabsf(dbGain(6.0206f) - 2.0f) < 2e-4f);
  CHECK(dbGain(-120.0f) < 1.1e-6f);
  float l, r;
  balance(0.0f, &l, &r);
  CHECK(l == 1.0f && r == 1.0f);
  balance(1.0f, &l, &r);
  CHECK(fabsf(l) < 1e-6f && r == 1.0f);
  balance(-1.0f, &l, &r);
  CHECK(l == 1.0f && fabsf(r) < 1e-6f);
  balance(0.5f, &l, &r);
  CHECK(fabsf(l - 0.70710678f) < 1e-5f && r == 1.0f);
  Ramp ramp;
  ramp.target = 0.0f;
  ramp.begin(4);
  float got[4];
  for (float& g : got) g = ramp.next();
  ramp.end();
  CHECK(got[0] == 0.75f && got[1] == 0.5f && got[2] == 0.25f && got[3] == 0.0f);
  CHECK(ramp.cur == 0.0f);
}

static std::vector<uint8_t> pattern16(int frames) {
  std::vector<uint8_t> d;
  for (int f = 0; f < frames; f++) {
    put16(d, (uint16_t)(int16_t)((f * 37) % 20000 - 10000));
    put16(d, (uint16_t)(int16_t)(-((f * 37) % 20000 - 10000)));
  }
  return d;
}

static float pattern(int f, int ch) {
  int v = (f * 37) % 20000 - 10000;
  return (ch == 0 ? v : -v) / 32768.0f;
}

// Two tracks from one 16-bit stereo WAV: track 0 (vocals) plays it from its
// start, track 1 (bass) plays it from frame 100 at a clip gain of -6 dB.
static std::unique_ptr<Graph> twoTrackGraph(const std::string& path) {
  MediaPart part;
  std::string code, message;
  CHECK(probeMedia(path, "master", &part, &code, &message));
  auto g = std::make_unique<Graph>(44100);
  for (int i = 0; i < 2; i++) {
    Track t;
    t.id = i == 0 ? "01JTRACKA" : "01JTRACKB";
    t.role = i == 0 ? kVocals : kBass;
    Clip c;
    c.at = 0;
    c.gain = i == 0 ? 1.0f : dbGain(-6.0f);
    c.source = ClipSource::open(path, part, i == 0 ? 0 : 100, 2000, true, &message);
    CHECK(c.source != nullptr);
    t.clips.push_back(std::move(c));
    g->tracks.push_back(std::move(t));
  }
  g->prepare();
  return g;
}

static void graphFoldsTracksIntoBusesAndBusesIntoTheMaster() {
  std::string dir = tempDir();
  std::string path = dir + "/a.wav";
  writeFile(path, wav(1, 2, 44100, 16, pattern16(3000)));
  auto g = twoTrackGraph(path);
  CHECK(g->nodes() == 7);
  CHECK(g->find("01JTRACKB") == 1);
  CHECK(g->find("bus:bass") == 5);
  CHECK(g->find("master") == 6);
  CHECK(g->find("bus:keys") == -1);
  CHECK(g->nodeId(3) == "bus:drums");
  CHECK(g->takes(6, Param::GainDb) && !g->takes(6, Param::Solo) && !g->takes(6, Param::Pan));

  wwav_shm_meter_entry meters;
  g->beginBlock(256);
  g->process(0, 0, 256, true, true, nullptr);
  CHECK(g->endBlock(256, &meters) == 7);
  float half = dbGain(-6.0f);
  bool exact = true, folded = true;
  for (int i = 0; i < 256; i++) {
    exact = exact && g->busL(kVocals)[i] == pattern(i, 0) && g->busR(kVocals)[i] == pattern(i, 1);
    exact = exact && g->busL(kBass)[i] == pattern(100 + i, 0) * half;
    exact = exact && g->busL(kDrums)[i] == 0.0f && g->busL(kOther)[i] == 0.0f;
    float sum = ((g->busL(0)[i] + g->busL(1)[i]) + g->busL(2)[i]) + g->busL(3)[i];
    folded = folded && g->masterL()[i] == sum;
  }
  CHECK(exact);
  CHECK(folded);
  // the vocals track (slot 0) and its bus (slot 2) meter the same; drums (slot 3) is silent
  CHECK(meters.slots[0].peak_l == meters.slots[2].peak_l && meters.slots[0].peak_l > 0.25f);
  CHECK(meters.slots[3].peak_l == 0.0f && meters.slots[3].rms_r == 0.0f);

  // Mute: the next stretch fades over its length, the one after is silent.
  g->set(0, Param::Mute, 1.0f);
  g->beginBlock(256);
  g->process(256, 0, 256, true, true, nullptr);
  g->endBlock(256, &meters);
  CHECK(g->busL(kVocals)[255] == 0.0f);
  CHECK(meters.slots[2].rms_l > 0.0f);
  g->beginBlock(256);
  g->process(512, 0, 256, true, true, nullptr);
  g->endBlock(256, &meters);
  CHECK(meters.slots[2].peak_l == 0.0f && meters.slots[0].peak_r == 0.0f);
  CHECK(meters.slots[5].peak_l > 0.0f);  // bass plays on

  // Solo on bass: unmuting vocals keeps it silent while bass is soloed.
  g->set(1, Param::Solo, 1.0f);
  g->set(0, Param::Mute, 0.0f);
  for (int b = 0; b < 2; b++) {
    g->beginBlock(256);
    g->process(768 + 256 * b, 0, 256, true, true, nullptr);
    g->endBlock(256, &meters);
  }
  CHECK(meters.slots[2].peak_l == 0.0f && meters.slots[5].peak_l > 0.0f);

  // Stopped: tracks give silence.
  g->beginBlock(256);
  g->process(0, 0, 256, false, true, nullptr);
  g->endBlock(256, &meters);
  CHECK(meters.slots[5].peak_l == 0.0f && meters.slots[6].peak_l == 0.0f);
}

static void graphStretchesAcrossALoopMatchTheFile() {
  std::string dir = tempDir();
  std::string path = dir + "/a.wav";
  writeFile(path, wav(1, 2, 44100, 16, pattern16(3000)));
  auto g = twoTrackGraph(path);
  // [1000, 1100) then [200, 356) of the session into one 256-frame block
  g->beginBlock(256);
  g->process(1000, 0, 100, true, true, nullptr);
  g->process(200, 100, 156, true, true, nullptr);
  g->endBlock(256, nullptr);
  bool ok = true;
  for (int i = 0; i < 256; i++) {
    int f = i < 100 ? 1000 + i : 200 + (i - 100);
    ok = ok && g->busL(kVocals)[i] == pattern(f, 0);
  }
  CHECK(ok);
}

// ---- the WAV writer --------------------------------------------------------------

static void wavWriterWritesCanonicalHeaders() {
  std::string dir = tempDir();
  float l[3] = {0.5f, -1.0f, 1.0f / 32768.0f};
  float r[3] = {2.0f, 0.25f, 0.0f};
  std::string message;
  {
    WavWriter w;
    CHECK(w.open(dir + "/f.wav", WavFormat::F32, 48000, 3, &message));
    CHECK(w.write(l, r, 3));
    CHECK(w.close(&message));
  }
  std::vector<uint8_t> f = readFile(dir + "/f.wav");
  CHECK(f.size() == 82);
  CHECK(memcmp(f.data(), "RIFF", 4) == 0 && get32(f, 4) == 74 && memcmp(f.data() + 8, "WAVEfmt ", 8) == 0);
  CHECK(get32(f, 16) == 18);            // fmt size, with cbSize
  CHECK((get32(f, 20) & 0xffff) == 3);  // WAVE_FORMAT_IEEE_FLOAT
  CHECK((get32(f, 20) >> 16) == 2);     // stereo
  CHECK(get32(f, 24) == 48000 && get32(f, 28) == 48000 * 8);
  CHECK((get32(f, 32) & 0xffff) == 8 && (get32(f, 32) >> 16) == 32);
  CHECK(memcmp(f.data() + 38, "fact", 4) == 0 && get32(f, 42) == 4 && get32(f, 46) == 3);
  CHECK(memcmp(f.data() + 50, "data", 4) == 0 && get32(f, 54) == 24);
  float first;
  memcpy(&first, f.data() + 58, 4);
  CHECK(first == 0.5f);
  {
    WavWriter w;
    CHECK(w.open(dir + "/s.wav", WavFormat::S16, 44100, 3, &message));
    CHECK(w.write(l, r, 3));
    CHECK(w.close(&message));
  }
  std::vector<uint8_t> s = readFile(dir + "/s.wav");
  CHECK(s.size() == 56 && get32(s, 4) == 48 && get32(s, 16) == 16 && (get32(s, 20) & 0xffff) == 1);
  CHECK(get32(s, 40) == 12);
  CHECK((int16_t)(s[44] | s[45] << 8) == 16384);
  CHECK((int16_t)(s[46] | s[47] << 8) == 32767);  // 2.0 clamps
  CHECK((int16_t)(s[48] | s[49] << 8) == -32768);
  CHECK((int16_t)(s[52] | s[53] << 8) == 1);
  // a short write is an error, not a quiet half-file
  {
    WavWriter w;
    CHECK(w.open(dir + "/short.wav", WavFormat::F32, 44100, 4, &message));
    CHECK(w.write(l, r, 3));
    CHECK(!w.close(&message));
    CHECK(!message.empty());
  }
  CHECK(toS16(0.5f / 32768.0f) == 0 && toS16(1.5f / 32768.0f) == 2 && toS16(-1.5f) == -32768);
}

// ---- finding a clip in a file ----------------------------------------------------

static bool probe(const std::string& path, const std::string& source, MediaPart* p, std::string* code,
                  std::string* message) {
  code->clear();
  message->clear();
  return probeMedia(path, source, p, code, message);
}

static void probeFindsMastersAndStems() {
  std::string dir = tempDir();
  std::string code, message;
  MediaPart p;
  float l[4], r[4];

  writeFile(dir + "/s16.wav", wav(1, 2, 48000, 16, pattern16(10)));
  CHECK(probe(dir + "/s16.wav", "master", &p, &code, &message));
  CHECK(p.rate == 48000 && p.channels == 2 && p.bits == 16 && !p.isFloat && p.dataAt == 44 && p.stride == 4 &&
        p.offset == 0 && p.frames == 10);

  std::vector<uint8_t> d24;
  for (int v : {0x123456, -2, 0x7fffff, -0x800000}) {
    d24.push_back(v & 255);
    d24.push_back((v >> 8) & 255);
    d24.push_back((v >> 16) & 255);
  }
  writeFile(dir + "/m24.wav", wav(1, 1, 44100, 24, d24, true));
  CHECK(probe(dir + "/m24.wav", "master", &p, &code, &message));
  CHECK(p.channels == 1 && p.bits == 24 && p.frames == 4 && p.dataAt == 68);
  decodeFrames(readFile(dir + "/m24.wav").data() + p.dataAt, 4, p, l, r);
  CHECK(l[0] == 0x123456 / 8388608.0f && r[0] == l[0] && l[1] == -2 / 8388608.0f && l[3] == -1.0f);

  std::vector<uint8_t> df;
  for (float v : {0.25f, -0.5f, 1.5f, 0.0f}) {
    uint32_t bits;
    memcpy(&bits, &v, 4);
    put32(df, bits);
  }
  writeFile(dir + "/f32.wav", wav(3, 2, 44100, 32, df));
  CHECK(probe(dir + "/f32.wav", "master", &p, &code, &message));
  CHECK(p.isFloat && p.bits == 32 && p.frames == 2);
  decodeFrames(readFile(dir + "/f32.wav").data() + p.dataAt, 2, p, l, r);
  CHECK(l[0] == 0.25f && r[0] == -0.5f && l[1] == 1.5f && r[1] == 0.0f);

  // a .wwav: its master, and a stem in place
  writeFile(dir + "/song.wwav", wwavFile(3000, 3000, 3000));
  CHECK(probe(dir + "/song.wwav", "master", &p, &code, &message));
  CHECK(p.dataAt == 44 && p.frames == 3000 && p.stride == 4);
  CHECK(probe(dir + "/song.wwav", "other", &p, &code, &message));
  CHECK(p.rate == 44100 && p.channels == 2 && p.bits == 16 && p.stride == 16 && p.offset == 8 && p.frames == 3000 &&
        p.dataAt % 512 == 0);
  std::vector<uint8_t> bytes = readFile(dir + "/song.wwav");
  decodeFrames(bytes.data() + p.dataAt + 16 * 7, 1, p, l, r);
  CHECK(l[0] == (7 + 2000) / 32768.0f && r[0] == -l[0]);

  // what the reference reader says when there are no stems, and what isn't read yet
  struct Case {
    std::vector<uint8_t> bytes;
    const char* source;
    const char* code;
    const char* says;
  } cases[] = {
      {wav(1, 2, 44100, 16, pattern16(10)), "vocals", "bad_clip", "the master only: no wmet and wlin, so a plain WAV"},
      {wwavFile(3000, 2999, 3000), "bass", "bad_clip", "frame counts differ (data 3000, wstm 2999, wmet 3000)"},
      {wwavFile(3000, 3000, 3000, 1), "bass", "bad_clip", "the master only: wstm is cut off"},
      {wav(1, 2, 48000, 16, pattern16(10)), "drums", "bad_clip",
       "not listed: the master isn't 44.1 kHz 16-bit stereo PCM"},
      {wav(1, 2, 44100, 8, std::vector<uint8_t>(20, 128)), "master", "unsupported", "8-bit"},
      {wav(1, 6, 44100, 16, std::vector<uint8_t>(120, 0)), "master", "unsupported", "6 channels"},
      {{'F', 'O', 'R', 'M', 0, 0, 0, 4, 'A', 'I', 'F', 'F'}, "master", "unsupported", "AIFF"},
      {{'h', 'e', 'l', 'l', 'o'}, "master", "bad_clip", "not a WAV"},
      {wwavFile(100, 100, 100), "keys", "bad_clip", "keys"},
  };
  for (Case& c : cases) {
    writeFile(dir + "/case.wav", c.bytes);
    bool ok = probe(dir + "/case.wav", c.source, &p, &code, &message);
    CHECK(!ok);
    CHECK(code == c.code);
    if (message.find(c.says) == std::string::npos) {
      fprintf(stderr, "  expected \"%s\" in \"%s\"\n", c.says, message.c_str());
      failures++;
    }
    CHECK(!message.empty() && message.back() == '.');
  }
  CHECK(!probe(dir + "/missing.wav", "master", &p, &code, &message));
  CHECK(code == "no_such_file");
}

// ---- the streamed ring -----------------------------------------------------------

static void streamedClipNeverReadsWrongAudio() {
  std::string dir = tempDir();
  const int frames = 400000;
  writeFile(dir + "/long.wav", wav(1, 2, 44100, 16, pattern16(frames)));
  MediaPart part;
  std::string code, message;
  CHECK(probe(dir + "/long.wav", "master", &part, &code, &message));
  const int64_t in = 1000, len = 380000;
  auto whole = ClipSource::open(dir + "/long.wav", part, in, len, true, &message);
  auto stream = ClipSource::open(dir + "/long.wav", part, in, len, false, &message);
  CHECK(whole && stream && !whole->streamed() && stream->streamed());
  stream->prefill(0);
  CHECK(stream->ready(0, 44100));

  Reader reader;
  reader.add(stream.get());
  std::vector<float> l(512), r(512);
  auto right = [&](int64_t c, int n) {
    for (int i = 0; i < n; i++)
      if (l[i] != pattern((int)(in + c + i), 0) || r[i] != pattern((int)(in + c + i), 1)) return false;
    return true;
  };
  // Playback at about four times real time, then a jump far ahead, then back.
  int misses = 0, wrong = 0;
  for (int64_t c = 0; c + 512 <= 200000; c += 512) {
    stream->want(c);
    if (!stream->read(c, 512, l.data(), r.data()))
      misses++;
    else if (!right(c, 512))
      wrong++;
    std::this_thread::sleep_for(std::chrono::microseconds(2900));
  }
  CHECK(misses == 0);
  for (int64_t start : {int64_t(300000), int64_t(5000)}) {
    int recovered = -1;
    for (int b = 0; b < 100; b++) {
      int64_t c = start + 512 * b;
      stream->want(c);
      bool ok = stream->read(c, 512, l.data(), r.data());
      if (ok && !right(c, 512)) wrong++;
      if (!ok) {
        bool silent = true;
        for (int i = 0; i < 512; i++) silent = silent && l[i] == 0.0f && r[i] == 0.0f;
        CHECK(silent);
      }
      if (ok && recovered < 0) recovered = b;
      std::this_thread::sleep_for(std::chrono::microseconds(2900));
    }
    CHECK(recovered >= 0 && recovered < 20);
  }
  CHECK(wrong == 0);
  reader.remove(stream.get());

  // The render thread's read and the held-whole clip agree with the file anywhere.
  for (int64_t c : {int64_t(0), int64_t(123457), int64_t(len - 100)}) {
    int n = (int)std::min<int64_t>(512, len - c);
    stream->readNow(c, n, l.data(), r.data());
    CHECK(right(c, n));
    whole->read(c, n, l.data(), r.data());
    CHECK(right(c, n));
  }
  // Past the file's end is silence, streamed or whole.
  for (bool held : {false, true}) {
    auto past = ClipSource::open(dir + "/long.wav", part, frames - 10, 100, held, &message);
    if (held)
      past->read(0, 100, l.data(), r.data());
    else
      past->readNow(0, 100, l.data(), r.data());
    CHECK(l[9] == pattern(frames - 1, 0) && l[10] == 0.0f && r[99] == 0.0f);
  }
  // A len far past the file's end holds only what the file has (it once
  // allocated the whole len, and died of it).
  auto huge = ClipSource::open(dir + "/long.wav", part, frames - 10, int64_t(1) << 40, true, &message);
  CHECK(huge && huge->length() == int64_t(1) << 40);
  huge->read(0, 100, l.data(), r.data());
  CHECK(l[9] == pattern(frames - 1, 0) && l[10] == 0.0f && r[99] == 0.0f);
  l[0] = 1.0f;
  huge->read(int64_t(1) << 39, 512, l.data(), r.data());
  CHECK(l[0] == 0.0f && r[511] == 0.0f);
}

static void aLoopFindsItsStartReady() {
  std::string dir = tempDir();
  const int frames = 400000;
  writeFile(dir + "/loop.wav", wav(1, 2, 44100, 16, pattern16(frames)));
  MediaPart part;
  std::string code, message;
  CHECK(probe(dir + "/loop.wav", "master", &part, &code, &message));
  auto stream = ClipSource::open(dir + "/loop.wav", part, 0, frames, false, &message);
  std::vector<float> l(512), r(512);
  int misses = 0, wrong = 0;
  auto play = [&](int64_t c) {
    stream->want(c);
    if (!stream->read(c, 512, l.data(), r.data())) {
      misses++;
      return;
    }
    for (int i = 0; i < 512; i++)
      if (l[i] != pattern((int)(c + i), 0) || r[i] != pattern((int)(c + i), 1)) return (void)wrong++;
  };
  // A loop longer than the main window keeps behind the playhead, at about
  // eight times real time: three times round, then a loop somewhere else.
  stream->wantLoop(50000);
  stream->prefill(50000);
  Reader reader;
  reader.add(stream.get());
  for (auto loop : {std::pair<int64_t, int64_t>{50000, 150000}, {200000, 290000}}) {
    stream->wantLoop(loop.first);
    std::this_thread::sleep_for(std::chrono::milliseconds(50));  // the reader fills the new loop's window
    for (int64_t c = loop.first, k = 0; k < 3 * (loop.second - loop.first) / 512; k++) {
      play(c);
      c = c + 1024 > loop.second ? loop.first : c + 512;
      std::this_thread::sleep_for(std::chrono::microseconds(1450));
    }
  }
  CHECK(misses == 0);
  CHECK(wrong == 0);
  reader.remove(stream.get());
}

// What a parse reports, as one line: the shape and the decoded values.
struct JsonTrace : json::Handler {
  std::string t;
  void beginObject() override { t += "{"; }
  void beginArray() override { t += "["; }
  void end() override { t += "}"; }
  void key(const std::string& k) override { t += "k:" + k + " "; }
  void string(const std::string& s) override { t += "s:" + s + " "; }
  void number(const char* p, size_t n, bool integral) override {
    t += (integral ? "i:" : "f:") + std::string(p, n) + " ";
  }
  void boolean(bool b) override { t += b ? "true " : "false "; }
  void null() override { t += "null "; }
  void nonFinite(double d) override { t += isnan(d) ? "nan " : d > 0 ? "inf " : "-inf "; }
};

static bool parses(const std::string& text, json::Dialect d, size_t depth = 0) {
  return json::parseObject(text.data(), text.size(), d, depth, nullptr);
}

static std::string nested(int depth) {
  return "{\"a\":" + std::string(depth - 1, '[') + std::string(depth - 1, ']') + "}";
}

static void jsonIsStrictAndPythonsDialectIsPythons() {
  using json::Dialect;
  // Both dialects: one object, whitespace around it, every kind of value.
  for (const char* ok : {"{}", " \t\r\n{} \n", "{\"a\":1}", "{\"a\":[1,2,{\"b\":null}],\"c\":true,\"d\":false}",
                         "{\"\":0}", "{\"a\":\"\\u0000\"}", "{\"a\":\"\\ud83d\\ude00\"}", "{\"a\":-0}",
                         "{\"a\":0.5e-3}", "{\"a\":1E+2}", "{\"a\":\"\\/\\b\\f\\n\\r\\t\\\"\\\\\"}",
                         "{\"a\":\"\xc3\xa9\xf0\x9f\x98\x80\"}", "{\"a\":\"\x7f\"}", "{\"a\":{},\"b\":[]}"}) {
    CHECK(parses(ok, Dialect::Strict));
    CHECK(parses(ok, Dialect::Python));
  }
  // Neither: not one object, not JSON, not UTF-8.
  for (const char* bad : {"[]",
                          "\"x\"",
                          "1",
                          "",
                          " ",
                          "{",
                          "}",
                          "{\"a\":1}x",
                          "{\"a\":1}{\"b\":2}",
                          "{\"a\":1,}",
                          "{\"a\":[1,]}",
                          "{,}",
                          "{a:1}",
                          "{'a':1}",
                          "{\"a\":01}",
                          "{\"a\":1.}",
                          "{\"a\":.5}",
                          "{\"a\":+1}",
                          "{\"a\":- 1}",
                          "{\"a\":1e}",
                          "{\"a\":\"\\x\"}",
                          "{\"a\":\"\\u12\"}",
                          "{\"a\":\"\\u12g4\"}",
                          "{\"a\":\"tab\there\"}",
                          "{\"a\":\"\xff\"}",
                          "{\"a\":\"\xc0\xaf\"}",
                          "{\"a\":\"\xed\xa0\x80\"}",
                          "{\"a\":\"\xf4\x90\x80\x80\"}",
                          "{\"a\":\"\xe2\x82\"}",
                          "{\"a\":1}/*c*/",
                          "{\"a\" 1}",
                          "{\"a\":1 \"b\":2}",
                          "{\"a\":tru}",
                          "{\"a\":nulls}",
                          "\xef\xbb\xbf{}",
                          "{\"a\":-NaN}",
                          "{\"a\":infinity}",
                          "{\"a\":+Infinity}",
                          "{\"a\":[}",
                          "{\"a\":]}",
                          "{\"a\":[1}",
                          "{\"a\":{]}",
                          "{\"a\":\"x}"}) {
    CHECK(!parses(bad, Dialect::Strict));
    CHECK(!parses(bad, Dialect::Python));
  }
  // Python's json module also takes NaN, the infinities and lone surrogates; RFC 8259 and I-JSON don't.
  for (const char* py : {"{\"a\":NaN}", "{\"a\":Infinity}", "{\"a\":-Infinity}", "{\"a\":\"\\ud800\"}",
                         "{\"a\":\"\\udc00x\"}", "{\"a\":\"\\ud800\\ud800\"}", "{\"a\":\"\\ud800\\u0041\"}"}) {
    CHECK(!parses(py, Dialect::Strict));
    CHECK(parses(py, Dialect::Python));
  }
  // Python's int refuses over 4300 digits; a float of any length is fine.
  const std::string digits(4300, '7');
  CHECK(parses("{\"a\":" + digits + "}", Dialect::Python));
  CHECK(parses("{\"a\":-" + digits + "}", Dialect::Python));
  CHECK(!parses("{\"a\":" + digits + "7}", Dialect::Python));
  CHECK(!parses("{\"a\":-" + digits + "7}", Dialect::Python));
  CHECK(parses("{\"a\":" + digits + "7.0}", Dialect::Python));
  CHECK(parses("{\"a\":" + digits + "7}", Dialect::Strict));
  // Depth: a limit when asked for one, and no stack to overflow without one.
  CHECK(parses(nested(128), Dialect::Strict, 128));
  CHECK(!parses(nested(129), Dialect::Strict, 128));
  CHECK(parses(nested(1000000), Dialect::Python));
  // What the handler hears, decoded.
  JsonTrace t;
  const std::string text = "{\"k\" : [1, -2.5e3, {\"b\":\"a\\u00e9\\ud83d\\ude00\\u0000\"}], \"n\":null,\"t\":true}";
  CHECK(json::parseObject(text.data(), text.size(), Dialect::Strict, 0, &t));
  CHECK(t.t == std::string("{k:k [i:1 f:-2.5e3 {k:b s:a\xc3\xa9\xf0\x9f\x98\x80") + std::string(1, '\0') +
                   " }}k:n null k:t true }");
  JsonTrace py;
  const std::string pyText = "{\"x\":[NaN,Infinity,-Infinity,\"\\ud800\"]}";
  CHECK(json::parseObject(pyText.data(), pyText.size(), Dialect::Python, 0, &py));
  CHECK(py.t == "{k:x [nan inf -inf s:\xed\xa0\x80 }}");
}

// Every expected string below is what Python 3.13 printed for the same input.
static void pythonReadsAndPrintsAsPython() {
  // repr() of floats: the shortest digits, laid out as Python lays them out.
  const struct {
    double d;
    const char* repr;
  } floats[] = {{0.0, "0.0"},
                {-0.0, "-0.0"},
                {1.0, "1.0"},
                {0x1.999999999999ap-4, "0.1"},
                {100.0, "100.0"},
                {44100.0, "44100.0"},
                {1e15, "1000000000000000.0"},
                {1e16, "1e+16"},
                {1234567890123456.0, "1234567890123456.0"},
                {0x1.5ee2a2eb5a5c4p+53, "1.2345678901234568e+16"},
                {0x1.a36e2eb1c432dp-14, "0.0001"},
                {0x1.4f8b588e368f1p-17, "1e-05"},
                {0x1.421f5f40d8376p-23, "1.5e-07"},
                {0x1.7e43c8800759cp+996, "1e+300"},
                {0x1.fffffffffffffp+1023, "1.7976931348623157e+308"},
                {0x0.0000000000001p-1022, "5e-324"},
                {0x1.0000000000000p-1022, "2.2250738585072014e-308"},
                {0x1.edd2f1a9fbe77p+6, "123.456"},
                {0x1.5555555555555p-2, "0.3333333333333333"},
                {0x1.0000000000000p+53, "9007199254740992.0"},
                {0x1.3333333333334p-2, "0.30000000000000004"},
                {-0x1.a36e2eb1c432dp-16, "-2.5e-05"},
                {0x1.0f0cf064dd592p+73, "1e+22"},
                {0x1.52d02c7e14af6p+76, "1e+23"},
                {0x1.1666666666666p+2, "4.35"},
                {0x1.5af1d78b58c40p+66, "1e+20"},
                {INFINITY, "inf"},
                {-INFINITY, "-inf"},
                {NAN, "nan"}};
  for (const auto& f : floats) {
    if (py::floatRepr(f.d) != f.repr) {
      printf("  floatRepr(%a) is %s, not %s\n", f.d, py::floatRepr(f.d).c_str(), f.repr);
      failures++;
    }
  }
  // re.match(r"^\d+", s) and int() of it, with any script's digits.
  bool positive;
  CHECK(py::leadingDigits("0.1", &positive) && !positive);
  CHECK(py::leadingDigits("000", &positive) && !positive);
  CHECK(py::leadingDigits("99999999999999999999", &positive) && positive);
  CHECK(py::leadingDigits("1.0", &positive) && positive);
  CHECK(py::leadingDigits("\xd9\xa3x", &positive) && positive);         // U+0663 ARABIC-INDIC DIGIT THREE
  CHECK(py::leadingDigits("\xef\xbc\x90.2", &positive) && !positive);   // U+FF10 FULLWIDTH DIGIT ZERO
  CHECK(py::leadingDigits("\xf0\x9f\xaf\xb7", &positive) && positive);  // U+1FBF7 SEGMENTED DIGIT SEVEN
  CHECK(!py::leadingDigits("", &positive) && !py::leadingDigits("v1", &positive));
  CHECK(!py::leadingDigits("-1", &positive) && !py::leadingDigits("\xc2\xb2", &positive));  // superscript two isn't Nd

  // json.loads, the last of a name winning, and str() of what it made.
  const struct {
    const char* json;
    const char* frames;
    const char* wwav;
  } objects[] = {
      {"{\"frames\": [1, \"a\", null, true, 1.5, {\"k\": \"it's\"}, [], {}], \"wwav\": \"0.1\"}",
       "[1, 'a', None, True, 1.5, {'k': \"it's\"}, [], {}]", "0.1"},
      {"{\"frames\": {\"x\\\"y\": \"\\u00a0\\u0007\\ud800\\t\", \"b\": [-0, 1e400, NaN, -Infinity]}}",
       "{'x\"y': '\\xa0\\x07\\ud800\\t', 'b': [0, inf, nan, -inf]}", ""},
      {"{\"frames\": 1, \"frames\": 2.50, \"wwav\": 7, \"wwav\": \"\\u0663x\"}", "2.5", "\xd9\xa3x"},
      {"{\"frames\": \"44100\", \"wwav\": false}", "44100", "False"},
      {"{\"frames\": {\"a\": {\"b\": [[1, 2], \"q\\\"'\"]}}, \"wwav\": null}", "{'a': {'b': [[1, 2], 'q\"\\'']}}",
       "None"},
  };
  for (const auto& o : objects) {
    std::vector<py::Value> v;
    CHECK(py::readObject(o.json, {"frames", "wwav"}, &v));
    if (v.size() != 2 || v[0].str != o.frames || v[1].str != o.wwav) {
      printf("  %s gave frames %s and wwav %s\n", o.json, v.size() == 2 ? v[0].str.c_str() : "?",
             v.size() == 2 ? v[1].str.c_str() : "?");
      failures++;
    }
  }
  std::vector<py::Value> v;
  CHECK(py::readObject("{}", {"wwav"}, &v) && v[0].kind == py::Value::Missing && v[0].str.empty());
  CHECK(!py::readObject("{\"a\": 1} x", {"a"}, &v));
  CHECK(!py::readObject("[1]", {}, &v));
  CHECK(!py::readObject("", {}, &v));
  // Python's ==, for the frame count.
  CHECK(py::readObject("{\"a\": 44100, \"b\": 44100.0, \"c\": true, \"d\": \"44100\", \"e\": -0, \"f\": 4.41e4}",
                       {"a", "b", "c", "d", "e", "f"}, &v));
  CHECK(py::equals(v[0], 44100) && py::equals(v[1], 44100) && py::equals(v[2], 1) && !py::equals(v[2], 44100));
  CHECK(!py::equals(v[3], 44100) && py::equals(v[4], 0) && py::equals(v[5], 44100) && !py::equals(v[1], 44101));
}

int main() {
  struct {
    const char* name;
    void (*run)();
  } tests[] = {
      {"seqlock never tears", seqlockNeverTears},
      {"clock recovers from a writer killed mid-write", clockRecoversFromAWriterKilledMidWrite},
      {"meter ring never tears for a lapped reader", meterRingNeverTearsForALappedReader},
      {"spsc keeps order", spscKeepsOrder},
      {"crumbs are standard FNV-1a 64", crumbsAreStandardFnv1a64},
      {"gains are exact where they must be", gainsAreExactWhereTheyMustBe},
      {"graph folds tracks into buses and buses into the master", graphFoldsTracksIntoBusesAndBusesIntoTheMaster},
      {"graph stretches across a loop match the file", graphStretchesAcrossALoopMatchTheFile},
      {"wav writer writes canonical headers", wavWriterWritesCanonicalHeaders},
      {"probe finds masters and stems", probeFindsMastersAndStems},
      {"streamed clip never reads wrong audio", streamedClipNeverReadsWrongAudio},
      {"a loop finds its start ready", aLoopFindsItsStartReady},
      {"json is strict, and python's dialect is python's", jsonIsStrictAndPythonsDialectIsPythons},
      {"python reads and prints as python", pythonReadsAndPrintsAsPython},
  };
  for (auto& t : tests) {
    int before = failures;
    t.run();
    printf("%s %s\n", failures == before ? "ok  " : "FAIL", t.name);
  }
  for (const std::string& d : tempDirs) std::filesystem::remove_all(d);
  return failures ? 1 : 0;
}
