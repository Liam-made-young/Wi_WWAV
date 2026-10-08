#!/usr/bin/env bash
# A local, native-architecture test bundle; not a notarized release.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD="${WI_WWAV_BUILD_DIR:-$HOME/Library/Developer/wi-wwav-build}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$BUILD/target-space}"
HELPERS="$BUILD/local-helpers"
CONFIG="$BUILD/tauri-local.json"
[[ "$(uname -s)" == Darwin ]] || { printf 'This build needs macOS.\n' >&2; exit 1; }
mkdir -p "$HELPERS/wwav-engine.app/Contents/MacOS"
cd "$ROOT"
cargo build --locked -p wwav-engine -p wi-mcp
ditto "$CARGO_TARGET_DIR/debug/wwav-engine" "$HELPERS/wwav-engine.app/Contents/MacOS/wwav-engine"
ditto "$ROOT/engine/macos/Info.plist" "$HELPERS/wwav-engine.app/Contents/Info.plist"
ditto "$CARGO_TARGET_DIR/debug/wi-mcp" "$HELPERS/wi-mcp"
swiftc -O "$ROOT/crates/wi-core/src/ocr/wi-ocr.swift" -o "$HELPERS/wi-ocr"
# RFC 7396 null removes the legacy, unbuilt JUCE scanner from the merged config.
node - "$HELPERS" "$CONFIG" "$ROOT" <<'JS'
const fs = require('node:fs');
const path = require('node:path');
const [helpers, config, root] = process.argv.slice(2);
fs.writeFileSync(config, JSON.stringify({ build: {
  beforeBuildCommand: { script: 'npm run build', cwd: path.join(root, 'app/ui') },
}, bundle: { macOS: { files: {
  'Helpers/wwav-scan': null,
  'Helpers/wwav-engine.app': path.join(helpers, 'wwav-engine.app'),
  'Helpers/wi-mcp': path.join(helpers, 'wi-mcp'),
  'Helpers/wi-ocr': path.join(helpers, 'wi-ocr'),
} } } }));
JS
cd "$ROOT/app"
cargo tauri build --debug --ci --bundles app --no-sign --config "$CONFIG"
APP="$CARGO_TARGET_DIR/debug/bundle/macos/Wi_WWAV.app"
xattr -cr "$APP"
codesign --force --sign - "$APP/Contents/Helpers/wwav-engine.app"
codesign --force --sign - "$APP/Contents/Helpers/wi-mcp"
codesign --force --sign - "$APP/Contents/Helpers/wi-ocr"
codesign --force --sign - --entitlements "$ROOT/app/src-tauri/entitlements.plist" "$APP"
codesign --verify --deep --strict "$APP"
printf '\nReady: %s\n' "$APP"
