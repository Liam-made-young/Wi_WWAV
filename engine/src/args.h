#pragma once
// The command line (docs/ENGINE.md 1):
//
//   wwav-engine --socket <dir>/engine.sock --shm <name> [--device <name|null>]
//               [--rate 48000] [--block 128] [--test]
#include <string>
#include <vector>

namespace wwav {

struct Args {
  std::string socket;
  std::string shm;
  std::string device;  // "null": the timer device; "": the system's default output
  int rate = 48000;
  int block = 128;
  bool test = false;  // the debug.* ops
};

extern const char* const kUsage;

// False, with a sentence saying what is wrong, when the flags don't parse.
bool parseArgs(const std::vector<std::string>& argv, Args* out, std::string* error);

}  // namespace wwav
