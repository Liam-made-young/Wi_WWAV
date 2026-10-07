#include "audio.h"

#include <errno.h>
#include <stdlib.h>
#include <time.h>

#include <algorithm>
#include <chrono>
#include <vector>

#include "monotonic.h"

namespace wwav {

namespace {

void sleepUntil(uint64_t ns) {
#if defined(__APPLE__)
  uint64_t now = monotonicNs();
  if (ns <= now) return;
  uint64_t d = ns - now;
  timespec ts{(time_t)(d / 1000000000ull), (long)(d % 1000000000ull)};
  nanosleep(&ts, nullptr);
#else
  timespec ts{(time_t)(ns / 1000000000ull), (long)(ns % 1000000000ull)};
  while (clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &ts, nullptr) == EINTR) {
  }
#endif
}

}  // namespace

AudioEngine::AudioEngine(SharedRegion& shm, juce::AudioDeviceManager& devices) : shm_(shm), devices_(devices) {}

AudioEngine::~AudioEngine() { close(); }

std::string AudioEngine::findDevice(const std::string& name, juce::String* type) {
  std::string found;
  juce::MessageManager::callSync([&] {
    for (juce::AudioIODeviceType* t : devices_.getAvailableDeviceTypes()) {
      t->scanForDevices();
      juce::StringArray names = t->getDeviceNames(false);
      int i = name.empty() ? t->getDefaultDeviceIndex(false) : names.indexOf(juce::String(name));
      if (i >= 0 && i < names.size()) {
        *type = t->getTypeName();
        found = names[i].toStdString();
        return;
      }
    }
  });
  return found;
}

bool AudioEngine::open(const std::string& name, int rate, int block, std::string* code, std::string* message) {
  juce::String type;
  std::string real;
  if (name != "null") {
    real = findDevice(name, &type);
    if (real.empty()) {
      *code = "no_such_device";
      *message = name.empty() ? "There is no audio output device." : "No audio device named \"" + name + "\".";
      return false;
    }
  }
  close();
  if (name == "null") {
    // No hardware: a timer thread calls the graph every block at the nominal
    // rate, so the clock and meters behave as with a device (docs/ENGINE.md 1).
    device_ = DeviceInfo{"null", rate, block, 0, 0};
    rate_ = rate;
    outputLatency_ = 0;
    nullStop_ = false;
    running_ = true;
    null_ = std::thread([this, rate, block] { nullDevice(rate, block); });
    shm_.setFormat((uint32_t)rate, (uint32_t)block);
    return true;
  }
  std::string failed;
  juce::MessageManager::callSync([&] {
    juce::AudioDeviceManager::AudioDeviceSetup setup;
    setup.outputDeviceName = real;
    setup.inputDeviceName = {};
    setup.sampleRate = rate;
    setup.bufferSize = block;
    setup.useDefaultInputChannels = false;
    setup.useDefaultOutputChannels = true;
    devices_.setCurrentAudioDeviceType(type, true);
    juce::String e = juceStarted_ ? devices_.setAudioDeviceSetup(setup, true)
                                  : devices_.initialise(0, 2, nullptr, false, {}, &setup);
    juceStarted_ = true;
    juce::AudioIODevice* d = devices_.getCurrentAudioDevice();
    if (e.isEmpty() && d == nullptr) e = "it didn't open";
    if (e.isNotEmpty()) {
      failed = e.toStdString();
      devices_.closeAudioDevice();
      return;
    }
    device_ = DeviceInfo{d->getName().toStdString(), (int)d->getCurrentSampleRate(), d->getCurrentBufferSizeSamples(),
                         d->getOutputLatencyInSamples(), d->getInputLatencyInSamples()};
    rate_ = device_.rate;
    outputLatency_ = device_.outputLatency;
    // The worker is waiting on this call, so nothing applies commands
    // itself once the callbacks start.
    running_ = true;
    juceDevice_ = true;
    devices_.addAudioCallback(this);
    startTimer(250);
  });
  if (!failed.empty()) {
    *code = "device_failed";
    *message = "The audio device \"" + real + "\" didn't open: " + failed + ".";
    return false;
  }
  shm_.setFormat((uint32_t)device_.rate, (uint32_t)device_.block);
  return true;
}

