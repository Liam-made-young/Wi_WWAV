#include "engine.h"

#include <juce_cryptography/juce_cryptography.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include <algorithm>
#include <chrono>
#include <set>

#include "build.h"
#include "monotonic.h"
#include "wav.h"

namespace wwav {

namespace {

const char* const kEngine = "wwav-engine " WWAV_ENGINE_VERSION;

// A JSON object, a field at a time.
class Json {
 public:
  Json& set(const char* key, const juce::var& v) {
    o_->setProperty(key, v);
    return *this;
  }
  juce::var var() const { return juce::var(o_.get()); }

 private:
  juce::DynamicObject::Ptr o_ = new juce::DynamicObject();
};

std::string text(const juce::var& v) { return v.toString().toStdString(); }

std::atomic<bool> quitting{false};

}  // namespace

void requestQuit(const char* why) {
  if (quitting.exchange(true)) return;
  fprintf(stderr, "wwav-engine: %s; stopping\n", why);
  // The contract gives the engine a second to exit. If tearing down hangs
  // (a hung audio thread, a device that won't close), leave anyway.
  std::thread([] {
    std::this_thread::sleep_for(std::chrono::milliseconds(900));
    fprintf(stderr, "wwav-engine: stopping took too long; leaving now\n");
    _exit(0);
  }).detach();
  juce::JUCEApplicationBase::quit();
}

// ---- Worker -----------------------------------------------------------------------

Worker::Worker() : thread_([this] { run(); }) {}

Worker::~Worker() { stop(); }

void Worker::post(std::function<void()> job) {
  std::lock_guard<std::mutex> lock(mutex_);
  jobs_.push_back(std::move(job));
  wake_.notify_one();
}

void Worker::stop() {
  {
    std::lock_guard<std::mutex> lock(mutex_);
    stopping_ = true;
  }
  wake_.notify_one();
  if (thread_.joinable()) thread_.join();
}

void Worker::run() {
  for (;;) {
    std::function<void()> job;
    {
      std::unique_lock<std::mutex> lock(mutex_);
      wake_.wait(lock, [&] { return stopping_ || !jobs_.empty(); });
      if (stopping_) return;
      job = std::move(jobs_.front());
      jobs_.pop_front();
    }
    job();
  }
}

// ---- Engine -----------------------------------------------------------------------

Engine::Engine(const Args& args) : args_(args), rate_(args.rate), block_(args.block) {}

Engine::~Engine() { stop(); }

bool Engine::start(std::string* error) {
  *error = shm_.open(args_.shm);
  if (!error->empty()) return false;
  shm_.writeHeader((uint32_t)args_.rate, (uint32_t)args_.block, (uint64_t)getpid(), monotonicNs());
  audio_ = std::make_unique<AudioEngine>(shm_, devices_);
  reader_ = std::make_unique<Reader>();
  worker_ = std::make_unique<Worker>();
  // --device null, a named device, or the system's default. One that won't
  // open is said on stderr; device.open can try again.
  std::string code, message;
  if (!audio_->open(args_.device, args_.rate, args_.block, &code, &message))
    fprintf(stderr, "wwav-engine: no audio device: %s\n", message.c_str());
  noteDevice();
  if (!server_.listen(args_.socket, error)) return false;
  server_.start([this](const juce::var& a) { return hello(a); },
                [this](uint64_t conn, const juce::var& id, const std::string& op, const juce::var& a) {
                  request(conn, id, op, a);
                });
  return true;
}

void Engine::stop() {
  if (stopping_.exchange(true)) return;  // a render running on the worker ends at its next block
  server_.stop();
  if (worker_) worker_->stop();
  if (audio_) {
    audio_->close();
    retire(audio_->swap(nullptr));
  }
  reader_.reset();
  audio_.reset();
}

void Engine::noteDevice() {
  std::lock_guard<std::mutex> lock(formatMutex_);
  const DeviceInfo& d = audio_->device();
  deviceName_ = d.name;
  if (!d.name.empty()) {
    rate_ = d.rate;
    block_ = d.block;
  }
}

void Engine::retire(std::unique_ptr<Graph> old) {
  if (!old) return;
  for (ClipSource* s : old->streamed()) reader_->remove(s);
  // `old` is freed here, on the worker (or main) thread: never on the audio thread.
}

void Engine::transportEvent(const Transport& t) {
  server_.event(Json()
                    .set("ev", "transport")
                    .set("state", t.playing ? "playing" : "stopped")
                    .set("sample", (juce::int64)t.pos)
                    .var());
}

Reply Engine::hello(const juce::var& args) {
  int64_t protocol;
  if (!wholeOf(args["protocol"], &protocol))
    return Reply::fail("bad_args", "hello needs the client's protocol number.");
  if (protocol != WWAV_PROTOCOL)
    return Reply::fail("protocol", "This engine speaks protocol " + std::to_string(WWAV_PROTOCOL) +
                                       "; the app speaks " + std::to_string(protocol) + ".");
  std::lock_guard<std::mutex> lock(formatMutex_);
  return Reply::done(Json()
                         .set("protocol", (int)WWAV_PROTOCOL)
                         .set("engine", kEngine)
                         .set("pid", (int)getpid())
                         .set("sample_rate", rate_)
                         .set("block", block_)
                         .set("device", deviceName_.empty() ? juce::var() : juce::var(juce::String(deviceName_)))
                         .set("shm_layout", (int)WWAV_SHM_LAYOUT)
                         .var());
}

void Engine::request(uint64_t conn, const juce::var& id, const std::string& op, const juce::var& args) {
  auto answer = [this, conn, id](const Reply& r) { server_.reply(conn, id, r); };
  if (op == "ping") {
    // The message thread answers, so a hung message loop shows as an
    // unanswered ping (docs/ENGINE.md 3.1).
    juce::MessageManager::callAsync(
        [this, conn, id] { server_.reply(conn, id, Reply::done(Json().set("t", (juce::int64)monotonicNs()).var())); });
    return;
  }
  if (op == "shutdown") {
    answer(Reply::done());
    requestQuit("asked to shut down");
    return;
  }
  static const std::set<std::string> plugins = {"plugin.state", "plugin.editor.open", "plugin.editor.close",
                                                "plugin.params"};
  if (plugins.count(op)) return answer(Reply::fail("unsupported", "Plugins come in a later stage (S0.4)."));
  if (op == "midi.inputs" || op == "midi.route")
    return answer(Reply::fail("unsupported", "MIDI comes in a later stage."));

  const bool debug = op == "debug.crash" || op == "debug.hang" || op == "debug.crumb";
  if (debug && !args_.test) return answer(Reply::fail("unknown_op", "No op named \"" + op + "\"."));
  if (debug && op != "debug.crumb") {
    const std::string in = text(args["in"]);
    if (in == "message") {
      if (op == "debug.crash") {
        juce::MessageManager::callAsync([] { abort(); });
        return;
      }
      answer(Reply::done());
      juce::MessageManager::callAsync([] {
        for (;;) std::this_thread::sleep_for(std::chrono::seconds(1));
      });
      return;
    }
    if (in != "audio")
      return answer(Reply::fail("bad_args", "debug.crash and debug.hang take in: \"audio\" or \"message\"."));
  }

  std::function<Reply()> run;
  if (op == "device.list")
    run = [this] { return deviceList(); };
  else if (op == "device.open")
    run = [this, args] { return deviceOpen(args); };
  else if (op == "session.load")
    run = [this, args] { return sessionLoad(args); };
  else if (op == "session.unload")
    run = [this] { return sessionUnload(); };
  else if (op == "param.set")
    run = [this, args] { return paramSet(args); };
  else if (op == "transport.play" || op == "transport.stop" || op == "transport.locate" || op == "transport.loop")
    run = [this, op, args] { return transportOp(op, args); };
  else if (op == "render")
    run = [this, args] { return render(args); };
  else if (!debug)
    return answer(Reply::fail("unknown_op", "No op named \"" + op + "\"."));
  if (debug) {
    // A crash has no answer: the connection closing is the answer.
    worker_->post([this, op, args, answer] {
      bool reply = true;
      Reply r = debugAudio(op, args, &reply);
      if (reply) answer(r);
    });
    return;
  }
  worker_->post([run, answer] { answer(run()); });
}

// ---- devices ----------------------------------------------------------------------

Reply Engine::deviceList() {
  juce::Array<juce::var> list, rates;
  for (int r : {44100, 48000, 88200, 96000}) rates.add(r);
  list.add(Json().set("name", "null").set("inputs", 0).set("outputs", 2).set("rates", rates).var());
  juce::MessageManager::callSync([&] {
    for (juce::AudioIODeviceType* type : devices_.getAvailableDeviceTypes()) {
      type->scanForDevices();
      const juce::StringArray inputs = type->getDeviceNames(true);
      for (const juce::String& name : type->getDeviceNames(false)) {
        std::unique_ptr<juce::AudioIODevice> d(type->createDevice(name, inputs.contains(name) ? name : juce::String()));
        if (!d) continue;
        juce::Array<juce::var> deviceRates;
        for (double r : d->getAvailableSampleRates()) deviceRates.add((int)r);
        list.add(Json()
                     .set("name", name)
                     .set("inputs", d->getInputChannelNames().size())
                     .set("outputs", d->getOutputChannelNames().size())
                     .set("rates", deviceRates)
                     .var());
      }
    }
  });
  return Reply::done(Json().set("devices", list).var());
}

Reply Engine::deviceOpen(const juce::var& args) {
  std::string name;
  const juce::var n = args["name"];
  if (n.isString())
    name = text(n);
  else if (n.isVoid())  // null: the default, which is the null device when the engine started with --device null
    name = args_.device == "null" ? "null" : "";
  else
    return Reply::fail("bad_args", "device.open's name is a device's name or null.");
  int64_t rate, block;
  {
    std::lock_guard<std::mutex> lock(formatMutex_);
    rate = rate_;
    block = block_;
  }
  if (args.hasProperty("sample_rate") && (!wholeOf(args["sample_rate"], &rate) || rate < 8000 || rate > 384000))
    return Reply::fail("bad_args", "device.open's sample_rate is a rate in Hz.");
  if (args.hasProperty("block") && (!wholeOf(args["block"], &block) || block < 16 || block > kMaxBlock))
    return Reply::fail("bad_args", "device.open's block is from 16 to " + std::to_string(kMaxBlock) + " frames.");
  if (Graph* g = audio_->graph(); g && g->sampleRate != rate)
    return Reply::fail("rate_mismatch", "The loaded session runs at " + std::to_string(g->sampleRate) +
                                            " Hz. Open the device at that rate, or unload the session first.");
  std::string code, message;
  bool opened = audio_->open(name, (int)rate, (int)block, &code, &message);
  noteDevice();
  if (!opened) return Reply::fail(code, message);
  const DeviceInfo& d = audio_->device();
  return Reply::done(Json()
                         .set("name", juce::String(d.name))
                         .set("sample_rate", d.rate)
                         .set("block", d.block)
                         .set("output_latency", d.outputLatency)
                         .set("input_latency", d.inputLatency)
                         .var());
}

// ---- the session ------------------------------------------------------------------

Reply Engine::sessionLoad(const juce::var& args) {
  int64_t playhead = -1;
  if (args.hasProperty("playhead") && (!wholeOf(args["playhead"], &playhead) || playhead < 0))
    return Reply::fail("bad_args", "session.load's playhead is a sample from 0.");
  Failure f;
  std::unique_ptr<Graph> g = buildGraph(args["graph"], args["off"], audio_->running() ? audio_->device().rate : 0, &f);
  if (!g) return Reply::fail(f.code, f.message);
  g->gen = ++gen_;
  // Streamed clips start with their window full where the playhead will be.
  g->prefill(playhead >= 0 ? playhead : audio_->transport().pos);
  for (ClipSource* s : g->streamed()) reader_->add(s);

  Json slots;
  for (int i = 0; i < g->nodes(); i++) slots.set(g->nodeId(i).c_str(), i);
  const int nodes = g->nodes();
  if (playhead >= 0) {
    Command c;
    c.kind = Command::Locate;
    c.a = playhead;
    transportEvent(audio_->apply(c));
  }
  retire(audio_->swap(std::move(g)));
  return Reply::done(Json().set("nodes", nodes).set("latency", Json().var()).set("meter_slots", slots.var()).var());
}

Reply Engine::sessionUnload() {
  retire(audio_->swap(nullptr));
  return Reply::done();
}

Reply Engine::paramSet(const juce::var& args) {
  Graph* g = audio_->graph();
  if (!g) return Reply::fail("no_session", "No session is loaded.");
  const std::string node = text(args["node"]), name = text(args["param"]);
  const int index = g->find(node);
  if (index < 0) return Reply::fail("no_such_node", "No node " + node + ".");
  if (name == "send.reverb" || name == "send.delay")
    return Reply::fail("unsupported", "Sends feed the built-in reverb and delay returns, which come in a later stage.");
  Param p;
  if (!paramOf(name, &p) || !g->takes(index, p))
    return Reply::fail("no_such_param",
                       node + " has no param \"" + name + "\". " +
                           (index == g->masterNode() ? "The master takes gain_db and mute."
                                                     : "Tracks and buses take gain_db, pan, mute and solo."));
  if (args.hasProperty("at"))
    return Reply::fail("unsupported",
                       "A change at a set sample comes with automation, in a later stage. Without at, it is heard "
                       "from the next block.");
  const juce::var v = args["value"];
  double value = 0.0;
  if (p == Param::Mute || p == Param::Solo) {
    if (!v.isBool()) return Reply::fail("bad_args", name + " is true or false.");
    value = (bool)v ? 1.0 : 0.0;
  } else if (p == Param::GainDb) {
    if (!numberOf(v, &value) || value > 24.0) return Reply::fail("bad_args", "gain_db is a number of dB up to +24.");
  } else if (!numberOf(v, &value) || value < -1.0 || value > 1.0) {
    return Reply::fail("bad_args", "pan is a number from -1 to 1.");
  }
  Command c;
  c.kind = Command::SetParam;
  c.param = p;
  c.node = index;
  c.value = (float)value;
  c.gen = g->gen;
  audio_->post(c);
  return Reply::done();
}

Reply Engine::transportOp(const std::string& op, const juce::var& args) {
  Command c;
  if (op == "transport.play") {
    c.kind = Command::Play;
    // Streamed clips need their first quarter second read before play starts.
    if (Graph* g = audio_->graph()) {
      const int64_t pos = audio_->transport().pos;
      const auto until = std::chrono::steady_clock::now() + std::chrono::milliseconds(300);
      while (!g->ready(pos, g->sampleRate / 4) && std::chrono::steady_clock::now() < until)
        std::this_thread::sleep_for(std::chrono::milliseconds(2));
    }
  } else if (op == "transport.stop") {
    c.kind = Command::Stop;
  } else if (op == "transport.locate") {
    c.kind = Command::Locate;
    if (!wholeOf(args["sample"], &c.a) || c.a < 0)
      return Reply::fail("bad_args", "transport.locate needs a sample from 0.");
  } else {
    c.kind = Command::Loop;
    if (!args["on"].isBool()) return Reply::fail("bad_args", "transport.loop needs on: true or false.");
    c.on = (bool)args["on"];
    if (c.on && (!wholeOf(args["start"], &c.a) || !wholeOf(args["end"], &c.b) || c.a < 0 || c.b <= c.a))
      return Reply::fail("bad_args", "A loop needs a start from 0 and an end after it.");
    audio_->apply(c);
    return Reply::done();
  }
  const Transport t = audio_->apply(c);
  transportEvent(t);
  return Reply::done(Json().set("sample", (juce::int64)t.pos).var());
}

// ---- render -------------------------------------------------------------------------

Reply Engine::render(const juce::var& args) {
  Graph* g = audio_->graph();
  const std::string dir = text(args["out_dir"]);
  int64_t start = 0, len = 0;
  if (dir.empty() || dir[0] != '/') return Reply::fail("bad_args", "render needs out_dir, an absolute path.");
  if (args.hasProperty("start") && (!wholeOf(args["start"], &start) || start < 0))
    return Reply::fail("bad_args", "render's start is a sample from 0.");
  if (!wholeOf(args["len"], &len) || len <= 0)
    return Reply::fail("bad_args", "render needs len, a number of frames above 0.");
  bool want[2] = {true, true};  // master, stems
  const char* keys[2] = {"master", "stems"};
  for (int i = 0; i < 2; i++) {
    if (!args.hasProperty(keys[i])) continue;
    if (!args[keys[i]].isBool())
      return Reply::fail("bad_args", std::string("render's ") + keys[i] + " is true or false.");
    want[i] = (bool)args[keys[i]];
  }
  if (!want[0] && !want[1]) return Reply::fail("bad_args", "render writes the master, the stems or both.");
  const std::string format = args.hasProperty("format") ? text(args["format"]) : "f32";
  if (format != "f32" && format != "s16") return Reply::fail("bad_args", "render's format is f32 or s16.");
  if (!g) return Reply::fail("no_session", "Load a session before rendering.");
  if (juce::File(dir).createDirectory().failed()) return Reply::fail("render_failed", "Can't make " + dir + ".");

  // Five files: the master and the stems, in stem order.
  struct Out {
    std::string name, path;
    WavWriter wav;
  };
  std::vector<std::unique_ptr<Out>> outs;
  std::string message;
  for (int i = -1; i < kRoles; i++) {
    if ((i < 0 && !want[0]) || (i >= 0 && !want[1])) continue;
    auto o = std::make_unique<Out>();
    o->name = i < 0 ? "master" : kRoleNames[i];
    o->path = dir + "/" + o->name + ".wav";
    if (!o->wav.open(o->path, format == "f32" ? WavFormat::F32 : WavFormat::S16, (uint32_t)g->sampleRate, (uint64_t)len,
                     &message))
      return Reply::fail("render_failed", message);
    outs.push_back(std::move(o));
  }

  // Live playback stops for the length of a render and comes back stopped.
  if (audio_->transport().playing) {
    Command stop;
    stop.kind = Command::Stop;
    transportEvent(audio_->apply(stop));
  }
  int block;
  {
    std::lock_guard<std::mutex> lock(formatMutex_);
    block = std::min(block_, kMaxBlock);
  }
  // The same graph objects as live, as fast as the CPU allows, at the session's block size.
  audio_->park();
  g->settle();
  bool ok = true;
  {
    juce::ScopedNoDenormals noDenormals;
    const std::string stage = want[1] ? "stems" : "master";
    int64_t tick = -1;
    for (int64_t done = 0; done < len && ok;) {
      if (stopping_) {
        ok = false;
        message = "The engine stopped during the render.";
        break;
      }
      const int n = (int)std::min<int64_t>(block, len - done);
      g->beginBlock(n);
      g->process(start + done, 0, n, true, false, &shm_);
      g->endBlock(n, nullptr);
      for (auto& o : outs) {
        const int r = roleOf(o->name);
        ok = ok && (r < 0 ? o->wav.write(g->masterL(), g->masterR(), n) : o->wav.write(g->busL(r), g->busR(r), n));
      }
      done += n;
      // About twenty progress events a render.
      if (done * 20 / len != tick) {
        tick = done * 20 / len;
        server_.event(Json()
                          .set("ev", "render.progress")
                          .set("stage", juce::String(stage))
                          .set("done", (juce::int64)done)
                          .set("total", (juce::int64)len)
                          .var());
      }
    }
  }
  audio_->resume();
  for (auto& o : outs) ok = o->wav.close(&message) && ok;
  if (!ok) return Reply::fail("render_failed", message);

  Json files, hashes;
  for (auto& o : outs) {
    files.set(o->name.c_str(), juce::String(o->path));
    hashes.set(o->name.c_str(), juce::SHA256(juce::File(o->path)).toHexString());
  }
  return Reply::done(
      Json().set("files", files.var()).set("frames", (juce::int64)len).set("sha256", hashes.var()).var());
}

// ---- debug (with --test) ------------------------------------------------------------

Reply Engine::debugAudio(const std::string& op, const juce::var& args, bool* answer) {
  if (!audio_->running()) return Reply::fail("no_device", "No audio device is running, so there is no audio thread.");
  Command c;
  if (op == "debug.crumb") {
    Graph* g = audio_->graph();
    if (!g) return Reply::fail("no_session", "No session is loaded.");
    const std::string node = text(args["node"]);
    c.node = g->find(node);
    if (c.node < 0) return Reply::fail("no_such_node", "No node " + node + ".");
    c.kind = Command::CrashIn;
    c.gen = g->gen;
    *answer = false;
  } else if (op == "debug.crash") {
    c.kind = Command::Crash;
    *answer = false;
  } else {
    c.kind = Command::Hang;
  }
  audio_->post(c);
  return Reply::done();
}

}  // namespace wwav
