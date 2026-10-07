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
  r.message = message;
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
  if (bound != 0 || chmod(path.c_str(), 0600) != 0 || ::listen(listenFd_, 1) != 0) {
    *error = "Can't listen on " + path + " (" + strerror(errno) + ").";
    return false;
  }
  path_ = path;
  return true;
}

void Server::start(Hello hello, Handler handler) {
  hello_ = std::move(hello);
  handler_ = std::move(handler);
  thread_ = std::thread([this] { acceptLoop(); });
}

void Server::stop() {
  {
    std::lock_guard<std::mutex> lock(mutex_);
    if (stopping_) return;
    stopping_ = true;
    if (connFd_ >= 0) shutdown(connFd_, SHUT_RDWR);
  }
  if (wake_[1] >= 0 && write(wake_[1], "x", 1) < 0) perror("wwav-engine: waking the socket thread");
  if (thread_.joinable()) thread_.join();
  for (int fd : {listenFd_, wake_[0], wake_[1]})
    if (fd >= 0) close(fd);
  listenFd_ = wake_[0] = wake_[1] = -1;
  if (!path_.empty()) unlink(path_.c_str());
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
    // Shut it first: a writer stuck on a client that stopped reading then
    // fails at once and lets go of writing_.
    shutdown(fd, SHUT_RDWR);
    {
      std::lock_guard<std::mutex> writing(writing_);
      std::lock_guard<std::mutex> lock(mutex_);
      connFd_ = -1;
      conn_ = 0;
      helloDone_ = false;
    }
    close(fd);
  }
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
    juce::var msg;
    if (juce::JSON::parse(juce::String::fromUTF8(payload.data(), (int)n), msg).failed() || !msg.getDynamicObject()) {
      fprintf(stderr, "wwav-engine: closing the connection: a frame that isn't a JSON object\n");
      return;
    }
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
      if (r.code == "protocol") return;
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

void Server::send(uint64_t conn, const juce::var& msg, bool afterHello) {
  const std::string payload =
      juce::JSON::toString(msg, juce::JSON::FormatOptions{}.withSpacing(juce::JSON::Spacing::none)).toStdString();
  const uint32_t n = (uint32_t)payload.size();
  const uint8_t head[4] = {(uint8_t)n, (uint8_t)(n >> 8), (uint8_t)(n >> 16), (uint8_t)(n >> 24)};
  std::lock_guard<std::mutex> writing(writing_);
  int fd;
  {
    std::lock_guard<std::mutex> lock(mutex_);
    if (connFd_ < 0 || (conn && conn != conn_) || (afterHello && !helloDone_)) return;
    fd = connFd_;
  }
  if (!writeFull(fd, head, 4) || !writeFull(fd, payload.data(), n)) shutdown(fd, SHUT_RDWR);
}

}  // namespace wwav
