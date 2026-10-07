#!/usr/bin/env bash
# Fetches JUCE at the pinned tag into engine/third_party/JUCE (ignored by git).
# 8.0.15 bundles the VST3 SDK 3.8.0 under MIT (docs/DECISIONS.md).
# A clone also fills the cache, so the next checkout on this machine copies it.
set -euo pipefail
JUCE_TAG="${JUCE_TAG:-8.0.15}"
DEST="$(cd "$(dirname "$0")/.." && pwd)/engine/third_party/JUCE"
CACHE="${JUCE_CACHE:-$HOME/.cache/juce-$JUCE_TAG}"
if [ -d "$DEST/modules" ]; then echo "JUCE already at $DEST"; exit 0; fi
rm -rf "$DEST"  # what an interrupted fetch left
mkdir -p "$(dirname "$DEST")"
if [ -d "$CACHE/modules" ]; then
  cp -a "$CACHE" "$DEST"
else
  git -c advice.detachedHead=false clone --quiet --depth 1 --branch "$JUCE_TAG" https://github.com/juce-framework/JUCE "$DEST"
  # Copy, then rename into place: a checkout running beside this one never
  # sees half a cache. rename(2) refuses a cache that another one filled first.
  mkdir -p "$(dirname "$CACHE")"
  TMP="$(mktemp -d "$CACHE.XXXXXX")"
  cp -a "$DEST/." "$TMP/"
  python3 -c 'import os, sys; os.rename(sys.argv[1], sys.argv[2])' "$TMP" "$CACHE" 2>/dev/null || rm -rf "$TMP"
fi
echo "JUCE $JUCE_TAG at $DEST"
