// wwav-engine (docs/ENGINE.md): the audio engine Wi_WWAV.app starts as a
// child process. The main thread runs JUCE's message loop, as plugin windows
// will need; everything else is in Engine.
#include <signal.h>
#include <stdio.h>
#include <unistd.h>

#include <memory>
#include <thread>

#include "args.h"
#include "engine.h"

namespace {

class EngineApp final : public juce::JUCEApplicationBase {
 public:
  const juce::String getApplicationName() override { return "wwav-engine"; }
  const juce::String getApplicationVersion() override { return WWAV_ENGINE_VERSION; }
  bool moreThanOneInstanceAllowed() override { return true; }
  void anotherInstanceStarted(const juce::String&) override {}
  void systemRequestedQuit() override { wwav::requestQuit("the system asked it to quit"); }
  void suspended() override {}
  void resumed() override {}
  void unhandledException(const std::exception*, const juce::String&, int) override {}

  void initialise(const juce::String&) override {
    // A client that hangs up mid-reply is a closed connection, not a reason to die.
    signal(SIGPIPE, SIG_IGN);
    std::vector<std::string> argv;
    for (const juce::String& a : getCommandLineParameterArray()) argv.push_back(a.toStdString());
    wwav::Args args;
    std::string error;
    if (!wwav::parseArgs(argv, &args, &error)) {
      fprintf(stderr, "wwav-engine: %s\n%s", error.c_str(), wwav::kUsage);
      setApplicationReturnValue(2);
      quit();
      return;
    }
    engine_ = std::make_unique<wwav::Engine>(args);
    if (!engine_->start(&error)) {
      fprintf(stderr, "wwav-engine: %s\n", error.c_str());
      setApplicationReturnValue(1);
      quit();
      return;
    }
    // The one line on stdout: the app waits for it before connecting.
    printf("wwav-engine listening %s\n", args.socket.c_str());
    fflush(stdout);
    // stdin is a pipe from the app. End of file means the app has gone.
    std::thread([] {
      char buf[256];
      ssize_t n;
      while ((n = read(STDIN_FILENO, buf, sizeof buf)) != 0) {
        if (n < 0 && errno != EINTR) break;
      }
      wwav::requestQuit("stdin closed");
    }).detach();
  }

  void shutdown() override {
    wwav::armExitDeadline();
    if (engine_) engine_->stop();
    engine_.reset();
  }

 private:
  std::unique_ptr<wwav::Engine> engine_;
};

}  // namespace

START_JUCE_APPLICATION(EngineApp)
