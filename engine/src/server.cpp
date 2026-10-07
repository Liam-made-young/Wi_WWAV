#include "server.h"

#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unistd.h>

#include <chrono>
#include <vector>

#include "json.h"

namespace wwav {

namespace {

bool readFull(int fd, void* buf, size_t n) {
  size_t got = 0;
  while (got < n) {
    ssize_t r = read(fd, (uint8_t*)buf + got, n - got);
    if (r < 0 && errno == EINTR) continue;
    if (r <= 0) return false;
    got += (size_t)r;
  }
  return true;
}

bool writeFull(int fd, const void* buf, size_t n) {
  size_t put = 0;
  while (put < n) {
    ssize_t w = write(fd, (const uint8_t*)buf + put, n - put);
    if (w < 0 && errno == EINTR) continue;
    if (w <= 0) return false;
    put += (size_t)w;
  }
  return true;
}

void closeOnExec(int fd) { fcntl(fd, F_SETFD, FD_CLOEXEC); }

// Far deeper than any request the contract has (a graph is six levels), and
// serde_json's own limit, which the app and mock-engine parse with. The
// parser itself has no stack to overflow, but a juce::var tree is freed and
// written out recursively.
constexpr size_t kMaxDepth = 127;

// Replies and events waiting for a client that has stopped reading: four
// whole frames' worth. Past it the client is let go.
constexpr size_t kMaxQueued = 4 * (size_t)kMaxFrame;

// An error's sentence can quote what the request said (a node's id, a
// path); past this many bytes the quote is cut, so a reply stays far below
// the frame limit.
constexpr size_t kMaxMessage = 16 * 1024;

// A strictly parsed payload (json.h) as a juce::var, as the ops read it.
// juce::String ends at U+0000, so a string holding one becomes binary data
// rather than a shorter string: no op takes it as a name, a path or an id.
// A key holding one, or an empty key, names nothing an op reads, and is left
// out rather than cut short into another key.
class VarBuilder final : public json::Handler {
 public:
  juce::var root;

  void beginObject() override { open(juce::var(new juce::DynamicObject())); }
  void beginArray() override { open(juce::var(juce::Array<juce::var>())); }
  void end() override {
    stack_.pop_back();
    keys_.pop_back();
  }
  void key(const std::string& k) override { keys_.back() = k; }
  void string(const std::string& s) override {
    if (s.find('\0') == std::string::npos)
      add(juce::String::fromUTF8(s.data(), (int)s.size()));
    else
      add(juce::var(s.data(), s.size()));
  }
  void number(const char* p, size_t n, bool integral) override {
    if (integral) {
      // Exactly, while it fits in 64 bits; as JUCE's own parser, an int when it fits in 32.
      const bool negative = *p == '-';
      uint64_t v = 0;
      bool fits = true;
      for (size_t i = negative; i < n && fits; i++) {
        const uint64_t d = (uint64_t)(p[i] - '0');
        fits = v <= (UINT64_C(0x8000000000000000) - d) / 10;
        v = v * 10 + d;
      }
      if (fits && (negative || v < UINT64_C(0x8000000000000000))) {
        const int64_t x = negative ? (int64_t)(0 - v) : (int64_t)v;
        add(x >= INT32_MIN && x <= INT32_MAX ? juce::var((int)x) : juce::var((juce::int64)x));
        return;
      }
    }
    add(json::toDouble(p, n));
  }
  void boolean(bool b) override { add(b); }
  void null() override { add(juce::var()); }

