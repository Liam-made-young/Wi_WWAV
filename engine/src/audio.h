#pragma once
// The audio thread and what feeds it (docs/SPEC.md 9.2, 9.4): the device
// (a real one through JUCE, or the null device's timer thread), the
// transport, and the graph, which the worker swaps in with one atomic
// pointer exchange. Commands reach the audio thread through a
// single-producer queue; the worker thread is the only producer.
//
// The audio thread never allocates or logs, and our code never locks on
// it. One lock it does take is JUCE's: AudioDeviceManager holds its
// callback lock around every callback of a real device. It is contended
// only while a callback is added or removed, which happens when a device
// opens or closes.
#include <juce_audio_devices/juce_audio_devices.h>

#include <atomic>
#include <memory>
#include <string>
#include <thread>

#include "graph.h"
#include "shm.h"
#include "spsc.h"

namespace wwav {

struct Command {
  enum Kind : uint8_t { SetParam, Play, Stop, Locate, Loop, Crash, Hang, CrashIn };
  Kind kind = Play;
  Param param = Param::GainDb;
  int32_t node = 0;
  float value = 0.0f;    // SetParam
  bool on = false;       // Loop
  int64_t a = 0, b = 0;  // Locate: a. Loop: a to b.
  uint64_t gen = 0;      // SetParam, CrashIn: the graph they were meant for
  uint64_t seq = 0;
};

struct Transport {
  bool playing = false;
  int64_t pos = 0;
};

struct DeviceInfo {
  std::string name;  // "" when none is open
  int rate = 0, block = 0;
  int outputLatency = 0, inputLatency = 0;
};

class AudioEngine final : private juce::AudioIODeviceCallback, private juce::Timer {
 public:
  AudioEngine(SharedRegion& shm, juce::AudioDeviceManager& devices);
  ~AudioEngine() override;

  // ---- the worker thread ----
  // Opens a device: "null" for the timer device, a name from device.list, or
  // "" for the system's default output. The old one is closed first.
  bool open(const std::string& name, int rate, int block, std::string* code, std::string* message);
  void close();
  bool running() const { return running_; }
  const DeviceInfo& device() const { return device_; }

  // Queues a command; with no device running, applies it now.
  void post(Command c);
  // The same, then waits until the audio thread has applied it; returns the
  // transport as that block left it.
  Transport apply(Command c);
  // The transport as the last command left it.
  Transport transport() const;
  // Where the playhead is now: after the last block, or the last command.
  int64_t playhead() const { return playhead_.load(std::memory_order_relaxed); }

  // One atomic pointer exchange; returns the old graph once the audio thread
  // has let go of it, to be freed off the audio thread.
  std::unique_ptr<Graph> swap(std::unique_ptr<Graph> g);
  Graph* graph() const { return graph_.load(); }

  // A render takes the graph: the audio thread plays silence and leaves the
  // graph (and the command queue) alone between park() and resume().
  void park();
  void resume();

 private:
  void audioDeviceIOCallbackWithContext(const float* const* in, int numIn, float* const* out, int numOut, int n,
                                        const juce::AudioIODeviceCallbackContext& context) override;
  void audioDeviceAboutToStart(juce::AudioIODevice* device) override;
  void audioDeviceStopped() override {}
  void timerCallback() override;

  void callback(float* const* out, int channels, int n);
  void run(const Command& c, Graph* g);
  void drain(Graph* g);
  void waitForCallbackEnd() const;
  void nullDevice(int rate, int block);
  std::string findDevice(const std::string& name, juce::String* type);

  SharedRegion& shm_;
  juce::AudioDeviceManager& devices_;
  DeviceInfo device_;
  bool running_ = false;  // the worker's view: a device is calling back
  bool juceDevice_ = false;
  std::thread null_;
  std::atomic<bool> nullStop_{false};

  Spsc<Command, 1024> commands_;
  uint64_t sent_ = 0;                    // the worker: the last command's seq
  std::atomic<uint64_t> applied_{0};     // the audio thread: the last command it applied
  std::atomic<bool> ackPlaying_{false};  // and the transport as that block left it
  std::atomic<int64_t> ackPos_{0};
  std::atomic<int64_t> playhead_{0};

  // The audio thread's own, or the worker's while no device runs.
  Transport transport_;
  bool loop_ = false;
  int64_t loopStart_ = 0, loopEnd_ = 0;
  uint64_t callbacks_ = 0;

  std::atomic<Graph*> graph_{nullptr};
  std::unique_ptr<Graph> owned_;    // the worker's handle on what graph_ points to
  std::atomic<uint64_t> epoch_{0};  // odd while a callback runs
  std::atomic<bool> parked_{false};

  int rate_ = 48000;  // the open device's, for the clock and DSP load
  int outputLatency_ = 0;
  std::atomic<uint32_t> late_{0};   // null device: blocks that started more than a block late
  std::atomic<uint32_t> xruns_{0};  // a real device's own count
};

}  // namespace wwav
