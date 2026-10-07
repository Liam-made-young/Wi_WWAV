#include "build.h"

#include <math.h>

#include <set>

namespace wwav {

namespace {

// The loudest a fader or clip gain goes. Anything above is a mistake, and
// would overflow to infinity long before it was a sound.
constexpr double kMaxGainDb = 24.0;

std::string text(const juce::var& v) { return v.toString().toStdString(); }

bool failWith(Failure* f, const char* code, const std::string& message) {
  f->code = code;
  f->message = message;
  return false;
}

std::unique_ptr<Graph> refuse(Failure* f, const char* code, const std::string& message) {
  failWith(f, code, message);
  return nullptr;
}

// gain_db, pan, mute and solo, each optional. The master has no pan or solo.
bool readStrip(const juce::var& o, const std::string& what, bool master, Strip* s, Failure* f) {
  double v;
  if (o.hasProperty("gain_db")) {
    if (!numberOf(o["gain_db"], &v) || v > kMaxGainDb)
      return failWith(f, "bad_session", what + "'s gain_db is a number of dB up to +24.");
    s->gainDb = (float)v;
  }
  if (!master && o.hasProperty("pan")) {
    if (!numberOf(o["pan"], &v) || v < -1.0 || v > 1.0)
      return failWith(f, "bad_session", what + "'s pan is a number from -1 to 1.");
    s->pan = (float)v;
  }
  if (o.hasProperty("mute")) {
    if (!o["mute"].isBool()) return failWith(f, "bad_session", what + "'s mute is true or false.");
    s->mute = (bool)o["mute"];
  }
  if (!master && o.hasProperty("solo")) {
    if (!o["solo"].isBool()) return failWith(f, "bad_session", what + "'s solo is true or false.");
    s->solo = (bool)o["solo"];
  }
  return true;
}

// Devices are a later stage. One that is off, or that load switched off,
// needs nothing from this one; one that is on is refused.
bool devicesOff(const juce::var& devices, const std::set<std::string>& off, const std::string& where, Failure* f) {
  if (devices.isVoid() || devices.isUndefined()) return true;
  if (!devices.isArray()) return failWith(f, "bad_session", "The devices on " + where + " are a list.");
  for (const juce::var& d : *devices.getArray()) {
    const std::string id = text(d["id"]), format = text(d["format"]);
    if (!d.isObject() || id.empty()) return failWith(f, "bad_session", "Every device on " + where + " needs an id.");
    if (format != "builtin" && format != "vst3" && format != "au")
      return failWith(f, "bad_session", "Device " + id + " has format \"" + format + "\"; it is builtin, vst3 or au.");
    const bool on = !d.hasProperty("on") || (bool)d["on"];
    if (!on || off.count(id)) continue;
    if (format == "builtin")
      return failWith(
          f, "unsupported",
          "Built-in devices come in a later stage: \"" + text(d["uid"]) + "\" (" + id + ") on " + where + " is on.");
    return failWith(f, "unsupported",
                    "Plugins come in a later stage: " + id + " on " + where + " is a " +
                        (format == "vst3" ? "VST3" : "AU") + " that is on.");
  }
  return true;
}

bool readClip(const juce::var& c, int rate, const std::string& track, Clip* out, Failure* f) {
  const std::string id = text(c["id"]), path = text(c["path"]), source = text(c["source"]);
  const std::string what = "Clip " + (id.empty() ? std::string("?") : id) + " on track " + track;
  if (!c.isObject() || path.empty() || path[0] != '/')
    return failWith(f, "bad_session", what + " needs an absolute path.");
  int64_t at, in, len;
  if (!wholeOf(c["at"], &at) || at < 0 || !wholeOf(c["in"], &in) || in < 0)
    return failWith(f, "bad_session", what + " needs at and in as whole numbers of frames from 0.");
  if (!wholeOf(c["len"], &len) || len <= 0)
    return failWith(f, "bad_session", what + " needs len as a whole number of frames above 0.");
  double gain = 0.0;
  if (c.hasProperty("gain_db") && (!numberOf(c["gain_db"], &gain) || gain > kMaxGainDb))
    return failWith(f, "bad_session", what + " has a gain_db that isn't a number of dB up to +24.");
  if (c.hasProperty("reverse") && (bool)c["reverse"])
    return failWith(f, "unsupported", "Reversed clips come in a later stage: " + what + " is reversed.");
  MediaPart part;
  if (!probeMedia(path, source, &part, &f->code, &f->message)) return false;
  if ((int)part.rate != rate)
    return failWith(f, "unsupported",
                    path + " is " + std::to_string(part.rate) + " Hz in a " + std::to_string(rate) +
                        " Hz session. Clips at another rate are resampled once the export resampler (F9) is built.");
  std::string message;
  // Files under 32 MB are held whole; bigger ones stream (docs/SPEC.md 9.4).
  out->source = ClipSource::open(path, part, in, len, (int64_t)part.fileSize < kHoldWhole, &message);
  if (!out->source) return failWith(f, "bad_clip", message);
  out->at = at;
  out->gain = dbGain((float)gain);
  return true;
}

bool readTrack(const juce::var& t, int rate, const std::set<std::string>& off, std::set<std::string>* ids, Track* out,
               Failure* f) {
  const std::string id = text(t["id"]), kind = text(t["kind"]);
  if (!t.isObject() || id.empty()) return failWith(f, "bad_session", "Every track needs an id.");
  if (ids->count(id)) return failWith(f, "bad_session", "Two tracks have the id " + id + ".");
  if (id == "master" || id.rfind("bus:", 0) == 0)
    return failWith(f, "bad_session", "A track can't be called " + id + "; the buses and the master are.");
  ids->insert(id);
  if (kind == "instrument")
    return failWith(f, "unsupported", "Instrument tracks come with MIDI and plugins, in a later stage: " + id + ".");
  if (kind == "bus")
    return failWith(f, "unsupported", "Bus tracks (returns and groups) come in a later stage: " + id + ".");
  if (kind != "audio" && kind != "stem")
    return failWith(f, "bad_session",
                    "Track " + id + " has kind \"" + kind + "\"; it is audio, instrument, stem or bus.");
  out->id = id;
  out->role = roleOf(text(t["role"]));
  if (out->role < 0) return failWith(f, "bad_session", "Track " + id + "'s role is vocals, drums, other or bass.");
  if (!readStrip(t, "Track " + id, false, &out->strip, f)) return false;
  const juce::var sends = t["sends"];
  for (const char* send : {"reverb", "delay"}) {
    double v = 0.0;
    if (sends.hasProperty(send) && numberOf(sends[send], &v) && v > 0.0)
      return failWith(f, "unsupported",
                      std::string("Sends feed the built-in reverb and delay returns, which come in a later stage: "
                                  "track ") +
                          id + " sends to its " + send + ".");
  }
  if (!devicesOff(t["devices"], off, "track " + id, f)) return false;
  const juce::var clips = t["clips"];
  if (!clips.isVoid() && !clips.isArray()) return failWith(f, "bad_session", "Track " + id + "'s clips are a list.");
  if (clips.isArray()) {
    for (const juce::var& c : *clips.getArray()) {
      Clip clip;
      if (!readClip(c, rate, id, &clip, f)) return false;
      out->clips.push_back(std::move(clip));
    }
  }
  return true;
}

}  // namespace

bool wholeOf(const juce::var& v, int64_t* out) {
  if (v.isInt() || v.isInt64()) {
    *out = static_cast<juce::int64>(v);
    return true;
  }
  const double d = static_cast<double>(v);
  if (!v.isDouble() || !std::isfinite(d) || fabs(d) > 9e15) return false;
  const int64_t whole = (int64_t)d;
  if ((double)whole < d || (double)whole > d) return false;  // 48000.0 is whole; 0.5 isn't
  *out = whole;
  return true;
}

bool numberOf(const juce::var& v, double* out) {
  if (!v.isInt() && !v.isInt64() && !v.isDouble()) return false;
  *out = (double)v;
  return std::isfinite(*out);
}

std::unique_ptr<Graph> buildGraph(const juce::var& graph, const juce::var& offList, int deviceRate, Failure* f) {
  if (!graph.isObject()) return refuse(f, "bad_session", "session.load needs a graph.");
  int64_t rate;
  if (!wholeOf(graph["sample_rate"], &rate) || rate < 8000 || rate > 384000)
    return refuse(f, "bad_session", "The graph's sample_rate is a rate in Hz.");
  if (deviceRate && rate != deviceRate)
    return refuse(f, "rate_mismatch",
                  "This session runs at " + std::to_string(rate) + " Hz and the device at " +
                      std::to_string(deviceRate) + " Hz. Open the device at " + std::to_string(rate) + " Hz first.");
  std::set<std::string> off;
  if (offList.isArray())
    for (const juce::var& id : *offList.getArray()) off.insert(text(id));

  auto g = std::make_unique<Graph>((int)rate);
  const juce::var tracks = graph["tracks"];
  if (!tracks.isArray()) return refuse(f, "bad_session", "The graph's tracks are a list.");
  if (tracks.size() > (int)WWAV_METER_SLOTS - kRoles - 1)
    return refuse(f, "bad_session",
                  "A session holds " + std::to_string(WWAV_METER_SLOTS - kRoles - 1) + " tracks at most.");
  std::set<std::string> ids;
  for (const juce::var& t : *tracks.getArray()) {
    Track track;
    if (!readTrack(t, (int)rate, off, &ids, &track, f)) return nullptr;
    g->tracks.push_back(std::move(track));
  }
  // The stem buses and the master. The contract's graph has the master's
  // gain_db; a bus's settings and the master's mute are read when present.
  const juce::var master = graph["master"];
  if (!readStrip(master, "The master", true, &g->master, f)) return nullptr;
  if (!devicesOff(master["devices"], off, "the master", f)) return nullptr;
  const juce::var buses = graph["buses"];
  for (int r = 0; r < kRoles; r++)
    if (!readStrip(buses[kRoleNames[r]], std::string("bus:") + kRoleNames[r], false, &g->buses[r], f)) return nullptr;
  g->prepare();
  return g;
}

}  // namespace wwav
