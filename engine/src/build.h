#pragma once
// session.load's graph, compiled by the app (docs/ENGINE.md 3.3), built
// into a Graph off the audio thread. Everything the engine can't play yet is
// refused as "unsupported" rather than quietly left out.
#include <juce_core/juce_core.h>

#include <memory>
#include <string>

#include "graph.h"

namespace wwav {

struct Failure {
  std::string code;
  std::string message;
};

// `deviceRate` is the open device's rate, or 0 when none is open.
std::unique_ptr<Graph> buildGraph(const juce::var& graph, const juce::var& off, int deviceRate, Failure* fail);

// JSON numbers: an integer exactly, or any finite number.
bool wholeOf(const juce::var& v, int64_t* out);
bool numberOf(const juce::var& v, double* out);

}  // namespace wwav
