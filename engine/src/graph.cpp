#include "graph.h"

#include <stdlib.h>

#include <algorithm>

#include "base/dmath.h"

namespace wwav {

const char* const kRoleNames[kRoles] = {"vocals", "drums", "other", "bass"};

int roleOf(const std::string& name) {
  for (int i = 0; i < kRoles; i++)
    if (name == kRoleNames[i]) return i;
  return -1;
}

bool paramOf(const std::string& name, Param* out) {
  if (name == "gain_db")
    *out = Param::GainDb;
  else if (name == "pan")
    *out = Param::Pan;
  else if (name == "mute")
    *out = Param::Mute;
  else if (name == "solo")
    *out = Param::Solo;
  else
    return false;
  return true;
}

// Not PRANA's base/hash.h: it starts from 1469598103934665603, a digit short
// of FNV-1a's offset basis, so its hashes match no other FNV-1a 64.
uint64_t fnv1a64(const std::string& s) {
  uint64_t h = 0xcbf29ce484222325ull;
  for (unsigned char c : s) {
    h ^= c;
    h *= 0x100000001b3ull;
  }
  return h;
}

float dbGain(float db) { return db == 0.0f ? 1.0f : prana::dbToGain(db); }

void balance(float pan, float* l, float* r) {
  *l = pan > 0.0f ? prana::dcos(pan * prana::kHalfPi) : 1.0f;
  *r = pan < 0.0f ? prana::dcos(-pan * prana::kHalfPi) : 1.0f;
}

namespace {

// The strip's gain over one stretch, ramping from where it was.
void applyStrip(Strip& s, float* l, float* r, int n) {
  s.l.begin(n);
  s.r.begin(n);
  for (int i = 0; i < n; i++) {
    l[i] *= s.l.next();
    r[i] *= s.r.next();
  }
  s.l.end();
  s.r.end();
}

void target(Strip& s, bool audible) {
  float g = audible ? dbGain(s.gainDb) : 0.0f, l, r;
  balance(s.pan, &l, &r);
  s.l.target = g * l;
  s.r.target = g * r;
}

}  // namespace

std::string Graph::nodeId(int node) const {
  const int t = (int)tracks.size();
  if (node < t) return tracks[(size_t)node].id;
  if (node < t + kRoles) return std::string("bus:") + kRoleNames[node - t];
  return "master";
}

int Graph::find(const std::string& id) const {
  auto it = ids_.find(id);
  return it == ids_.end() ? -1 : it->second;
}

bool Graph::takes(int node, Param p) const {
  if (node < 0 || node >= nodes()) return false;
  return node != masterNode() || p == Param::GainDb || p == Param::Mute;
}

void Graph::prepare() {
  buffers_.assign((size_t)kBuffers * kMaxBlock, 0.0f);
  levels_.assign((size_t)nodes(), Level{});
  crumbs_.clear();
  ids_.clear();
  for (int i = 0; i < nodes(); i++) {
    std::string id = nodeId(i);
    crumbs_.push_back(fnv1a64(id));
    ids_[id] = i;
  }
  // A new graph starts at its levels, without ramping in.
  retarget();
  settle();
}

void Graph::settle() {
  auto at = [](Strip& s) {
    s.l.end();
    s.r.end();
  };
  for (Track& t : tracks) at(t.strip);
  for (Strip& b : buses) at(b);
  at(master);
}

void Graph::retarget() {
  bool trackSolo = false, busSolo = false;
  for (const Track& t : tracks) trackSolo = trackSolo || t.strip.solo;
  for (const Strip& b : buses) busSolo = busSolo || b.solo;
  for (Track& t : tracks) target(t.strip, !t.strip.mute && (!trackSolo || t.strip.solo));
  for (Strip& b : buses) target(b, !b.mute && (!busSolo || b.solo));
  target(master, !master.mute);
}

void Graph::set(int node, Param p, float value) {
  const int t = (int)tracks.size();
  Strip& s = node < t ? tracks[(size_t)node].strip : node < t + kRoles ? buses[node - t] : master;
  switch (p) {
    case Param::GainDb:
      s.gainDb = value;
      break;
    case Param::Pan:
      s.pan = value;
      break;
    case Param::Mute:
      s.mute = value != 0.0f;
      break;
    case Param::Solo:
      s.solo = value != 0.0f;
      break;
  }
  retarget();
}

void Graph::beginBlock(int n) {
  for (int b = kBusL; b < kMasterL; b++) std::fill(buf(b), buf(b) + n, 0.0f);
  for (Level& l : levels_) l = Level{};
}

void Graph::meter(int node, const float* l, const float* r, int n) {
  Level& lv = levels_[(size_t)node];
  for (int i = 0; i < n; i++) {
    lv.peak[0] = std::max(lv.peak[0], prana::dabs(l[i]));
    lv.peak[1] = std::max(lv.peak[1], prana::dabs(r[i]));
    lv.sumSq[0] += l[i] * l[i];
    lv.sumSq[1] += r[i] * r[i];
  }
}

void Graph::process(int64_t pos, int at, int n, bool playing, bool realtime, CrumbSink* crumbs) {
  // The crumb names the node now running, so a crash can be pinned on it
  // (docs/ENGINE.md 4.3).
  auto enter = [&](int node) {
    if (crumbs) crumbs->crumb(crumbs_[(size_t)node]);
    if (node == crashNode_) abort();
  };
  auto leave = [&] {
    if (crumbs) crumbs->crumb(0);
  };
  float *tl = buf(kTrackL), *tr = buf(kTrackR), *cl = buf(kClipL), *cr = buf(kClipR);
  const int count = (int)tracks.size();
  for (int i = 0; i < count; i++) {
    Track& t = tracks[(size_t)i];
    enter(i);
    std::fill(tl, tl + n, 0.0f);
    std::fill(tr, tr + n, 0.0f);
    for (Clip& c : t.clips) {
      ClipSource& src = *c.source;
      const int64_t len = src.length();
      if (realtime && src.streamed()) src.want(std::min(std::max<int64_t>(pos - c.at, 0), len));
      if (!playing) continue;
      const int64_t a = std::max(pos, c.at), b = std::min(pos + n, c.at + len);
      if (a >= b) continue;
      const int m = (int)(b - a), dst = (int)(a - pos);
      if (realtime)
        src.read(a - c.at, m, cl, cr);
      else
        src.readNow(a - c.at, m, cl, cr);
      for (int k = 0; k < m; k++) {
        tl[dst + k] += cl[k] * c.gain;
        tr[dst + k] += cr[k] * c.gain;
      }
    }
    applyStrip(t.strip, tl, tr, n);
    meter(i, tl, tr, n);
    float *bl = buf(kBusL + 2 * t.role) + at, *br = buf(kBusL + 2 * t.role + 1) + at;
    for (int k = 0; k < n; k++) {
      bl[k] += tl[k];
      br[k] += tr[k];
    }
    leave();
  }
  for (int r = 0; r < kRoles; r++) {
    enter(busNode(r));
    float *bl = buf(kBusL + 2 * r) + at, *br = buf(kBusL + 2 * r + 1) + at;
    applyStrip(buses[r], bl, br, n);
    meter(busNode(r), bl, br, n);
    leave();
  }
  // The fold rule: the master is the sum of the four stems, in stem order,
  // then its own gain.
  enter(masterNode());
  float *ml = buf(kMasterL) + at, *mr = buf(kMasterR) + at;
  const float *v = buf(kBusL) + at, *d = buf(kBusL + 2) + at, *o = buf(kBusL + 4) + at, *b = buf(kBusL + 6) + at;
  for (int k = 0; k < n; k++) ml[k] = ((v[k] + d[k]) + o[k]) + b[k];
  v = buf(kBusL + 1) + at, d = buf(kBusL + 3) + at, o = buf(kBusL + 5) + at, b = buf(kBusL + 7) + at;
  for (int k = 0; k < n; k++) mr[k] = ((v[k] + d[k]) + o[k]) + b[k];
  applyStrip(master, ml, mr, n);
  meter(masterNode(), ml, mr, n);
  leave();
}

uint32_t Graph::endBlock(int n, wwav_shm_meter_entry* meters) {
  const uint32_t used = (uint32_t)nodes();
  if (!meters) return used;
  for (uint32_t i = 0; i < used; i++) {
    const Level& lv = levels_[i];
    wwav_shm_meter_slot& s = meters->slots[i];
    s.peak_l = lv.peak[0];
    s.peak_r = lv.peak[1];
    s.rms_l = prana::dsqrt(lv.sumSq[0] / (float)n);
    s.rms_r = prana::dsqrt(lv.sumSq[1] / (float)n);
  }
  meters->slots_used = used;
  return used;
}

void Graph::prefill(int64_t pos) {
  for (Track& t : tracks)
    for (Clip& c : t.clips)
      if (c.source->streamed()) c.source->prefill(std::min(std::max<int64_t>(pos - c.at, 0), c.source->length()));
}

void Graph::wantLoop(int64_t start) {
  for (Track& t : tracks) {
    for (Clip& c : t.clips) {
      if (!c.source->streamed()) continue;
      // A loop that starts before the clip comes back to the clip's first frame.
      const int64_t at = start < 0 ? -1 : std::max<int64_t>(start - c.at, 0);
      c.source->wantLoop(at < c.source->length() ? at : -1);
    }
  }
}

bool Graph::ready(int64_t pos, int64_t frames) const {
  for (const Track& t : tracks) {
    for (const Clip& c : t.clips) {
      const int64_t len = c.source->length();
      if (!c.source->streamed() || pos + frames <= c.at || pos >= c.at + len) continue;
      const int64_t from = std::max<int64_t>(pos - c.at, 0);
      if (!c.source->ready(from, std::min(len, pos + frames - c.at) - from)) return false;
    }
  }
  return true;
}

std::vector<ClipSource*> Graph::streamed() const {
  std::vector<ClipSource*> out;
  for (const Track& t : tracks)
    for (const Clip& c : t.clips)
      if (c.source->streamed()) out.push_back(c.source.get());
  return out;
}

}  // namespace wwav