 private:
  void add(const juce::var& v) {
    if (stack_.empty()) {
      root = v;
      return;
    }
    if (juce::DynamicObject* o = stack_.back().getDynamicObject()) {
      const std::string& k = keys_.back();
      if (!k.empty() && k.find('\0') == std::string::npos)
        o->setProperty(juce::Identifier(juce::String::fromUTF8(k.data(), (int)k.size())), v);
    } else {
      stack_.back().append(v);
    }
  }
  void open(const juce::var& v) {
    add(v);
    stack_.push_back(v);
    keys_.emplace_back();
  }
  std::vector<juce::var> stack_;
  std::vector<std::string> keys_;
};

// At most `n` bytes of `s`, not splitting a UTF-8 sequence, then "…" when cut.
std::string cut(const std::string& s, size_t n) {
  if (s.size() <= n) return s;
  while (n > 0 && ((unsigned char)s[n] & 0xC0) == 0x80) n--;
  return s.substr(0, n) + "\xe2\x80\xa6";
}

}  // namespace

Reply Reply::done(juce::var result) {
  Reply r;
  r.result = result;
  return r;
}

Reply Reply::fail(const std::string& code, const std::string& message) {
  Reply r;
  r.ok = false;
  r.code = code;
  r.message = cut(message, kMaxMessage);
  return r;
}

Server::~Server() { stop(); }

bool Server::listen(const std::string& path, std::string* error) {
  sockaddr_un addr{};
  addr.sun_family = AF_UNIX;
  if (path.size() >= sizeof addr.sun_path) {
    *error = "The socket path " + path + " is " + std::to_string(path.size()) + " bytes; the most is " +
             std::to_string(sizeof addr.sun_path - 1) + ".";
    return false;
  }
  memcpy(addr.sun_path, path.c_str(), path.size() + 1);
  // A socket left by an engine that was killed is ours to replace; anything else at the path is not.
  struct stat st;
  if (lstat(path.c_str(), &st) == 0) {
    if (!S_ISSOCK(st.st_mode)) {
      *error = path + " exists and isn't a socket.";
      return false;
    }
    unlink(path.c_str());
  }
  listenFd_ = socket(AF_UNIX, SOCK_STREAM, 0);
  if (listenFd_ < 0 || pipe(wake_) != 0) {
    *error = std::string("Can't make the socket (") + strerror(errno) + ").";
    return false;
  }
  closeOnExec(listenFd_);
  closeOnExec(wake_[0]);
  closeOnExec(wake_[1]);
  // Owner-only from the moment it exists.
  mode_t mask = umask(0177);
  int bound = bind(listenFd_, (sockaddr*)&addr, sizeof addr);
  umask(mask);
  struct stat mine;
  if (bound != 0 || chmod(path.c_str(), 0600) != 0 || ::listen(listenFd_, 1) != 0 || lstat(path.c_str(), &mine) != 0) {
    *error = "Can't listen on " + path + " (" + strerror(errno) + ").";
    return false;
  }
  path_ = path;
  boundDev_ = mine.st_dev;
  boundIno_ = mine.st_ino;
  return true;
}

void Server::start(Hello hello, Handler handler) {
  hello_ = std::move(hello);
  handler_ = std::move(handler);
  writer_ = std::thread([this] { writeLoop(); });
  thread_ = std::thread([this] { acceptLoop(); });
}

void Server::stop() {
  {
    std::unique_lock<std::mutex> lock(mutex_);
    if (stopping_) return;
    stopping_ = true;
    queued_.notify_all();
    // What is queued (the answer to shutdown, say) goes out first, if the
    // client takes it soon.
    sent_.wait_for(lock, std::chrono::milliseconds(200), [this] { return out_.empty() && !writerBusy_; });
    if (connFd_ >= 0) shutdown(connFd_, SHUT_RDWR);
  }
  if (wake_[1] >= 0 && write(wake_[1], "x", 1) < 0) perror("wwav-engine: waking the socket thread");
  if (thread_.joinable()) thread_.join();
  if (writer_.joinable()) writer_.join();
  for (int fd : {listenFd_, wake_[0], wake_[1]})
    if (fd >= 0) close(fd);
  listenFd_ = wake_[0] = wake_[1] = -1;
  // Only the socket this engine bound: a replacement engine may have
  // replaced it at the same path since (listen() does that).
  struct stat st;
  if (!path_.empty() && lstat(path_.c_str(), &st) == 0 && st.st_dev == boundDev_ && st.st_ino == boundIno_)
    unlink(path_.c_str());
}

void Server::acceptLoop() {
  for (;;) {
    pollfd p[2] = {{listenFd_, POLLIN, 0}, {wake_[0], POLLIN, 0}};
    if (poll(p, 2, -1) < 0) {
      if (errno == EINTR) continue;
      break;
    }
    if (p[1].revents) break;
    if (!(p[0].revents & POLLIN)) continue;
    int fd = accept(listenFd_, nullptr, nullptr);
    if (fd < 0) continue;
    closeOnExec(fd);
    uint64_t conn;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      if (stopping_) {
        close(fd);
        break;
      }
      conn = ++conns_;
      connFd_ = fd;
      conn_ = conn;
      helloDone_ = false;
    }
    serve(fd, conn);
    // Shut it first: a write stuck on a client that stopped reading then
    // fails at once and lets go of writing_.
    shutdown(fd, SHUT_RDWR);
    {
      std::lock_guard<std::mutex> writing(writing_);
      std::lock_guard<std::mutex> lock(mutex_);
      connFd_ = -1;
      conn_ = 0;
      helloDone_ = false;
      out_.clear();  // what was still queued for it goes nowhere
      outBytes_ = 0;
    }
    sent_.notify_all();
    close(fd);
  }
}

void Server::writeLoop() {
  for (;;) {
    Frame f;
    {
      std::unique_lock<std::mutex> lock(mutex_);
      queued_.wait(lock, [this] { return stopping_ || !out_.empty(); });
      if (out_.empty()) return;  // stopping, with everything sent
      f = std::move(out_.front());
      out_.pop_front();
      outBytes_ -= f.bytes.size();
      writerBusy_ = true;
    }
    {
      std::lock_guard<std::mutex> writing(writing_);
      int fd = -1;
      {
        std::lock_guard<std::mutex> lock(mutex_);
        if (connFd_ >= 0 && f.conn == conn_) fd = connFd_;
      }
      if (fd >= 0 && !writeFull(fd, f.bytes.data(), f.bytes.size())) shutdown(fd, SHUT_RDWR);
    }
    {
      std::lock_guard<std::mutex> lock(mutex_);
      writerBusy_ = false;
    }
    sent_.notify_all();
  }
}