void AudioEngine::close() {
  if (!running_) return;
  if (juceDevice_) {
    juce::MessageManager::callSync([&] {
      stopTimer();
      devices_.removeAudioCallback(this);
      devices_.closeAudioDevice();
    });
    juceDevice_ = false;
  } else {
    nullStop_ = true;
    null_.join();
  }
  running_ = false;
  device_ = DeviceInfo{};
  // What the audio thread didn't get to, the worker applies now, in order.
  drain(graph_.load());
}

void AudioEngine::timerCallback() { xruns_.store((uint32_t)std::max(0, devices_.getXRunCount())); }

void AudioEngine::audioDeviceAboutToStart(juce::AudioIODevice* device) {
  // Called before the device's callbacks start, also when it restarts with
  // new settings of its own.
  rate_ = (int)device->getCurrentSampleRate();
  outputLatency_ = device->getOutputLatencyInSamples();
}

void AudioEngine::audioDeviceIOCallbackWithContext(const float* const*, int, float* const* out, int numOut, int n,
                                                   const juce::AudioIODeviceCallbackContext&) {
  // The clock's host time is the moment the callback runs. On a Mac it
  // should come from CoreAudio's own timestamp (context.hostTimeNs), which
  // needs a Mac to check, so this cut doesn't use it yet.
  callback(out, numOut, n);
}

void AudioEngine::nullDevice(int rate, int block) {
  std::vector<float> l((size_t)block), r((size_t)block);
  float* out[2] = {l.data(), r.data()};
  const uint64_t perSecond = (uint64_t)rate, second = 1000000000;
  const uint64_t period = (uint64_t)block * second / perSecond;
  uint64_t start = monotonicNs(), k = 0;
  while (!nullStop_.load(std::memory_order_relaxed)) {
    callback(out, 2, block);
    k++;
    const uint64_t frames = k * (uint64_t)block;
    uint64_t next = start + frames / perSecond * second + frames % perSecond * second / perSecond;
    if (monotonicNs() > next + period) {
      // More than a block behind: a dropout. Count again from now rather
      // than racing to catch up.
      late_.fetch_add(1, std::memory_order_relaxed);
      start = next = monotonicNs();
      k = 0;
    }
    sleepUntil(next);
  }
}

// ---- the audio thread ----------------------------------------------------------

void AudioEngine::run(const Command& c, Graph* g) {
  switch (c.kind) {
    case Command::SetParam:
      if (g && g->gen == c.gen) g->set(c.node, c.param, c.value);
      break;
    case Command::Play:
      transport_.playing = true;
      break;
    case Command::Stop:
      transport_.playing = false;
      break;
    case Command::Locate:
      transport_.pos = c.a;
      break;
    case Command::Loop:
      loop_ = c.on;
      loopStart_ = c.a;
      loopEnd_ = c.b;
      break;
    case Command::Crash:  // debug.crash
      abort();
    case Command::Hang:  // debug.hang
      for (;;) std::this_thread::sleep_for(std::chrono::seconds(1));
    case Command::CrashIn:  // debug.crumb
      if (g && g->gen == c.gen) g->crashIn(c.node);
      break;
  }
}

void AudioEngine::drain(Graph* g) {
  Command c;
  uint64_t last = 0;
  while (commands_.pop(&c)) {
    run(c, g);
    last = c.seq;
  }
  if (last) {
    ackPlaying_.store(transport_.playing, std::memory_order_relaxed);
    ackPos_.store(transport_.pos, std::memory_order_relaxed);
    applied_.store(last, std::memory_order_release);
  }
}

