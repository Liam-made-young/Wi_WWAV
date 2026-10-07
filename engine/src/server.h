#pragma once
// The command socket (docs/ENGINE.md 1, 2): a Unix domain socket, mode 0600,
// one client at a time. Every message is a frame: a u32 little-endian length
// (1 .. 16 MiB), then one JSON object. A bad frame closes the connection.
//
// One thread reads frames and one writes them, so neither the reader (which
// answers hello and param.set itself) nor the worker ever blocks on a client
// that has stopped reading: the reader still sees that client's next bad
// frame, or the next client's connection once this one goes.
#include <juce_core/juce_core.h>
#include <sys/types.h>

#include <atomic>
#include <condition_variable>
#include <deque>
#include <functional>
#include <mutex>
#include <string>
#include <thread>

namespace wwav {

constexpr uint32_t kMaxFrame = 16u * 1024 * 1024;

// One request's answer: a result object, or an error code and a sentence.
struct Reply {
  bool ok = true;
  juce::var result;
  std::string code, message;
  static Reply done(juce::var result = juce::var(new juce::DynamicObject()));
  static Reply fail(const std::string& code, const std::string& message);
};

class Server {
 public:
  // Answers hello. A refused protocol closes the connection.
  using Hello = std::function<Reply(const juce::var& args)>;
  // Every other request, after hello. It answers through reply(), at once or
  // later, from any thread.
  using Handler = std::function<void(uint64_t conn, const juce::var& id, const std::string& op, const juce::var& args)>;

  Server() = default;
  Server(const Server&) = delete;
  Server& operator=(const Server&) = delete;
  ~Server();

  bool listen(const std::string& path, std::string* error);
  void start(Hello hello, Handler handler);
  void stop();

  // Thread-safe. A reply for a connection that has closed goes nowhere.
  void reply(uint64_t conn, const juce::var& id, const Reply& r);
  // To the connected client, once it has said hello.
  void event(const juce::var& ev);

 private:
  void acceptLoop();
  void writeLoop();
  void serve(int fd, uint64_t conn);
  void send(uint64_t conn, const juce::var& msg, bool afterHello);
  // Waits (a second at most) until the frames queued so far have gone out.
  void flush();

  std::string path_;
  dev_t boundDev_ = 0;  // the socket file this engine bound, so stop() unlinks only that
  ino_t boundIno_ = 0;
  int listenFd_ = -1;
  int wake_[2] = {-1, -1};
  Hello hello_;
  Handler handler_;
  std::thread thread_;  // accepts connections and reads their frames
  std::thread writer_;  // writes every frame, in the order they were queued
  // Two locks, taken in this order: writing_ while the writer has a frame
  // going out (so the fd isn't closed under it), and mutex_ for the
  // connection's state and the queue, held only briefly. stop() takes only
  // mutex_, so a write stuck on a client that stopped reading can't hold it up.
  std::mutex writing_;
  std::mutex mutex_;                // the connection: its fd, its number, whether it said hello; the queue
  std::condition_variable queued_;  // a frame was queued, or the server is stopping
  std::condition_variable sent_;    // the writer has finished with a frame
  struct Frame {
    uint64_t conn;
    std::string bytes;  // the length, then the payload
  };
  std::deque<Frame> out_;
  size_t outBytes_ = 0;
  bool writerBusy_ = false;
  int connFd_ = -1;
  uint64_t conn_ = 0, conns_ = 0;
  bool helloDone_ = false;
  bool stopping_ = false;
};

}  // namespace wwav
