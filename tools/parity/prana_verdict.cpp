// What PRANA lists a file as, decided by the device's own code: each file
// goes on a RAM disc as "01 Corpus.wwav" at the root, where a loose song
// sits on a PRANA disc, and prana/core's Library scans it (disc/library.cpp
// Library::wwavDone, through disc/wav.cpp and disc/wwav.cpp).
//
// The device gives no sentence, only an outcome, so this prints the outcome
// as the start of the sentence it stands for (tools/parity/check.py has
// the table): "not listed", "the master only" or "4 stems, and the master",
// a tab, then the path. One line per file named on the command line.
//
// Built by tools/parity/check.py against formats/prana/core and the RAM
// disk in formats/prana/tests/ramdisk.h.
#include <cstdio>
#include <memory>
#include <vector>

#include "disc/library.h"
#include "ramdisk.h"

using namespace prana;

static bool slurp(const char* path, std::vector<uint8_t>* out) {
  FILE* f = std::fopen(path, "rb");
  if (!f) return false;
  uint8_t buf[65536];
  size_t n;
  while ((n = std::fread(buf, 1, sizeof buf, f)) > 0) out->insert(out->end(), buf, buf + n);
  std::fclose(f);
  return true;
}

static const char* outcome(const std::vector<uint8_t>& file) {
  auto disc = std::make_unique<RamDisk>();
  disc->files["01 Corpus.wwav"] = file;
  auto sc = std::make_unique<StorageClient>();
  auto lib = std::make_unique<Library>();
  lib->attach(sc.get());
  lib->scan();
  // the device's loop: the library asks, storage answers
  for (int i = 0; i < 100000 && lib->state() == Library::State::Scanning; i++) {
    lib->tick();
    SReq r;
    while (sc->take(&r)) {
      SRes res;
      disc->exec(r, &res);
      sc->complete(res);
    }
  }
  if (lib->state() == Library::State::Empty) return "not listed";
  if (lib->state() != Library::State::Ready || lib->count() != 1) return "the scan didn't finish";
  const SongInfo& s = lib->song(0);
  if (s.hasStems()) return "4 stems, and the master";
  return s.hasMaster ? "the master only" : "not listed";
}

int main(int argc, char** argv) {
  for (int i = 1; i < argc; i++) {
    std::vector<uint8_t> file;
    if (!slurp(argv[i], &file)) {
      std::fprintf(stderr, "prana_verdict: can't read %s\n", argv[i]);
      return 1;
    }
    std::printf("%s\t%s\n", outcome(file), argv[i]);
  }
  return 0;
}
