#pragma once
// Clip audio: finding a clip's part of a WAV or .wwav file, and reading it
// on time (docs/SPEC.md 9.4). Files under 32 MB are held whole; bigger ones
// stream: the reader thread keeps about 2 s of each clip ahead of the
// playhead in a ring, in 1024-frame runs, as PRANA reads wstm.
#include <stdint.h>

#include <atomic>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

namespace wwav {

constexpr int kRun = 1024;                          // frames per read
constexpr int kMaxBlock = 4096;                     // the most frames one stretch of a block asks for
constexpr int64_t kHoldWhole = 32ll * 1024 * 1024;  // files under this are held whole

// Where one part of a file is: the master of a WAV or .wwav (its data
// chunk), or one stem of a .wwav (its two channels inside wstm's frames).
struct MediaPart {
  uint32_t rate = 0;
  uint16_t channels = 0;  // 1 or 2; mono plays on both sides
  uint16_t bits = 0;      // 16, 24 or 32
  bool isFloat = false;
  uint64_t dataAt = 0;  // file offset of frame 0
  uint32_t stride = 0;  // bytes from one frame to the next
  uint32_t offset = 0;  // bytes into a frame where this part's samples are
  uint64_t frames = 0;
  uint64_t fileSize = 0;
};

// Finds `source` ("master", or a stem: "vocals", "drums", "other", "bass")
// in the file at `path`. A stem needs a .wwav that PRANA would list with its
// four stems; otherwise the reason is the reference reader's verdict
// (formats/prana/tools/wwav_pack.py). On failure, `code` is no_such_file,
// bad_clip or unsupported and `message` a sentence.
bool probeMedia(const std::string& path, const std::string& source, MediaPart* out, std::string* code,
                std::string* message);

// `n` frames laid out as `part` (starting at its frame, not its offset) to
// float left and right.
void decodeFrames(const uint8_t* raw, int n, const MediaPart& part, float* l, float* r);

// Frames [in, in + len) of one part of a file. Frames past the file's end
// are silence. Positions below are relative to the clip.
class ClipSource {
 public:
  ~ClipSource();
  ClipSource(const ClipSource&) = delete;
  ClipSource& operator=(const ClipSource&) = delete;

  // Held whole (decoded now) or streamed. nullptr and a sentence on failure.
  static std::unique_ptr<ClipSource> open(const std::string& path, const MediaPart& part, int64_t in, int64_t len,
                                          bool whole, std::string* message);

  int64_t length() const { return len_; }
  bool streamed() const { return !whole_; }

  // Audio thread: frames [c, c + n) into l and r. A streamed clip whose
  // reader hasn't got there gives silence and false. Never blocks.
  bool read(int64_t c, int n, float* l, float* r);
  // Audio thread: where the playhead needs this clip next.
  void want(int64_t c) { want_.store(c, std::memory_order_relaxed); }

  // Render thread: the same frames, read from the file now if need be.
  void readNow(int64_t c, int n, float* l, float* r);

  // Reader thread: moves the window to the wanted frame if it is outside
  // it, then reads ahead. True when it read anything.
  bool fill();
  // Before the reader thread knows this source: fills the window at `c`.
  void prefill(int64_t c);
  // Any thread: whether `frames` frames from `c` are in the window.
  bool ready(int64_t c, int64_t frames) const;

 private:
  ClipSource() = default;
  // Decodes frames from the file, with `raw` as the bytes' buffer. Never on the audio thread.
  void readFile(int64_t c, int n, float* l, float* r, std::vector<uint8_t>& raw);

  MediaPart part_;
  int fd_ = -1;
  int64_t in_ = 0, len_ = 0;
  bool whole_ = true;
  std::vector<float> l_, r_;     // whole: the clip; streamed: the ring
  std::vector<uint8_t> raw_;     // file bytes for one run (the reader's)
  std::vector<uint8_t> rawNow_;  // and for readNow (the render thread's)
  std::vector<float> runL_, runR_;
  int64_t ahead_ = 0;  // frames kept ahead of the playhead
  // The ring holds clip frames [start_, end_). gen_ is odd while the reader
  // moves the window somewhere else.
  std::atomic<int64_t> want_{0};
  std::atomic<int64_t> start_{0};
  std::atomic<int64_t> end_{0};
  std::atomic<uint32_t> gen_{0};
};

// The disk thread: keeps every streamed clip ahead of the playhead.
class Reader {
 public:
  Reader();
  ~Reader();
  void add(ClipSource* s);
  // Returns once the reader no longer touches `s`.
  void remove(ClipSource* s);

 private:
  void run();
  std::mutex mutex_;
  std::vector<ClipSource*> sources_;
  std::atomic<bool> quit_{false};
  std::thread thread_;
};

}  // namespace wwav
