#pragma once
// WAVE files as a render writes them (docs/ENGINE.md 3.6): stereo at the
// session rate, 32-bit float (WAVE_FORMAT_IEEE_FLOAT, with its fact chunk)
// or 16-bit PCM. The length is known before the first frame, so the header
// is final from the start and the bytes depend on nothing but the audio.
#include <stdint.h>
#include <stdio.h>

#include <string>
#include <vector>

namespace wwav {

enum class WavFormat { F32, S16 };

class WavWriter {
 public:
  WavWriter() = default;
  WavWriter(const WavWriter&) = delete;
  WavWriter& operator=(const WavWriter&) = delete;
  ~WavWriter();

  bool open(const std::string& path, WavFormat format, uint32_t rate, uint64_t frames, std::string* message);
  bool write(const float* l, const float* r, int n);
  // Checks every frame was written and the file closed cleanly.
  bool close(std::string* message);

 private:
  FILE* f_ = nullptr;
  WavFormat format_ = WavFormat::F32;
  uint64_t frames_ = 0, written_ = 0;
  bool failed_ = false;
  std::vector<uint8_t> buf_;
};

// A float sample as 16-bit PCM: scaled by 32768, rounded to nearest (ties to
// even), clamped. No dither: export dithers once, at its last step
// (docs/SPEC.md 6.7), and a render is not an export.
int16_t toS16(float x);

}  // namespace wwav
