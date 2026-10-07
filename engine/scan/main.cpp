// wwav-scan (docs/SPEC.md 9.1, 9.4): checks plugins one at a time, with a
// 30 s limit each, and exits. Scanning comes with plugin hosting in a later
// stage; until then this says what it will do and scans nothing.
#include <stdio.h>

int main() {
  fputs(
      "usage: wwav-scan <plugin>...\n"
      "\n"
      "Checks each VST3 (and on macOS, AU) plugin in a process of its own, one at a\n"
      "time with a 30 s limit, and writes what it found for the engine.\n"
      "\n"
      "Not built yet: plugin scanning comes with plugin hosting (S0.4). This build\n"
      "scans nothing.\n",
      stderr);
  return 2;
}
