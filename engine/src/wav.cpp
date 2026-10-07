#include "wav.h"

#include <errno.h>
#include <math.h>
#include <string.h>

// Float samples are copied as they are in memory, which is the file's byte
// order only on a little-endian host (arm64 and x86_64 both are).
static_assert(__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__, "WAVE is little-endian");

namespace wwav {

namespace {

void put16(uint8_t* p, uint32_t v) {
  p[0] = v & 255;
  p[1] = (v >> 8) & 255;
}

void put32(uint8_t* p, uint32_t v) {
  put16(p, v & 0xffff);
  put16(p + 2, v >> 16);
}

}  // namespace

int16_t toS16(float x) {
  float v = x * 32768.0f;
  if (v >= 32767.0f) return 32767;
  if (v <= -32768.0f) return -32768;
  return (int16_t)lrintf(v);
}

WavWriter::~WavWriter() {
  if (f_) fclose(f_);
}

bool WavWriter::open(const std::string& path, WavFormat format, uint32_t rate, uint64_t frames, std::string* message) {
  format_ = format;
  frames_ = frames;
  const uint32_t bytesPerFrame = format == WavFormat::F32 ? 8 : 4;
  // f32: RIFF, fmt (18: the non-PCM form with cbSize), fact, data. s16: the canonical 44 bytes.
  const uint32_t head = format == WavFormat::F32 ? 58 : 44;
  if (frames * bytesPerFrame > 0xFFFFFFFFull - head) {
    *message = "A WAVE file holds 4 GB; this render needs " + std::to_string(frames * bytesPerFrame) + " bytes.";
    return false;
  }
  const uint32_t data = (uint32_t)(frames * bytesPerFrame);
  f_ = fopen(path.c_str(), "wb");
  if (!f_) {
    *message = "Can't write " + path + " (" + strerror(errno) + ").";
    return false;
  }
  uint8_t h[58];
  memcpy(h, "RIFF", 4);
  put32(h + 4, head - 8 + data);
  memcpy(h + 8, "WAVEfmt ", 8);
  if (format == WavFormat::F32) {
    put32(h + 16, 18);
    put16(h + 20, 3);  // WAVE_FORMAT_IEEE_FLOAT
  } else {
    put32(h + 16, 16);
    put16(h + 20, 1);  // PCM
  }
  put16(h + 22, 2);
  put32(h + 24, rate);
  put32(h + 28, rate * bytesPerFrame);
  put16(h + 32, bytesPerFrame);
  put16(h + 34, bytesPerFrame * 4);  // bits: 32 or 16
  uint8_t* d = h + 36;
  if (format == WavFormat::F32) {
    put16(h + 36, 0);  // cbSize
    memcpy(h + 38, "fact", 4);
    put32(h + 42, 4);
    put32(h + 46, (uint32_t)frames);
    d = h + 50;
  }
  memcpy(d, "data", 4);
  put32(d + 4, data);
  buf_.resize((size_t)4096 * bytesPerFrame);
  failed_ = fwrite(h, 1, head, f_) != head;
  if (failed_) {
    *message = "Can't write " + path + " (" + strerror(errno) + ").";
    return false;
  }
  return true;
}

bool WavWriter::write(const float* l, const float* r, int n) {
  while (n > 0 && !failed_) {
    int m = n < 4096 ? n : 4096;
    uint8_t* p = buf_.data();
    for (int i = 0; i < m; i++) {
      if (format_ == WavFormat::F32) {
        memcpy(p, &l[i], 4);
        memcpy(p + 4, &r[i], 4);
        p += 8;
      } else {
        put16(p, (uint16_t)toS16(l[i]));
        put16(p + 2, (uint16_t)toS16(r[i]));
        p += 4;
      }
    }
    size_t bytes = (size_t)(p - buf_.data());
    failed_ = fwrite(buf_.data(), 1, bytes, f_) != bytes;
    written_ += (uint64_t)m;
    l += m;
    r += m;
    n -= m;
  }
  return !failed_;
}

bool WavWriter::close(std::string* message) {
  if (!f_) return false;
  bool closed = fclose(f_) == 0;
  f_ = nullptr;
  if (failed_ || !closed) {
    *message = "Writing the render failed (" + std::string(strerror(errno)) + ").";
    return false;
  }
  if (written_ != frames_) {
    *message = "The render wrote " + std::to_string(written_) + " of " + std::to_string(frames_) + " frames.";
    return false;
  }
  return true;
}

}  // namespace wwav
