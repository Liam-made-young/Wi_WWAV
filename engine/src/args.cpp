#include "args.h"

#include <stdlib.h>

#include "media.h"

namespace wwav {

const char* const kUsage =
    "usage: wwav-engine --socket <path> --shm <name> [--device <name|null>]\n"
    "                   [--rate 48000] [--block 128] [--test]\n"
    "\n"
    "The app starts the engine with a pipe on its stdin; when the pipe closes, the\n"
    "engine stops its audio and exits. See docs/ENGINE.md.\n";

namespace {

bool number(const std::string& s, int lo, int hi, int* out) {
  char* end = nullptr;
  long v = strtol(s.c_str(), &end, 10);
  if (s.empty() || *end != '\0' || v < lo || v > hi) return false;
  *out = (int)v;
  return true;
}

}  // namespace

bool parseArgs(const std::vector<std::string>& argv, Args* out, std::string* error) {
  for (size_t i = 0; i < argv.size(); i++) {
    const std::string& a = argv[i];
    if (a == "--test") {
      out->test = true;
      continue;
    }
    if (a != "--socket" && a != "--shm" && a != "--device" && a != "--rate" && a != "--block") {
      *error = "Unknown flag " + a + ".";
      return false;
    }
    if (i + 1 >= argv.size()) {
      *error = a + " needs a value.";
      return false;
    }
    const std::string& v = argv[++i];
    if (a == "--socket") {
      out->socket = v;
    } else if (a == "--shm") {
      out->shm = v;
    } else if (a == "--device") {
      out->device = v;
    } else if (a == "--rate" && !number(v, 8000, 384000, &out->rate)) {
      *error = "--rate is a sample rate from 8000 to 384000.";
      return false;
    } else if (a == "--block" && !number(v, 16, kMaxBlock, &out->block)) {
      *error = "--block is from 16 to " + std::to_string(kMaxBlock) + " frames.";
      return false;
    }
  }
  if (out->socket.empty() || out->shm.empty()) {
    *error = "--socket and --shm are required.";
    return false;
  }
  return true;
}

}  // namespace wwav
