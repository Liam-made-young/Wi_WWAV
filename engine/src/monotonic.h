#pragma once
// The engine's one clock (docs/ENGINE.md 4.2: host_time_ns, ping's t, the
// header's start time). It is the clock the app's Rust core reads with
// std::time::Instant and the OS stamps audio with: CLOCK_MONOTONIC on Linux,
// CLOCK_UPTIME_RAW (mach_absolute_time) on macOS. Safe on the audio thread.
#include <stdint.h>
#include <time.h>

namespace wwav {

inline uint64_t monotonicNs() {
#if defined(__APPLE__)
  return clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
#else
  timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
#endif
}

}  // namespace wwav
