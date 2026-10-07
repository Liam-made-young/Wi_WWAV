// The device's wrmx text for a spread of mixes, for the Rust writer to
// match (crates/wwav-formats/tests/wwav_write.rs builds and runs this).
//
// Each line holds the values prana/core's writeWrmx (app/remix.cpp)
// formats, as it computes them from the Mix (the floats as hex bit
// patterns, so nothing is lost on the way), then a tab and the text it
// writes:
//
//   per stem: vol mute fx0 fx1 fx2 fx3, then: pitch speed time lpf hpf
//   mode start length <tab> {"tracks": ...}
//
// The first line is a new Mix (PRANA's defaults); the rest are drawn from
// a fixed LCG, so every build prints the same lines.
#include <cstdio>
#include <cstring>

#include "app/mix.h"
#include "app/remix.h"

using namespace prana;
using namespace prana::mixmath;

static uint32_t state = 12345;
static uint32_t next() {
  state = state * 1664525u + 1013904223u;
  return state >> 8;
}
static float unit() { return (float)(next() % 100001u) / 100000.0f; }
static uint32_t bits(float f) {
  uint32_t b;
  std::memcpy(&b, &f, sizeof b);
  return b;
}

int main() {
  for (int i = 0; i < 3000; i++) {
    Mix m;
    RemixMeta rm;
    if (i > 0) {
      for (int s = 0; s < 4; s++) {
        m.vol[s] = unit();
        for (int f = 0; f < dsp::kFxCount; f++) m.fx[s][f] = (next() & 1) ? unit() : 0.0f;
      }
      for (int k = 0; k < 4; k++) m.master[k] = unit();
      m.speed = (int16_t)((int)(next() % 143u) - 71);
      m.mute = (uint8_t)(next() & 15u);
      m.mode = (next() & 1u) ? dsp::kModeMaster : dsp::kModeStems;
      rm.start = next();
    }
    rm.length = next();
    char json[1400];
    size_t n = writeWrmx(m, rm, json, sizeof json);
    for (int s = 0; s < 4; s++) {
      std::printf("%08x %u", bits(m.vol[s]), (m.mute >> s) & 1u);
      for (int f = 0; f < dsp::kFxCount; f++) std::printf(" %08x", bits(m.fx[s][f]));
      std::printf(" ");
    }
    std::printf("%d %08x %08x %08x %08x %s %u %u\t%.*s\n", pitchSemitones(m.master[kPitch]), bits(speedRatio(m.speed)),
                bits(timeRatio(m.master[kTime])), bits(1.0f - m.master[kLpf]), bits(m.master[kHpf]),
                m.mode == dsp::kModeMaster ? "master" : "stems", rm.start, rm.length, (int)n, json);
  }
  return 0;
}
