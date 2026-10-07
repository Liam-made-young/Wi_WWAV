#!/usr/bin/env bash
# Fills formats/ from a local copy of Mi-WWAV instead of GitHub: for git
# worktrees and offline machines. WWAV_FORMATS_SOURCE names the local copy
# (default: the formats/ of the main checkout this worktree belongs to).
set -euo pipefail
cd "$(dirname "$0")/.."
if [ -e formats/prana/tools/wwav_pack.py ]; then echo "formats/ already checked out"; exit 0; fi
MAIN="$(git rev-parse --path-format=absolute --git-common-dir)/.."
SRC="${WWAV_FORMATS_SOURCE:-$MAIN/formats}"
PIN="$(git ls-tree HEAD formats | awk '{print $3}')"
rm -rf formats
git clone --quiet --shared --no-checkout "$SRC" formats
git -C formats sparse-checkout init --no-cone
printf '%s\n' /prana/core/ /prana/tools/ /prana/tests/ /prana/hal/native/ /prana/SPEC.md \
  /prana/CMakeLists.txt /prana/web/src/sim/ /formats/swav/ /wi/src/formats/ /wi/test/ /wi/GATES.md \
  > "$(git -C formats rev-parse --path-format=absolute --git-path info/sparse-checkout)"
git -C formats checkout --quiet "$PIN"
echo "formats/ at $(git -C formats rev-parse --short HEAD), from $SRC"
