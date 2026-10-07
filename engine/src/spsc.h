#pragma once
// A fixed ring for one producer thread and one consumer thread, with no
// locks and no allocation after construction: how commands reach the audio
// thread (docs/SPEC.md 9.2).
#include <stddef.h>

#include <atomic>
#include <type_traits>

namespace wwav {

template <typename T, size_t N>
class Spsc {
  static_assert((N & (N - 1)) == 0, "the ring's size must be a power of two");
  static_assert(std::is_trivially_copyable<T>::value, "items are copied in and out");

 public:
  // Producer. False when the ring is full.
  bool push(const T& v) {
    size_t t = tail_.load(std::memory_order_relaxed);
    if (t - head_.load(std::memory_order_acquire) == N) return false;
    items_[t & (N - 1)] = v;
    tail_.store(t + 1, std::memory_order_release);
    return true;
  }

  // Consumer. False when the ring is empty.
  bool pop(T* out) {
    size_t h = head_.load(std::memory_order_relaxed);
    if (h == tail_.load(std::memory_order_acquire)) return false;
    *out = items_[h & (N - 1)];
    head_.store(h + 1, std::memory_order_release);
    return true;
  }

 private:
  alignas(64) std::atomic<size_t> head_{0};
  alignas(64) std::atomic<size_t> tail_{0};
  T items_[N];
};

}  // namespace wwav
