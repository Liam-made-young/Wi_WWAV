#include "shm.h"

#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

// The fields live in memory another process reads, so every access goes
// through the compiler's atomic builtins: a plain store could be torn or
// reordered, and a data race is undefined behaviour besides.
namespace {

template <typename T>
void put(T* p, T v) {
  __atomic_store(p, &v, __ATOMIC_RELAXED);
}

template <typename T>
T get(const T* p) {
  T v;
  __atomic_load(p, &v, __ATOMIC_RELAXED);
  return v;
}

}  // namespace

namespace wwav {

void writeClock(wwav_shm_clock* c, const ClockState& s) {
  uint64_t seq = get(&c->seq);
  // An engine killed between the two halves leaves seq odd; start from the
  // next even number so readers never take a write in progress as done.
  if (seq & 1) seq++;
  put(&c->seq, seq + 1);
  __atomic_thread_fence(__ATOMIC_RELEASE);
  put(&c->sample_pos, s.samplePos);
  put(&c->host_time_ns, s.hostTimeNs);
  put(&c->rate, s.rate);
  put(&c->state, s.state);
  put(&c->dropouts, s.dropouts);
  put(&c->callbacks, s.callbacks);
  __atomic_store_n(&c->seq, seq + 2, __ATOMIC_RELEASE);
}

ClockState readClock(const wwav_shm_clock* c) {
  for (;;) {
    uint64_t before = __atomic_load_n(&c->seq, __ATOMIC_ACQUIRE);
    if (before & 1) continue;
    ClockState s;
    s.samplePos = get(&c->sample_pos);
    s.hostTimeNs = get(&c->host_time_ns);
    s.rate = get(&c->rate);
    s.state = get(&c->state);
    s.dropouts = get(&c->dropouts);
    s.callbacks = get(&c->callbacks);
    __atomic_thread_fence(__ATOMIC_ACQUIRE);
    if (get(&c->seq) == before) return s;
  }
}

SharedRegion::~SharedRegion() {
  if (shm_) munmap(shm_, size_);
}

std::string SharedRegion::open(const std::string& name) {
  int fd = shm_open(name.c_str(), O_RDWR, 0);
  if (fd < 0) {
    return "Can't open the shared memory " + name + " (" + strerror(errno) +
           "). The app creates it before it starts the engine.";
  }
  struct stat st;
  if (fstat(fd, &st) != 0 || (size_t)st.st_size < WWAV_SHM_TOTAL_BYTES) {
    close(fd);
    return "The shared memory " + name + " holds " + std::to_string((long long)st.st_size) + " bytes; layout " +
           std::to_string(WWAV_SHM_LAYOUT) + " needs " + std::to_string(WWAV_SHM_TOTAL_BYTES) + ".";
  }
  void* p = mmap(nullptr, WWAV_SHM_TOTAL_BYTES, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
  close(fd);
  if (p == MAP_FAILED) return "Can't map the shared memory " + name + " (" + strerror(errno) + ").";
  shm_ = static_cast<wwav_shm*>(p);
  size_ = WWAV_SHM_TOTAL_BYTES;
  // An engine that restarts after a crash carries on the ring the last one
  // left, so a reader never sees the count go backwards.
  meterWrite_ = get(&shm_->meters.meter_write);
  return "";
}

void SharedRegion::writeHeader(uint32_t rate, uint32_t block, uint64_t pid, uint64_t startNs) {
  wwav_shm_header* h = &shm_->header;
  memcpy(h->magic, WWAV_SHM_MAGIC, 4);
  put(&h->layout, WWAV_SHM_LAYOUT);
  put(&h->sample_rate, rate);
  put(&h->block_size, block);
  put(&h->meter_slots, WWAV_METER_SLOTS);
  put(&h->meter_ring, WWAV_METER_RING);
  put(&h->peaks_bytes, WWAV_SHM_PEAKS_BYTES);
  put(&h->input_bytes, WWAV_SHM_INPUT_BYTES);
  put(&h->engine_pid, pid);
  put(&h->engine_start_ns, startNs);
  __atomic_thread_fence(__ATOMIC_RELEASE);
}

void SharedRegion::setFormat(uint32_t rate, uint32_t block) {
  put(&shm_->header.sample_rate, rate);
  put(&shm_->header.block_size, block);
}

void SharedRegion::crumb(uint64_t node) {
  put(&shm_->crumb.crumb, node);
  __atomic_fetch_add(&shm_->crumb.crumb_seq, 1, __ATOMIC_RELEASE);
}

void SharedRegion::publishMeters() {
  meterWrite_++;
  __atomic_store_n(&shm_->meters.meter_write, meterWrite_, __ATOMIC_RELEASE);
}

}  // namespace wwav
