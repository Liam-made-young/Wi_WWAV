#include "media.h"

#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include <algorithm>
#include <chrono>

#include "base/json.h"
#include "disc/wav.h"
#include "disc/wwav.h"

namespace wwav {

namespace {

constexpr const char* kStems[4] = {"vocals", "drums", "other", "bass"};
constexpr int64_t kRing = 1 << 17;  // frames a streamed clip keeps: about 3 s

std::string fileName(const std::string& path) {
  size_t slash = path.find_last_of('/');
  return slash == std::string::npos ? path : path.substr(slash + 1);
}

bool fail(std::string* code, std::string* message, const char* c, const std::string& m) {
  *code = c;
  *message = m;
  return false;
}

// pread until `n` bytes are in, the file ends or an error; returns the bytes read.
size_t readAt(int fd, uint64_t at, void* buf, size_t n) {
  size_t got = 0;
  while (got < n) {
    ssize_t r = pread(fd, (uint8_t*)buf + got, n - got, (off_t)(at + got));
    if (r < 0 && errno == EINTR) continue;
    if (r <= 0) break;
    got += (size_t)r;
  }
  return got;
}

struct Fd {
  int fd;
  ~Fd() {
    if (fd >= 0) close(fd);
  }
};

uint32_t le32(const uint8_t* p) { return p[0] | p[1] << 8 | p[2] << 16 | (uint32_t)p[3] << 24; }

// The chunks a .wwav reader looks at, found as wwav_pack.py finds them: the
// first of each id, and a chunk cut off by the end of the file keeps what is
// there.
struct Chunk {
  bool found = false;
  uint64_t at = 0, size = 0;
};
struct WwavChunks {
  Chunk data, wmet, wstm, wlin;
};

WwavChunks walk(int fd, uint64_t fileSize) {
  WwavChunks c;
  uint64_t at = 12;
  while (at + 8 <= fileSize) {
    uint8_t h[8];
    if (readAt(fd, at, h, 8) != 8) break;
    uint64_t n = le32(h + 4), payload = at + 8;
    Chunk chunk{true, payload, payload + n > fileSize ? fileSize - payload : n};
    Chunk* slot = memcmp(h, "data", 4) == 0   ? &c.data
                  : memcmp(h, "wmet", 4) == 0 ? &c.wmet
                  : memcmp(h, "wstm", 4) == 0 ? &c.wstm
                  : memcmp(h, "wlin", 4) == 0 ? &c.wlin
                                              : nullptr;
    if (slot && !slot->found) *slot = chunk;
    if (payload + n > fileSize) break;
    at = payload + n + (n & 1);
  }
  return c;
}

std::string readText(int fd, const Chunk& c) {
  std::string s(c.size, '\0');
  if (c.size > (1u << 20) || readAt(fd, c.at, &s[0], c.size) != c.size) return "";
  return s;
}

bool isObject(const std::string& s) {
  return !s.empty() && prana::json::find(s.data(), s.size(), "").type == prana::json::Type::Object;
}

// The reference reader's verdict on a .wwav's stems (wwav_pack.py's
// Wwav.verdict), in its own words; "" when PRANA would play all four.
std::string stemVerdict(int fd, uint64_t fileSize, const prana::WavInfo& w, MediaPart* stems) {
  WwavChunks c = walk(fd, fileSize);
  if (w.format != 1 || w.channels != 2 || w.rate != 44100 || w.bits != 16 || !c.data.found)
    return "not listed: the master isn't 44.1 kHz 16-bit stereo PCM";
  std::string wmet = c.wmet.found ? readText(fd, c.wmet) : "", wlin = c.wlin.found ? readText(fd, c.wlin) : "";
  if (!isObject(wmet) || !isObject(wlin)) return "the master only: no wmet and wlin, so a plain WAV";
  char version[32] = {0};
  if (!prana::json::string(wmet.data(), wmet.size(), "wwav", version, sizeof version)) {
    prana::json::Value v = prana::json::find(wmet.data(), wmet.size(), "wwav");
    if (v.type == prana::json::Type::Number) memcpy(version, v.p, std::min(v.n, sizeof version - 1));
  }
  if (version[0] < '0' || version[0] > '9') return "the master only: wmet has no version";
  if (atoi(version) > prana::kWwavMajor)
    return std::string("the master only: version ") + version + " is newer than this reader (0.x)";
  uint8_t h[prana::kWstmHeader];
  if (!c.wstm.found || c.wstm.size < prana::kWstmHeader || readAt(fd, c.wstm.at, h, sizeof h) != sizeof h)
    return "the master only: no wstm";
  prana::WwavStems s;
  if (!prana::parseWstm(h, &s)) return "the master only: wstm isn't v1, 4 stereo 16-bit stems at 44.1 kHz";
  if (c.wstm.size < prana::kWstmHeader + s.pad + (uint64_t)s.frames * prana::kWwavFrameBytes)
    return "the master only: wstm is cut off";
  int64_t metFrames = -1;
  bool hasFrames = prana::json::whole(wmet.data(), wmet.size(), "frames", &metFrames);
  uint64_t dataFrames = c.data.size / 4;
  if (!(s.frames == dataFrames && hasFrames && (uint64_t)metFrames == dataFrames))
    return "the master only: frame counts differ (data " + std::to_string(dataFrames) + ", wstm " +
           std::to_string(s.frames) + ", wmet " + (hasFrames ? std::to_string(metFrames) : "None") + ")";
  stems->rate = s.rate;
  stems->channels = 2;
  stems->bits = 16;
  stems->dataAt = c.wstm.at + prana::kWstmHeader + s.pad;
  stems->stride = prana::kWwavFrameBytes;
  stems->frames = s.frames;
  return "";
}

inline float sample(const uint8_t* s, const MediaPart& p) {
  switch (p.bits) {
    case 16:
      return (float)(int16_t)(s[0] | s[1] << 8) * (1.0f / 32768.0f);
    case 24: {
      int32_t v = s[0] | s[1] << 8 | s[2] << 16;
      return (float)((v ^ 0x800000) - 0x800000) * (1.0f / 8388608.0f);
    }
    default: {
      if (p.isFloat) {
        float f;
        memcpy(&f, s, 4);  // little-endian hosts, as the file
        return f;
      }
      return (float)(int32_t)le32(s) * (1.0f / 2147483648.0f);
    }
  }
}

}  // namespace

bool probeMedia(const std::string& path, const std::string& source, MediaPart* out, std::string* code,
                std::string* message) {
  const std::string name = fileName(path);
  int stem = -1;
  for (int i = 0; i < 4; i++)
    if (source == kStems[i]) stem = i;
  if (stem < 0 && source != "master")
    return fail(code, message, "bad_clip",
                "A clip's source is master, vocals, drums, other or bass, not \"" + source + "\".");
  Fd f{::open(path.c_str(), O_RDONLY | O_CLOEXEC)};
  if (f.fd < 0) {
    if (errno == ENOENT) return fail(code, message, "no_such_file", "No file at " + path + ".");
    return fail(code, message, "bad_clip", "Can't read " + path + " (" + strerror(errno) + ").");
  }
  struct stat st;
  if (fstat(f.fd, &st) != 0 || !S_ISREG(st.st_mode)) return fail(code, message, "bad_clip", path + " isn't a file.");
  const uint64_t fileSize = (uint64_t)st.st_size;
  if (fileSize > 0xFFFFFFFFull)
    return fail(code, message, "bad_clip", name + " is over 4 GB, more than a WAVE file can hold.");

  // fmt and data as PRANA's own reader finds them (core/disc/wav.cpp).
  std::vector<uint8_t> head(std::min<uint64_t>(fileSize, 65536));
  if (readAt(f.fd, 0, head.data(), head.size()) != head.size())
    return fail(code, message, "bad_clip", "Can't read " + path + ".");
  if (head.size() >= 12 && memcmp(head.data(), "FORM", 4) == 0)
    return fail(code, message, "unsupported", name + " is AIFF; the engine reads WAV and .wwav for now.");
  if (head.size() >= 4 && memcmp(head.data(), "caff", 4) == 0)
    return fail(code, message, "unsupported", name + " is CAF; the engine reads WAV and .wwav for now.");
  prana::WavInfo w;
  uint32_t need = 0;
  prana::WavParse parsed;
  while ((parsed = prana::parseWav(head.data(), (uint32_t)head.size(), (uint32_t)fileSize, &w, &need)) ==
             prana::WavParse::NeedMore &&
         need > head.size() && need <= fileSize && need <= (16u << 20)) {
    head.resize(need);
    if (readAt(f.fd, 0, head.data(), need) != need) break;
  }
  if (parsed != prana::WavParse::Ok) return fail(code, message, "bad_clip", name + " is not a WAV file.");

  if (stem >= 0) {
    std::string verdict = stemVerdict(f.fd, fileSize, w, out);
    if (!verdict.empty())
      return fail(code, message, "bad_clip", name + " has no " + source + " stem: " + verdict + ".");
    out->offset = (uint32_t)stem * 4;
    out->isFloat = false;
    out->fileSize = fileSize;
    return true;
  }
  if (w.format != 1 && w.format != 3)
    return fail(code, message, "unsupported",
                name + " is WAVE format " + std::to_string(w.format) +
                    ", which the engine doesn't read; the app decodes compressed audio to a WAV first.");
  bool pcm = w.format == 1 && (w.bits == 16 || w.bits == 24 || w.bits == 32);
  if (!pcm && !(w.format == 3 && w.bits == 32))
    return fail(code, message, "unsupported",
                name + " is " + std::to_string(w.bits) + "-bit " + (w.format == 1 ? "PCM" : "float") +
                    "; clips are 16-, 24- or 32-bit PCM or 32-bit float for now.");
  if (w.channels != 1 && w.channels != 2)
    return fail(code, message, "unsupported",
                name + " has " + std::to_string(w.channels) + " channels; clips are mono or stereo for now.");
  if (w.rate == 0) return fail(code, message, "bad_clip", name + " says its rate is 0 Hz.");
  out->rate = w.rate;
  out->channels = w.channels;
  out->bits = w.bits;
  out->isFloat = w.format == 3;
  out->dataAt = w.dataOffset;
  out->stride = (uint32_t)w.channels * w.bits / 8;
  out->offset = 0;
  out->frames = w.frames;
  out->fileSize = fileSize;
  return true;
}

void decodeFrames(const uint8_t* raw, int n, const MediaPart& part, float* l, float* r) {
  const uint8_t* f = raw + part.offset;
  const int bytes = part.bits / 8;
  for (int i = 0; i < n; i++, f += part.stride) {
    l[i] = sample(f, part);
    r[i] = part.channels == 2 ? sample(f + bytes, part) : l[i];
  }
}

// ---- ClipSource -----------------------------------------------------------------

ClipSource::~ClipSource() {
  if (fd_ >= 0) close(fd_);
}

std::unique_ptr<ClipSource> ClipSource::open(const std::string& path, const MediaPart& part, int64_t in, int64_t len,
                                             bool whole, std::string* message) {
  std::unique_ptr<ClipSource> s(new ClipSource());
  s->part_ = part;
  s->in_ = in;
  s->len_ = len;
  s->whole_ = whole;
  s->fd_ = ::open(path.c_str(), O_RDONLY | O_CLOEXEC);
  if (s->fd_ < 0) {
    *message = "Can't open " + path + " (" + strerror(errno) + ").";
    return nullptr;
  }
  s->raw_.resize((size_t)kRun * part.stride);
  s->runL_.resize(kRun);
  s->runR_.resize(kRun);
  if (whole) {
    s->l_.assign((size_t)len, 0.0f);
    s->r_.assign((size_t)len, 0.0f);
    for (int64_t c = 0; c < len; c += kRun) {
      int n = (int)std::min<int64_t>(kRun, len - c);
      s->readFile(c, n, &s->l_[(size_t)c], &s->r_[(size_t)c], s->raw_);
    }
    close(s->fd_);
    s->fd_ = -1;
  } else {
    s->l_.assign(kRing, 0.0f);
    s->r_.assign(kRing, 0.0f);
    s->rawNow_.resize((size_t)kRun * part.stride);
    s->ahead_ = std::min<int64_t>(2 * (int64_t)part.rate, kRing - 2 * kMaxBlock);
  }
  return s;
}

void ClipSource::readFile(int64_t c, int n, float* l, float* r, std::vector<uint8_t>& raw) {
  // Frames past the file's end (a clip longer than its file) are silence.
  const int64_t frame = in_ + c;
  const int64_t have = frame < (int64_t)part_.frames ? std::min<int64_t>(n, (int64_t)part_.frames - frame) : 0;
  int got = 0;
  while (got < have) {
    int m = (int)std::min<int64_t>(kRun, have - got);
    size_t want = (size_t)m * part_.stride;
    size_t read = readAt(fd_, part_.dataAt + (uint64_t)(frame + got) * part_.stride, raw.data(), want);
    if (read < want) memset(raw.data() + read, 0, want - read);  // cut off: what's there, then silence
    decodeFrames(raw.data(), m, part_, l + got, r + got);
    got += m;
  }
  for (int i = got; i < n; i++) l[i] = r[i] = 0.0f;
}

bool ClipSource::read(int64_t c, int n, float* l, float* r) {
  if (whole_) {
    memcpy(l, &l_[(size_t)c], (size_t)n * sizeof(float));
    memcpy(r, &r_[(size_t)c], (size_t)n * sizeof(float));
    return true;
  }
  // A seqlock over the window: copy, then check the reader hasn't moved the
  // window or overwritten what was copied. The copy itself races with the
  // reader's writes by design; the check after it throws such a copy away.
  uint32_t gen = gen_.load(std::memory_order_acquire);
  int64_t start = start_.load(std::memory_order_acquire), end = end_.load(std::memory_order_acquire);
  if (!(gen & 1) && c >= start && c + n <= end) {
    size_t at = (size_t)(c % kRing), first = std::min<size_t>((size_t)n, kRing - at);
    memcpy(l, &l_[at], first * sizeof(float));
    memcpy(r, &r_[at], first * sizeof(float));
    memcpy(l + first, &l_[0], ((size_t)n - first) * sizeof(float));
    memcpy(r + first, &r_[0], ((size_t)n - first) * sizeof(float));
    std::atomic_thread_fence(std::memory_order_acquire);
    if (start_.load(std::memory_order_relaxed) <= c && gen_.load(std::memory_order_relaxed) == gen) return true;
  }
  memset(l, 0, (size_t)n * sizeof(float));
  memset(r, 0, (size_t)n * sizeof(float));
  return false;
}

void ClipSource::readNow(int64_t c, int n, float* l, float* r) {
  if (whole_)
    read(c, n, l, r);
  else
    readFile(c, n, l, r, rawNow_);
}

bool ClipSource::fill() {
  if (whole_) return false;
  const int64_t w = std::min(std::max<int64_t>(want_.load(std::memory_order_relaxed), 0), len_);
  int64_t start = start_.load(std::memory_order_relaxed), end = end_.load(std::memory_order_relaxed);
  if (w < start || w > end) {
    // The playhead jumped out of the window: start a new one where it is.
    uint32_t gen = gen_.load(std::memory_order_relaxed);
    gen_.store(gen + 1, std::memory_order_relaxed);
    std::atomic_thread_fence(std::memory_order_release);
    start_.store(w, std::memory_order_relaxed);
    end_.store(w, std::memory_order_relaxed);
    gen_.store(gen + 2, std::memory_order_release);
    start = end = w;
  }
  const int64_t limit = std::min(len_, w + ahead_);
  if (end >= limit) return false;
  const int n = (int)std::min<int64_t>(kRun, limit - end);
  readFile(end, n, runL_.data(), runR_.data(), raw_);
  // The run overwrites the oldest frames: say so before writing over them.
  const int64_t oldest = std::max(start, end + n - kRing);
  if (oldest != start) start_.store(oldest, std::memory_order_relaxed);
  std::atomic_thread_fence(std::memory_order_release);
  size_t at = (size_t)(end % kRing), first = std::min<size_t>((size_t)n, kRing - at);
  memcpy(&l_[at], runL_.data(), first * sizeof(float));
  memcpy(&r_[at], runR_.data(), first * sizeof(float));
  memcpy(&l_[0], runL_.data() + first, ((size_t)n - first) * sizeof(float));
  memcpy(&r_[0], runR_.data() + first, ((size_t)n - first) * sizeof(float));
  end_.store(end + n, std::memory_order_release);
  return true;
}

void ClipSource::prefill(int64_t c) {
  want_.store(c, std::memory_order_relaxed);
  while (fill()) {
  }
}

bool ClipSource::ready(int64_t c, int64_t frames) const {
  if (whole_ || c >= len_) return true;
  uint32_t gen = gen_.load(std::memory_order_acquire);
  return !(gen & 1) && start_.load(std::memory_order_acquire) <= c &&
         end_.load(std::memory_order_acquire) >= std::min(len_, c + frames);
}

// ---- Reader -----------------------------------------------------------------------

Reader::Reader() : thread_([this] { run(); }) {}

Reader::~Reader() {
  quit_ = true;
  thread_.join();
}

void Reader::add(ClipSource* s) {
  std::lock_guard<std::mutex> lock(mutex_);
  sources_.push_back(s);
}

void Reader::remove(ClipSource* s) {
  std::lock_guard<std::mutex> lock(mutex_);
  sources_.erase(std::remove(sources_.begin(), sources_.end(), s), sources_.end());
}

void Reader::run() {
  while (!quit_.load(std::memory_order_relaxed)) {
    // One run per clip per pass, so every clip moves ahead together. The
    // lock is held for one run at a time, so remove() waits for one read at
    // most; a clip removed mid-pass can make the pass skip one other.
    bool busy = false;
    for (size_t i = 0;; i++) {
      std::lock_guard<std::mutex> lock(mutex_);
      if (i >= sources_.size()) break;
      busy = sources_[i]->fill() || busy;
    }
    if (!busy) std::this_thread::sleep_for(std::chrono::milliseconds(5));
  }
}

}  // namespace wwav