void AudioEngine::callback(float* const* out, int channels, int n) {
  epoch_.fetch_add(1);  // odd: inside
  const uint64_t start = monotonicNs();
  juce::ScopedNoDenormals noDenormals;
  const bool parked = parked_.load();
  Graph* g = parked ? nullptr : graph_.load();
  // While a render holds the graph, commands wait (the worker sends none).
  if (!parked) drain(g);

  const bool playing = transport_.playing;
  ClockState clock;
  clock.samplePos = playing ? transport_.pos - outputLatency_ : transport_.pos;
  clock.hostTimeNs = start;
  clock.rate = playing ? (double)rate_ : 0.0;
  clock.state = playing ? WWAV_STATE_PLAYING : WWAV_STATE_STOPPED;
  clock.dropouts = late_.load(std::memory_order_relaxed) + xruns_.load(std::memory_order_relaxed);
  clock.callbacks = ++callbacks_;
  shm_.clock(clock);

  wwav_shm_meter_entry* meters = shm_.meterEntry();
  meters->slots_used = 0;
  for (int done = 0; done < n;) {
    // A device asking for more than kMaxBlock frames gets them in pieces; the meters show the last.
    const int m = std::min(n - done, kMaxBlock);
    if (g) g->beginBlock(m);
    for (int at = 0; at < m;) {
      // A block that crosses the loop's end plays to it, then on from its start.
      int k = m - at;
      if (playing && loop_ && transport_.pos < loopEnd_) k = (int)std::min<int64_t>(k, loopEnd_ - transport_.pos);
      if (g) g->process(transport_.pos, at, k, playing, true, &shm_);
      at += k;
      if (playing) {
        transport_.pos += k;
        if (loop_ && transport_.pos == loopEnd_) transport_.pos = loopStart_;
      }
    }
    for (int ch = 0; ch < channels; ch++) {
      float* o = out[ch] + done;
      if (!g || ch > 1) {
        std::fill(o, o + m, 0.0f);
      } else if (channels == 1) {
        for (int i = 0; i < m; i++) o[i] = 0.5f * (g->masterL()[i] + g->masterR()[i]);
      } else {
        const float* master = ch == 0 ? g->masterL() : g->masterR();
        std::copy(master, master + m, o);
      }
    }
    if (g) g->endBlock(m, done + m == n ? meters : nullptr);
    done += m;
  }
  meters->callback = callbacks_;
  meters->dsp_load = (float)((double)(monotonicNs() - start) * rate_ / ((double)n * 1e9));
  meters->dropouts = clock.dropouts;
  shm_.publishMeters();
  epoch_.fetch_add(1);  // even: outside
}

// ---- the worker thread ----------------------------------------------------------

void AudioEngine::post(Command c) {
  c.seq = ++sent_;
  if (!running_) {
    // No callbacks: the worker is the only one touching the transport.
    run(c, graph_.load());
    ackPlaying_.store(transport_.playing);
    ackPos_.store(transport_.pos);
    applied_.store(c.seq);
    return;
  }
  while (!commands_.push(c)) std::this_thread::sleep_for(std::chrono::milliseconds(1));
}

Transport AudioEngine::apply(Command c) {
  post(c);
  // A block is 0.1 s at most; a device that stops calling back gets a second.
  const auto until = std::chrono::steady_clock::now() + std::chrono::seconds(1);
  while (running_ && applied_.load(std::memory_order_acquire) < sent_ && std::chrono::steady_clock::now() < until)
    std::this_thread::sleep_for(std::chrono::microseconds(100));
  return transport();
}

Transport AudioEngine::transport() const {
  applied_.load(std::memory_order_acquire);  // pairs with drain()'s release: the ack below is that block's
  return Transport{ackPlaying_.load(std::memory_order_relaxed), ackPos_.load(std::memory_order_relaxed)};
}

void AudioEngine::waitForCallbackEnd() const {
  // A callback that began before now may still hold the old graph: wait for
  // it to end. Callbacks that begin later see what was just stored.
  const uint64_t e = epoch_.load();
  if (e & 1)
    while (epoch_.load() == e) std::this_thread::sleep_for(std::chrono::microseconds(50));
}

std::unique_ptr<Graph> AudioEngine::swap(std::unique_ptr<Graph> g) {
  graph_.exchange(g.get());
  waitForCallbackEnd();
  std::unique_ptr<Graph> old = std::move(owned_);
  owned_ = std::move(g);
  return old;
}

void AudioEngine::park() {
  parked_.store(true);
  waitForCallbackEnd();
  // The audio thread no longer reads the queue, so the worker does: a
  // param.set sent just before the render is in it.
  drain(graph_.load());
}

void AudioEngine::resume() { parked_.store(false); }

}  // namespace wwav