void Server::flush() {
  std::unique_lock<std::mutex> lock(mutex_);
  sent_.wait_for(lock, std::chrono::seconds(1), [this] { return out_.empty() && !writerBusy_; });
}

void Server::serve(int fd, uint64_t conn) {
  std::string payload;
  for (;;) {
    uint8_t head[4];
    if (!readFull(fd, head, 4)) return;
    const uint32_t n = (uint32_t)head[0] | (uint32_t)head[1] << 8 | (uint32_t)head[2] << 16 | (uint32_t)head[3] << 24;
    if (n == 0 || n > kMaxFrame) {
      fprintf(stderr, "wwav-engine: closing the connection: a frame of %u bytes\n", n);
      return;
    }
    payload.resize(n);
    if (!readFull(fd, &payload[0], n)) {
      fprintf(stderr, "wwav-engine: closing the connection: a frame cut short\n");
      return;
    }
    // RFC 8259 strictly: one object, valid UTF-8, nothing after it.
    VarBuilder parsed;
    if (!json::parseObject(payload.data(), n, json::Dialect::Strict, kMaxDepth, &parsed)) {
      fprintf(stderr,
              "wwav-engine: closing the connection: a frame that isn't one JSON object (or nests over %zu deep)\n",
              kMaxDepth);
      return;
    }
    const juce::var msg = parsed.root;
    const juce::var id = msg["id"];
    if (!(id.isInt() || id.isInt64()) || (juce::int64)id <= 0) {
      fprintf(stderr, "wwav-engine: closing the connection: a request without a positive integer id\n");
      return;
    }
    const juce::var op = msg["op"], args = msg["args"];
    if (!op.isString()) {
      reply(conn, id, Reply::fail("bad_request", "A request needs an op."));
      continue;
    }
    if (!args.isVoid() && !args.getDynamicObject()) {
      reply(conn, id, Reply::fail("bad_args", "A request's args are an object."));
      continue;
    }
    const std::string name = op.toString().toStdString();
    if (name == "hello") {
      Reply r = hello_(args);
      reply(conn, id, r);
      if (r.code == "protocol") {
        flush();  // the refusal, then the close
        return;
      }
      if (r.ok) {
        std::lock_guard<std::mutex> lock(mutex_);
        helloDone_ = true;
      }
      continue;
    }
    bool said;
    {
      std::lock_guard<std::mutex> lock(mutex_);
      said = helloDone_;
    }
    if (!said) {
      reply(conn, id, Reply::fail("hello_first", "Send hello first."));
      continue;
    }
    handler_(conn, id, name, args);
  }
}

void Server::reply(uint64_t conn, const juce::var& id, const Reply& r) {
  auto* m = new juce::DynamicObject();
  m->setProperty("id", id);
  m->setProperty("ok", r.ok);
  if (r.ok) {
    m->setProperty("result", r.result);
  } else {
    auto* e = new juce::DynamicObject();
    e->setProperty("code", juce::String(r.code));
    e->setProperty("message", juce::String(r.message));
    m->setProperty("error", juce::var(e));
  }
  send(conn, juce::var(m), false);
}

void Server::event(const juce::var& ev) { send(0, ev, true); }

namespace {

std::string serialise(const juce::var& msg) {
  return juce::JSON::toString(msg, juce::JSON::FormatOptions{}.withSpacing(juce::JSON::Spacing::none)).toStdString();
}

}  // namespace

void Server::send(uint64_t conn, const juce::var& msg, bool afterHello) {
  std::string payload = serialise(msg);
  if (payload.size() > kMaxFrame) {
    // No frame goes out over the limit, which the client would take for a
    // broken connection. A reply says why instead; an event is dropped.
    fprintf(stderr, "wwav-engine: a message of %zu bytes is over the frame limit\n", payload.size());
    if (!msg.hasProperty("id")) return;
    auto* m = new juce::DynamicObject();
    auto* e = new juce::DynamicObject();
    e->setProperty("code", "too_big");
    e->setProperty("message", juce::String("The answer would be " + std::to_string(payload.size()) +
                                           " bytes, more than a frame holds (16 MiB)."));
    m->setProperty("id", msg["id"]);
    m->setProperty("ok", false);
    m->setProperty("error", juce::var(e));
    payload = serialise(juce::var(m));
  }
  const uint32_t n = (uint32_t)payload.size();
  std::string frame(4 + payload.size(), '\0');
  frame[0] = (char)n;
  frame[1] = (char)(n >> 8);
  frame[2] = (char)(n >> 16);
  frame[3] = (char)(n >> 24);
  memcpy(&frame[4], payload.data(), n);
  std::lock_guard<std::mutex> lock(mutex_);
  if (connFd_ < 0 || (conn && conn != conn_) || (afterHello && !helloDone_)) return;
  if (outBytes_ + frame.size() > kMaxQueued) {
    fprintf(stderr, "wwav-engine: closing the connection: the client isn't reading its answers\n");
    shutdown(connFd_, SHUT_RDWR);
    return;
  }
  outBytes_ += frame.size();
  out_.push_back(Frame{conn_, std::move(frame)});
  queued_.notify_one();
}

}  // namespace wwav
