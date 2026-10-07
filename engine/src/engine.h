#pragma once
// wwav-engine: the ops of docs/ENGINE.md 3 over the socket, the audio engine
// behind them, and the threads that run them.
//
//   main thread   JUCE's message loop: ping, debug.* "message", the device manager
//   socket        reads frames, answers hello and param.set, hands every other op on
//   worker        runs ops in the order they came: devices, sessions, the
//                 transport, renders. Long ones (a load, a render) hold up
//                 neither ping, which the main thread answers, nor param.set.
//   audio         the device's callback, or the null device's timer thread
//   reader        keeps streamed clips ahead of the playhead
#include <juce_audio_devices/juce_audio_devices.h>

#include <atomic>
#include <condition_variable>
#include <deque>
#include <functional>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "args.h"
#include "audio.h"
#include "media.h"
#include "server.h"
#include "shm.h"

namespace wwav {

// Ops that run one at a time, in order, off the socket and message threads.
class Worker {
 public:
  Worker();
  ~Worker();
  void post(std::function<void()> job);
  void stop();

 private:
  void run();
  std::mutex mutex_;
  std::condition_variable wake_;
  std::deque<std::function<void()>> jobs_;
  bool stopping_ = false;
  std::thread thread_;
};

// Asks the whole engine to stop its audio and exit (stdin closed, shutdown):
// the message loop ends and main tears down. Any thread may call it.
void requestQuit(const char* why);
// Ends the process 0.75 s from now if it is still running: the contract
// gives it a second. requestQuit arms it, and so does main once the message
// loop ends however it ended (JUCE quits on SIGINT by itself).
void armExitDeadline();

class Engine {
 public:
  explicit Engine(const Args& args);
  ~Engine();

  // On the message thread, before the loop runs: maps the shared memory,
  // opens the device, listens. False, with a sentence, when it can't.
  bool start(std::string* error);
  // On the message thread, after the loop: stops everything, audio first.
  void stop();

 private:
  void request(uint64_t conn, const juce::var& id, const std::string& op, const juce::var& args);
  Reply hello(const juce::var& args);
  Reply deviceList();
  Reply deviceOpen(const juce::var& args);
  Reply sessionLoad(const juce::var& args, size_t changesFrom);
  Reply sessionUnload();
  void paramSetNow(uint64_t conn, const juce::var& id, const juce::var& args);
  Reply paramSet(const juce::var& args);
  Reply transportOp(const std::string& op, const juce::var& args);
  Reply render(const juce::var& args);
  Reply debugAudio(const std::string& op, const juce::var& args, bool* answer);
  void transportEvent(const Transport& t);
  void noteDevice();
  void retire(std::unique_ptr<Graph> old);

  Args args_;
  SharedRegion shm_;
  juce::AudioDeviceManager devices_;
  std::unique_ptr<AudioEngine> audio_;
  std::unique_ptr<Reader> reader_;
  Server server_;
  std::unique_ptr<Worker> worker_;
  std::atomic<bool> stopping_{false};
  uint64_t gen_ = 0;  // the worker's count of session.loads
  // The worker's: the loop it last sent, for the streamed clips of every
  // graph it loads to keep ready.
  bool loopOn_ = false;
  int64_t loopFrom_ = 0;

  // param.set is answered on the socket thread and goes straight to the
  // audio thread, so a mute never waits behind a load, a play or a render
  // on the worker (docs/ENGINE.md 3.4: heard within one block). This lock
  // guards what it needs from the worker:
  std::mutex paramMutex_;
  // The graph the audio thread plays, for looking nodes up. The worker
  // changes it only while holding the lock, so it is never freed under a
  // lookup.
  Graph* live_ = nullptr;
  // session.loads received and not yet finished on the worker. A param.set
  // that comes meanwhile changes the playing graph now and is kept in
  // `changes_`, and each pending load replays the ones that came after it
  // onto its new graph before swapping it in: the load's JSON was compiled
  // before them.
  int loadsPending_ = 0;
  struct Change {
    std::string node;
    Param param;
    float value;
  };
  std::vector<Change> changes_;
  // Nodes with a param.set waiting on the worker (one for a node only a
  // pending load has, or sent while no device runs). Later ones for the
  // same node wait behind it, so a node's changes land in the order sent.
  std::map<std::string, int> deferred_;

  // What hello reports, kept by the worker, read by the socket thread.
  std::mutex formatMutex_;
  std::string deviceName_;
  int rate_, block_;
};

}  // namespace wwav
