#pragma once
// The command socket (docs/ENGINE.md 1, 2): a Unix domain socket, mode 0600,
// one client at a time. Every message is a frame: a u32 little-endian length
// (1 .. 16 MiB), then one JSON object. A bad frame closes the connection.
#include <juce_core/juce_core.h>

#include <atomic>
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
  void serve(int fd, uint64_t conn);
  void send(uint64_t conn, const juce::var& msg, bool afterHello);

  std::string path_;
  int listenFd_ = -1;
  int wake_[2] = {-1, -1};
  Hello hello_;
  Handler handler_;
  std::thread thread_;
  // Two locks, taken in this order: writing_ while a frame goes out (so
  // frames never interleave, and the fd isn't closed under a writer), and
  // mutex_ for the connection's state, held only briefly. stop() takes only
  // mutex_, so a writer stuck on a client that stopped reading can't hold it up.
  std::mutex writing_;
  std::mutex mutex_;  // the connection: its fd, its number, whether it said hello
  int connFd_ = -1;
  uint64_t conn_ = 0, conns_ = 0;
  bool helloDone_ = false;
  bool stopping_ = false;
};

}  // namespace wwav
