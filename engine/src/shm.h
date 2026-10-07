#pragma once
// The shared-memory region (docs/ENGINE.md 4, engine/include/wwav_shm.h).
// The app creates and zeroes it; the engine maps it, writes the header, then
// writes the clock, crumb and meters from the audio thread without locks.
#include <stddef.h>
#include <stdint.h>

#include <string>

#include "wwav_shm.h"

namespace wwav {

// What one callback publishes (4.2).
struct ClockState {
  int64_t samplePos = 0;
  uint64_t hostTimeNs = 0;
  double rate = 0.0;
  uint32_t state = WWAV_STATE_STOPPED;
  uint32_t dropouts = 0;
  uint64_t callbacks = 0;
};

// The seqlock. writeClock is the engine's side: seq goes odd, the fields are
// written, seq goes even, with release ordering. readClock is any reader's:
// it retries until it reads the same even seq on both sides of the fields.
void writeClock(wwav_shm_clock* c, const ClockState& s);
ClockState readClock(const wwav_shm_clock* c);

// Where the audio thread leaves its crumb (docs/ENGINE.md 4.3): the node it
// is about to run, and 0 once it has left it.
class CrumbSink {
 public:
  virtual void crumb(uint64_t node) = 0;

 protected:
  ~CrumbSink() = default;
};

class SharedRegion final : public CrumbSink {
 public:
  SharedRegion() = default;
  SharedRegion(const SharedRegion&) = delete;
  SharedRegion& operator=(const SharedRegion&) = delete;
  ~SharedRegion();

  // Maps the region the app created under `name` (shm_open). Returns what
  // went wrong as a sentence, or "" when it is mapped.
  std::string open(const std::string& name);
  void writeHeader(uint32_t rate, uint32_t block, uint64_t pid, uint64_t startNs);
  // After device.open: the rate and block the clock and meters now run at.
  void setFormat(uint32_t rate, uint32_t block);

  // The audio thread, or the render thread while the audio thread is parked.
  void clock(const ClockState& s) { writeClock(&shm_->clock, s); }
  void crumb(uint64_t node) override;
  // The meter entry this block fills, then publishes as the newest.
  wwav_shm_meter_entry* meterEntry() { return &shm_->ring[meterWrite_ % WWAV_METER_RING]; }
  void publishMeters();

 private:
  wwav_shm* shm_ = nullptr;
  size_t size_ = 0;
  uint64_t meterWrite_ = 0;  // the audio thread's copy of meters.meter_write
};

}  // namespace wwav
