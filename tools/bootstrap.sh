#!/usr/bin/env bash
# Checks out formats/ (Mi-WWAV at its pinned commit) with only the folders this
# repo uses. Mi-WWAV is private: set MI_WWAV_TOKEN, or have git credentials for it.
set -euo pipefail
cd "$(dirname "$0")/.."

PATHS=(
  /prana/core/ /prana/tools/ /prana/tests/ /prana/hal/native/
  /prana/SPEC.md /prana/CMakeLists.txt /prana/web/src/sim/
  /formats/swav/ /wi/src/formats/ /wi/test/ /wi/GATES.md
)

if [ -n "${MI_WWAV_TOKEN:-}" ]; then
  git config --global url."https://x-access-token:${MI_WWAV_TOKEN}@github.com/".insteadOf "https://github.com/"
fi

git submodule init formats
if [ ! -e formats/.git ]; then
  git submodule update --no-fetch --filter=blob:none --no-checkout formats 2>/dev/null \
    || git submodule update --no-checkout formats
fi
git -C formats sparse-checkout init --no-cone
printf '%s\n' "${PATHS[@]}" > "$(git -C formats rev-parse --path-format=absolute --git-path info/sparse-checkout)"
git -C formats checkout --quiet "$(git ls-tree HEAD formats | awk '{print $3}')"
echo "formats/ at $(git -C formats rev-parse --short HEAD), sparse"
