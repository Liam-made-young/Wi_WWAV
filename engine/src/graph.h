#pragma once
// The graph (docs/SPEC.md 9.4, 5.3, 6.6): tracks, then the four stem buses,
// then the master. Every track sums into the bus of its role; the master is
// the sum of the four buses, then its own gain. The same code runs live on
// the audio thread and offline on the render thread, so what you hear is
// what you render.
#include <stdint.h>

#include <map>
#include <memory>
#include <string>
#include <vector>

#include "media.h"
#include "shm.h"

namespace wwav {

// Stem order is fixed: PRANA's fader order, Wi's, and the order in the file.
enum Role : int { kVocals = 0, kDrums, kOther, kBass, kRoles };
extern const char* const kRoleNames[kRoles];
int roleOf(const std::string& name);  // -1 when it isn't one

// A gain that ramps linearly to its target over one stretch of a block, so
// a change never clicks and every build ramps the same (PRANA's dsp::Ramp).
// A gain that isn't changing steps by zero: unity stays exact.
struct Ramp {
  float cur = 1.0f;
  float target = 1.0f;
  float step = 0.0f;
  void begin(int n) { step = (target - cur) / (float)n; }
  float next() {
    cur += step;
    return cur;
  }
  void end() {
    cur = target;
    step = 0.0f;
  }
};

enum class Param : uint8_t { GainDb, Pan, Mute, Solo };
bool paramOf(const std::string& name, Param* out);

// A node id's crumb: its FNV-1a 64, as wwav_ids::fnv1a64 computes it.
uint64_t fnv1a64(const std::string& s);

// dB to a linear gain with PRANA's deterministic maths; 0 dB is exactly 1.
float dbGain(float db);
// A stereo balance: centre leaves both sides at unity (so a .wwav's stems
// sum to its master at 0 dB); turning toward one side lowers the other
// along a quarter cosine, to silence at the end.
void balance(float pan, float* l, float* r);

// The fader end of a track, a bus or the master.
struct Strip {
  float gainDb = 0.0f;
  float pan = 0.0f;
  bool mute = false;
  bool solo = false;
  Ramp l, r;
};

struct Clip {
  int64_t at = 0;  // session frame of the clip's first frame
  float gain = 1.0f;
  std::unique_ptr<ClipSource> source;
};

struct Track {
  std::string id;
  int role = kOther;
  std::vector<Clip> clips;
  Strip strip;
};

class Graph {
 public:
  explicit Graph(int rate) : sampleRate(rate) {}
  const int sampleRate;
  uint64_t gen = 0;  // which load this is: params sent for another are dropped
  std::vector<Track> tracks;
  Strip buses[kRoles];
  Strip master;

  // Node numbers, which are also the meter slots: one per track, then the
  // four buses, then the master.
  int nodes() const { return (int)tracks.size() + kRoles + 1; }
  int busNode(int role) const { return (int)tracks.size() + role; }
  int masterNode() const { return (int)tracks.size() + kRoles; }
  std::string nodeId(int node) const;
  int find(const std::string& id) const;  // -1 when there is no such node
  bool takes(int node, Param p) const;    // the master has no pan or solo

  // After the tracks are in: buffers, crumbs, and every gain at its target.
  void prepare();
  // Every gain at its target now, without ramping there. A render starts
  // so: what it writes can't depend on whether a block ran since the last
  // param.set.
  void settle();
  // Before the reader thread knows them: fills streamed clips at `pos`.
  void prefill(int64_t pos);
  // The worker: where the transport's loop jumps back to, as a session
  // sample, or -1 when there is no loop. Streamed clips keep the frames
  // from there ready.
  void wantLoop(int64_t start);

  // Audio thread: a parameter change, heard from the next stretch.
  void set(int node, Param p, float value);

  // One block in stretches of the session: beginBlock, process() for each
  // stretch (two when the block crosses a loop's end), then endBlock.
  void beginBlock(int n);
  void process(int64_t pos, int at, int n, bool playing, bool realtime, CrumbSink* crumbs);
  // Writes the slots of a meter entry; returns how many it used.
  uint32_t endBlock(int n, wwav_shm_meter_entry* meters);

  const float* busL(int role) const { return buf(kBusL + 2 * role); }
  const float* busR(int role) const { return buf(kBusL + 2 * role + 1); }
  const float* masterL() const { return buf(kMasterL); }
  const float* masterR() const { return buf(kMasterR); }

  // Whether streamed clips hold `frames` frames from `pos` (any thread).
  bool ready(int64_t pos, int64_t frames) const;
  std::vector<ClipSource*> streamed() const;

  // debug.crumb: the audio thread dies inside this node.
  void crashIn(int node) { crashNode_ = node; }

 private:
  enum Buffer : int { kBusL = 0, kMasterL = kBusL + 2 * kRoles, kMasterR, kTrackL, kTrackR, kClipL, kClipR, kBuffers };
  float* buf(int b) { return &buffers_[(size_t)b * kMaxBlock]; }
  const float* buf(int b) const { return &buffers_[(size_t)b * kMaxBlock]; }
  void retarget();
  void meter(int node, const float* l, const float* r, int n);

  struct Level {
    float peak[2];
    float sumSq[2];
  };
  std::vector<float> buffers_;
  std::vector<Level> levels_;
  std::vector<uint64_t> crumbs_;
  std::map<std::string, int> ids_;
  int crashNode_ = -1;
};

}  // namespace wwav
