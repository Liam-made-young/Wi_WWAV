#!/usr/bin/env bash
# Fetches JUCE at the pinned tag into engine/third_party/JUCE (ignored by git).
# 8.0.15 bundles the VST3 SDK 3.8.0 under MIT (docs/DECISIONS.md).
set -euo pipefail
JUCE_TAG="${JUCE_TAG:-8.0.15}"
DEST="$(cd "$(dirname "$0")/.." && pwd)/engine/third_party/JUCE"
CACHE="${JUCE_CACHE:-$HOME/.cache/juce-$JUCE_TAG}"
if [ -d "$DEST/modules" ]; then echo "JUCE already at $DEST"; exit 0; fi
mkdir -p "$(dirname "$DEST")"
if [ -d "$CACHE/modules" ]; then
  cp -a "$CACHE" "$DEST"
else
  git clone --quiet --depth 1 --branch "$JUCE_TAG" https://github.com/juce-framework/JUCE "$DEST"
fi
echo "JUCE $JUCE_TAG at $DEST"
